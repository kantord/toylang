//! The Go backend.
//!
//! Go is the first target that is both statically typed and has no runtime type information to
//! fall back on, and that changes what an emitter has to do. Lua, JavaScript and jq all accept a
//! depth-polymorphic helper -- one `tl_field(v, k, depth)` serves every shape -- because the
//! value carries its own type. LLVM sidesteps the same problem by erasing everything into
//! `tl_vec`. Go will not do either: a `[]User` and a `[][]User` are different types, so the
//! distribution over dimensions has to be spelled out at each use site, with the intermediate
//! types written in. The checker computed those types already, so what looks like extra work is
//! really the first target that asks for what was always there.
//!
//! The second thing Go asks for is a *name*. A record type is structural here and anonymous
//! everywhere else; Go needs a declared struct before a value of one can exist, so every record
//! the program mentions is collected up front and given one.

use std::collections::BTreeSet;

use crate::ast::{BinOp, LogicOp};
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::{self, Enums, Type};

/// The binding the input value is read into. Unspellable in source, since every source name is
/// prefixed.
const INPUT: &str = "t_input";
const INPUTS: &str = "t_inputs";

/// Absence is a field rather than a nil pointer: `Opt<Vec<T>>` would otherwise have two
/// spellings of empty, and a nil slice is a perfectly ordinary present value here.
const OPT_TYPE: &str = r#"type tlOpt[T any] struct {
	ok bool
	v  T
}
"#;

const FAIL_HELPER: &str = r#"func tlFail(msg string) {
	fmt.Fprintln(os.Stderr, "toylang: "+msg)
	os.Exit(1)
}
"#;

const MAP_HELPER: &str = r#"func tlMap[A, B any](src []A, f func(A) B) []B {
	out := make([]B, len(src))
	for i, e := range src {
		out[i] = f(e)
	}
	return out
}
"#;

const SELECT_HELPER: &str = r#"func tlSelect[T any](src []T, pred func(T) bool) []T {
	out := []T{}
	for _, e := range src {
		if pred(e) {
			out = append(out, e)
		}
	}
	return out
}
"#;

const AT_HELPER: &str = r#"func tlAt[T any](v []T, i int32) tlOpt[T] {
	n := int32(len(v))
	if i < 0 {
		i = n + i
	}
	if i < 0 || i >= n {
		return tlOpt[T]{}
	}
	return tlOpt[T]{true, v[i]}
}
"#;

// Go's own slicing panics out of range, so the jq clamp has to be explicit: negatives count
// from the end, then both bounds clamp to [0, n], and a crossed window is empty. A bound left
// out is passed as its sentinel (`MinInt32` for the start, `MaxInt32` for the end), each of
// which the clamp folds to the array's own boundary.
const SLICE_HELPER: &str = r#"func tlSlice[T any](v []T, lo int32, hi int32) []T {
	n := int32(len(v))
	if lo < 0 {
		lo += n
	}
	if hi < 0 {
		hi += n
	}
	if lo < 0 {
		lo = 0
	}
	if lo > n {
		lo = n
	}
	if hi < 0 {
		hi = 0
	}
	if hi > n {
		hi = n
	}
	if lo >= hi {
		return []T{}
	}
	return v[lo:hi]
}
"#;

const UNWRAP_HELPER: &str = r#"func tlUnwrap[T any](o tlOpt[T]) T {
	if !o.ok {
		tlFail("unwrapped a value that is not there")
	}
	return o.v
}
"#;

const TAIL_HELPER: &str = r#"func tlTail[T any](v []T) tlOpt[[]T] {
	if len(v) == 0 {
		return tlOpt[[]T]{}
	}
	return tlOpt[[]T]{true, v[1:]}
}
"#;

const FLATTEN_HELPER: &str = r#"func tlFlatten[T any](vv [][]T) []T {
	out := []T{}
	for _, v := range vv {
		out = append(out, v...)
	}
	return out
}
"#;

// `cmp.Ordered` is exactly the constraint the checker's own `orderable` restricts `sort`'s
// element type to, so nothing here has to name Int, Int64, Str, or Char individually.
const SORT_HELPER: &str = r#"func tlSort[T cmp.Ordered](v []T) []T {
	out := append([]T{}, v...)
	slices.Sort(out)
	return out
}
"#;

const REVERSE_HELPER: &str = r#"func tlReverse[T any](v []T) []T {
	out := make([]T, len(v))
	for i, x := range v {
		out[len(v)-1-i] = x
	}
	return out
}
"#;

// `int32`/`int64` `+=` wraps by definition, so a fold needs nothing but the loop itself -- the
// same "the width wraps on its own" story ARITH_HELPER and ARITH64_HELPER tell for one operator.
const SUM_HELPER: &str = r#"func tlSum(v []int32) int32 {
	var acc int32
	for _, x := range v {
		acc += x
	}
	return acc
}

func tlSum64(v []int64) int64 {
	var acc int64
	for _, x := range v {
		acc += x
	}
	return acc
}
"#;

// `cmp.Ordered` covers both integer widths, so one helper serves Int and Int64. Absence is the
// zero `tlOpt`, the same answer an out-of-range index gives.
const MAX_HELPER: &str = r#"func tlMax[T cmp.Ordered](v []T) tlOpt[T] {
	if len(v) == 0 {
		return tlOpt[T]{}
	}
	m := v[0]
	for _, x := range v[1:] {
		if x > m {
			m = x
		}
	}
	return tlOpt[T]{true, m}
}
"#;

/// Go's `int32` wraps on overflow by definition, and its `/` and `%` truncate toward zero, so
/// only the zero divisor needs a guard. `MIN / -1` is defined to be `MIN` here, which is the
/// wrapping answer the other backends were made to give.
const ARITH_HELPER: &str = r#"func tlDiv(a, b int32) int32 {
	if b == 0 {
		tlFail("divided by zero")
	}
	return a / b
}

func tlRem(a, b int32) int32 {
	if b == 0 {
		tlFail("divided by zero")
	}
	return a % b
}
"#;

/// The same shape at 64 bits (kantord/toylang#83): Go defines `MIN / -1` as `MIN` for int64
/// exactly as for int32, so only the zero divisor needs the guard here too.
const ARITH64_HELPER: &str = r#"func tlDiv64(a, b int64) int64 {
	if b == 0 {
		tlFail("divided by zero")
	}
	return a / b
}

