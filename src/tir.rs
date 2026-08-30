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
    /// A record literal, its fields in declaration order so a field's position here matches
    /// its position in the type. That is what lets a backend address one by index rather than
    /// searching for it.
    RecordLit {
        fields: Vec<(String, Tir)>,
    },
    /// A constructed enum value. The node's type is the enum, which carries the variant list,
    /// so a backend finds the variant's position (its tag, where one is needed) and its payload
    /// type there rather than in the node. `payload` is `None` for a unit variant, which every
    /// backend renders as the bare variant-name string; a payload variant is the single-key
    /// wrapper (ADR 0009).
    EnumLit {
        variant: String,
        payload: Option<Box<Tir>>,
    },
    /// Rebuilds an Opt: present-preserving, absent-preserving. `body` runs with `param` bound
    /// to the unwrapped payload only when `source` is present; an absent `source` passes
    /// through untouched. Never surface syntax -- Opt has no general map or match a program can
    /// reach (kantord/toylang#47's totality round owns that) -- this is only how the checker's
    /// own reorder pass reaches inside an Opt payload (kantord/toylang#66): every other enum
    /// reorders through `Match`/`EnumLit`, but Opt keeps a representation of its own in three
    /// backends, so those two general nodes cannot reach it.
    OptMap {
        source: Box<Tir>,
        param: LocalId,
        body: Box<Tir>,
    },
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
        /// `None` for a call to a nullary function.
        arg: Option<Box<Tir>>,
    },
    Concat(Box<Tir>, Box<Tir>),
    /// Wrapping integer arithmetic, at the width the node's own type names: 32 bits for `Int`,
    /// 64 for `Int64` (kantord/toylang#83) -- both operands always share it, since nothing
    /// converts implicitly. Division and remainder stop the program on a zero divisor, which
    /// is the only way arithmetic can fail.
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
    Builtin {
        which: Builtin,
        arg: Box<Tir>,
    },
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
    /// First-match-wins dispatch over the subject: variant arms test its shape, guard arms
    /// evaluate a Bool of their own. The checker has already resolved every name a pattern
    /// bound, so an arm is only a test, a payload local to bind, and a body.
    ///
    /// When the chain is total (`partial` is false) -- every variant covered, or a default at
    /// the end -- a backend may take the last arm without a test. A partial chain has no such
    /// arm: every test is emitted, and falling off the end produces the absent `Opt`, which is
    /// why the node's type is then `Opt` of the arms' common body type.
    Match {
        subject: Box<Tir>,
        arms: Vec<MatchArm>,
        partial: bool,
    },
}

pub struct MatchArm {
    /// `Some` tests the subject for this variant. `None` with a `guard` is a guard arm; `None`
    /// with no guard is the default arm, which the checker keeps last.
    pub variant: Option<String>,
    /// The Bool deciding a guard arm, already checked in the subject's scope.
    pub guard: Option<Tir>,
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
    /// `i64(n)`, `Int -> Int64`: the one bridge between the two integer types, explicit
    /// because nothing widens implicitly (kantord/toylang#83). A no-op on every backend whose
    /// runtime integers are already 64 bits wide; JS builds a BigInt, and Go and Rust spell
    /// the cast.
    IntToI64,
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
    /// `fields(r)`, a record's field names in declaration order (kantord/toylang#63, the
    /// accessor #60's ratification promised once order left type identity). The names come from
    /// `r`'s checked type, not its runtime value, so every backend can emit them as a literal;
    /// `r` is still evaluated for whatever it would otherwise do, the same as any other argument.
    Fields,
    /// `chars(s)`, `Str -> Vec<Char>`: every Unicode scalar value in `s`, in order. Decoded by
    /// codepoint on every backend, so a character outside the Basic Multilingual Plane is one
    /// element here even on a target whose own strings need a surrogate pair to spell it
    /// (kantord/toylang#75).
    Chars,
}

pub struct Func {
    pub name: String,
    /// `None` for a nullary function.
    pub param: Option<String>,
    pub param_ty: Option<Type>,
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

/// The element type a backend iterates over. Under eager lowering a stream is materialized as
/// the Vec of its entries, so a Stream's element counts exactly as a Vec's here; the checker's
/// `Type::elem` deliberately does not agree, keeping the reducers Vec-only at the surface.
pub fn runtime_elem(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Vec(t) | Type::Stream(t) => Some(t),
        _ => None,
    }
}

