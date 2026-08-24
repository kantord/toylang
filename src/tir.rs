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
    /// A name written in the source: today only a function parameter.
    Var(String),
    Local(LocalId),
    /// The value read from stdin.
    Input,
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
    /// `unlines(v)`, joining with newlines. Named for Haskell's, because `lines` is spoken for
    /// by the splitting direction that `stdin.lines` will need.
    Unlines,
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