func tlRem64(a, b int64) int64 {
	if b == 0 {
		tlFail("divided by zero")
	}
	return a % b
}
"#;

// An enum value carries its payload behind a pointer (tlPtr below), so Go's own `==` on two
// of them compares addresses: `circle{r: 1} == circle{r: 1}` was false here and true on
// Python (kantord/toylang#68). reflect.DeepEqual is the structural walk, following the
// pointer and comparing what it points at.
const EQ_HELPER: &str = r#"func tlEq[T any](a, b T) bool { return reflect.DeepEqual(a, b) }
"#;

/// Ad-hoc address-of: `&expr` only works on composite literals, and a payload is whatever
/// expression the program wrote. The call inlines away.
const PTR_HELPER: &str = r#"func tlPtr[T any](v T) *T { return &v }
"#;

/// Go folds constant arithmetic exactly and rejects a result that does not fit, which is the
/// direct opposite of the wrapping rule: `2147483647 + 1` is a compile error rather than
/// `-2147483648`. Passing every literal through a function makes the expression non-constant, so
/// the wrap happens at runtime where it is defined. The call inlines away.
const INT_HELPER: &str = r#"func tlInt(n int32) int32 { return n }
"#;

/// The same constant-folding escape at 64 bits: `tlInt64(9223372036854775807) + tlInt64(1)`
/// must wrap at runtime rather than fail Go's exact constant arithmetic.
const INT64_HELPER: &str = r#"func tlInt64(n int64) int64 { return n }
"#;

// bufio.ScanLines strips a trailing \r along with the \n, which the other five backends do
// not, so the split function is copied here with that one line removed rather than reused.
const COLLECT_HELPER: &str = r#"func tlScanLines(data []byte, atEOF bool) (advance int, token []byte, err error) {
	if atEOF && len(data) == 0 {
		return 0, nil, nil
	}
	if i := bytes.IndexByte(data, '\n'); i >= 0 {
		return i + 1, data[0:i], nil
	}
	if atEOF {
		return len(data), data, nil
	}
	return 0, nil, nil
}

func tlCollectLines() []string {
	out := []string{}
	s := bufio.NewScanner(os.Stdin)
	s.Buffer(make([]byte, 0, 65536), 1024*1024)
	s.Split(tlScanLines)
	for s.Scan() {
		out = append(out, s.Text())
	}
	return out
}
"#;

// RFC 4180 field splitting over the raw lines `tlCollectLines` keeps: a field wrapped in double
// quotes may contain the delimiter, a doubled `""` is a literal quote, and a quoted field may
// span lines. The delimiter is matched literally, never as a pattern.
const DSV_HELPER: &str = r#"func tlDsv(lines []string, sep string) [][]string {
	out := [][]string{}
	row := []string{}
	field := ""
	inQuotes := false
	for _, line := range lines {
		i := 0
		for i < len(line) {
			if inQuotes {
				if line[i] == '"' {
					if i+1 < len(line) && line[i+1] == '"' {
						field += "\""
						i += 2
					} else {
						inQuotes = false
						i++
					}
				} else {
					_, size := utf8.DecodeRuneInString(line[i:])
					field += line[i : i+size]
					i += size
				}
			} else if strings.HasPrefix(line[i:], sep) {
				row = append(row, field)
				field = ""
				i += len(sep)
			} else if line[i] == '"' && field == "" {
				inQuotes = true
				i++
			} else {
				_, size := utf8.DecodeRuneInString(line[i:])
				field += line[i : i+size]
				i += size
			}
		}
		if inQuotes {
			field += "\n"
		} else {
			row = append(row, field)
			out = append(out, row)
			row = []string{}
			field = ""
		}
	}
	if inQuotes {
		field = field[:len(field)-1]
		row = append(row, field)
		out = append(out, row)
	}
	return out
}
"#;

const RANGE_HELPER: &str = r#"func tlRange(n int32) []int32 {
	if n < 0 {
		n = 0
	}
	out := make([]int32, n)
	for i := int32(0); i < n; i++ {
		out[i] = i
	}
	return out
}
"#;

// Go's `for range` over a string already decodes UTF-8 into runes -- Unicode scalar values --
// rather than bytes, so there is no decoding to get right here.
const CHARS_HELPER: &str = r#"func tlChars(s string) []int32 {
	out := []int32{}
	for _, r := range s {
		out = append(out, int32(r))
	}
	return out
}
"#;

const JSONLINES_HELPER: &str = r#"func tlJsonlines[T any](v []T, f func(T) string) string {
	parts := make([]string, len(v))
	for i, e := range v {
		parts[i] = f(e)
	}
	return strings.Join(parts, "\n")
}
"#;

const JOIN_HELPER: &str = r#"func tlJoin[T any](v []T, f func(T) string) string {
	parts := make([]string, len(v))
	for i, e := range v {
		parts[i] = f(e)
	}
	return "[" + strings.Join(parts, ",") + "]"
}
"#;

