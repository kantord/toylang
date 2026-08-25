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

use crate::ast::BinOp;
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::Type;

/// The binding the input value is read into. Unspellable in source, since every source name is
/// prefixed.
const INPUT: &str = "t_input";

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

const UNWRAP_HELPER: &str = r#"func tlUnwrap[T any](o tlOpt[T]) T {
	if !o.ok {
		tlFail("unwrapped a value that is not there")
	}
	return o.v
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

/// Go folds constant arithmetic exactly and rejects a result that does not fit, which is the
/// direct opposite of the wrapping rule: `2147483647 + 1` is a compile error rather than
/// `-2147483648`. Passing every literal through a function makes the expression non-constant, so
/// the wrap happens at runtime where it is defined. The call inlines away.
const INT_HELPER: &str = r#"func tlInt(n int32) int32 { return n }
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
    let mut ctx = Collect { used: &mut used, records: &mut records };
    for f in &program.funcs {
        ctx.ty(&f.param_ty);
        ctx.walk(&f.body);
    }
    ctx.walk(&program.body);
    if let Some(ty) = &program.input {
        ctx.ty(ty);
    }
    records.sort_by_key(|t| t.to_string());

    let e = Emitter { records };

    let mut decls = String::new();
    for (i, rec) in e.records.iter().enumerate() {
        let Type::Record(fields) = rec else { unreachable!("only records are collected") };
        decls.push_str(&format!("type tlRec{i} struct {{\n"));
        for (name, ty) in fields {
            // Exported, because encoding/json only fills fields it can see. The `F` prefix keeps
            // the mapping injective, so two toylang fields cannot collide on one Go field.
            decls.push_str(&format!("\tF{name} {} `json:\"{name}\"`\n", e.go_type(ty)));
        }
        decls.push_str("}\n\n");
    }

    // Package-level functions are visible in any order, so the forward reference the checker
    // accepts needs nothing here. Lua wanted declarations and JavaScript relied on hoisting.
    for f in &program.funcs {
        decls.push_str(&format!(
            "func {}({} {}) {} {{\n\treturn {}\n}}\n\n",
            e.user(&f.name),
            e.user(&f.param),
            e.go_type(&f.param_ty),
            e.go_type(&f.body.ty),
            e.expr(&f.body)
        ));
    }

    decls.push_str("func main() {\n");
    if let Some(ty) = &program.input {
        decls.push_str(&format!("\tvar {INPUT} {}\n", e.go_type(ty)));
        decls.push_str(&format!(
            "\tif err := json.NewDecoder(os.Stdin).Decode(&{INPUT}); err != nil {{\n\t\ttlFail(err.Error())\n\t}}\n"
        ));
    }
    let body = e.expr(&program.body);
    // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON.
    let printed = if program.body.ty == Type::Str {
        body
    } else {
        e.show(&program.body.ty, &body, 0)
    };
    decls.push_str(&format!("\tfmt.Println({printed})\n}}\n"));

    // Which helpers to include is read off the emitted text rather than tracked alongside it. An
    // unused function is not an error in Go, so a false positive costs nothing, while a missed
    // one would not compile -- the asymmetry is what makes reading the output safe. Imports are
    // the opposite (an unused one is an error), so those come from the walk instead.
    let uses = |name: &str| decls.contains(name);
    let unwrap = uses("tlUnwrap(");
    let arith = uses("tlDiv(") || uses("tlRem(");
    let collect = uses("tlCollectLines(");
    let fail = unwrap || arith || program.input.is_some();
    let quote = uses("tlQuote(");
    let join = uses("tlJoin(");

    let mut helpers = String::new();
    // tlOpt is what tlAt and tlUnwrap are written in terms of, and inference means the emitted
    // text need never spell it. Helper-to-helper dependencies are stated rather than read back.
    if uses("tlOpt[") || uses("tlAt(") || unwrap {
        helpers.push_str(OPT_TYPE);
        helpers.push('\n');
    }
    for (on, text) in [
        (fail, FAIL_HELPER),
        (uses("tlInt("), INT_HELPER),
        (uses("tlMap("), MAP_HELPER),
        (uses("tlSelect("), SELECT_HELPER),
        (uses("tlAt("), AT_HELPER),
        (unwrap, UNWRAP_HELPER),
        (arith, ARITH_HELPER),
        (uses("tlRange("), RANGE_HELPER),
        (collect, COLLECT_HELPER),
        (join, JOIN_HELPER),
        (quote, QUOTE_HELPER),
    ] {
        if on {
            helpers.push_str(text);
            helpers.push('\n');
        }
    }

    let mut imports = BTreeSet::new();
    imports.insert("fmt");
    if fail || program.input.is_some() || collect {
        imports.insert("os");
    }
    if collect {
        imports.insert("bufio");
        imports.insert("bytes");
    }
    if program.input.is_some() {
        imports.insert("encoding/json");
    }
    if join || quote || used.unlines {
        imports.insert("strings");
    }
    if used.itoa || (program.body.ty != Type::Str && has_scalar(&program.body.ty)) {
        imports.insert("strconv");
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
fn has_scalar(ty: &Type) -> bool {
    match ty {
        // Only ever called on the program's own result type, which the checker guarantees is
        // never Lines and never contains one.
        Type::Lines => unreachable!("Lines cannot reach has_scalar"),
        Type::Int | Type::Bool => true,
        Type::Str => false,
        Type::Vec(t) | Type::Opt(t) => has_scalar(t),
        Type::Record(fields) => fields.iter().any(|(_, t)| has_scalar(t)),
    }
}

#[derive(Default)]
struct Used {
    itoa: bool,
    unlines: bool,
}

/// One walk, collecting the record types that need declaring and the two builtins whose imports
/// cannot be read back off the emitted text without risking a false positive.
struct Collect<'a> {
    used: &'a mut Used,
    records: &'a mut Vec<Type>,
}