/// How many dimension layers wrap a scalar -- `Vec`s, plus the outermost `Stream` when there
/// is one. Field access distributes over each of them.
pub fn vec_depth(ty: &Type) -> usize {
    let mut depth = 0;
    let mut inner = ty;
    while let Some(elem) = runtime_elem(inner) {
        depth += 1;
        inner = elem;
    }
    depth
}

/// One `map`/`select` applied between reading a record and printing it, in source order (the
/// stage nearest the source first).
pub enum Stage<'a> {
    Map { param: LocalId, body: &'a Tir },
    Select { param: LocalId, pred: &'a Tir },
}

/// What a fused loop reads one entry at a time: parsed JSON values, or raw lines.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Inputs,
    Lines,
}

/// A stream-typed pipeline ending in `jsonlines`, compiled as a read-one/transform-one/
/// write-one loop. Decided by the types, not by a structural guess: the old `recognize_fusion`
/// pattern match retired when `Stream` entered the type grammar (plans/streams.md step 5), so
/// whether a program streams is now exactly whether its types say so.
pub struct Fusion<'a> {
    pub source: Source,
    pub stages: Vec<Stage<'a>>,
}

/// What a stream chain bottoms out at: a source, a function's own parameter, or the local a
/// `|` bound.
enum Base<'a> {
    Inputs,
    Lines,
    Var(&'a String),
    Local(LocalId),
}

/// Append the per-element stages of the stream-typed `t`, returning what its chain bottoms out
/// at.
///
/// Total, not a recognizer: the checker leaves exactly these kinds able to carry a Stream type
/// (sources, bindings, mappers, calls -- a conditional or match yielding one is refused, and
/// projection is normalized to a Map), so the `unreachable!` arms are compiler bugs, never
/// program shapes. That is the invariant this step lands: a stream-typed program that failed
/// to fuse would silently materialize stdin, which is the defect the feature exists to remove.
fn flatten<'a>(t: &'a Tir, program: &'a Program, stages: &mut Vec<Stage<'a>>) -> Base<'a> {
    match &t.kind {
        Kind::Inputs => Base::Inputs,
        Kind::Lines => Base::Lines,
        Kind::Var(name) => Base::Var(name),
        Kind::Local(id) => Base::Local(*id),
        Kind::Bind { local, value, body } => {
            let base = flatten(value, program, stages);
            match flatten(body, program, stages) {
                Base::Local(id) if id == *local => base,
                _ => unreachable!("a piped stream is consumed by the pipe's own body"),
            }
        }
        Kind::Map {
            source,
            param,
            body,
        } => {
            let base = flatten(source, program, stages);
            stages.push(Stage::Map {
                param: *param,
                body,
            });
            base
        }
        Kind::Select {
            source,
            param,
            pred,
        } => {
            let base = flatten(source, program, stages);
            stages.push(Stage::Select {
                param: *param,
                pred,
            });
            base
        }
        // The argument's stages first, then the callee's body inlined: its chain bottoms at
        // its own parameter (a stream-returning function must take a stream, and linearity
        // makes the parameter the base of whatever it returns).
        Kind::Call { func, arg } => {
            let f = program
                .funcs
                .iter()
                .find(|f| &f.name == func)
                .expect("the checker resolved every call");
            // A stream-returning function must take a stream (see `signatures`), so a call
            // reaching here -- one whose result is stream-typed -- always carries an argument.
            let arg = arg
                .as_deref()
                .expect("a stream-returning function is never nullary");
            let base = flatten(arg, program, stages);
            let param = f
                .param
                .as_deref()
                .expect("a stream-returning function is never nullary");
            match flatten(&f.body, program, stages) {
                Base::Var(name) if name == param => base,
                _ => unreachable!("a stream-returning function's chain bottoms at its parameter"),
            }
        }
        _ => unreachable!("the checker leaves no other kind stream-typed"),
    }
}

/// The type question `streams_inputs` used to answer by pattern match: a `jsonlines` program
/// whose argument is stream-typed fuses, and one whose argument is a Vec (already collected)
/// stays an ordinary value and runs eagerly, exactly as its types say.
pub fn fusion(program: &Program) -> Option<Fusion<'_>> {
    let Kind::Builtin {
        which: Builtin::JsonLines,
        arg,
    } = &program.body.kind
    else {
        return None;
    };
    if !matches!(arg.ty, Type::Stream(_)) {
        return None;
    }
    let mut stages = Vec::new();
    let source = match flatten(arg, program, &mut stages) {
        Base::Inputs => Source::Inputs,
        Base::Lines => Source::Lines,
        Base::Var(_) | Base::Local(_) => {
            unreachable!("a program-level stream chain bottoms at its source")
        }
    };
    Some(Fusion { source, stages })
}