/// Hand-written rather than `json.Marshal`, which escapes `<`, `>` and `&` and would print a
/// different string than the other four backends for the same value.
const QUOTE_HELPER: &str = r#"func tlQuote(s string) string {
	var b strings.Builder
	b.WriteByte('"')
	for i := 0; i < len(s); i++ {
		c := s[i]
		switch c {
		case '"':
			b.WriteString("\\\"")
		case '\\':
			b.WriteString("\\\\")
		case '\n':
			b.WriteString("\\n")
		case '\r':
			b.WriteString("\\r")
		case '\t':
			b.WriteString("\\t")
		default:
			if c < 0x20 {
				fmt.Fprintf(&b, "\\u%04x", c)
			} else {
				b.WriteByte(c)
			}
		}
	}
	b.WriteByte('"')
	return b.String()
}
"#;

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
        decls.push_str(&format!("type tlRec{i} struct {{\n"));
        for (name, ty) in fields {
            // Exported, because encoding/json only fills fields it can see. The `F` prefix keeps
            // the mapping injective, so two toylang fields cannot collide on one Go field.
            decls.push_str(&format!("\tF{name} {} `json:\"{name}\"`\n", e.go_type(ty)));
        }
        decls.push_str("}\n\n");
    }

    // Go has no sum type, so an enum is a tag (the variant's declaration index) plus one
    // pointer per payload variant, nil except for the one the tag names.
    for en in &e.enums {
        let Type::Enum { name, .. } = en else {
            unreachable!("only enums are collected")
        };
        decls.push_str(&format!("type {} struct {{\n\ttag int32\n", e.go_type(en)));
        for (i, (_, payload)) in ty::variants(&program.enums, en).iter().enumerate() {
            if let Some(p) = payload {
                decls.push_str(&format!("\tp{i} *{}\n", e.go_type(p)));
            }
        }
        decls.push_str("}\n\n");
        // encoding/json cannot guess which of the enum's two wire shapes (ADR 0009) it is
        // looking at, so decoding is spelled out. Only a program that reads input decodes.
        if program.input.is_some() || program.inputs.is_some() {
            decls.push_str(&e.enum_unmarshal(en, name));
        }
    }

    decls.push_str(&e.printers(program));

    // Package-level functions are visible in any order, so the forward reference the checker
    // accepts needs nothing here. Lua wanted declarations and JavaScript relied on hoisting.
    for f in &program.funcs {
        let param = match (&f.param, &f.param_ty) {
            (Some(name), Some(ty)) => format!("{} {}", e.user(name), e.go_type(ty)),
            (None, None) => String::new(),
            _ => unreachable!("a function's param and param_ty agree"),
        };
        decls.push_str(&format!(
            "func {}({}) {} {{\n\treturn {}\n}}\n\n",
            e.user(&f.name),
            param,
            e.go_type(&f.body.ty),
            e.expr(&f.body)
        ));
    }

    if let Some(fusion) = tir::fusion(program) {
        decls.push_str(&e.fused_main(program, &fusion));
    } else {
        decls.push_str("func main() {\n");
        if let Some(ty) = &program.input {
            decls.push_str(&format!("\tvar {INPUT} {}\n", e.go_type(ty)));
            decls.push_str(&format!(
                "\tif err := json.NewDecoder(os.Stdin).Decode(&{INPUT}); err != nil {{\n\t\ttlFail(err.Error())\n\t}}\n"
            ));
        }
        if let Some(ty) = &program.inputs {
            // Decode in a loop until io.EOF rather than reading one array token: the wire format
            // is consecutive top-level JSON values, one per line, not a single `[...]`.
            decls.push_str(&format!(
                "\tvar {INPUTS} []{}\n\t{{\n\t\tdec := json.NewDecoder(os.Stdin)\n\t\tfor {{\n\t\t\tvar item {}\n\t\t\tif err := dec.Decode(&item); err != nil {{\n\t\t\t\tif err == io.EOF {{\n\t\t\t\t\tbreak\n\t\t\t\t}}\n\t\t\t\ttlFail(err.Error())\n\t\t\t}}\n\t\t\t{INPUTS} = append({INPUTS}, item)\n\t\t}}\n\t}}\n",
                e.go_type(ty),
                e.go_type(ty)
            ));
        }
        let body = e.expr(&program.body);
        // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON.
        let printed = if matches!(program.body.ty, Type::Str | Type::Sink) {
            body
        } else {
            e.show(&program.body.ty, &body, 0)
        };
        decls.push_str(&format!("\tfmt.Println({printed})\n}}\n"));
    }

    // Which helpers to include is read off the emitted text rather than tracked alongside it. An
    // unused function is not an error in Go, so a false positive costs nothing, while a missed
    // one would not compile -- the asymmetry is what makes reading the output safe. Imports are
    // the opposite (an unused one is an error), so those come from the walk instead.
    let uses = |name: &str| decls.contains(name);
    let unwrap = uses("tlUnwrap(");
    let arith = uses("tlDiv(") || uses("tlRem(");
    let arith64 = uses("tlDiv64(") || uses("tlRem64(");
    let collect = uses("tlCollectLines(") || uses("tlScanLines");
    let fail = unwrap || arith || arith64 || program.input.is_some() || program.inputs.is_some();
    let quote = uses("tlQuote(");
    let join = uses("tlJoin(");

    let mut helpers = String::new();
    // tlOpt is what tlAt and tlUnwrap are written in terms of, and inference means the emitted
    // text need never spell it. Helper-to-helper dependencies are stated rather than read back.
    if uses("tlOpt[") || uses("tlAt(") || uses("tlTail(") || unwrap {
        helpers.push_str(OPT_TYPE);
        helpers.push('\n');
    }
    for (on, text) in [
        (fail, FAIL_HELPER),
        (uses("tlPtr("), PTR_HELPER),
        (uses("tlEq("), EQ_HELPER),
        (uses("tlInt("), INT_HELPER),
        (uses("tlInt64("), INT64_HELPER),
        (uses("tlMap("), MAP_HELPER),
        (uses("tlSelect("), SELECT_HELPER),
        (uses("tlAt("), AT_HELPER),
        (uses("tlSlice("), SLICE_HELPER),
        (uses("tlTail("), TAIL_HELPER),
        (uses("tlFlatten("), FLATTEN_HELPER),
        (uses("tlSort("), SORT_HELPER),
        (uses("tlReverse("), REVERSE_HELPER),
        (uses("tlSum(") || uses("tlSum64("), SUM_HELPER),
        (uses("tlMax("), MAX_HELPER),
        (unwrap, UNWRAP_HELPER),
        (arith, ARITH_HELPER),
        (arith64, ARITH64_HELPER),
        (uses("tlRange("), RANGE_HELPER),
        (uses("tlChars("), CHARS_HELPER),
        (collect, COLLECT_HELPER),
        (uses("tlDsv("), DSV_HELPER),
        (used.jsonlines, JSONLINES_HELPER),
        (join, JOIN_HELPER),
        (quote, QUOTE_HELPER),
    ] {
        if on {
            helpers.push_str(text);
            helpers.push('\n');
        }
    }

    // Go rejects an unused import, so each one is gated on the thing that spells it. Written as
    // a table rather than a chain of `if`s, the same shape the helper text above already uses.
    let reads_stdin = program.input.is_some() || program.inputs.is_some();
    let mut imports = BTreeSet::new();
    imports.insert("fmt");
    for (on, names) in [
        (fail || reads_stdin || collect, &["os"][..]),
        (collect, &["bufio", "bytes"]),
        (reads_stdin, &["encoding/json"]),
        (program.inputs.is_some(), &["io"]),
        (join || quote || used.jsonlines || uses("tlDsv("), &["strings"]),
        (uses("tlDsv("), &["unicode/utf8"]),
        (uses("tlSort("), &["cmp", "slices"]),
        (uses("tlMax("), &["cmp"]),
        (uses("tlEq("), &["reflect"]),
        (
            used.itoa
                || used.jsonlines_has_scalar
                || (program.body.ty != Type::Str && has_scalar(&program.enums, &program.body.ty)),
            &["strconv"],
        ),
    ] {
        if on {
            imports.extend(names);
        }
    }

    let mut out = String::from("package main\n\nimport (\n");
    for name in &imports {
        out.push_str(&format!("\t\"{name}\"\n"));
    }
    out.push_str(")\n\n");
    out.push_str(&helpers);
    out.push_str(&decls);
    out
}

