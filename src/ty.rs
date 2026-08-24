#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Str,
    Int,
    Bool,
    Vec(Box<Type>),
    /// Zero or one. Produced by collapsing a dimension, since the entry may not be there.
    Opt(Box<Type>),
    /// Field names are kept sorted, so two records written in different orders are one type.
    Record(Vec<(String, Type)>),
}

impl Type {
    pub fn from_name(name: &str) -> Option<Type> {
        match name {
            "Str" => Some(Type::Str),
            "Int" => Some(Type::Int),
            "Bool" => Some(Type::Bool),
            _ => None,
        }
    }

    pub fn elem(&self) -> Option<&Type> {
        match self {
            Type::Vec(t) => Some(t),
            _ => None,
        }
    }

    pub fn record(mut fields: Vec<(String, Type)>) -> Type {
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        Type::Record(fields)
    }

    pub fn field(&self, name: &str) -> Option<&Type> {
        match self {
            Type::Record(fields) => fields.iter().find(|(n, _)| n == name).map(|(_, t)| t),
            _ => None,
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Str => write!(f, "Str"),
            Type::Int => write!(f, "Int"),
            Type::Bool => write!(f, "Bool"),
            Type::Vec(t) => write!(f, "Vec<{t}>"),
            Type::Opt(t) => write!(f, "Opt<{t}>"),
            Type::Record(fields) => {
                let parts: Vec<String> =
                    fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
        }
    }
}

/// Functions are unary, so a signature is one parameter and one result.
#[derive(Debug, Clone)]
pub struct Sig {
    pub param: Type,
    pub ret: Type,
}
