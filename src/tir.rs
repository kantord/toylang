//! The typed IR the backends consume.
//!
//! Every node carries the type it was checked at, so a backend never has to ask what a value is
//! at runtime and never has to look a type up in a table beside the tree. A static target cannot
//! work from anything less: it has to know that an add is an integer add before it can emit one.
//!
//! Names are deliberately *not* mangled here. Which identifiers are reserved is a property of
//! the target, not of toylang, so each backend renders these as it needs to.

use crate::ast::BinOp;
use crate::ty::Type;

pub struct Tir {
    pub ty: Type,
    pub kind: Kind,
}

impl Tir {
    pub fn new(ty: Type, kind: Kind) -> Tir {
        Tir { ty, kind }
    }
}

/// A binding the source cannot name, introduced for `|` and for `select`'s parameter.
pub type LocalId = u32;

pub enum Kind {
    Str(String),
    Int(i64),
    VecLit(Vec<Tir>),
    /// A record literal, its fields sorted by name so a field's position here matches
    /// its position in the type. That is what lets a backend address one by index rather than
    /// searching for it.
    RecordLit { fields: Vec<(String, Tir)> },
    /// A constructed enum value. The node's type is the enum, which carries the variant list,
    /// so a backend finds the variant's position (its tag, where one is needed) and its payload
    /// type there rather than in the node. `payload` is `None` for a unit variant, which every
    /// backend renders as the bare variant-name string; a payload variant is the single-key
    /// wrapper (ADR 0009).
    EnumLit { variant: String, payload: Option<Box<Tir>> },
    /// A name written in the source: today only a function parameter.
    Var(String),
    Local(LocalId),
    /// The value read from stdin.
    Input,
    /// Every remaining JSON value on stdin, one per line, eagerly collected into a `Vec<T>`.
    Inputs,
    /// The stream of lines read from stdin, read incrementally by whatever consumes it.
    Lines,
    Call {
        func: String,
        arg: Box<Tir>,
    },
    Concat(Box<Tir>, Box<Tir>),
    /// Wrapping 32-bit arithmetic. Division and remainder stop the program on a zero divisor,
    /// which is the only way arithmetic can fail.
    Arith {
        op: BinOp,
        lhs: Box<Tir>,
        rhs: Box<Tir>,
    },
    /// The condition is exactly one Bool, which is what turns jq's run-both-branches behaviour
    /// into a type error here.
    Cond {
        cond: Box<Tir>,
        then: Box<Tir>,
        otherwise: Box<Tir>,
    },
    Compare {
        op: BinOp,
        lhs: Box<Tir>,
        rhs: Box<Tir>,
    },
    /// `let local = value in body`, which is what `|` becomes once `.` has a name.
    Bind {
        local: LocalId,
        value: Box<Tir>,
        body: Box<Tir>,
    },
    /// Every element replaced by `body`, with `param` bound to each. Same loop as Select, kept
    /// separate because the result's element type is the body's rather than the source's.
    Map {
        source: Box<Tir>,
        param: LocalId,
        body: Box<Tir>,
    },
    Select {
        source: Box<Tir>,
        param: LocalId,
        pred: Box<Tir>,
    },
    /// Read `name` off `base`. How many Vec layers to descend through is `base.ty`'s doing and
    /// is not stored, so it cannot disagree with the type.
    Field {
        base: Box<Tir>,
        name: String,
    },
    /// Collapse one dimension of `base` at `index`, `depth` layers down.
    ///
    /// Unlike a field access this has to store its depth. A field access leaves a record behind,
    /// so the depth is every Vec layer of the base; an index leaves a Vec behind, so the layers
    /// below the one being collapsed are indistinguishable from the ones above it.
    /// A unary builtin. Unary like every other function, so it needs no special call form.
    Builtin { which: Builtin, arg: Box<Tir> },
    /// Insist an Opt is present, `depth` layers down. Like a field access and unlike an index,
    /// the depth is every Vec layer of the base, because an Opt is not a dimension.
    Unwrap {
        base: Box<Tir>,
    },
    Index {
        base: Box<Tir>,
        index: Box<Tir>,
        depth: usize,
        /// Whether an entry is a record, which decides if collapsing has to gather columns.
        elem_is_record: bool,
    },
    /// First-match-wins dispatch over an enum subject's variants. The checker has already
    /// proved the arms exhaustive, so a backend may take the last arm without a test, and has
    /// already resolved every name a pattern bound, so an arm is only a variant to test for, a
    /// payload local to bind, and a body.
    Match {
        subject: Box<Tir>,
        arms: Vec<MatchArm>,
    },
}