/// Whether printing this type reaches an Int or a Bool, which are the two `strconv` needs.
fn has_scalar(enums: &Enums, ty: &Type) -> bool {
    /// `seen` is what a recursive enum needs: its own payload leads back to it, and an enum
    /// already under consideration answers nothing new.
    fn reaches(enums: &Enums, ty: &Type, seen: &mut Vec<Type>) -> bool {
        match ty {
            // Only ever called on the program's own result type, which the checker guarantees is
            // never a stream and never contains one.
            Type::Stream(_) => unreachable!("a stream cannot reach has_scalar"),
            // The checker refuses a program whose result contains a Char, the same as a stream.
            Type::Char => unreachable!("a Char cannot reach has_scalar"),
            Type::Int | Type::Int64 | Type::Bool => true,
            // A Float prints as a plain number, which needs no strconv import.
            Type::Float => false,
            Type::Str => false,
            Type::Sink => false,
            Type::Vec(t) => reaches(enums, t, seen),
            Type::Record(fields) => fields.iter().any(|(_, t)| reaches(enums, t, seen)),
            Type::Enum { .. } => {
                if seen.contains(ty) {
                    return false;
                }
                seen.push(ty.clone());
                ty::variants(enums, ty)
                    .iter()
                    .any(|(_, p)| p.as_ref().is_some_and(|p| reaches(enums, p, seen)))
            }
            Type::Param(_) => unreachable!("params are substituted before emit"),
        }
    }
    reaches(enums, ty, &mut Vec::new())
}

#[derive(Default)]
struct Used {
    itoa: bool,
    /// Whether `jsonlines` was called at all, which decides whether `strings` needs importing.
    jsonlines: bool,
    /// Whether `strconv` is needed for a scalar inside a `jsonlines` element type, which the
    /// ordinary `has_scalar` check on the program's own result type misses whenever that result
    /// is exactly `Str` -- true for every `jsonlines` call, since that is what it returns.
    jsonlines_has_scalar: bool,
}

