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
