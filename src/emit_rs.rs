//! The Rust backend.
//!
//! Closer to Go than to any other target: both are statically typed with no runtime type
//! information, so field distribution over a Vec has to be spelled out at each depth the same
//! way, and a record needs a declared struct before a value of one can exist. Rust gets three
//! things Go does not: a real `Option<T>` (no `tlOpt[T]` wrapper needed), a real conditional
//! expression (`if`/`else` needs no closure the way Go's does), and expression-level `let` (a
//! pipe is a block, not an immediately-invoked closure).
//!
//! No external crate. `rustc` compiles one self-contained file, the same way `cc` compiles
//! native's, so a built program depends on nothing this compiler did not already assume; JSON
//! reading and writing are both hand-written here for the same reason every other backend's are.
//!
//! Values are cloned rather than borrowed. Toylang has no mutation and nothing here optimises for
//! speed, so fighting the borrow checker for a value that is read twice would buy nothing; every
//! `Kind::Var`/`Kind::Local` reference clones, unconditionally, the same "do not bother, it is
//! cheap enough and always correct" choice the other five backends make by simply copying.

use crate::ast::{BinOp, LogicOp};
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::{self, Enums, Type};

const INPUT: &str = "t_input";
const INPUTS: &str = "t_inputs";

const FAIL_HELPER: &str = r#"fn tl_fail(msg: &str) -> ! {
    eprintln!("toylang: {}", msg);
    std::process::exit(1);
}
"#;

/// `wrapping_add`/`sub`/`mul` never panic, unlike plain `+`/`-`/`*` in a debug build, and `n`
/// arriving through a function call rather than as a bare literal is what stops the compiler
/// from constant-folding a literal overflow into a compile error the way Go's `tlInt` also
/// exists to avoid.
const INT_HELPER: &str = r#"fn tl_int(n: i32) -> i32 {
    n
}
"#;

const ARITH_HELPER: &str = r#"fn tl_div(a: i32, b: i32) -> i32 {
    if b == 0 {
        tl_fail("divided by zero");
    }
    a.wrapping_div(b)
}

fn tl_rem(a: i32, b: i32) -> i32 {
    if b == 0 {
        tl_fail("divided by zero");
    }
    a.wrapping_rem(b)
}
"#;

/// The same shape at 64 bits (kantord/toylang#83): `wrapping_div`/`wrapping_rem` already give
/// `MIN / -1` the wrapping answer, so only the zero divisor needs the guard.
const ARITH64_HELPER: &str = r#"fn tl_div64(a: i64, b: i64) -> i64 {
    if b == 0 {
        tl_fail("divided by zero");
    }
    a.wrapping_div(b)
}

fn tl_rem64(a: i64, b: i64) -> i64 {
    if b == 0 {
        tl_fail("divided by zero");
    }
    a.wrapping_rem(b)
}
"#;

const AT_HELPER: &str = r#"fn tl_at<T: Clone>(v: &[T], i: i32) -> Option<T> {
    let n = v.len() as i32;
    let i = if i < 0 { n + i } else { i };
    if i < 0 || i >= n {
        None
    } else {
        Some(v[i as usize].clone())
    }
}
"#;

// Out-of-range bounds clamp jq-style rather than answering absence, so this never fails.
// A `None` bound is passed as its sentinel: `i32::MIN` for the start (clamps to 0) and
// `i32::MAX` for the end (clamps to the length).
const SLICE_HELPER: &str = r#"fn tl_slice<T: Clone>(v: &[T], lo: i32, hi: i32) -> Vec<T> {
    let n = v.len() as i32;
    let lo = (if lo < 0 { n + lo } else { lo }).clamp(0, n);
    let hi = (if hi < 0 { n + hi } else { hi }).clamp(0, n);
    if lo >= hi {
        Vec::new()
    } else {
        v[lo as usize..hi as usize].to_vec()
    }
}
"#;

const UNWRAP_HELPER: &str = r#"fn tl_unwrap<T>(o: Option<T>) -> T {
    match o {
        Some(v) => v,
        None => tl_fail("unwrapped a value that is not there"),
    }
}
"#;

const TAIL_HELPER: &str = r#"fn tl_tail<T: Clone>(v: &[T]) -> Option<Vec<T>> {
    if v.is_empty() {
        None
    } else {
        Some(v[1..].to_vec())
    }
}
"#;

const FLATTEN_HELPER: &str = r#"fn tl_flatten<T: Clone>(vv: &[Vec<T>]) -> Vec<T> {
    let mut out = Vec::new();
    for v in vv {
        out.extend(v.iter().cloned());
    }
    out
}
"#;

// `Ord` is exactly the constraint the checker's own `orderable` restricts `sort`'s element
// type to (Int, Int64, Str, Char all implement it natively), so nothing here has to name them.
const SORT_HELPER: &str = r#"fn tl_sort<T: Clone + Ord>(v: &[T]) -> Vec<T> {
    let mut out = v.to_vec();
    out.sort();
    out
}
"#;

const REVERSE_HELPER: &str = r#"fn tl_reverse<T: Clone>(v: &[T]) -> Vec<T> {
    let mut out = v.to_vec();
    out.reverse();
    out
}
"#;

// `wrapping_add` is the same wrap `+` gets through INT_HELPER/ARITH_HELPER, so a fold is the
// repeated addition with nothing else to spell.
const SUM_HELPER: &str = r#"fn tl_sum(v: &[i32]) -> i32 {
    v.iter().fold(0i32, |acc, x| acc.wrapping_add(*x))
}

fn tl_sum64(v: &[i64]) -> i64 {
    v.iter().fold(0i64, |acc, x| acc.wrapping_add(*x))
}
"#;

// `Option::max` is exactly the empty-is-absent answer, and `Ord` covers both integer widths.
const MAX_HELPER: &str = r#"fn tl_max<T: Clone + Ord>(v: &[T]) -> Option<T> {
    v.iter().cloned().max()
}
"#;

const RANGE_HELPER: &str = r#"fn tl_range(n: i32) -> Vec<i32> {
    (0..n.max(0)).collect()
}
"#;

/// Rust's own `char` already is a Unicode scalar value, and `str::chars` already decodes by
/// codepoint rather than by byte or UTF-16 unit, so there is no decoding to get right here --
/// only the cast down to the `i32` every other backend represents a `Char` as.
const CHARS_HELPER: &str = r#"fn tl_chars(s: &str) -> Vec<i32> {
    s.chars().map(|c| c as i32).collect()
}
"#;

