#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Str,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Str => write!(f, "Str"),
        }
    }
}
