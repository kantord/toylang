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

#[derive(Debug)]
pub enum Expr {
    Str { text: String, span: Span },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Str { span, .. } | Expr::Binary { span, .. } => *span,
        }
    }
}
