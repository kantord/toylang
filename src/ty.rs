use std::collections::HashMap;

/// Every enum a program declares, resolved once and keyed by name, which is an enum's identity.
///
/// A `Type::Enum` in hand is not always enough to work from. Where an enum's payload reaches
/// back to itself through a `Vec`, that occurrence carries a placeholder in place of the real
/// variant list (`check::types::resolve_named`, kantord/toylang#76), since expanding it is what
/// has no end. So nothing reads `variants` off a type directly: everything that needs the list
/// asks `variants` below, which re-derives it here, one layer at a time and exactly as deep as
/// the program itself navigates (kantord/toylang#94).
pub type Enums = HashMap<String, Type>;

#[derive(Debug, Clone)]
pub enum Type {
    Str,
    Int,
    /// A signed 64-bit integer that wraps, `Int`'s rules at twice the width (kantord/toylang#83,
    /// extending ADR 0006). Deliberately a separate type rather than a widening of `Int`:
    /// nothing converts implicitly, so mixing the two demands an explicit `i64(x)`. Literals
    /// carry no suffix -- one that fits `Int` is `Int`, and any literal resolves as `Int64`
    /// wherever one is expected, the `[]` rule applied to numbers. Barred from `input`, like
    /// `Opt`: not because a 64-bit integer has no JSON spelling but because two backends
    /// cannot yet read one faithfully (JS parses numbers into doubles), and that codec design
    /// has not been done.
    Int64,
    Bool,
    /// A single Unicode scalar value (kantord/toylang#75): never a surrogate half, so a
    /// codepoint outside the Basic Multilingual Plane is one `Char`, on every backend, even
    /// where a backend's own strings need a surrogate pair to spell it. Produced only by
    /// `chars`, which is also the only way one is compared against a boundary -- there is no
    /// literal syntax, so a program spells `'a'` as `chars("a")[0]!`. Barred from `input` (no
    /// wire form to read one from) and from the program's own printed result (no wire form to
    /// write one either); everywhere else -- function signatures, records, a `Vec<Char>` --
    /// it is an ordinary type.
    Char,
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
    /// A declared enum: nominal, so the name is the identity. A name (and, for a generic enum,
    /// its arguments) determines the variants within a program, so `PartialEq` below compares
    /// only those and not `variants` itself -- which matters because a self-referential enum's
    /// own payload carries a placeholder here in place of the real list
    /// (`check::types::resolve_named`, kantord/toylang#76) rather than the list every other
    /// occurrence of the same name carries. A variant's payload is `None` for a unit variant,
    /// in declaration order.
    ///
    /// The list riding along is a convenience, not the source of truth: read it and a
    /// placeholder reads as an enum with no variants at all, which is how a nested recursive
    /// value used to reach every backend's printer as something it could not render
    /// (kantord/toylang#94). Ask `variants` instead.
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
            "Int64" => Some(Type::Int64),
            "Bool" => Some(Type::Bool),
            "Char" => Some(Type::Char),
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