/// Read all of stdin, and every line of it, both by the one primitive read needs: reading to
/// EOF is `lines`/`inputs`/`input` doing the exact same underlying thing three ways.
const READ_HELPER: &str = r#"fn tl_read_all_stdin() -> Vec<u8> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::io::stdin()
        .read_to_end(&mut buf)
        .unwrap_or_else(|e| tl_fail(&format!("could not read stdin: {e}")));
    buf
}

/// Split on `\n` only, matching `jq -R` and Python's raw stdin iteration rather than Rust's own
/// `BufRead::lines`, which also swallows a `\r` before it -- CRLF is ordinary content here, not a
/// line terminator. The final line is yielded even with no trailing `\n`, deliberately not `wc
/// -l`'s undercount; empty stdin yields zero lines, not one empty one.
fn tl_read_lines() -> Vec<String> {
    let bytes = tl_read_all_stdin();
    if bytes.is_empty() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let mut out: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
    if text.ends_with('\n') {
        out.pop();
    }
    out
}
"#;

/// A cursor over raw JSON bytes. Kept separate from the per-type parse functions below the same
/// way `tl_json` is in native's runtime: the cursor does not know what shape it is reading,
/// which is what lets `tl_parse_vec` reuse it for any element type.
const PARSER_HELPER: &str = r#"struct TlParser<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> TlParser<'a> {
    fn new(b: &'a [u8]) -> TlParser<'a> {
        TlParser { b, p: 0 }
    }

    fn skip_ws(&mut self) {
        while self.p < self.b.len() && matches!(self.b[self.p], b' ' | b'\t' | b'\n' | b'\r') {
            self.p += 1;
        }
    }

    fn peek(&mut self) -> u8 {
        self.skip_ws();
        if self.p >= self.b.len() {
            tl_fail("unexpected end of input");
        }
        self.b[self.p]
    }

    fn expect(&mut self, c: u8) {
        if self.peek() != c {
            tl_fail(&format!("expected `{}`", c as char));
        }
        self.p += 1;
    }

    /// Whether the next non-whitespace byte is `c`, without consuming it -- used to look ahead
    /// for an empty `[]`/`{}` and for the `,` that continues a list.
    fn at(&mut self, c: u8) -> bool {
        self.skip_ws();
        self.p < self.b.len() && self.b[self.p] == c
    }

    fn parse_str(&mut self) -> String {
        self.expect(b'"');
        let mut out = String::new();
        loop {
            if self.p >= self.b.len() {
                tl_fail("unterminated string");
            }
            match self.b[self.p] {
                b'"' => {
                    self.p += 1;
                    break;
                }
                b'\\' => {
                    self.p += 1;
                    if self.p >= self.b.len() {
                        tl_fail("unterminated escape");
                    }
                    out.push(match self.b[self.p] {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        _ => tl_fail("unsupported escape"),
                    });
                    self.p += 1;
                }
                c => {
                    // Multi-byte UTF-8 sequences pass through one byte at a time; `push` below
                    // only ever sees a full sequence, since the outer loop keeps consuming until
                    // it does.
                    let start = self.p;
                    self.p += 1;
                    let extra = match c {
                        0x00..=0x7f => 0,
                        0xc0..=0xdf => 1,
                        0xe0..=0xef => 2,
                        0xf0..=0xff => 3,
                        _ => 0,
                    };
                    for _ in 0..extra {
                        if self.p >= self.b.len() {
                            tl_fail("truncated utf-8");
                        }
                        self.p += 1;
                    }
                    out.push_str(std::str::from_utf8(&self.b[start..self.p]).unwrap_or("?"));
                }
            }
        }
        out
    }

    /// Skip one JSON value without interpreting it, for fields the type does not declare.
    fn skip_value(&mut self) {
        match self.peek() {
            b'"' => {
                self.parse_str();
            }
            b'[' | b'{' => {
                let (open, close) = if self.peek() == b'[' { (b'[', b']') } else { (b'{', b'}') };
                let mut depth = 0i32;
                loop {
                    if self.p >= self.b.len() {
                        tl_fail("unterminated value");
                    }
                    if self.b[self.p] == b'"' {
                        self.parse_str();
                        continue;
                    }
                    if self.b[self.p] == open {
                        depth += 1;
                    } else if self.b[self.p] == close {
                        depth -= 1;
                        if depth == 0 {
                            self.p += 1;
                            return;
                        }
                    }
                    self.p += 1;
                }
            }
            _ => {
                while self.p < self.b.len()
                    && !matches!(self.b[self.p], b',' | b'}' | b']')
                {
                    self.p += 1;
                }
            }
        }
    }
}

fn tl_parse_str(p: &mut TlParser) -> String {
    if p.peek() != b'"' {
        tl_fail("expected a string");
    }
    p.parse_str()
}

fn tl_parse_i32(p: &mut TlParser) -> i32 {
    p.skip_ws();
    let start = p.p;
    if p.p < p.b.len() && matches!(p.b[p.p], b'-' | b'+') {
        p.p += 1;
    }
    while p.p < p.b.len() && p.b[p.p].is_ascii_digit() {
        p.p += 1;
    }
    if p.p == start {
        tl_fail("expected an integer");
    }
    if p.p < p.b.len() && matches!(p.b[p.p], b'.' | b'e' | b'E') {
        tl_fail("expected an integer, found a non-integer number");
    }
    let text = std::str::from_utf8(&p.b[start..p.p]).expect("digits and sign are ascii");
    let n: i64 = text.parse().unwrap_or_else(|_| tl_fail("integer is out of range"));
    i32::try_from(n).unwrap_or_else(|_| tl_fail("integer is out of range"))
}

fn tl_parse_bool(p: &mut TlParser) -> bool {
    p.skip_ws();
    if p.b[p.p..].starts_with(b"true") {
        p.p += 4;
        true
    } else if p.b[p.p..].starts_with(b"false") {
        p.p += 5;
        false
    } else {
        tl_fail("expected a boolean")
    }
}

fn tl_parse_vec<T>(p: &mut TlParser, elem: fn(&mut TlParser) -> T) -> Vec<T> {
    p.expect(b'[');
    let mut out = Vec::new();
    if p.at(b']') {
        p.p += 1;
        return out;
    }
    loop {
        out.push(elem(p));
        if p.at(b',') {
            p.p += 1;
            continue;
        }
        break;
    }
    p.expect(b']');
    out
}

/// Reads a value from one already-owned line of text, refusing anything left over -- the same
/// "trailing content after the value" check `tl_read_value` uses for a whole document, applied
/// per line since `inputs` is many documents, not one.
fn tl_parse_line<T>(line: &str, elem: fn(&mut TlParser) -> T) -> T {
    let mut p = TlParser::new(line.as_bytes());
    let v = elem(&mut p);
    if p.at(b'\0') || p.p != p.b.len() {
        p.skip_ws();
        if p.p != p.b.len() {
            tl_fail("trailing content after the value");
        }
    }
    v
}

