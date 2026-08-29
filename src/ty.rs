#[derive(Debug, Clone)]
pub enum Type {
    Str,
    Int,
    Bool,
    Vec(Box<Type>),
    /// Zero or one. Produced by collapsing a dimension, since the entry may not be there.
    Opt(Box<Type>),
    /// Fields in declaration order. Order is not part of the type's identity (kantord/toylang#60:
    /// `{a: Int, b: Int}` and `{b: Int, a: Int}` are one type, so `PartialEq` below compares the
    /// field set, not the spelling), but it survives as metadata on every concrete value: a
    /// literal's own written order is what it prints in on every backend and what the
    /// native/Go column layouts key on. Whichever type a value is checked against is the order
    /// it ends up carrying (`check::reorder_record` rebuilds a value that arrives shuffled), so
    /// two differently-ordered values only coexist when nothing ever placed them in the same
    /// checked position.
    Record(Vec<(String, Type)>),
    /// Effect-layer multiplicity: the entries arrive one at a time as evaluation proceeds, and
    /// no stream object ever exists as a value (ADR 0001). Spellable in function signatures,
    /// and only there -- the type grammar refuses it inside a Vec, a record, or another Stream
    /// -- born at a source (`lines` is `Stream<Str>`), consumed exactly once per binding, and
    /// exiting only through `collect`.
    Stream(Box<Type>),
    /// A declared enum: nominal, so the name is the identity. The variants ride along so that
    /// every consumer of a `Type` -- printers, backends, input validation -- has them in hand
    /// without a registry beside the tree; a name determines its variants within a program, so
    /// the derived equality is still name equality in practice. A variant's payload is `None`
    /// for a unit variant, in declaration order.
    Enum {
        name: String,
        variants: Vec<(String, Option<Type>)>,
    },
}

/// Names reserved for the built-in type constructors, on top of `Str`/`Int`/`Bool`
/// (`Type::from_name`): a program cannot declare an alias or enum under any of these.
const RESERVED_TYPE_NAMES: [&str; 3] = ["Vec", "Opt", "Stream"];

/// Whether `name` is a built-in type and so cannot be redefined as an alias or an enum.
pub fn is_builtin_type_name(name: &str) -> bool {
    Type::from_name(name).is_some() || RESERVED_TYPE_NAMES.contains(&name)
}

/// Whether `name` takes a `<...>` type argument in the grammar. All three reserved
/// constructors do: `Opt` can be produced by the checker (collapsing a dimension) as well as
/// spelled directly, which is what lets a function declare one as a parameter or return type.
pub fn takes_type_arg(name: &str) -> bool {
    matches!(name, "Vec" | "Stream" | "Opt")
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

    /// Whether `ty` could hold a stream anywhere inside it -- through a Vec, an Opt, or a
    /// record field. A stream cannot be printed, so this is what the checker asks before
    /// letting a type become the program's final result.
    pub fn contains_stream(&self) -> bool {
        match self {
            Type::Stream(_) => true,
            Type::Vec(t) | Type::Opt(t) => t.contains_stream(),
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains_stream()),
            // An enum payload cannot hold a stream: resolve_enum refuses the declaration.
            _ => false,
        }
    }

    pub fn elem(&self) -> Option<&Type> {
        match self {
            Type::Vec(t) => Some(t),
            _ => None,
        }
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
            Type::Stream(t) => write!(f, "Stream<{t}>"),
            Type::Str => write!(f, "Str"),
            Type::Int => write!(f, "Int"),
            Type::Bool => write!(f, "Bool"),
            Type::Vec(t) => write!(f, "Vec<{t}>"),
            Type::Opt(t) => write!(f, "Opt<{t}>"),
            Type::Record(fields) => {
                let parts: Vec<String> = fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            // The name is the identity, so the name is the display: spelling the variants out
            // would print the definition where the reader wants the reference.
            Type::Enum { name, .. } => write!(f, "{name}"),
        }
    }
}

impl PartialEq for Type {
    /// Structural equality, except `Record`: its fields compare as a set, keyed by name and
    /// type, ignoring position (kantord/toylang#60). A record's own field order is real data --
    /// it drives printing and backend column layout -- but it is not part of what makes two
    /// types the same type.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Type::Str, Type::Str) | (Type::Int, Type::Int) | (Type::Bool, Type::Bool) => true,
            (Type::Vec(a), Type::Vec(b)) => a == b,
            (Type::Opt(a), Type::Opt(b)) => a == b,
            (Type::Stream(a), Type::Stream(b)) => a == b,
            (Type::Record(a), Type::Record(b)) => {
                a.len() == b.len() && a.iter().all(|field| b.contains(field))
            }
            (
                Type::Enum {
                    name: n1,
                    variants: v1,
                },
                Type::Enum {
                    name: n2,
                    variants: v2,
                },
            ) => n1 == n2 && v1 == v2,
            _ => false,
        }
    }
}

/// Functions are unary, so a signature is one parameter and one result.
#[derive(Debug, Clone)]
pub struct Sig {
    pub param: Type,
    pub ret: Type,
}