    /// Whether `Int64` appears anywhere in this type. What the checker asks of an input type:
    /// unlike `Char` this is one-directional -- an `Int64` result prints fine on every backend
    /// -- but reading one back is codec design nobody has done (JS parses JSON numbers into
    /// doubles and loses exactness past 2^53), so `input` refuses it, the same reversible
    /// direction `contains_opt` takes for absence.
    pub fn contains_int64(&self) -> bool {
        match self {
            Type::Int64 => true,
            Type::Vec(t) | Type::Stream(t) => t.contains_int64(),
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains_int64()),
            Type::Enum { args, variants, .. } => {
                args.iter().any(Type::contains_int64)
                    || variants
                        .iter()
                        .any(|(_, p)| p.as_ref().is_some_and(Type::contains_int64))
            }
            _ => false,
        }
    }

    /// Whether `Char` appears anywhere in this type. What the checker asks of both `input`'s
    /// type and the program's own printed result: neither has a wire form for a bare Unicode
    /// scalar value, so both refuse it, the same reasoning `contains_opt` states for absence.
    pub fn contains_char(&self) -> bool {
        match self {
            Type::Char => true,
            Type::Vec(t) | Type::Stream(t) => t.contains_char(),
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains_char()),
            Type::Enum { args, variants, .. } => {
                args.iter().any(Type::contains_char)
                    || variants
                        .iter()
                        .any(|(_, p)| p.as_ref().is_some_and(Type::contains_char))
            }
            _ => false,
        }
    }

    /// Whether a `Vec` appears anywhere in this type. What the checker asks of a comparison's
    /// operands: an operator applied to a Vec is Q2 -- broadcast, zip, or whole-value -- and Q2
    /// is open, so a Vec reached through a record field or an enum payload is refused for the
    /// same reason a bare one is, rather than being handed whole-value equality by the back
    /// door (kantord/toylang#95).
    pub fn contains_vec(&self) -> bool {
        match self {
            Type::Vec(_) | Type::Stream(_) => true,
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains_vec()),
            Type::Enum { args, variants, .. } => {
                args.iter().any(Type::contains_vec)
                    || variants
                        .iter()
                        .any(|(_, p)| p.as_ref().is_some_and(Type::contains_vec))
            }
            _ => false,
        }
    }

    /// Whether a value of this type has an interior to walk. Equality on one is structural on
    /// every backend (kantord/toylang#68), which four of them need spelled out because their
    /// native `==` on the runtime shape means identity or compares a payload pointer.
    pub fn is_composite(&self) -> bool {
        matches!(self, Type::Record(_) | Type::Enum { .. })
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
            Type::Int64 => "Int64".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::Char => "Char".to_string(),
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

    /// The name of `self`'s own printer function, shared by every backend that emits one
    /// (kantord/toylang#94). Go camel-cases its own variant at the call site.
    pub fn show_fn(&self) -> String {
        format!("tl_show_{}", self.ident())
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Stream(t) => write!(f, "Stream<{t}>"),
            Type::Str => write!(f, "Str"),
            Type::Int => write!(f, "Int"),
            Type::Int64 => write!(f, "Int64"),
            Type::Bool => write!(f, "Bool"),
            Type::Char => write!(f, "Char"),
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
    ///
    /// `Enum` compares `name` and `args` only, not `variants`: those two already determine the
    /// variant list within one program (a redeclaration is refused, and an instantiation's
    /// payloads are `args` substituted into the one declaration), so comparing `variants` too
    /// would be redundant on every ordinary type and actively wrong on a self-referential one,
    /// whose nested occurrence of itself carries a placeholder rather than the list every other
    /// occurrence of the same name carries (kantord/toylang#76).
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Type::Str, Type::Str)
            | (Type::Int, Type::Int)
            | (Type::Int64, Type::Int64)
            | (Type::Bool, Type::Bool)
            | (Type::Char, Type::Char) => true,
            (Type::Vec(a), Type::Vec(b)) => a == b,
            (Type::Stream(a), Type::Stream(b)) => a == b,
            (Type::Record(a), Type::Record(b)) => {
                a.len() == b.len() && a.iter().all(|field| b.contains(field))
            }
            (
                Type::Enum {
                    name: n1, args: a1, ..
                },
                Type::Enum {
                    name: n2, args: a2, ..
                },
            ) => n1 == n2 && a1 == a2,
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

/// Replace every `Type::Param` in `t` with its binding. Together with the checker's `unify`,
/// this is the whole of generic instantiation: nothing else ever sees a parameter.
pub fn substitute(t: &Type, map: &HashMap<String, Type>) -> Type {
    match t {
        Type::Param(p) => map
            .get(p)
            .cloned()
            .unwrap_or_else(|| unreachable!("substitute runs only once every param is bound")),
        Type::Vec(e) => Type::Vec(Box::new(substitute(e, map))),
        Type::Stream(e) => Type::Stream(Box::new(substitute(e, map))),
        Type::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|(n, t)| (n.clone(), substitute(t, map)))
                .collect(),
        ),
        Type::Enum {
            name,
            args,
            variants,
        } => Type::Enum {
            name: name.clone(),
            args: args.iter().map(|a| substitute(a, map)).collect(),
            variants: variants
                .iter()
                .map(|(n, p)| (n.clone(), p.as_ref().map(|p| substitute(p, map))))
                .collect(),
        },
        Type::Str | Type::Int | Type::Int64 | Type::Bool | Type::Char => t.clone(),
    }
}

/// `name`'s variant list at `args`, re-derived from the registry rather than trusted from
/// whichever `Type::Enum` a caller happened to be holding. One layer deep: a payload that is
/// itself the same enum comes back carrying the placeholder in turn, so asking again is what
/// descends, and nothing tries to build a list that has no bottom.
pub fn variants_of(enums: &Enums, name: &str, args: &[Type]) -> Vec<(String, Option<Type>)> {
    let Type::Enum {
        args: template_args,
        variants,
        ..
    } = &enums[name]
    else {
        unreachable!("the registry holds enum types")
    };
    if template_args.is_empty() {
        return variants.clone();
    }
    let bindings: HashMap<String, Type> = template_args
        .iter()
        .zip(args)
        .filter_map(|(p, a)| match p {
            Type::Param(p) => Some((p.clone(), a.clone())),
            _ => None,
        })
        .collect();
    variants
        .iter()
        .map(|(n, p)| (n.clone(), p.as_ref().map(|t| substitute(t, &bindings))))
        .collect()
}

/// `variants_of` for an enum type in hand.
pub fn variants(enums: &Enums, ty: &Type) -> Vec<(String, Option<Type>)> {
    let Type::Enum { name, args, .. } = ty else {
        unreachable!("only an enum has variants")
    };
    variants_of(enums, name, args)
}

/// Whether a value of this enum type can hold another of the same type, however deep.
///
/// Always through a `Vec`, since that is the only self-reference the checker allows, but not
/// always directly: the path can run through a record field or a second enum. What it decides
/// is whether a backend may expand this type inline -- a printer, a parser, a declaration
/// written by walking the payloads -- or has to emit a named function it can call back into.
/// Expanding a recursive one inline does not terminate.
pub fn is_recursive(enums: &Enums, ty: &Type) -> bool {
    fn reaches(enums: &Enums, from: &Type, target: &Type, seen: &mut Vec<Type>) -> bool {
        match from {
            Type::Vec(e) | Type::Stream(e) => reaches(enums, e, target, seen),
            Type::Record(fields) => fields.iter().any(|(_, t)| reaches(enums, t, target, seen)),
            Type::Enum { name, args, .. } => {
                if from == target {
                    return true;
                }
                if seen.contains(from) {
                    return false;
                }
                seen.push(from.clone());
                args.iter().any(|a| reaches(enums, a, target, seen))
                    || variants_of(enums, name, args)
                        .iter()
                        .any(|(_, p)| p.as_ref().is_some_and(|p| reaches(enums, p, target, seen)))
            }
            _ => false,
        }
    }
    if !matches!(ty, Type::Enum { .. }) {
        return false;
    }
    let mut seen = vec![ty.clone()];
    variants(enums, ty)
        .iter()
        .any(|(_, p)| p.as_ref().is_some_and(|p| reaches(enums, p, ty, &mut seen)))
}
