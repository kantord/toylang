#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

#[derive(Debug)]
pub enum Expr {
    Str { text: String, span: Span },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Str { span, .. } => *span,
        }
    }
}