pub struct MatchArm {
    /// `None` is the default arm (`any()`), which the checker keeps last.
    pub variant: Option<String>,
    /// The local the payload binds to in a payload-variant arm; `.` and destructured field
    /// names in the body both read through it.
    pub payload: Option<LocalId>,
    pub body: Tir,
}

/// The functions the language provides. Each is unary, and so is every user function: something
/// wanting two arguments takes a record, which is what a record literal is for in argument
/// position.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    /// `str(n)`, rendering an Int the way the printer does but reachable from a program.
    IntToStr,
    /// `range(n)`, the integers from zero up to but not including n. Zero-based, matching jq,
    /// Python, and this language's own indices.
    Range,
    /// `collect(s)`, `Stream<T> -> Vec<T>`: the one place a stream stops being a stream. What
    /// comes back is an ordinary value, exactly as sized as it needs to be, with no trace left
    /// of how it arrived.
    Collect,
    /// `jsonlines(v)`, printing each element of a `Vec<T>` on its own line rather than wrapping
    /// the whole thing in `[...]`. Named for the format (jsonlines.org, also called NDJSON):
    /// one JSON value per line. Polymorphic over the element type, so it is not in the fixed
    /// signature table `builtin()` reads from; `synth` checks it directly, the way `map` and
    /// `select` are checked from their own arm rather than through a table.
    JsonLines,
    /// `extent(v)`, a Vec's length. Named for CONTEXT.md's glossary term rather than `length`,
    /// which the glossary lists under Avoid. A dense Vec already tracks this at runtime, so
    /// reading it out costs nothing -- there is no fold or scan hiding behind the name.
    Extent,
    /// `concat(vv)`, flattening a `Vec<Vec<T>>` into one `Vec<T>`. A named function rather than
    /// an overload of `+`, so it does not touch or prejudge Q2 (draft.md), which is still open
    /// for the general question of binary operators over two Vecs.
    Concat,
    /// `tail(v)`, every element but the first, `None` when `v` is empty. Consistent with how
    /// `Index` already turns "reaching past what's there" into `Opt` rather than a runtime
    /// failure.
    Tail,
}

pub struct Func {
    pub name: String,
    pub param: String,
    pub param_ty: Type,
    pub body: Tir,
}

pub struct Program {
    pub funcs: Vec<Func>,
    pub body: Tir,
    /// The type stdin must have, if the program reads it.
    pub input: Option<Type>,
    /// The element type each line of stdin parses as, if the program reads `inputs`.
    pub inputs: Option<Type>,
    /// Whether the program reads `lines`. A separate flag from `input`, since the two are
    /// unrelated readers of the same real stdin and a program using `lines` alone still needs
    /// it connected, even though `input` is `None`.
    pub uses_lines: bool,
}

/// How many `Vec` layers wrap a scalar. Field access distributes over each of them.
pub fn vec_depth(ty: &Type) -> usize {
    let mut depth = 0;
    let mut inner = ty;
    while let Some(elem) = inner.elem() {
        depth += 1;
        inner = elem;
    }
    depth
}

/// One `map`/`select` applied between reading a record and printing it, in source order (the
/// stage nearest `inputs` first).
pub enum Stage<'a> {
    Map { param: LocalId, body: &'a Tir },
    Select { param: LocalId, pred: &'a Tir },
}

/// A program shaped as `jsonlines(chain of map/select over inputs)`, recognized so a backend can
/// compile it as a read-one/transform-one/write-one loop instead of collecting all of stdin into
/// a `Vec` before printing anything.
///
/// This is deliberately narrow: a structural match on this one shape, not a general `Stream<T>`
/// type. draft.md's open question on first-class streams (Q1) is still open -- a real `Stream`
/// type would let a program of *any* shape stream, where this only fuses the one idiom actually
/// written for it today. Widening this into Q1's answer is future work, not started here.
pub struct Fusion<'a> {
    pub stages: Vec<Stage<'a>>,
}

/// What a chain of `map`/`select` bottoms out at. `Inputs` is what `recognize_fusion` needs at
/// the top; `Var` shows up one level down, inside a function's own body, where the chain bottoms
/// out at that function's parameter rather than at stdin directly.
enum Base<'a> {
    Inputs,
    Var(&'a String),
}

