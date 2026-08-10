#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    Str,
    Int,
}

impl Type {
    pub fn from_name(name: &str) -> Option<Type> {
        match name {
            "Str" => Some(Type::Str),
            "Int" => Some(Type::Int),
            _ => None,
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Str => write!(f, "Str"),
            Type::Int => write!(f, "Int"),
        }
    }
}

/// Functions are unary, so a signature is one parameter and one result.
#[derive(Debug, Clone, Copy)]
pub struct Sig {
    pub param: Type,
    pub ret: Type,
}