fn tl_read_value<T>(elem: fn(&mut TlParser) -> T) -> T {
    let bytes = tl_read_all_stdin();
    let mut p = TlParser::new(&bytes);
    let v = elem(&mut p);
    p.skip_ws();
    if p.p != p.b.len() {
        tl_fail("trailing content after the value");
    }
    v
}

fn tl_read_values<T>(elem: fn(&mut TlParser) -> T) -> Vec<T> {
    tl_read_lines()
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| tl_parse_line(&l, elem))
        .collect()
}
"#;

const QUOTE_HELPER: &str = r#"fn tl_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
"#;

const JOIN_HELPER: &str = r#"fn tl_join<T>(v: &[T], f: fn(&T) -> String) -> String {
    let parts: Vec<String> = v.iter().map(f).collect();
    format!("[{}]", parts.join(","))
}
"#;

const JSONLINES_HELPER: &str = r#"fn tl_jsonlines<T>(v: &[T], f: fn(&T) -> String) -> String {
    let parts: Vec<String> = v.iter().map(f).collect();
    parts.join("\n")
}
"#;

/// The derive line every emitted record and enum carries. `PartialEq` is unconditional: it is
/// the structural equality the other backends have to hand-write (kantord/toylang#68), and
/// gating it on the program actually comparing something would cost a traversal to save a line
/// of output nobody reads.
const DERIVE: &str = "#[derive(Clone, PartialEq)]";

pub fn emit(program: &Program) -> String {
    let mut used = Used::default();
    let mut records = Vec::new();
    let mut enums = Vec::new();
    let mut ctx = Collect {
        used: &mut used,
        records: &mut records,
        enums: &mut enums,
        registry: &program.enums,
    };
    for f in &program.funcs {
        if let Some(param_ty) = &f.param_ty {
            ctx.ty(param_ty);
        }
        ctx.walk(&f.body);
    }
    ctx.walk(&program.body);
    if let Some(ty) = &program.input {
        ctx.ty(ty);
    }
    if let Some(ty) = &program.inputs {
        ctx.ty(ty);
    }
    records.sort_by_key(|t| t.to_string());
    enums.sort_by_key(|t| t.to_string());

    // Only the types stdin can actually deliver get a parser. Every collected record used to,
    // harmlessly, until a type became unparseable on purpose: a program that reads input and
    // also passes an `{a: Int64, ...}` record between its own functions must not force a
    // parser for a shape the checker already promised can never cross the wire.
    let mut wire: Vec<Type> = Vec::new();
    for ty in [&program.input, &program.inputs].into_iter().flatten() {
        collect_wire(&program.enums, ty, &mut wire);
    }

    let e = Emitter {
        records,
        enums,
        registry: &program.enums,
    };

    let mut decls = String::new();
    for (i, rec) in e.records.iter().enumerate() {
        let Type::Record(fields) = rec else {
            unreachable!("only records are collected")
        };
        decls.push_str(&format!("{DERIVE}\nstruct TlRec{i} {{\n"));
        for (name, ty) in fields {
            decls.push_str(&format!("    {}: {},\n", rs_field(name), e.rs_type(ty)));
        }
        decls.push_str("}\n\n");
        if wire.contains(rec) {
            decls.push_str(&e.record_parser(i, fields));
        }
    }

    // The one backend whose target has the construct being compiled: a toylang enum is a Rust
    // enum. `allow(non_camel_case_types)` because variant names are data and keep their source
    // spelling rather than being case-mangled.
    for (i, en) in e.enums.iter().enumerate() {
        let Type::Enum { name, .. } = en else {
            unreachable!("only enums are collected")
        };
        let variants = ty::variants(&program.enums, en);
        decls.push_str(&format!(
            "{DERIVE}\n#[allow(non_camel_case_types)]\nenum {} {{\n",
            e.rs_type(en)
        ));
        for (vname, payload) in &variants {
            match payload {
                None => decls.push_str(&format!("    V_{vname},\n")),
                Some(p) => decls.push_str(&format!("    V_{vname}({}),\n", e.rs_type(p))),
            }
        }
        decls.push_str("}\n\n");
        if wire.contains(en) {
            decls.push_str(&e.enum_parser(i, en, name));
        }
    }

    decls.push_str(&e.printers(program));

    for f in &program.funcs {
        let param = match (&f.param, &f.param_ty) {
            (Some(name), Some(ty)) => format!("{}: {}", e.user(name), e.rs_type(ty)),
            (None, None) => String::new(),
            _ => unreachable!("a function's param and param_ty agree"),
        };
        decls.push_str(&format!(
            "fn {}({}) -> {} {{\n    {}\n}}\n\n",
            e.user(&f.name),
            param,
            e.rs_type(&f.body.ty),
            e.expr(&f.body)
        ));
    }

    if let Some(fusion) = tir::fusion(program) {
        decls.push_str(&e.fused_main(program, &fusion));
    } else {
        decls.push_str("fn main() {\n");
        if let Some(ty) = &program.input {
            decls.push_str(&format!(
                "    let {INPUT}: {} = tl_read_value({});\n",
                e.rs_type(ty),
                e.parser_expr(ty)
            ));
        }
        if let Some(ty) = &program.inputs {
            decls.push_str(&format!(
                "    let {INPUTS}: Vec<{}> = tl_read_values({});\n",
                e.rs_type(ty),
                e.parser_expr(ty)
            ));
        }
        let body = e.expr(&program.body);
        let printed = if program.body.ty == Type::Str {
            body
        } else {
            e.show(&program.body.ty, &body, 0)
        };
        decls.push_str(&format!("    println!(\"{{}}\", {printed});\n}}\n"));
    }

    let uses = |name: &str| decls.contains(name);
    let unwrap = uses("tl_unwrap(");
    let arith = uses("tl_div(") || uses("tl_rem(");
    let arith64 = uses("tl_div64(") || uses("tl_rem64(");
    let reads_value = program.input.is_some() || program.inputs.is_some();
    // `tl_read_lines` alone (a program whose only stdin-touching builtin is `lines`) emits
    // READ_HELPER, whose body calls `tl_fail` via `tl_read_all_stdin`; the `uses` scan only sees
    // the program decls, not other helpers' source, so the trigger has to be named here too.
    let fail = unwrap
        || arith
        || arith64
        || reads_value
        || uses("tl_at(")
        || uses("tl_tail(")
        || uses("tl_range(")
        || uses("tl_read_all_stdin(")
        || uses("tl_read_lines(")
        || uses("tl_fail(");

    let mut helpers = String::new();
    for (on, text) in [
        (fail, FAIL_HELPER),
        (uses("tl_int("), INT_HELPER),
        (arith, ARITH_HELPER),
        (arith64, ARITH64_HELPER),
        (uses("tl_at("), AT_HELPER),
        (uses("tl_slice("), SLICE_HELPER),
        (unwrap, UNWRAP_HELPER),
        (uses("tl_tail("), TAIL_HELPER),
        (uses("tl_flatten("), FLATTEN_HELPER),
        (uses("tl_sort("), SORT_HELPER),
        (uses("tl_reverse("), REVERSE_HELPER),
        (uses("tl_sum(") || uses("tl_sum64("), SUM_HELPER),
        (uses("tl_max("), MAX_HELPER),
        (uses("tl_range("), RANGE_HELPER),
        (uses("tl_chars("), CHARS_HELPER),
        (
            reads_value || uses("tl_read_all_stdin(") || uses("tl_read_lines("),
            READ_HELPER,
        ),
        (reads_value || uses("TlParser"), PARSER_HELPER),
        (uses("tl_quote("), QUOTE_HELPER),
        (uses("tl_join("), JOIN_HELPER),
        (used.jsonlines, JSONLINES_HELPER),
    ] {
        if on {
            helpers.push_str(text);
            helpers.push('\n');
        }
    }

    // Every generated identifier is used somewhere by construction, but not every generated
    // struct field always is (an input-only record's fields are read by the parser, not by any
    // expression, if the program never reads the field back out) -- rustc is right that this can
    // happen and wrong that it matters here.
    format!("#![allow(dead_code)]\n\n{helpers}{decls}")
}