/// `inputs` can only ever be checked with its type already known (see `check.rs`'s `expect` vs
/// `synth` split), so it can never sit directly under `select`/`map` the way a plain Vec can --
/// it only ever appears as a whole function call's argument, e.g. `f(inputs)`. That is why this
/// walks through at most one `Call` back into `program.funcs`: it is not chasing an arbitrary
/// call graph, only unwrapping the one indirection `inputs`'s own typing rule forces every real
/// program to go through.
///
/// Each `|` desugars to `Bind { value, body, .. }` with the piped-from expression as `value` and
/// the next stage as `body`, so the chain is walked by recursing into `value` first and treating
/// `body` as one more stage on the way back out -- which is also why a stage that is not exactly
/// a `map`/`select` call (a bare field projection like `.[].name`, say) ends the recognition
/// rather than being folded in: only the two are represented as their own `Kind` variant here.
fn flatten<'a>(t: &'a Tir, program: &'a Program, stages: &mut Vec<Stage<'a>>) -> Option<Base<'a>> {
    match &t.kind {
        Kind::Bind { value, body, .. } => {
            let base = flatten(value, program, stages)?;
            match &body.kind {
                Kind::Map { param, body, .. } => stages.push(Stage::Map { param: *param, body }),
                Kind::Select { param, pred, .. } => stages.push(Stage::Select { param: *param, pred }),
                _ => return None,
            }
            Some(base)
        }
        Kind::Call { func, arg } => {
            if !matches!(arg.kind, Kind::Inputs) {
                return None;
            }
            let f = program.funcs.iter().find(|f| &f.name == func)?;
            match flatten(&f.body, program, stages)? {
                Base::Var(name) if name == &f.param => Some(Base::Inputs),
                _ => None,
            }
        }
        Kind::Var(name) => Some(Base::Var(name)),
        Kind::Inputs => Some(Base::Inputs),
        _ => None,
    }
}

pub fn recognize_fusion(program: &Program) -> Option<Fusion<'_>> {
    let Kind::Builtin { which: Builtin::JsonLines, arg } = &program.body.kind else {
        return None;
    };
    let mut stages = Vec::new();
    if !matches!(flatten(arg, program, &mut stages)?, Base::Inputs) {
        return None;
    }

    // A stage (inside or outside the one function this chain called into) that reads `inputs`
    // again would need the whole materialized Vec this loop deliberately never builds. Nothing in
    // the corpus does this and it is arguably nonsensical (the source stdin is not yet fully
    // read), but falling back to the eager path is free and correct, where forcing fusion would
    // reference a Vec that was never declared.
    let stage_reads_inputs = stages.iter().any(|s| match s {
        Stage::Map { body, .. } => mentions_inputs(body),
        Stage::Select { pred, .. } => mentions_inputs(pred),
    });
    if stage_reads_inputs || program.funcs.iter().any(|f| mentions_inputs(&f.body)) {
        return None;
    }
    Some(Fusion { stages })
}

fn mentions_inputs(t: &Tir) -> bool {
    match &t.kind {
        Kind::Inputs => true,
        Kind::Str(_) | Kind::Int(_) | Kind::Var(_) | Kind::Local(_) | Kind::Input | Kind::Lines => false,
        Kind::EnumLit { payload, .. } => payload.as_deref().is_some_and(mentions_inputs),
        Kind::VecLit(items) => items.iter().any(mentions_inputs),
        Kind::RecordLit { fields } => fields.iter().any(|(_, v)| mentions_inputs(v)),
        Kind::Call { arg, .. } => mentions_inputs(arg),
        Kind::Concat(l, r)
        | Kind::Compare { lhs: l, rhs: r, .. }
        | Kind::Arith { lhs: l, rhs: r, .. } => mentions_inputs(l) || mentions_inputs(r),
        Kind::Bind { value, body, .. } => mentions_inputs(value) || mentions_inputs(body),
        Kind::Map { source, body, .. } => mentions_inputs(source) || mentions_inputs(body),
        Kind::Select { source, pred, .. } => mentions_inputs(source) || mentions_inputs(pred),
        Kind::Cond { cond, then, otherwise } => {
            mentions_inputs(cond) || mentions_inputs(then) || mentions_inputs(otherwise)
        }
        Kind::Field { base, .. } | Kind::Unwrap { base } => mentions_inputs(base),
        Kind::Index { base, index, .. } => mentions_inputs(base) || mentions_inputs(index),
        Kind::Builtin { arg, .. } => mentions_inputs(arg),
        Kind::Match { subject, arms } => {
            mentions_inputs(subject) || arms.iter().any(|a| mentions_inputs(&a.body))
        }
    }
}
