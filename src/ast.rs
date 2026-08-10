#[derive(Debug, Clone, Copy, PartialEq)]
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
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
        }
    }
}

/// A type as written in source, before it is resolved to a `Type`. Only named types exist, so
/// this is a string and a span, but keeping it distinct is what lets an unknown type name be
/// reported at the place it was written.
#[derive(Debug)]
pub struct TypeExpr {
    pub name: String,
    pub span: Span,
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
    Var { name: String, span: Span },
    Call { func: String, func_span: Span, arg: Box<Expr>, span: Span },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Str { span, .. }
            | Expr::Int { span, .. }
            | Expr::Var { span, .. }
            | Expr::Call { span, .. }
            | Expr::Binary { span, .. } => *span,
        }
    }
}