fn rs_field(name: &str) -> String {
    format!("f_{name}")
}

/// Every record and enum type nested anywhere in `ty`, which is an input's declared type: the
/// set of shapes that need a JSON parser. An Opt cannot appear (the checker refuses one in an
/// input), so the prelude's special case needs no carve-out here.
fn collect_wire(enums: &Enums, ty: &Type, out: &mut Vec<Type>) {
    match ty {
        Type::Vec(e) | Type::Stream(e) => collect_wire(enums, e, out),
        Type::Record(fields) => {
            if !out.contains(ty) {
                out.push(ty.clone());
            }
            for (_, f) in fields {
                collect_wire(enums, f, out);
            }
        }
        Type::Enum { .. } => {
            // Having it already is what stops a recursive enum here, as in `Collect::ty`.
            if out.contains(ty) {
                return;
            }
            out.push(ty.clone());
            for (_, p) in ty::variants(enums, ty) {
                if let Some(p) = p {
                    collect_wire(enums, &p, out);
                }
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct Used {
    jsonlines: bool,
}

/// One walk, collecting the record types that need a struct declaration (and a parser, if the
/// program reads `input`/`inputs`) plus whether `jsonlines` was called at all.
struct Collect<'a> {
    used: &'a mut Used,
    records: &'a mut Vec<Type>,
    enums: &'a mut Vec<Type>,
    /// Every enum the program declared. The variant list on a `Type::Enum` in hand may be a
    /// placeholder, so the payloads to descend into are read from here (`ty::variants`).
    registry: &'a Enums,
}

impl Collect<'_> {
    fn ty(&mut self, t: &Type) {
        match t {
            Type::Vec(e) | Type::Stream(e) => self.ty(e),
            Type::Record(fields) => {
                if !self.records.contains(t) {
                    self.records.push(t.clone());
                }
                for (_, f) in fields {
                    self.ty(f);
                }
            }
            // Opt keeps the Option<T> it always had (already tagged), so it is not
            // harvested as a declared enum; only what it holds is.
            Type::Enum { .. } => {
                if let Some(inner) = t.as_opt() {
                    self.ty(inner);
                    return;
                }
                // Having it already is what stops a recursive enum here: its own payload leads
                // back to a type this list holds.
                if self.enums.contains(t) {
                    return;
                }
                self.enums.push(t.clone());
                for (_, p) in ty::variants(self.registry, t) {
                    if let Some(p) = p {
                        self.ty(&p);
                    }
                }
            }
            _ => {}
        }
    }

    fn walk(&mut self, t: &Tir) {
        self.ty(&t.ty);
        match &t.kind {
            Kind::Str(_)
            | Kind::Int(_)
            | Kind::Var(_)
            | Kind::Local(_)
            | Kind::Input
            | Kind::Inputs
            | Kind::Lines => {}
            Kind::VecLit(items) => items.iter().for_each(|i| self.walk(i)),
            Kind::RecordLit { fields } => fields.iter().for_each(|(_, v)| self.walk(v)),
            Kind::EnumLit { payload, .. } => {
                if let Some(p) = payload {
                    self.walk(p);
                }
            }
            Kind::Call { arg, .. } => {
                if let Some(a) = arg {
                    self.walk(a);
                }
            }
            Kind::Concat(l, r)
            | Kind::Compare { lhs: l, rhs: r, .. }
            | Kind::Logic { lhs: l, rhs: r, .. }
            | Kind::Arith { lhs: l, rhs: r, .. } => {
                self.walk(l);
                self.walk(r);
            }
            Kind::Bind { value, body, .. } => {
                self.walk(value);
                self.walk(body);
            }
            Kind::Map { source, body, .. } | Kind::OptMap { source, body, .. } => {
                self.walk(source);
                self.walk(body);
            }
            Kind::Select { source, pred, .. } => {
                self.walk(source);
                self.walk(pred);
            }
            Kind::Cond {
                cond,
                then,
                otherwise,
            } => {
                self.walk(cond);
                self.walk(then);
                self.walk(otherwise);
            }
            Kind::Field { base, .. } | Kind::Unwrap { base } | Kind::Not(base) => self.walk(base),
            Kind::Index { base, index, .. } => {
                self.walk(base);
                self.walk(index);
            }
            Kind::Slice { base, start, end, .. } => {
                self.walk(base);
                if let Some(s) = start {
                    self.walk(s);
                }
                if let Some(e) = end {
                    self.walk(e);
                }
            }
            Kind::Match { subject, arms, .. } => {
                self.walk(subject);
                for a in arms {
                    if let Some(g) = &a.guard {
                        self.walk(g);
                    }
                    self.walk(&a.body);
                }
            }
            Kind::Builtin { which, arg } => {
                if *which == Builtin::JsonLines {
                    self.used.jsonlines = true;
                }
                self.walk(arg);
            }
        }
    }
}

struct Emitter<'a> {
    records: Vec<Type>,
    enums: Vec<Type>,
    /// Every enum the program declared, for `ty::variants`: what a `Type::Enum` carries is a
    /// placeholder wherever a recursive enum's payload reaches back to itself.
    registry: &'a Enums,
}

impl Emitter<'_> {
    fn user(&self, name: &str) -> String {
        format!("v_{name}")
    }

    fn local(&self, id: LocalId) -> String {
        format!("t_{id}")
    }

    fn record_index(&self, ty: &Type) -> usize {
        self.records
            .iter()
            .position(|r| r == ty)
            .expect("every record reachable from the program was collected")
    }

    fn enum_index(&self, ty: &Type) -> usize {
        let key = ty.to_string();
        self.enums
            .iter()
            .position(|r| r.to_string() == key)
            .expect("every enum reachable from the program was collected")
    }

    fn rs_type(&self, ty: &Type) -> String {
        match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            Type::Str => "String".to_string(),
            Type::Int => "i32".to_string(),
            Type::Int64 => "i64".to_string(),
            Type::Bool => "bool".to_string(),
            // Same width as Int: a Char is a codepoint, and the checker already refuses to mix
            // the two, so nothing here needs to tell them apart.
            Type::Char => "i32".to_string(),
            Type::Vec(e) => format!("Vec<{}>", self.rs_type(e)),
            Type::Enum { .. } if ty.as_opt().is_some() => {
                format!("Option<{}>", self.rs_type(ty.as_opt().expect("guarded")))
            }
            // Materialized eagerly as the Vec of its entries, so it is a Vec here too.
            // Fusion is what will remove this materialization.
            Type::Stream(e) => format!("Vec<{}>", self.rs_type(e)),
            Type::Record(_) => format!("TlRec{}", self.record_index(ty)),
            // The enum's identity -- name plus arguments -- names the Rust enum: `ident()`
            // embeds the arguments so each instantiation gets its own declaration.
            Type::Enum { .. } => format!("TlE_{}", ty.ident()),
        }
    }

    /// The expression that reads one value of `ty`: a bare function name for a scalar or a
    /// record, a closure wrapping `tl_parse_vec` for a Vec -- always `fn(&mut TlParser) -> T`,
    /// so a Vec of Vecs nests the same way without needing its own case.
    fn parser_expr(&self, ty: &Type) -> String {
        match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            Type::Str => "tl_parse_str".to_string(),
            Type::Int => "tl_parse_i32".to_string(),
            // The checker refuses Int64 anywhere in an input type: its wire codec is undecided.
            Type::Int64 => unreachable!("input cannot contain an Int64, refused by the checker"),
            Type::Bool => "tl_parse_bool".to_string(),
            // The checker refuses Char anywhere in an input type: it has no wire form.
            Type::Char => unreachable!("input cannot contain a Char, refused by the checker"),
            Type::Vec(e) => format!(
                "(|p: &mut TlParser| tl_parse_vec(p, {}))",
                self.parser_expr(e)
            ),
            // The checker refuses Opt anywhere in an input type: absence has no wire form.
            Type::Stream(_) => unreachable!("Stream cannot be declared, so input never has one"),
            Type::Enum { .. } => format!("tl_parse_enum{}", self.enum_index(ty)),
            Type::Record(_) => format!("tl_parse_rec{}", self.record_index(ty)),
        }
    }

    /// A named parsing function for one enum, dispatching on which of the two JSON shapes
    /// (ADR 0009) arrived: a bare string resolves among the unit variants, a single-key object
    /// among the payload ones, and a near miss is refused naming the enum.
    fn enum_parser(&self, i: usize, en: &Type, name: &str) -> String {
        let variants = ty::variants(self.registry, en);
        let ename = self.rs_type(en);
        let mut out = String::new();
        out.push_str(&format!(
            "fn tl_parse_enum{i}(p: &mut TlParser) -> {ename} {{\n"
        ));
        out.push_str("    if p.peek() == b'\"' {\n");
        out.push_str("        let s = p.parse_str();\n");
        out.push_str("        match s.as_str() {\n");
        for (vname, payload) in &variants {
            if payload.is_none() {
                out.push_str(&format!("            \"{vname}\" => {ename}::V_{vname},\n"));
            }
        }
        out.push_str(&format!(
            "            _ => tl_fail(&format!(\"`{{s}}` is not a unit variant of {name}\")),\n"
        ));
        out.push_str("        }\n    } else {\n");
        out.push_str("        p.expect(b'{');\n");
        out.push_str("        let key = tl_parse_str(p);\n");
        out.push_str("        p.expect(b':');\n");
        out.push_str("        let v = match key.as_str() {\n");
        for (vname, payload) in &variants {
            if let Some(pty) = payload {
                out.push_str(&format!(
                    "            \"{vname}\" => {ename}::V_{vname}(({})(p)),\n",
                    self.parser_expr(pty)
                ));
            }
        }
        out.push_str(&format!(
            "            _ => tl_fail(&format!(\"`{{key}}` is not a payload variant of {name}\")),\n"
        ));
        out.push_str("        };\n");
        // One key is the whole shape, so the wrapper closes right here.
        out.push_str("        p.expect(b'}');\n");
        out.push_str("        v\n    }\n}\n\n");
        out
    }

    /// A named parsing function for one record shape, reading fields in whatever order they
    /// arrive and refusing to finish if any declared field never showed up. Undeclared fields are
    /// skipped rather than rejected, matching `input::validate` on the compiler side.
    fn record_parser(&self, i: usize, fields: &[(String, Type)]) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "fn tl_parse_rec{i}(p: &mut TlParser) -> TlRec{i} {{\n"
        ));
        for (name, ty) in fields {
            out.push_str(&format!(
                "    let mut {}: Option<{}> = None;\n",
                rs_field(name),
                self.rs_type(ty)
            ));
        }
        out.push_str("    p.expect(b'{');\n");
        out.push_str("    if !p.at(b'}') {\n        loop {\n");
        out.push_str("            let key = tl_parse_str(p);\n");
        out.push_str("            p.expect(b':');\n");
        out.push_str("            match key.as_str() {\n");
        for (name, ty) in fields {
            out.push_str(&format!(
                "                \"{name}\" => {} = Some(({})(p)),\n",
                rs_field(name),
                self.parser_expr(ty)
            ));
        }
        out.push_str("                _ => p.skip_value(),\n");
        out.push_str("            }\n");
        out.push_str("            if p.at(b',') { p.p += 1; continue; }\n");
        out.push_str("            break;\n        }\n    }\n");
        out.push_str("    p.expect(b'}');\n");
        out.push_str(&format!("    TlRec{i} {{\n"));
        for (name, _) in fields {
            out.push_str(&format!(
                "        {}: {}.unwrap_or_else(|| tl_fail(\"missing field `{}`\")),\n",
                rs_field(name),
                rs_field(name),
                name
            ));
        }
        out.push_str("    }\n}\n\n");
        out
    }

    /// A stream-typed `jsonlines` program, compiled as a loop that reads one line, runs it through
    /// `fusion`'s stages, and prints it, rather than the eager path's read-everything-then-print.
    /// `read_line` (not `tl_read_lines`) and an explicit `flush()` are both load-bearing: Rust's
    /// stdout is fully buffered rather than line-buffered whenever it is not a terminal, which is
    /// exactly the case piping into another process needs, and buffering the whole run would
    /// defeat the one thing this loop exists for.
    fn fused_main(&self, program: &Program, fusion: &tir::Fusion) -> String {
        let mut out = String::new();
        out.push_str("fn main() {\n");
        out.push_str("    use std::io::{BufRead, Write};\n");
        out.push_str("    let stdin = std::io::stdin();\n");
        out.push_str("    let mut stdin = stdin.lock();\n");
        out.push_str("    let stdout = std::io::stdout();\n");
        out.push_str("    let mut stdout = stdout.lock();\n");
        out.push_str("    let mut line = String::new();\n");
        out.push_str("    loop {\n");
        out.push_str("        line.clear();\n");
        out.push_str(
            "        let n = stdin.read_line(&mut line).unwrap_or_else(|e| tl_fail(&format!(\"could not read stdin: {e}\")));\n",
        );
        out.push_str("        if n == 0 { break; }\n");
        out.push_str("        if line.ends_with('\\n') { line.pop(); }\n");
        let (mut current, mut current_ty) = match fusion.source {
            tir::Source::Inputs => {
                let elem = program
                    .inputs
                    .as_ref()
                    .expect("an inputs source recorded its element");
                out.push_str("        if line.trim().is_empty() { continue; }\n");
                out.push_str(&format!(
                    "        let t_line: {} = tl_parse_line(&line, {});\n",
                    self.rs_type(elem),
                    self.parser_expr(elem)
                ));
                ("t_line".to_string(), elem.clone())
            }
            // A raw line is already the element, blank ones included: `lines` keeps them.
            tir::Source::Lines => {
                out.push_str("        let t_line: String = line.clone();\n");
                ("t_line".to_string(), Type::Str)
            }
        };
        for stage in &fusion.stages {
            match stage {
                tir::Stage::Map { param, body } => {
                    out.push_str(&format!(
                        "        let {}: {} = {};\n",
                        self.local(*param),
                        self.rs_type(&current_ty),
                        current
                    ));
                    current = self.expr(body);
                    current_ty = body.ty.clone();
                }
                tir::Stage::Select { param, pred } => {
                    out.push_str(&format!(
                        "        let {}: {} = {};\n",
                        self.local(*param),
                        self.rs_type(&current_ty),
                        current
                    ));
                    out.push_str(&format!(
                        "        if !({}) {{ continue; }}\n",
                        self.expr(pred)
                    ));
                    current = format!("{}.clone()", self.local(*param));
                }
            }
        }
        let printed = self.show(&current_ty, &current, 0);
        out.push_str(&format!(
            "        writeln!(stdout, \"{{}}\", {printed}).unwrap();\n"
        ));
        out.push_str("        stdout.flush().unwrap();\n");
        out.push_str("    }\n}\n");
        out
    }

    /// Wrap `leaf` in `depth` layers of `.iter().map(...).collect()`, spelling the element type
    /// at each layer the same reason Go's `distribute` does: nothing here carries its own type at
    /// runtime, so the type the checker already computed has to be written back in.
    fn distribute(
        &self,
        value: &str,
        value_ty: &Type,
        result_ty: &Type,
        depth: usize,
        leaf: &dyn Fn(&str) -> String,
    ) -> String {
        if depth == 0 {
            return leaf(value);
        }
        let elem = tir::runtime_elem(value_ty).expect("a dimension to distribute over");
        let result_elem = tir::runtime_elem(result_ty).expect("the result keeps the dimension");
        let var = format!("m{depth}");
        format!(
            "{value}.iter().map(|{var}: &{}| -> {} {{ {} }}).collect::<Vec<_>>()",
            self.rs_type(elem),
            self.rs_type(result_elem),
            self.distribute(
                &format!("{var}.clone()"),
                elem,
                result_elem,
                depth - 1,
                leaf
            )
        )
    }

    fn expr(&self, t: &Tir) -> String {
        match &t.kind {
            Kind::Str(s) => rs_string(s),
            Kind::Int(n) => int_lit(&t.ty, *n),
            Kind::Var(name) => format!("{}.clone()", self.user(name)),
            Kind::Local(id) => format!("{}.clone()", self.local(*id)),
            Kind::Input => format!("{INPUT}.clone()"),
            Kind::Inputs => format!("{INPUTS}.clone()"),
            // The stream, materialized eagerly: whatever consumes it -- `collect`, a mapper --
            // works on the Vec of its entries.
            Kind::Lines => "tl_read_lines()".to_string(),
            Kind::RecordLit { fields } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", rs_field(name), self.expr(value)))
                    .collect();
                format!("{} {{ {} }}", self.rs_type(&t.ty), parts.join(", "))
            }
            // Opt's constructors are Option's own: `some(x)` is presence, `none` absence.
            Kind::EnumLit { variant, payload } if t.ty.as_opt().is_some() => match payload {
                Some(p) => format!("Some({})", self.expr(p)),
                None => "None".to_string(),
            },
            Kind::EnumLit { variant, payload } => match payload {
                None => format!("{}::V_{variant}", self.rs_type(&t.ty)),
                Some(p) => format!("{}::V_{variant}({})", self.rs_type(&t.ty), self.expr(p)),
            },
            Kind::VecLit(items) => {
                let parts: Vec<String> = items.iter().map(|i| self.expr(i)).collect();
                format!("vec![{}]", parts.join(", "))
            }
            Kind::Call { func, arg } => format!(
                "{}({})",
                self.user(func),
                arg.as_deref().map_or_else(String::new, |a| self.expr(a))
            ),
            Kind::Concat(l, r) => concat(&t.ty, self.expr(l), self.expr(r)),
            Kind::Arith { op, lhs, rhs } => arith(&t.ty, *op, self.expr(lhs), self.expr(rhs)),
            // A genuine expression, unlike Go: both branches stay unevaluated except the taken
            // one, which is what `if`/`else` already guarantees.
            Kind::Cond {
                cond,
                then,
                otherwise,
            } => format!(
                "(if {} {{ {} }} else {{ {} }})",
                self.expr(cond),
                self.expr(then),
                self.expr(otherwise)
            ),
            Kind::Builtin { which, arg } => match which {
                Builtin::IntToStr => format!("({}).to_string()", self.expr(arg)),
                Builtin::IntToI64 => format!("(({}) as i64)", self.expr(arg)),
                Builtin::Range => format!("tl_range({})", self.expr(arg)),
                Builtin::Chars => format!("tl_chars(&{})", self.expr(arg)),
                Builtin::JsonLines => {
                    let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                    let e = "e0".to_string();
                    format!(
                        "tl_jsonlines(&{}, |{e}: &{}| -> String {{ {} }})",
                        self.expr(arg),
                        self.rs_type(elem),
                        self.show(elem, &format!("{e}.clone()"), 1)
                    )
                }
                // The source already materialized, so the exit has nothing left to do.
                Builtin::Collect => self.expr(arg),
                Builtin::Length => format!("(({}).len() as i32)", self.expr(arg)),
                Builtin::Tail => format!("tl_tail(&{})", self.expr(arg)),
                Builtin::Flatten => format!("tl_flatten(&{})", self.expr(arg)),
                Builtin::Sort => format!("tl_sort(&{})", self.expr(arg)),
                Builtin::Reverse => format!("tl_reverse(&{})", self.expr(arg)),
                // `i32` and `i64` are distinct types in Rust, so the fold is chosen by the
                // element width.
                Builtin::Sum => {
                    if tir::runtime_elem(&arg.ty) == Some(&Type::Int) {
                        format!("tl_sum(&{})", self.expr(arg))
                    } else {
                        format!("tl_sum64(&{})", self.expr(arg))
                    }
                }
                Builtin::Max => format!("tl_max(&{})", self.expr(arg)),
                // The names come from the checked type, not the struct value, so `arg` is
                // evaluated only for whatever else it does (a division inside it must still
                // trap) and its value discarded.
                Builtin::Fields => {
                    let Type::Record(fields) = &arg.ty else {
                        unreachable!("checked to be a record")
                    };
                    let names: Vec<String> = fields.iter().map(|(n, _)| rs_string(n)).collect();
                    format!(
                        "{{ let _ = {}; vec![{}] }}",
                        self.expr(arg),
                        names.join(", ")
                    )
                }
            },
            Kind::Compare { op, lhs, rhs } => {
                format!("({} {} {})", self.expr(lhs), rs_op(*op), self.expr(rhs))
            }
            Kind::Logic { op, lhs, rhs } => {
                let op = match op {
                    LogicOp::And => "&&",
                    LogicOp::Or => "||",
                };
                format!("({} {op} {})", self.expr(lhs), self.expr(rhs))
            }
            Kind::Not(base) => format!("(!{})", self.expr(base)),
            // A block expression, not a closure: Rust has expression-level `let`, so the pipe
            // needs no IIFE the way Go's does. Wrapped in an extra `(...)` so a later `.field`
            // or binary operator chains onto it safely -- a bare `{ ... }` at the start of a
            // statement parses as a block statement, not an expression, and a parenthesized
            // block can never be mistaken for one.
            Kind::Bind {
                local: id,
                value,
                body,
            } => format!(
                "({{ let {}: {} = {}; {} }})",
                self.local(*id),
                self.rs_type(&value.ty),
                self.expr(value),
                self.expr(body)
            ),
            Kind::Map {
                source,
                param,
                body,
            } => format!(
                "{}.iter().map(|{}: &{}| -> {} {{ {} }}).collect::<Vec<_>>()",
                self.expr(source),
                self.local(*param),
                self.rs_type(tir::runtime_elem(&source.ty).expect("map runs over a dimension")),
                self.rs_type(&body.ty),
                self.expr(body)
            ),
            // Opt's reorder pass (kantord/toylang#66): `Option::map` is exactly the
            // present-preserving, absent-preserving rebuild it needs, since Opt already is
            // Rust's own `Option`.
            Kind::OptMap {
                source,
                param,
                body,
            } => format!(
                "({}.map(|{}| {}))",
                self.expr(source),
                self.local(*param),
                self.expr(body)
            ),
            // `.cloned()` before `.filter()` keeps the closure's parameter at exactly one level
            // of reference (`&T`, not `&&T` the way `.iter().filter()` alone would give):
            // `.clone()` on `&T` derefs one level to `T`, but on `&&T` it only strips one level
            // of reference, leaving `&T` -- verified directly, not assumed.
            Kind::Select {
                source,
                param,
                pred,
            } => format!(
                "{}.iter().cloned().filter(|{}: &{}| -> bool {{ {} }}).collect::<Vec<_>>()",
                self.expr(source),
                self.local(*param),
                self.rs_type(tir::runtime_elem(&source.ty).expect("select runs over a dimension")),
                self.expr(pred)
            ),
            Kind::Field { base, name } => {
                let depth = tir::vec_depth(&base.ty);
                self.distribute(&self.expr(base), &base.ty, &t.ty, depth, &|v| {
                    format!("{v}.{}", rs_field(name))
                })
            }
            Kind::Unwrap { base } => {
                let depth = tir::vec_depth(&base.ty);
                self.distribute(&self.expr(base), &base.ty, &t.ty, depth, &|v| {
                    format!("tl_unwrap({v})")
                })
            }
            Kind::Index {
                base, index, depth, ..
            } => {
                let i = self.expr(index);
                self.distribute(&self.expr(base), &base.ty, &t.ty, *depth, &|v| {
                    format!("tl_at(&{v}, {i})")
                })
            }
            Kind::Slice {
                base, start, end, depth,
            } => {
                let lo = match start {
                    Some(s) => self.expr(s),
                    None => "i32::MIN".to_string(),
                };
                let hi = match end {
                    Some(e) => self.expr(e),
                    None => "i32::MAX".to_string(),
                };
                self.distribute(&self.expr(base), &base.ty, &t.ty, *depth, &|v| {
                    format!("tl_slice(&{v}, {lo}, {hi})")
                })
            }
            // The one target with the construct itself: a toylang match is a Rust match, and
            // for a chain of variant arms rustc re-proves the exhaustiveness the checker
            // already established. A default arm is `_`, a guard arm `_ if cond` (the guard
            // reads the subject's own local, not the scrutinee, so no borrow of the match is
            // involved); a duplicate or dead arm costs a warning, not an error. A partial
            // chain wraps each body in `Some` and ends `_ => None`, which is also what keeps
            // rustc's exhaustiveness proof satisfied there.
            Kind::Match {
                subject,
                arms,
                partial,
            } => {
                let ename = self.rs_type(&subject.ty);
                let mut rendered: Vec<String> = arms
                    .iter()
                    .map(|arm| {
                        let body = if *partial {
                            format!("Some({})", self.expr(&arm.body))
                        } else {
                            self.expr(&arm.body)
                        };
                        match (&arm.variant, &arm.guard) {
                            (None, Some(g)) => format!("_ if {} => {body}", self.expr(g)),
                            (None, None) => format!("_ => {body}"),
                            (Some(v), _) => {
                                let has_payload = ty::variants(self.registry, &subject.ty)
                                    .iter()
                                    .find(|(n, _)| n == v)
                                    .expect("the checker resolved the variant")
                                    .1
                                    .is_some();
                                if has_payload {
                                    let pid = arm
                                        .payload
                                        .expect("a payload arm always binds its payload");
                                    format!("{ename}::V_{v}({}) => {body}", self.local(pid))
                                } else {
                                    format!("{ename}::V_{v} => {body}")
                                }
                            }
                        }
                    })
                    .collect();
                if *partial {
                    rendered.push("_ => None".to_string());
                }
                format!(
                    "(match {} {{ {} }})",
                    self.expr(subject),
                    rendered.join(", ")
                )
            }
        }
    }

    /// The printer is built from the type rather than by inspecting the value, on every backend
    /// with no runtime type information: a Rust value cannot be asked what it is any more than a
    /// Go one can.
    fn show(&self, ty: &Type, value: &str, depth: usize) -> String {
        match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
            // The checker refuses a program whose result contains a Char: it has no wire form.
            Type::Char => unreachable!("Char cannot reach the printer, refused by the checker"),
            Type::Str => format!("tl_quote(&{value})"),
            Type::Int | Type::Int64 => format!("({value}).to_string()"),
            Type::Bool => format!("({value}).to_string()"),
            Type::Vec(elem) => {
                let e = format!("e{depth}");
                format!(
                    "tl_join(&{value}, |{e}: &{}| -> String {{ {} }})",
                    self.rs_type(elem),
                    self.show(elem, &format!("{e}.clone()"), depth + 1)
                )
            }
            Type::Enum { .. } if ty.as_opt().is_some() => {
                let inner = ty.as_opt().expect("guarded");
                let v = format!("o{depth}");
                format!(
                    "(match {value} {{ None => \"null\".to_string(), Some({v}) => {} }})",
                    self.show(inner, &v, depth + 1)
                )
            }
            // A recursive enum prints through a function of its own (`printers`), because
            // expanding one here has no bottom: its payload leads back to the same type.
            Type::Enum { .. } if ty::is_recursive(self.registry, ty) => {
                format!("{}({value})", ty.show_fn())
            }
            Type::Enum { .. } => self.show_enum(ty, value, depth),
            Type::Record(fields) => {
                if fields.is_empty() {
                    return "\"{}\".to_string()".to_string();
                }
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(name, fty)| {
                        let read = format!("({value}).{}", rs_field(name));
                        let key = rs_string(&format!("\"{name}\":"));
                        format!("{key}.to_string() + &{}", self.show(fty, &read, depth + 1))
                    })
                    .collect();
                format!(
                    "(\"{{\".to_string() + &{} + \"}}\")",
                    parts.join(" + \",\" + &")
                )
            }
        }
    }

    /// A named printer for every recursive enum the program prints. The call in `show` is what a
    /// nested occurrence renders as, so the recursion in the type becomes recursion in the
    /// emitted function rather than in this compiler (kantord/toylang#94). `non_snake_case`
    /// because the name embeds the enum's own, which is capitalised.
    fn printers(&self, program: &Program) -> String {
        let mut out = String::new();
        for ty in tir::printed_recursive_enums(program) {
            out.push_str(&format!(
                "#[allow(non_snake_case)]\nfn {}(v: {}) -> String {{\n    {}\n}}\n\n",
                ty.show_fn(),
                self.rs_type(&ty),
                self.show_enum(&ty, "v", 0)
            ));
        }
        out
    }

    /// The printer for one enum, inline. The match is the shape dispatch ADR 0009 asks of every
    /// printer, native here: a unit variant renders as its quoted name, a payload variant as the
    /// single-key wrapper.
    fn show_enum(&self, ty: &Type, value: &str, depth: usize) -> String {
        let variants = ty::variants(self.registry, ty);
        let n = format!("n{depth}");
        let arms: Vec<String> = variants
            .iter()
            .map(|(vname, payload)| match payload {
                None => format!(
                    "{}::V_{vname} => {}",
                    self.rs_type(ty),
                    rs_string(&format!("\"{vname}\""))
                ),
                Some(p) => format!(
                    "{}::V_{vname}({n}) => ({} + &{} + \"}}\")",
                    self.rs_type(ty),
                    rs_string(&format!("{{\"{vname}\":")),
                    self.show(p, &n, depth + 1)
                ),
            })
            .collect();
        format!("(match {value} {{ {} }})", arms.join(", "))
    }
}

