//! Regenerates prelude.toy's checked form on every build (kantord/toylang#73), so `prelude::checked`
//! can hand back its functions as already-typed `tir::Func` values instead of the library parsing
//! and type-checking prelude.toy itself on every compile. Reuses the library's own parser and
//! checker rather than duplicating their logic: `ast`, `error`, `ty`, `tir`, `parse`, and `check`
//! below are the same source files `src/lib.rs` compiles, pulled in as this script's own modules
//! because a build script cannot depend on the crate it is building. None of them touch a backend,
//! `main.rs`, or anything else outside this closed set, so there is nothing else to pull in.
//!
//! What comes out is not those checked values themselves (this process ends when the script
//! does) but Rust source text that reconstructs them: a `ToRust` impl per type below renders each
//! one as the literal expression that builds it, so `prelude::checked` gets back a plain
//! `Vec<tir::Func>`, no parsing, deserialization, or unsafe transmute involved.
//!
//! A `Kind` field that is `Box<Tir>` in `tir.rs` still passes as plain `&Tir` to the helpers
//! below (deref coercion) -- `boxed()` re-adds the `Box::new(...)` explicitly, at the one call
//! site that field needs it, rather than a helper declaring `&Box<Tir>` to get it automatically.

#[path = "src/ast.rs"]
mod ast;
#[path = "src/check/mod.rs"]
mod check;
#[path = "src/error.rs"]
mod error;
#[path = "src/parse.rs"]
mod parse;
#[path = "src/tir.rs"]
mod tir;
#[path = "src/ty.rs"]
mod ty;

/// `check::mod`'s `check` function (unused here -- this script only calls `check::check_module`)
/// calls `crate::prelude::checked()`, so that name has to resolve for the file to compile at
/// all. The real `src/prelude.rs` can't fill in for it: its own `checked()` is generated from
/// the very file this script is producing. An empty stand-in is enough, since `check` never
/// actually runs in this binary.
mod prelude {
    pub fn checked() -> Vec<crate::tir::Func> {
        Vec::new()
    }
}

trait ToRust {
    /// The Rust expression that builds a value equal to this one.
    fn to_rust(&self) -> String;
}

impl ToRust for String {
    fn to_rust(&self) -> String {
        format!("{self:?}.to_string()")
    }
}

impl ToRust for str {
    fn to_rust(&self) -> String {
        format!("{self:?}")
    }
}

impl ToRust for bool {
    fn to_rust(&self) -> String {
        self.to_string()
    }
}

impl ToRust for i64 {
    fn to_rust(&self) -> String {
        format!("{self}i64")
    }
}

impl ToRust for u32 {
    fn to_rust(&self) -> String {
        format!("{self}u32")
    }
}

impl ToRust for usize {
    fn to_rust(&self) -> String {
        format!("{self}usize")
    }
}

impl<T: ToRust> ToRust for Option<T> {
    fn to_rust(&self) -> String {
        match self {
            Some(v) => format!("Some({})", v.to_rust()),
            None => "None".to_string(),
        }
    }
}

impl<T: ToRust> ToRust for Box<T> {
    fn to_rust(&self) -> String {
        format!("Box::new({})", (**self).to_rust())
    }
}

impl<T: ToRust> ToRust for [T] {
    fn to_rust(&self) -> String {
        let items: Vec<String> = self.iter().map(ToRust::to_rust).collect();
        format!("vec![{}]", items.join(", "))
    }
}

impl<A: ToRust, B: ToRust> ToRust for (A, B) {
    fn to_rust(&self) -> String {
        format!("({}, {})", self.0.to_rust(), self.1.to_rust())
    }
}

impl ToRust for ast::BinOp {
    fn to_rust(&self) -> String {
        let variant = match self {
            ast::BinOp::Add => "Add",
            ast::BinOp::Sub => "Sub",
            ast::BinOp::Mul => "Mul",
            ast::BinOp::Div => "Div",
            ast::BinOp::Rem => "Rem",
            ast::BinOp::Eq => "Eq",
            ast::BinOp::Ne => "Ne",
            ast::BinOp::Lt => "Lt",
            ast::BinOp::Le => "Le",
            ast::BinOp::Gt => "Gt",
            ast::BinOp::Ge => "Ge",
        };
        format!("crate::ast::BinOp::{variant}")
    }
}