/// One walk, collecting the record types that need declaring and the two builtins whose imports
/// cannot be read back off the emitted text without risking a false positive.
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
            // Opt keeps the tlOpt[T] struct it always had (already tagged), so it is not
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
            | Kind::Float(_)
            | Kind::Var(_)
            | Kind::Local(_)
            | Kind::Input
            | Kind::Inputs
            | Kind::Lines
            | Kind::Dsv { .. } => {}
            Kind::VecLit(items) => items.iter().for_each(|i| self.walk(i)),
            Kind::RecordLit { fields } => {
                fields.iter().for_each(|(_, v)| self.walk(v));
            }
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
                match which {
                    Builtin::IntToStr => self.used.itoa = true,
                    Builtin::JsonLines => {
                        self.used.jsonlines = true;
                        let elem =
                            tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                        self.used.jsonlines_has_scalar |= has_scalar(self.registry, elem);
                    }
                    // Purely textually gated below, like tlAt and tlRange: nothing here needs
                    // the element type, so there is nothing to record on the walk.
                    Builtin::IntToI64
                    | Builtin::Range
                    | Builtin::Collect
                    | Builtin::Length
                    | Builtin::Flatten
                    | Builtin::Tail
                    | Builtin::Fields
                    | Builtin::Chars
                    | Builtin::Sort
                    | Builtin::Reverse
                    | Builtin::Sum
                    | Builtin::Max => {}
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

    fn go_type(&self, ty: &Type) -> String {
        match ty {
            Type::Str => "string".to_string(),
            // A sink is a joined string at runtime, so a `-> Sink` function has one here too.
            Type::Sink => "string".to_string(),
            // The default Int is 32 bits and wraps, and Go's int32 does exactly that for free.
            Type::Int => "int32".to_string(),
            // Same story a word wider (kantord/toylang#83).
            Type::Int64 => "int64".to_string(),
            Type::Bool => "bool".to_string(),
            // Same width as Int: a Char is a codepoint, and the checker already refuses to mix
            // the two.
            Type::Char => "int32".to_string(),
            Type::Float => unreachable!("Float is JS-only in this row"),
            Type::Vec(e) => format!("[]{}", self.go_type(e)),
            Type::Enum { .. } if ty.as_opt().is_some() => {
                format!("tlOpt[{}]", self.go_type(ty.as_opt().expect("guarded")))
            }
            // Materialized eagerly as the slice of its entries, so it is a slice here too.
            // Fusion is what will remove this materialization.
            Type::Stream(e) => format!("[]{}", self.go_type(e)),
            Type::Record(_) => {
                let i = self
                    .records
                    .iter()
                    .position(|r| r == ty)
                    .expect("every record reachable from the program was collected");
                format!("tlRec{i}")
            }
            // The enum's identity -- name plus arguments -- names the struct: `ident()`
            // embeds the arguments so each instantiation gets its own.
            Type::Enum { .. } => format!("tlE_{}", ty.ident()),
            Type::Param(_) => unreachable!("params are substituted before emit"),
        }
    }

    /// The declaration index of `variant`, which is its tag and its pointer field's suffix.
    /// A partial chain's yield is an Opt, so a present arm wraps its value in the tag
    /// struct; total chains return the body bare. Split out of `expr`'s match arm to keep
    /// the tagging decision from deepening it (kantord/toylang#62).
    fn arm_yield(go_ty: String, body: String, partial: bool) -> String {
        if partial {
            format!("{go_ty}{{true, {body}}}")
        } else {
            body
        }
    }

    fn variant_index(variants: &[(String, Option<Type>)], variant: &str) -> usize {
        variants
            .iter()
            .position(|(n, _)| n == variant)
            .expect("the checker resolved the variant against this enum")
    }

    /// The decoder for one enum: a bare string resolves among the unit variants, a single-key
    /// object among the payload ones, and a near miss is refused naming the enum.
    fn enum_unmarshal(&self, en: &Type, name: &str) -> String {
        let variants = ty::variants(self.registry, en);
        let ename = self.go_type(en);
        let mut out = String::new();
        out.push_str(&format!(
            "func (e *{ename}) UnmarshalJSON(b []byte) error {{\n"
        ));
        out.push_str("\tvar s string\n");
        out.push_str("\tif json.Unmarshal(b, &s) == nil {\n");
        out.push_str("\t\tswitch s {\n");
        for (i, (vname, payload)) in variants.iter().enumerate() {
            if payload.is_none() {
                out.push_str(&format!(
                    "\t\tcase \"{vname}\":\n\t\t\t*e = {ename}{{tag: {i}}}\n\t\t\treturn nil\n"
                ));
            }
        }
        out.push_str("\t\t}\n");
        out.push_str(&format!(
            "\t\treturn fmt.Errorf(\"`%s` is not a unit variant of {name}\", s)\n\t}}\n"
        ));
        let payloads: Vec<(usize, &String, &Type)> = variants
            .iter()
            .enumerate()
            .filter_map(|(i, (v, p))| p.as_ref().map(|p| (i, v, p)))
            .collect();
        if payloads.is_empty() {
            out.push_str(&format!("\treturn fmt.Errorf(\"expected {name}\")\n}}\n\n"));
            return out;
        }
        out.push_str("\tvar m map[string]json.RawMessage\n");
        out.push_str("\tif err := json.Unmarshal(b, &m); err != nil || len(m) != 1 {\n");
        out.push_str(&format!(
            "\t\treturn fmt.Errorf(\"expected {name}\")\n\t}}\n"
        ));
        out.push_str("\tfor k, v := range m {\n");
        out.push_str("\t\tswitch k {\n");
        for (i, vname, pty) in &payloads {
            out.push_str(&format!("\t\tcase \"{vname}\":\n"));
            out.push_str(&format!("\t\t\tvar p {}\n", self.go_type(pty)));
            out.push_str("\t\t\tif err := json.Unmarshal(v, &p); err != nil {\n\t\t\t\treturn err\n\t\t\t}\n");
            out.push_str(&format!(
                "\t\t\t*e = {ename}{{tag: {i}, p{i}: &p}}\n\t\t\treturn nil\n"
            ));
        }
        out.push_str("\t\t}\n");
        out.push_str(&format!(
            "\t\treturn fmt.Errorf(\"`%s` is not a payload variant of {name}\", k)\n\t}}\n"
        ));
        out.push_str(&format!("\treturn fmt.Errorf(\"expected {name}\")\n}}\n\n"));
        out
    }

    /// A `jsonlines(f(inputs))` program, compiled as a loop that decodes one JSON value at a
    /// time (the same `json.Decoder`-until-`io.EOF` loop the eager `inputs` path already used,
    /// just without collecting into a slice first) and prints it immediately. No explicit flush
    /// is needed: unlike Rust, Python, jq and Lua, `fmt.Println` writes straight through to the
    /// underlying `os.Stdout` file descriptor with no userspace buffering in between.
    ///
    /// Each stage's binding is followed by `_ = t_N`: it is a `:=` local, not a closure parameter
    /// the way the eager path's `tlMap`/`tlSelect` callback argument is, and Go rejects an unused
    /// local at compile time -- a `select` whose predicate is the only use, or a `map` whose body
    /// ignores its argument entirely, would otherwise fail to build only in the fused case.
    fn fused_main(&self, program: &Program, fusion: &tir::Fusion) -> String {
        let mut out = String::new();
        out.push_str("func main() {\n");
        let (mut current, mut current_ty) = match fusion.source {
            tir::Source::Inputs => {
                let elem = program
                    .inputs
                    .as_ref()
                    .expect("an inputs source recorded its element");
                out.push_str("\tdec := json.NewDecoder(os.Stdin)\n");
                out.push_str("\tfor {\n");
                out.push_str(&format!("\t\tvar t_line {}\n", self.go_type(elem)));
                out.push_str(
                    "\t\tif err := dec.Decode(&t_line); err != nil {\n\t\t\tif err == io.EOF {\n\t\t\t\tbreak\n\t\t\t}\n\t\t\ttlFail(err.Error())\n\t\t}\n",
                );
                ("t_line".to_string(), elem.clone())
            }
            // The same scanner the eager collect helper uses, one line per Scan; a raw line is
            // already the element, blank ones included.
            tir::Source::Lines => {
                out.push_str("\ts := bufio.NewScanner(os.Stdin)\n");
                out.push_str("\ts.Buffer(make([]byte, 0, 65536), 1024*1024)\n");
                out.push_str("\ts.Split(tlScanLines)\n");
                out.push_str("\tfor s.Scan() {\n");
                out.push_str("\t\tt_line := s.Text()\n");
                ("t_line".to_string(), Type::Str)
            }
            // The bound is evaluated once; the loop counter is the element. A negative bound
            // clamps to zero, the same answer `tlRange` gives eagerly.
            tir::Source::Range(bound) => {
                out.push_str(&format!("\tn := {}\n", self.expr(bound)));
                out.push_str("\tif n < 0 {\n\t\tn = 0\n\t}\n");
                out.push_str("\tfor t_i := int32(0); t_i < n; t_i++ {\n");
                ("t_i".to_string(), Type::Int)
            }
        };
        for stage in &fusion.stages {
            match stage {
                tir::Stage::Map { param, body } => {
                    out.push_str(&format!(
                        "\t\t{} := {}\n\t\t_ = {}\n",
                        self.local(*param),
                        current,
                        self.local(*param)
                    ));
                    current = self.expr(body);
                    current_ty = body.ty.clone();
                }
                tir::Stage::Select { param, pred } => {
                    out.push_str(&format!(
                        "\t\t{} := {}\n\t\t_ = {}\n",
                        self.local(*param),
                        current,
                        self.local(*param)
                    ));
                    out.push_str(&format!(
                        "\t\tif !({}) {{\n\t\t\tcontinue\n\t\t}}\n",
                        self.expr(pred)
                    ));
                    current = self.local(*param);
                }
            }
        }
        let printed = self.show(&current_ty, &current, 0);
        out.push_str(&format!("\t\tfmt.Println({printed})\n"));
        out.push_str("\t}\n}\n");
        out
    }

    /// Wrap `leaf` in `depth` layers of `tlMap`, spelling the element type at each one.
    ///
    /// This is what the other backends get for free from a `depth` argument. Here the map at
    /// each level is a different instantiation, so the types have to be written down -- and they
    /// can be, because every node in the IR carries the type it was checked at.
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
            "tlMap({value}, func({var} {}) {} {{ return {} }})",
            self.go_type(elem),
            self.go_type(result_elem),
            self.distribute(&var, elem, result_elem, depth - 1, leaf)
        )
    }

    fn expr(&self, t: &Tir) -> String {
        match &t.kind {
            Kind::Str(s) => go_string(s),
            Kind::Int(n) => int_lit(&t.ty, *n),
            Kind::Float(_) => unreachable!("Float is JS-only in this row"),
            Kind::Var(name) => self.user(name),
            Kind::Local(id) => self.local(*id),
            Kind::Input => INPUT.to_string(),
            Kind::Inputs => INPUTS.to_string(),
            // The stream, materialized eagerly: whatever consumes it -- `collect`, a mapper --
            // works on the slice of its entries.
            Kind::Lines => "tlCollectLines()".to_string(),
            Kind::Dsv { delim } => format!(
                "tlDsv(tlCollectLines(), {})",
                go_string(delim)
            ),
            // go_type resolves the struct name, and the collector registered it because a
            // record literal carries its own record type.
            Kind::RecordLit { fields } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("F{name}: {}", self.expr(value)))
                    .collect();
                format!("{}{{{}}}", self.go_type(&t.ty), parts.join(", "))
            }

            Kind::EnumLit { variant, payload } => {
                // Opt's constructors build its tlOpt encoding, not a tag struct: `some(x)`
                // is presence, `none` is the zero value.
                if t.ty.as_opt().is_some() {
                    return match payload {
                        Some(p) => {
                            format!("{}{{true, {}}}", self.go_type(&t.ty), self.expr(p))
                        }
                        None => format!("{}{{}}", self.go_type(&t.ty)),
                    };
                }
                let i = Self::variant_index(&ty::variants(self.registry, &t.ty), variant);
                match payload {
                    None => format!("{}{{tag: {i}}}", self.go_type(&t.ty)),
                    Some(p) => format!(
                        "{}{{tag: {i}, p{i}: tlPtr({})}}",
                        self.go_type(&t.ty),
                        self.expr(p)
                    ),
                }
            }

            Kind::VecLit(items) => {
                let parts: Vec<String> = items.iter().map(|i| self.expr(i)).collect();
                format!("{}{{{}}}", self.go_type(&t.ty), parts.join(", "))
            }
            Kind::Call { func, arg } => format!(
                "{}({})",
                self.user(func),
                arg.as_deref().map_or_else(String::new, |a| self.expr(a))
            ),
            Kind::Concat(l, r) => self.concat(&t.ty, l, r),
            Kind::Arith { op, lhs, rhs } => arith(&t.ty, *op, self.expr(lhs), self.expr(rhs)),
            // Go's own `&&`/`||`, which short-circuit, so the right side stays unevaluated
            // exactly where toylang says it does.
            Kind::Logic { op, lhs, rhs } => {
                let op = match op {
                    LogicOp::And => "&&",
                    LogicOp::Or => "||",
                };
                format!("({} {op} {})", self.expr(lhs), self.expr(rhs))
            }
            Kind::Not(base) => format!("(!{})", self.expr(base)),
            Kind::Builtin { which, arg } => match which {
                Builtin::IntToStr => format!("strconv.FormatInt(int64({}), 10)", self.expr(arg)),
                Builtin::IntToI64 => format!("int64({})", self.expr(arg)),
                Builtin::Range => format!("tlRange({})", self.expr(arg)),
                Builtin::Chars => format!("tlChars({})", self.expr(arg)),
                Builtin::JsonLines => {
                    let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                    let e = "e0".to_string();
                    format!(
                        "tlJsonlines({}, func({e} {}) string {{ return {} }})",
                        self.expr(arg),
                        self.go_type(elem),
                        self.show(elem, &e, 1)
                    )
                }
                // The source already materialized, so the exit has nothing left to do.
                Builtin::Collect => self.expr(arg),
                Builtin::Length => format!("int32(len({}))", self.expr(arg)),
                Builtin::Tail => format!("tlTail({})", self.expr(arg)),
                Builtin::Flatten => format!("tlFlatten({})", self.expr(arg)),
                Builtin::Sort => format!("tlSort({})", self.expr(arg)),
                Builtin::Reverse => format!("tlReverse({})", self.expr(arg)),
                // Which fold depends on the element width: int32 and int64 are distinct types in
                // Go, so they cannot share one helper the way a dynamically typed backend can.
                Builtin::Sum => {
                    if tir::runtime_elem(&arg.ty) == Some(&Type::Int) {
                        format!("tlSum({})", self.expr(arg))
                    } else {
                        format!("tlSum64({})", self.expr(arg))
                    }
                }
                Builtin::Max => format!("tlMax({})", self.expr(arg)),
                // The names come from the checked type, not the struct value, so `arg` runs in
                // an ignored parameter -- the same IIFE shape `Bind` uses -- purely for whatever
                // else it does.
                Builtin::Fields => {
                    let Type::Record(fields) = &arg.ty else {
                        unreachable!("checked to be a record")
                    };
                    let names: Vec<String> = fields.iter().map(|(n, _)| go_string(n)).collect();
                    format!(
                        "func(_ {}) []string {{ return []string{{{}}} }}({})",
                        self.go_type(&arg.ty),
                        names.join(", "),
                        self.expr(arg)
                    )
                }
            },
            Kind::Compare { op, lhs, rhs } => self.compare(*op, lhs, rhs),
            Kind::Bind {
                local: id,
                value,
                body,
            } => format!(
                "func({} {}) {} {{ return {} }}({})",
                self.local(*id),
                self.go_type(&value.ty),
                self.go_type(&t.ty),
                self.expr(body),
                self.expr(value)
            ),
            Kind::Map {
                source,
                param,
                body,
            } => format!(
                "tlMap({}, func({} {}) {} {{ return {} }})",
                self.expr(source),
                self.local(*param),
                self.go_type(tir::runtime_elem(&source.ty).expect("map runs over a dimension")),
                self.go_type(&body.ty),
                self.expr(body)
            ),
            Kind::Select {
                source,
                param,
                pred,
            } => format!(
                "tlSelect({}, func({} {}) bool {{ return {} }})",
                self.expr(source),
                self.local(*param),
                self.go_type(tir::runtime_elem(&source.ty).expect("select runs over a dimension")),
                self.expr(pred)
            ),
            // Opt's reorder pass (kantord/toylang#66): the same `!o.ok`/`.v` shape tlUnwrap and
            // the printer already branch on, generalised to rebuild the tlOpt instead of
            // reading through it. `__srcOpt` binds the source once, so evaluating it twice (the
            // `.ok` test, then `.v`) costs nothing beyond a field read.
            Kind::OptMap {
                source,
                param,
                body,
            } => format!(
                "func(__srcOpt {}) {} {{ if !__srcOpt.ok {{ return {1}{{}} }}; {} := __srcOpt.v; _ = {2}; return {1}{{true, {}}} }}({})",
                self.go_type(&source.ty),
                self.go_type(&t.ty),
                self.local(*param),
                self.expr(body),
                self.expr(source)
            ),
            Kind::Field { base, name } => {
                let depth = tir::vec_depth(&base.ty);
                self.distribute(&self.expr(base), &base.ty, &t.ty, depth, &|v| {
                    format!("({v}).F{name}")
                })
            }
            Kind::Unwrap { base } => {
                let depth = tir::vec_depth(&base.ty);
                self.distribute(&self.expr(base), &base.ty, &t.ty, depth, &|v| {
                    format!("tlUnwrap({v})")
                })
            }
            Kind::Index {
                base, index, depth, ..
            } => {
                let i = self.expr(index);
                self.distribute(&self.expr(base), &base.ty, &t.ty, *depth, &|v| {
                    format!("tlAt({v}, {i})")
                })
            }
            Kind::Slice {
                base, start, end, depth,
            } => {
                let lo = match start {
                    Some(s) => self.expr(s),
                    None => "-2147483648".to_string(),
                };
                let hi = match end {
                    Some(e) => self.expr(e),
                    None => "2147483647".to_string(),
                };
                self.distribute(&self.expr(base), &base.ty, &t.ty, *depth, &|v| {
                    format!("tlSlice({v}, {lo}, {hi})")
                })
            }
            // Tests over the subject (a plain local, so re-reading it is free): a tag test for
            // a variant arm, the same chain the enum printer uses, and the guard's own Bool
            // for a guard arm. A total chain's last arm needs no test, the checker having
            // proved nothing else can reach it; a partial chain tests every arm, wraps each
            // body in the present Opt, and falls through to the absent one. `_ =` after a
            // payload binding, as in the fused loop, because Go rejects an unused local and a
            // body is free to ignore its payload.
            Kind::Match {
                subject,
                arms,
                partial,
            } => {
                let subj = self.expr(subject);
                let mut body = String::new();
                for (i, arm) in arms.iter().enumerate() {
                    let mut run = String::new();
                    if let Some(pid) = arm.payload {
                        let variant = arm
                            .variant
                            .as_ref()
                            .expect("only a variant arm has a payload");
                        let vi =
                            Self::variant_index(&ty::variants(self.registry, &subject.ty), variant);
                        run.push_str(&format!(
                            "{} := *{subj}.p{vi}; _ = {}; ",
                            self.local(pid),
                            self.local(pid)
                        ));
                    }
                    let produced =
                        Self::arm_yield(self.go_type(&t.ty), self.expr(&arm.body), *partial);
                    run.push_str(&format!("return {produced}"));
                    let test = match (&arm.variant, &arm.guard) {
                        (Some(v), _) => Some(format!(
                            "{subj}.tag == {}",
                            Self::variant_index(&ty::variants(self.registry, &subject.ty), v)
                        )),
                        (None, Some(g)) => Some(self.expr(g)),
                        (None, None) => None,
                    };
                    match test {
                        Some(test) if *partial || i + 1 < arms.len() => {
                            body.push_str(&format!("if {test} {{ {run} }}; "));
                        }
                        _ => body.push_str(&run),
                    }
                }
                if *partial {
                    body.push_str(&format!("return {}{{}}", self.go_type(&t.ty)));
                }
                format!("func() {} {{ {body} }}()", self.go_type(&t.ty))
            }
        }
    }

    /// Go has no `+` for slices, so a Vec joins through the same helper `flatten` uses, wrapped
    /// around a two-entry outer literal rather than through a second helper.
    fn concat(&self, ty: &Type, l: &Tir, r: &Tir) -> String {
        match ty {
            Type::Vec(_) => {
                let go_ty = self.go_type(ty);
                let (l, r) = (self.expr(l), self.expr(r));
                format!("tlFlatten([]{go_ty}{{{l}, {r}}})")
            }
            _ => format!("({} + {})", self.expr(l), self.expr(r)),
        }
    }

    /// The printer is built from the type rather than by inspecting the value, as on every other
    /// backend. Here there is no choice at all: a Go value cannot be asked what it is.
    /// Equality on a composite is structural, which Go's own `==` is not: it compares an enum's
    /// payload pointer by address (kantord/toylang#68). Ordering on a composite is a separate
    /// open question and keeps whatever Go does with it.
    fn compare(&self, op: BinOp, lhs: &Tir, rhs: &Tir) -> String {
        if !lhs.ty.is_composite() || !matches!(op, BinOp::Eq | BinOp::Ne) {
            return format!("({} {} {})", self.expr(lhs), go_op(op), self.expr(rhs));
        }
        let call = format!("tlEq({}, {})", self.expr(lhs), self.expr(rhs));
        match op {
            BinOp::Ne => format!("(!{call})"),
            _ => call,
        }
    }

    fn show(&self, ty: &Type, value: &str, depth: usize) -> String {
        match ty {
            Type::Param(_) => unreachable!("params are substituted before emit"),
            // The checker refuses a program whose result contains a stream, since there is
            // nothing to print: a stream has no value, only a promise that collect can redeem.
            Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
            Type::Char => unreachable!("Char cannot reach the printer, refused by the checker"),
            Type::Str => format!("tlQuote({value})"),
            Type::Sink => unreachable!("a sink only ever prints raw, never through the printer"),
            Type::Int => format!("strconv.FormatInt(int64({value}), 10)"),
            Type::Int64 => format!("strconv.FormatInt({value}, 10)"),
            Type::Float => unreachable!("Float is JS-only in this row"),
            Type::Bool => format!("strconv.FormatBool({value})"),
            Type::Vec(elem) => {
                let e = format!("e{depth}");
                format!(
                    "tlJoin({value}, func({e} {}) string {{ return {} }})",
                    self.go_type(elem),
                    self.show(elem, &e, depth + 1)
                )
            }
            Type::Enum { .. } if ty.as_opt().is_some() => {
                let inner = ty.as_opt().expect("guarded");
                let v = format!("o{depth}");
                format!(
                    "func({v} {}) string {{ if !{v}.ok {{ return \"null\" }}; return {} }}({value})",
                    self.go_type(ty),
                    self.show(inner, &format!("{v}.v"), depth + 1)
                )
            }
            // A recursive enum prints through a function of its own (`printers`), because
            // expanding one here has no bottom: its payload leads back to the same type.
            Type::Enum { .. } if ty::is_recursive(self.registry, ty) => {
                format!("{}({value})", show_fn(ty))
            }
            Type::Enum { .. } => self.show_enum(ty, value, depth),
            Type::Record(fields) => {
                if fields.is_empty() {
                    return "\"{}\"".to_string();
                }
                // The type's field list is declaration order, so this prints as declared.
                // Field names are identifiers, so the JSON key needs no escaping and is one
                // literal.
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(name, fty)| {
                        let read = format!("({value}).F{name}");
                        let key = go_string(&format!("\"{name}\":"));
                        format!("{key} + {}", self.show(fty, &read, depth + 1))
                    })
                    .collect();
                format!("(\"{{\" + {} + \"}}\")", parts.join(" + \",\" + "))
            }
        }
    }

    /// A named printer for every recursive enum the program prints. The call in `show` is what a
    /// nested occurrence renders as, so the recursion in the type becomes recursion in the
    /// emitted function rather than in this compiler (kantord/toylang#94).
    fn printers(&self, program: &Program) -> String {
        let mut out = String::new();
        for ty in tir::printed_recursive_enums(program) {
            out.push_str(&format!(
                "func {}(v {}) string {{\n\treturn {}\n}}\n\n",
                show_fn(&ty),
                self.go_type(&ty),
                self.show_enum(&ty, "v", 0)
            ));
        }
        out
    }

    /// The printer for one enum, inline. The tag says which of the two JSON shapes (ADR 0009)
    /// this value is: a unit variant renders as its quoted name, a payload variant as the
    /// single-key wrapper. The last variant needs no test, since the type says nothing else is
    /// left.
    fn show_enum(&self, ty: &Type, value: &str, depth: usize) -> String {
        let variants = ty::variants(self.registry, ty);
        let n = format!("n{depth}");
        let render = |i: usize, vname: &str, payload: &Option<Type>| match payload {
            None => go_string(&format!("\"{vname}\"")),
            Some(p) => format!(
                "({} + {} + \"}}\")",
                go_string(&format!("{{\"{vname}\":")),
                self.show(p, &format!("(*{n}.p{i})"), depth + 1)
            ),
        };
        let mut body = String::new();
        for (i, (vname, payload)) in variants.iter().enumerate() {
            let rendered = render(i, vname, payload);
            if i + 1 < variants.len() {
                body.push_str(&format!("if {n}.tag == {i} {{ return {rendered} }}; "));
            } else {
                body.push_str(&format!("return {rendered}"));
            }
        }
        format!(
            "func({n} {}) string {{ {body} }}({value})",
            self.go_type(ty)
        )
    }
}