impl Collect<'_> {
    fn ty(&mut self, t: &Type) {
        match t {
            Type::Vec(e) | Type::Opt(e) => self.ty(e),
            Type::Record(fields) => {
                if !self.records.contains(t) {
                    self.records.push(t.clone());
                }
                for (_, f) in fields {
                    self.ty(f);
                }
            }
            _ => {}
        }
    }

    fn walk(&mut self, t: &Tir) {
        self.ty(&t.ty);
        match &t.kind {
            Kind::Str(_) | Kind::Int(_) | Kind::Var(_) | Kind::Local(_) | Kind::Input | Kind::Lines => {}
            Kind::VecLit(items) => items.iter().for_each(|i| self.walk(i)),
            Kind::RecordLit { fields } => {
                fields.iter().for_each(|(_, v)| self.walk(v));
            }
            Kind::Call { arg, .. } => self.walk(arg),
            Kind::Concat(l, r)
            | Kind::Compare { lhs: l, rhs: r, .. }
            | Kind::Arith { lhs: l, rhs: r, .. } => {
                self.walk(l);
                self.walk(r);
            }
            Kind::Bind { value, body, .. } => {
                self.walk(value);
                self.walk(body);
            }
            Kind::Map { source, body, .. } => {
                self.walk(source);
                self.walk(body);
            }
            Kind::Select { source, pred, .. } => {
                self.walk(source);
                self.walk(pred);
            }
            Kind::Cond { cond, then, otherwise } => {
                self.walk(cond);
                self.walk(then);
                self.walk(otherwise);
            }
            Kind::Field { base, .. } | Kind::Unwrap { base } => self.walk(base),
            Kind::Index { base, index, .. } => {
                self.walk(base);
                self.walk(index);
            }
            Kind::Builtin { which, arg } => {
                match which {
                    Builtin::IntToStr => self.used.itoa = true,
                    Builtin::Unlines => self.used.unlines = true,
                    Builtin::Range | Builtin::Collect => {}
                }
                self.walk(arg);
            }
        }
    }
}

struct Emitter {
    records: Vec<Type>,
}

impl Emitter {
    fn user(&self, name: &str) -> String {
        format!("v_{name}")
    }

    fn local(&self, id: LocalId) -> String {
        format!("t_{id}")
    }

