#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    pub fn to(self, other: Span) -> Span {
        Span::new(self.start, other.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }

    /// True for the operators that only ever mean arithmetic. `+` is missing because it also
    /// concatenates, which is the one place an operator's meaning depends on its operands.
    pub fn is_arithmetic(self) -> bool {
        matches!(self, BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem)
    }
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        };
        write!(f, "{s}")
    }
}

/// A type as written in source, before it is resolved to a `Type`.
#[derive(Debug)]
pub enum TypeExpr {
    Named { name: String, span: Span },
    Vec { elem: Box<TypeExpr>, span: Span },
    Record { fields: Vec<(String, TypeExpr)>, span: Span },
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named { span, .. }
            | TypeExpr::Vec { span, .. }
            | TypeExpr::Record { span, .. } => *span,
        }
    }
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug)]
pub struct Def {
    pub name: String,
    pub param: Param,
    pub ret: TypeExpr,
    pub body: Expr,
    pub span: Span,
}

/// Zero or more definitions followed by the expression that is the program.
#[derive(Debug)]
pub struct File {
    pub defs: Vec<Def>,
    pub body: Expr,
}

#[derive(Debug)]
pub enum Expr {
    Str { text: String, span: Span },
    Int { value: i64, span: Span },
    VecLit { items: Vec<Expr>, span: Span },
    /// `.`, the value the enclosing pipeline or filter is currently working on.
    Subject { span: Span },
    Var { name: String, span: Span },
    Call { func: String, func_span: Span, arg: Box<Expr>, span: Span },
    /// `base[]`. A spec that keeps a dimension.
    Project { base: Box<Expr>, span: Span },
    /// `base[i]`. A spec that collapses a dimension, so the entry may not be there.
    Index { base: Box<Expr>, index: Box<Expr>, span: Span },
    /// `base!`. Insist the value is there, and stop the program if it is not.
    Unwrap { base: Box<Expr>, span: Span },
    /// `-base`.
    Neg { base: Box<Expr>, span: Span },
    /// `then if cond else otherwise`. An expression, in a language that has only those.
    Cond { then: Box<Expr>, cond: Box<Expr>, otherwise: Box<Expr>, span: Span },
    /// `base.name`. Distributes over a Vec rather than needing a map.
    Field { base: Box<Expr>, name: String, span: Span },
    /// The value read from stdin. It has no type of its own and can only be checked against an
    /// expected one, which is the same rule the draft gives for lambdas.
    Input { span: Span },
    /// `lhs | rhs`, which binds `.` in `rhs` to the value of `lhs`.
    Pipe { lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    /// `select(pred)`, where `pred` is checked with `.` bound to the element type rather than
    /// evaluated in the enclosing scope.
    Select { pred: Box<Expr>, span: Span },
    /// `map(f)`, where `f` is checked with `.` bound to the element type. Primitive here rather
    /// than sugar for reflect-apply-reify, since neither half of that exists.
    Map { body: Box<Expr>, span: Span },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Str { span, .. }
            | Expr::Int { span, .. }
            | Expr::VecLit { span, .. }
            | Expr::Subject { span }
            | Expr::Var { span, .. }
            | Expr::Call { span, .. }
            | Expr::Project { span, .. }
            | Expr::Index { span, .. }
            | Expr::Unwrap { span, .. }
            | Expr::Neg { span, .. }
            | Expr::Cond { span, .. }
            | Expr::Field { span, .. }
            | Expr::Input { span }
            | Expr::Pipe { span, .. }
            | Expr::Select { span, .. }
            | Expr::Map { span, .. }
            | Expr::Binary { span, .. } => *span,
        }
    }
}