impl ToRust for ast::LogicOp {
    fn to_rust(&self) -> String {
        let variant = match self {
            ast::LogicOp::And => "And",
            ast::LogicOp::Or => "Or",
        };
        format!("crate::ast::LogicOp::{variant}")
    }
}

impl ToRust for ty::Type {
    fn to_rust(&self) -> String {
        match self {
            ty::Type::Str => "crate::ty::Type::Str".to_string(),
            ty::Type::Int => "crate::ty::Type::Int".to_string(),
            ty::Type::Int64 => "crate::ty::Type::Int64".to_string(),
            ty::Type::Bool => "crate::ty::Type::Bool".to_string(),
            ty::Type::Char => "crate::ty::Type::Char".to_string(),
            ty::Type::Vec(elem) => format!("crate::ty::Type::Vec({})", elem.to_rust()),
            ty::Type::Stream(elem) => format!("crate::ty::Type::Stream({})", elem.to_rust()),
            ty::Type::Record(fields) => {
                format!("crate::ty::Type::Record({})", fields.to_rust())
            }
            ty::Type::Enum {
                name,
                args,
                variants,
            } => format!(
                "crate::ty::Type::Enum {{ name: {}, args: {}, variants: {} }}",
                name.to_rust(),
                args.to_rust(),
                variants.to_rust(),
            ),
            ty::Type::Param(name) => format!("crate::ty::Type::Param({})", name.to_rust()),
        }
    }
}

impl ToRust for tir::Builtin {
    fn to_rust(&self) -> String {
        let variant = match self {
            tir::Builtin::IntToStr => "IntToStr",
            tir::Builtin::IntToI64 => "IntToI64",
            tir::Builtin::Range => "Range",
            tir::Builtin::Collect => "Collect",
            tir::Builtin::JsonLines => "JsonLines",
            tir::Builtin::Length => "Length",
            tir::Builtin::Flatten => "Flatten",
            tir::Builtin::Tail => "Tail",
            tir::Builtin::Fields => "Fields",
            tir::Builtin::Chars => "Chars",
            tir::Builtin::Sort => "Sort",
            tir::Builtin::Reverse => "Reverse",
            tir::Builtin::Sum => "Sum",
            tir::Builtin::Max => "Max",
        };
        format!("crate::tir::Builtin::{variant}")
    }
}

impl ToRust for tir::Tir {
    fn to_rust(&self) -> String {
        format!(
            "crate::tir::Tir::new({}, {})",
            self.ty.to_rust(),
            self.kind.to_rust()
        )
    }
}

impl ToRust for tir::MatchArm {
    fn to_rust(&self) -> String {
        format!(
            "crate::tir::MatchArm {{ variant: {}, guard: {}, payload: {}, body: {} }}",
            self.variant.to_rust(),
            self.guard.to_rust(),
            self.payload.to_rust(),
            self.body.to_rust(),
        )
    }
}

/// `crate::tir::Kind::{name} { k1: v1, k2: v2, ... }`, the shape shared by every struct-like
/// `Kind` variant. One small function per variant below builds its own field list and calls
/// this, which is what keeps the dispatching match in `Kind::to_rust` to one line per variant --
/// laid out as a table, not a 100-line function apiece.
fn variant(name: &str, fields: &[(&str, String)]) -> String {
    let rendered: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    format!("crate::tir::Kind::{name} {{ {} }}", rendered.join(", "))
}

/// `tir.rs` stores a `Tir` child as `Box<Tir>`; this is the `Box::new(...)` that reconstructs
/// it, called explicitly at each field that needs it rather than a helper below taking `&Box<Tir>`
/// to get the wrapping automatically -- a plain `&Tir` parameter is what a `Box<Tir>` field
/// coerces to at the call site regardless, so declaring the box type would buy nothing.
fn boxed(t: &tir::Tir) -> String {
    format!("Box::new({})", t.to_rust())
}