/// The node's type picks the literal's spelling (kantord/toylang#83). The `i64` suffix types
/// the wide literal directly; `tl_int`'s constant-folding escape is not needed at 64 bits,
/// since every arithmetic spelling here is a `wrapping_*` call the compiler never folds into
/// an overflow error.
fn int_lit(ty: &Type, n: i64) -> String {
    if *ty == Type::Int64 {
        format!("{n}i64")
    } else {
        format!("tl_int({n})")
    }
}

/// `Vec<T>` has no `Add` impl, but the standard library's slice `concat` is exactly
/// this operation for an owned pair.
fn concat(ty: &Type, l: String, r: String) -> String {
    match ty {
        Type::Vec(_) => format!("[{l}, {r}].concat()"),
        _ => format!("({l} + &{r})"),
    }
}

/// One arithmetic expression at the width the node's type names. `wrapping_*` are
/// width-generic method names, so only the div/rem helpers change at 64 bits.
fn arith(ty: &Type, op: BinOp, l: String, r: String) -> String {
    match op {
        BinOp::Div if *ty == Type::Int64 => format!("tl_div64({l}, {r})"),
        BinOp::Rem if *ty == Type::Int64 => format!("tl_rem64({l}, {r})"),
        BinOp::Div => format!("tl_div({l}, {r})"),
        BinOp::Rem => format!("tl_rem({l}, {r})"),
        BinOp::Add => format!("({l}).wrapping_add({r})"),
        BinOp::Sub => format!("({l}).wrapping_sub({r})"),
        BinOp::Mul => format!("({l}).wrapping_mul({r})"),
        other => unreachable!("{other} is not arithmetic"),
    }
}

fn rs_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        other => unreachable!("{other} is not a comparison"),
    }
}

fn rs_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out.push_str(".to_string()");
    out
}