    fn go_type(&self, ty: &Type) -> String {
        match ty {
            Type::Str => "string".to_string(),
            // The default Int is 32 bits and wraps, and Go's int32 does exactly that for free.
            Type::Int => "int32".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Vec(e) => format!("[]{}", self.go_type(e)),
            Type::Opt(e) => format!("tlOpt[{}]", self.go_type(e)),
            // A phantom marker: Go still needs a real type for the closure-based Bind pattern
            // to name, even though no value of it is ever read.
            Type::Lines => "struct{}".to_string(),
            Type::Record(_) => {
                let key = ty.to_string();
                let i = self
                    .records
                    .iter()
                    .position(|r| r.to_string() == key)
                    .expect("every record reachable from the program was collected");
                format!("tlRec{i}")
            }
        }
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
        let elem = value_ty.elem().expect("a dimension to distribute over");
        let result_elem = result_ty.elem().expect("the result keeps the dimension");
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
            Kind::Int(n) => format!("tlInt({n})"),
            Kind::Var(name) => self.user(name),
            Kind::Local(id) => self.local(*id),
            Kind::Input => INPUT.to_string(),
        // `lines` has no value of its own -- it is a promise that the real stdin has not been
        // read yet, made good only by `collect`. The empty struct is never actually inspected.
        Kind::Lines => "struct{}{}".to_string(),
            // go_type resolves the struct name, and the collector registered it because a
            // record literal carries its own record type.
            Kind::RecordLit { fields } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("F{name}: {}", self.expr(value)))
                    .collect();
                format!("{}{{{}}}", self.go_type(&t.ty), parts.join(", "))
            }

            Kind::VecLit(items) => {
                let parts: Vec<String> = items.iter().map(|i| self.expr(i)).collect();
                format!("{}{{{}}}", self.go_type(&t.ty), parts.join(", "))
            }
            Kind::Call { func, arg } => format!("{}({})", self.user(func), self.expr(arg)),
            Kind::Concat(l, r) => format!("({} + {})", self.expr(l), self.expr(r)),
            Kind::Arith { op, lhs, rhs } => match op {
                BinOp::Div => format!("tlDiv({}, {})", self.expr(lhs), self.expr(rhs)),
                BinOp::Rem => format!("tlRem({}, {})", self.expr(lhs), self.expr(rhs)),
                // int32 wraps by definition in Go, so +, - and * need no guard at all. This is
                // the only backend where the wrapping rule costs nothing to state.
                BinOp::Add => format!("({} + {})", self.expr(lhs), self.expr(rhs)),
                BinOp::Sub => format!("({} - {})", self.expr(lhs), self.expr(rhs)),
                BinOp::Mul => format!("({} * {})", self.expr(lhs), self.expr(rhs)),
                other => unreachable!("{other} is not arithmetic"),
            },
            // Go has no conditional expression, so this is a call to a function literal rather
            // than an operator. Both branches stay unevaluated, which a `tlCond(c, a, b)` helper
            // could not manage: its arguments would both run, and one of them may divide by zero.
            Kind::Cond { cond, then, otherwise } => format!(
                "func() {} {{ if {} {{ return {} }}; return {} }}()",
                self.go_type(&t.ty),
                self.expr(cond),
                self.expr(then),
                self.expr(otherwise)
            ),
            Kind::Builtin { which, arg } => match which {
                Builtin::IntToStr => format!("strconv.FormatInt(int64({}), 10)", self.expr(arg)),
                Builtin::Range => format!("tlRange({})", self.expr(arg)),
                Builtin::Unlines => format!("strings.Join({}, \"\\n\")", self.expr(arg)),
                Builtin::Collect => "tlCollectLines()".to_string(),
            },
            Kind::Compare { op, lhs, rhs } => {
                format!("({} {} {})", self.expr(lhs), go_op(*op), self.expr(rhs))
            }
            Kind::Bind { local: id, value, body } => format!(
                "func({} {}) {} {{ return {} }}({})",
                self.local(*id),
                self.go_type(&value.ty),
                self.go_type(&t.ty),
                self.expr(body),
                self.expr(value)
            ),
            Kind::Map { source, param, body } => format!(
                "tlMap({}, func({} {}) {} {{ return {} }})",
                self.expr(source),
                self.local(*param),
                self.go_type(source.ty.elem().expect("map runs over a Vec")),
                self.go_type(&body.ty),
                self.expr(body)
            ),
            Kind::Select { source, param, pred } => format!(
                "tlSelect({}, func({} {}) bool {{ return {} }})",
                self.expr(source),
                self.local(*param),
                self.go_type(source.ty.elem().expect("select runs over a Vec")),
                self.expr(pred)
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
            Kind::Index { base, index, depth, .. } => {
                let i = self.expr(index);
                self.distribute(&self.expr(base), &base.ty, &t.ty, *depth, &|v| {
                    format!("tlAt({v}, {i})")
                })
            }
        }
    }

    /// The printer is built from the type rather than by inspecting the value, as on every other
    /// backend. Here there is no choice at all: a Go value cannot be asked what it is.
    fn show(&self, ty: &Type, value: &str, depth: usize) -> String {
        match ty {
            // The checker refuses a program whose result contains Lines, since there is
            // nothing to print: a stream has no value, only a promise that collect can redeem.
            Type::Lines => unreachable!("Lines cannot reach the printer"),
            Type::Str => format!("tlQuote({value})"),
            Type::Int => format!("strconv.FormatInt(int64({value}), 10)"),
            Type::Bool => format!("strconv.FormatBool({value})"),
            Type::Vec(elem) => {
                let e = format!("e{depth}");
                format!(
                    "tlJoin({value}, func({e} {}) string {{ return {} }})",
                    self.go_type(elem),
                    self.show(elem, &e, depth + 1)
                )
            }
            Type::Opt(inner) => {
                let v = format!("o{depth}");
                format!(
                    "func({v} {}) string {{ if !{v}.ok {{ return \"null\" }}; return {} }}({value})",
                    self.go_type(ty),
                    self.show(inner, &format!("{v}.v"), depth + 1)
                )
            }
            Type::Record(fields) => {
                if fields.is_empty() {
                    return "\"{}\"".to_string();
                }
                // Type::record keeps fields sorted, so this order is the type's order. Field
                // names are identifiers, so the JSON key needs no escaping and is one literal.
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
