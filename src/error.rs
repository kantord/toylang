use crate::ast::Span;

/// Spans are recorded from the first commit because retrofitting them means touching every
/// node that ever gets constructed. Nothing renders them against the source yet.
#[derive(Debug)]
pub struct Error {
    pub span: Span,
    pub msg: String,
}

impl Error {
    pub fn new(span: Span, msg: impl Into<String>) -> Self {
        Error {
            span,
            msg: msg.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at byte {})", self.msg, self.span.start)
    }
}