/// The name of `ty`'s own printer function.
fn show_fn(ty: &Type) -> String {
    format!("tlShow_{}", ty.ident())
}

/// The node's type picks which constant-folding escape a literal goes through
/// (kantord/toylang#83): tlInt for an Int, tlInt64 for an Int64.
fn int_lit(ty: &Type, n: i64) -> String {
    if *ty == Type::Int64 {
        format!("tlInt64({n})")
    } else {
        format!("tlInt({n})")
    }
}

/// One arithmetic expression at the width the node's type names. Both of Go's fixed-width
/// integers wrap by definition, so +, - and * need no guard at either width -- the only
/// backend where the wrapping rule costs nothing to state -- and the width changes nothing
/// but the div/rem helper names.
fn arith(ty: &Type, op: BinOp, l: String, r: String) -> String {
    if *ty == Type::Int64 {
        match op {
            BinOp::Div => format!("tlDiv64({l}, {r})"),
            BinOp::Rem => format!("tlRem64({l}, {r})"),
            BinOp::Add => format!("({l} + {r})"),
            BinOp::Sub => format!("({l} - {r})"),
            BinOp::Mul => format!("({l} * {r})"),
            other => unreachable!("{other} is not arithmetic"),
        }
    } else {
        match op {
            BinOp::Div => format!("tlDiv({l}, {r})"),
            BinOp::Rem => format!("tlRem({l}, {r})"),
            BinOp::Add => format!("({l} + {r})"),
            BinOp::Sub => format!("({l} - {r})"),
            BinOp::Mul => format!("({l} * {r})"),
            other => unreachable!("{other} is not arithmetic"),
        }
    }
}

fn go_op(op: BinOp) -> &'static str {
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

fn go_string(s: &str) -> String {
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