fn record_lit(fields: &[(String, tir::Tir)]) -> String {
    variant("RecordLit", &[("fields", fields.to_rust())])
}
fn enum_lit(variant_name: &String, payload: &Option<Box<tir::Tir>>) -> String {
    variant(
        "EnumLit",
        &[
            ("variant", variant_name.to_rust()),
            ("payload", payload.to_rust()),
        ],
    )
}
fn opt_map(source: &tir::Tir, param: &tir::LocalId, body: &tir::Tir) -> String {
    variant(
        "OptMap",
        &[
            ("source", boxed(source)),
            ("param", param.to_rust()),
            ("body", boxed(body)),
        ],
    )
}
fn call(func: &String, arg: &Option<Box<tir::Tir>>) -> String {
    variant("Call", &[("func", func.to_rust()), ("arg", arg.to_rust())])
}
fn arith(op: &ast::BinOp, lhs: &tir::Tir, rhs: &tir::Tir) -> String {
    variant(
        "Arith",
        &[
            ("op", op.to_rust()),
            ("lhs", boxed(lhs)),
            ("rhs", boxed(rhs)),
        ],
    )
}
fn cond(cond: &tir::Tir, then: &tir::Tir, otherwise: &tir::Tir) -> String {
    variant(
        "Cond",
        &[
            ("cond", boxed(cond)),
            ("then", boxed(then)),
            ("otherwise", boxed(otherwise)),
        ],
    )
}
fn compare(op: &ast::BinOp, lhs: &tir::Tir, rhs: &tir::Tir) -> String {
    variant(
        "Compare",
        &[
            ("op", op.to_rust()),
            ("lhs", boxed(lhs)),
            ("rhs", boxed(rhs)),
        ],
    )
}
fn logic(op: &ast::LogicOp, lhs: &tir::Tir, rhs: &tir::Tir) -> String {
    variant(
        "Logic",
        &[
            ("op", op.to_rust()),
            ("lhs", boxed(lhs)),
            ("rhs", boxed(rhs)),
        ],
    )
}
fn bind(local: &tir::LocalId, value: &tir::Tir, body: &tir::Tir) -> String {
    variant(
        "Bind",
        &[
            ("local", local.to_rust()),
            ("value", boxed(value)),
            ("body", boxed(body)),
        ],
    )
}
fn map(source: &tir::Tir, param: &tir::LocalId, body: &tir::Tir) -> String {
    variant(
        "Map",
        &[
            ("source", boxed(source)),
            ("param", param.to_rust()),
            ("body", boxed(body)),
        ],
    )
}
fn select(source: &tir::Tir, param: &tir::LocalId, pred: &tir::Tir) -> String {
    variant(
        "Select",
        &[
            ("source", boxed(source)),
            ("param", param.to_rust()),
            ("pred", boxed(pred)),
        ],
    )
}
fn field(base: &tir::Tir, name: &String) -> String {
    variant("Field", &[("base", boxed(base)), ("name", name.to_rust())])
}
fn builtin(which: &tir::Builtin, arg: &tir::Tir) -> String {
    variant(
        "Builtin",
        &[("which", which.to_rust()), ("arg", boxed(arg))],
    )
}
fn index(base: &tir::Tir, index: &tir::Tir, depth: &usize, elem_is_record: &bool) -> String {
    variant(
        "Index",
        &[
            ("base", boxed(base)),
            ("index", boxed(index)),
            ("depth", depth.to_rust()),
            ("elem_is_record", elem_is_record.to_rust()),
        ],
    )
}
fn match_(subject: &tir::Tir, arms: &[tir::MatchArm], partial: &bool) -> String {
    variant(
        "Match",
        &[
            ("subject", boxed(subject)),
            ("arms", arms.to_rust()),
            ("partial", partial.to_rust()),
        ],
    )
}

