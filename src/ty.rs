#[derive(Debug, Clone)]
pub enum Type {
    Str,
    Int,
    Bool,
    Vec(Box<Type>),
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
        /// The type arguments this instantiation was built with, in declaration order; empty
        /// for an enum declared without parameters. Part of the identity: `Pair<Int>` and
        /// `Pair<Str>` are different types even though both are `Pair`.
        args: Vec<Type>,
        variants: Vec<(String, Option<Type>)>,
    },
    /// A type parameter inside a generic enum's registry template, standing for whatever a
    /// use site will supply. Never the type of any checked expression: instantiation
    /// substitutes every one away before a type leaves resolution.
    Param(String),
}

/// Names reserved for the built-in type constructors, on top of `Str`/`Int`/`Bool`
/// (`Type::from_name`): a program cannot declare an alias or enum under any of these.
const RESERVED_TYPE_NAMES: [&str; 2] = ["Vec", "Stream"];

/// Whether `name` is a built-in type and so cannot be redefined as an alias or an enum.
pub fn is_builtin_type_name(name: &str) -> bool {
    Type::from_name(name).is_some() || RESERVED_TYPE_NAMES.contains(&name)
}

/// Whether `name` is one of the built-in type constructors with its own `TypeExpr` node.
/// `Opt` is no longer here: it is the prelude's enum, reached through the ordinary generic
/// path like any declared name.
pub fn takes_type_arg(name: &str) -> bool {
    matches!(name, "Vec" | "Stream")
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

    /// Whether `ty` could hold a stream anywhere inside it -- through a Vec or a record
    /// field. A stream cannot be printed, so this is what the checker asks before letting a
    /// type become the program's final result.
    pub fn contains_stream(&self) -> bool {
        match self {
            Type::Stream(_) => true,
            Type::Vec(t) => t.contains_stream(),
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains_stream()),
            // An enum payload cannot hold a stream: resolve_enum refuses the declaration,
            // and instantiation refuses a stream as a type argument.
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

    /// The prelude's `Opt<T>`, and what its `T` is. The name test is sound because `Opt` is
    /// the prelude's declaration and a program redeclaring it is a duplicate-type error, so
    /// no other enum can carry the name. This is what every place that used to match the
    /// built-in `Type::Opt` keys on now: the unwrap operator, the producers' wrapping, and
    /// the printers' bare-value-or-null serialization rule.
    pub fn as_opt(&self) -> Option<&Type> {
        match self {
            Type::Enum { name, args, .. } if name == "Opt" && args.len() == 1 => Some(&args[0]),
            _ => None,
        }
    }

    /// Whether `Opt` appears anywhere in this type. What the checker asks of an input type:
    /// absence has no ratified wire form (`null` reading as `none` is codec design nobody has
    /// done), so an input cannot be Opt-typed anywhere in its shape.
    pub fn contains_opt(&self) -> bool {
        if self.as_opt().is_some() {
            return true;
        }
        match self {
            Type::Vec(t) | Type::Stream(t) => t.contains_opt(),
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains_opt()),
            Type::Enum { args, variants, .. } => {
                args.iter().any(Type::contains_opt)
                    || variants
                        .iter()
                        .any(|(_, p)| p.as_ref().is_some_and(Type::contains_opt))
            }
            _ => false,
        }
    }

    /// A deterministic identifier fragment for this type, for backends whose targets are
    /// nominally typed: `Pair<Int>` and `Pair<Str>` must become distinct emitted types, so
    /// their names embed the arguments (`Pair_Int`). Record fields sort by name, because
    /// field order is not part of a record type's identity (kantord/toylang#60). An enum
    /// declared without parameters keeps its bare name, which is what every existing
    /// snapshot pins.
    pub fn ident(&self) -> String {
        match self {
            Type::Str => "Str".to_string(),
            Type::Int => "Int".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::Vec(t) => format!("Vec_{}", t.ident()),
            Type::Stream(t) => format!("Stream_{}", t.ident()),
            Type::Record(fields) => {
                let mut parts: Vec<String> = fields
                    .iter()
                    .map(|(n, t)| format!("{n}_{}", t.ident()))
                    .collect();
                parts.sort();
                format!("R_{}", parts.join("_"))
            }
            Type::Enum { name, args, .. } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let parts: Vec<String> = args.iter().map(Type::ident).collect();
                    format!("{name}_{}", parts.join("_"))
                }
            }
            Type::Param(_) => unreachable!("params are substituted before any backend runs"),
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
            Type::Record(fields) => {
                let parts: Vec<String> = fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            // The name is the identity, so the name is the display: spelling the variants out
            // would print the definition where the reader wants the reference.
            Type::Enum { name, args, .. } => {
                write!(f, "{name}")?;
                if !args.is_empty() {
                    let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                    write!(f, "<{}>", parts.join(", "))?;
                }
                Ok(())
            }
            Type::Param(name) => write!(f, "{name}"),
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
            (Type::Stream(a), Type::Stream(b)) => a == b,
            (Type::Record(a), Type::Record(b)) => {
                a.len() == b.len() && a.iter().all(|field| b.contains(field))
            }
            (
                Type::Enum {
                    name: n1,
                    args: a1,
                    variants: v1,
                },
                Type::Enum {
                    name: n2,
                    args: a2,
                    variants: v2,
                },
            ) => n1 == n2 && a1 == a2 && v1 == v2,
            (Type::Param(a), Type::Param(b)) => a == b,
            _ => false,
        }
    }
}

/// A function takes at most one parameter; `None` is a nullary function.
#[derive(Debug, Clone)]
pub struct Sig {
    pub param: Option<Type>,
    pub ret: Type,
}