/// Exhaustive on purpose: a `Kind` variant this does not know renders nothing rather than
/// something wrong, and a match with no wildcard arm is what turns that into a build error the
/// moment the checker can produce it, naming exactly the arm to add (kantord/toylang#73's
/// "independent of its growth").
impl ToRust for tir::Kind {
    fn to_rust(&self) -> String {
        use tir::Kind::*;
        match self {
            Str(s) => format!("crate::tir::Kind::Str({})", s.to_rust()),
            Int(n) => format!("crate::tir::Kind::Int({})", n.to_rust()),
            VecLit(items) => format!("crate::tir::Kind::VecLit({})", items.to_rust()),
            RecordLit { fields } => record_lit(fields),
            EnumLit { variant, payload } => enum_lit(variant, payload),
            OptMap {
                source,
                param,
                body,
            } => opt_map(source, param, body),
            Var(name) => format!("crate::tir::Kind::Var({})", name.to_rust()),
            Local(id) => format!("crate::tir::Kind::Local({})", id.to_rust()),
            Input => "crate::tir::Kind::Input".to_string(),
            Inputs => "crate::tir::Kind::Inputs".to_string(),
            Lines => "crate::tir::Kind::Lines".to_string(),
            Dsv { delim } => format!("crate::tir::Kind::Dsv {{ delim: {} }}", delim.to_rust()),
            Call { func, arg } => call(func, arg),
            Concat(a, b) => format!("crate::tir::Kind::Concat({}, {})", a.to_rust(), b.to_rust()),
            Arith { op, lhs, rhs } => arith(op, lhs, rhs),
            Cond {
                cond: c,
                then,
                otherwise,
            } => cond(c, then, otherwise),
            Compare { op, lhs, rhs } => compare(op, lhs, rhs),
            Logic { op, lhs, rhs } => logic(op, lhs, rhs),
            Not(base) => format!("crate::tir::Kind::Not({})", boxed(base)),
            Bind { local, value, body } => bind(local, value, body),
            Map {
                source,
                param,
                body,
            } => map(source, param, body),
            Select {
                source,
                param,
                pred,
            } => select(source, param, pred),
            Field { base, name } => field(base, name),
            Builtin { which, arg } => builtin(which, arg),
            Unwrap { base } => variant("Unwrap", &[("base", base.to_rust())]),
            Index {
                base,
                index: i,
                depth,
                elem_is_record,
            } => index(base, i, depth, elem_is_record),
            Match {
                subject,
                arms,
                partial,
            } => match_(subject, arms, partial),
        }
    }
}

impl ToRust for tir::Func {
    fn to_rust(&self) -> String {
        format!(
            "crate::tir::Func {{ name: {}, param: {}, param_ty: {}, body: {} }}",
            self.name.to_rust(),
            self.param.to_rust(),
            self.param_ty.to_rust(),
            self.body.to_rust(),
        )
    }
}

fn main() {
    println!("cargo::rerun-if-changed=prelude.toy");
    for src in [
        "src/ast.rs",
        "src/error.rs",
        "src/parse.rs",
        "src/tir.rs",
        "src/ty.rs",
        "src/check/mod.rs",
        "src/check/types.rs",
        "src/check/linearity.rs",
    ] {
        println!("cargo::rerun-if-changed={src}");
    }

    let src = std::fs::read_to_string("prelude.toy").expect("prelude.toy is readable");
    let module = parse::parse_module(&src).expect("prelude.toy is valid toylang");
    // Same filter as `prelude::module()`: a non-`pub` definition is parsed but never part of the
    // always-available set, so it has nothing to precompile.
    let module = ast::Module {
        defs: module.defs.into_iter().filter(|d| d.is_pub).collect(),
        enums: module.enums.into_iter().filter(|e| e.is_pub).collect(),
    };
    let funcs = check::check_module(&module).expect("prelude.toy checks");

    let generated = format!(
        "/// Every `pub fn` in prelude.toy, already checked. Generated by build.rs from \
         prelude.toy -- edit that, not this.\n\
         pub fn checked() -> Vec<crate::tir::Func> {{\n    {}\n}}\n",
        funcs.to_rust(),
    );
    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR for a build script");
    std::fs::write(
        std::path::Path::new(&out_dir).join("prelude_checked.rs"),
        generated,
    )
    .expect("can write the generated prelude source");
}
