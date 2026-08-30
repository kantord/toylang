//! Type resolution: turns the surface syntax of types (`TypeExpr`, `EnumDecl`) into `ty::Type`.
//! Runs once, eagerly, before any expression is checked, and touches neither `Expr` nor `Tir`.

use std::collections::HashMap;

use crate::ast::{Alias, Def, EnumDecl, Span, TypeExpr};
use crate::error::Error;
use crate::ty::{self, Sig, Type};

use super::BUILTIN_NAMES;

/// Capitalised names are types and lowercase names are values.
///
/// Every name in the language already followed this, unenforced. Making it a rule is what lets a
/// constructor share a spelling with a call without sharing a namespace: `User {..}` builds and
/// `area {..}` calls, decided by the first letter rather than by looking either one up.
///
/// Field names are deliberately not covered. They come from data, and JSON objects are entitled
/// to a key spelled `Name`.
fn value_name(name: &str, span: Span, what: &str) -> Result<(), Error> {
    if name.chars().next().is_some_and(char::is_uppercase) {
        return Err(Error::new(
            span,
            format!("a {what} starts with a lowercase letter, and `{name}` reads as a type"),
        ));
    }
    Ok(())
}

/// What a type name stands for. An alias is an abbreviation, so this maps to the written form
/// and `resolve` expands it; nothing downstream ever learns a name was involved.
type Aliases<'a> = HashMap<String, &'a TypeExpr>;

/// The named types a written annotation can refer to. Aliases expand away; an enum resolves to
/// the identity its declaration created.
pub(super) struct TypeEnv<'a> {
    pub(super) aliases: Aliases<'a>,
    pub(super) enums: HashMap<String, &'a EnumDecl>,
}

pub(super) fn enum_map(enums: &[EnumDecl]) -> Result<HashMap<String, &EnumDecl>, Error> {
    let mut map: HashMap<String, &EnumDecl> = HashMap::new();
    for e in enums {
        if !e.name.chars().next().is_some_and(char::is_uppercase) {
            return Err(Error::new(
                e.span,
                format!(
                    "a type name starts with a capital letter, and `{}` reads as a value",
                    e.name
                ),
            ));
        }
        if ty::is_builtin_type_name(&e.name) {
            return Err(Error::new(
                e.span,
                format!("`{}` is a built-in type and cannot be redefined", e.name),
            ));
        }
        for (i, v) in e.variants.iter().enumerate() {
            if e.variants[..i].iter().any(|earlier| earlier.name == v.name) {
                return Err(Error::new(
                    v.span,
                    format!("variant `{}` is declared twice in `{}`", v.name, e.name),
                ));
            }
        }
        if map.insert(e.name.clone(), e).is_some() {
            return Err(Error::new(
                e.span,
                format!("type `{}` is defined twice", e.name),
            ));
        }
    }
    Ok(map)
}

/// Resolve one enum declaration to its type. `seen` carries the names being expanded and the
/// arguments each was expanded with, exactly as for aliases (whose entry carries no arguments),
/// so an enum whose payload mentions itself is refused rather than expanded forever -- unless it
/// does so behind a `Vec`, which `resolve_named` treats as legal (kantord/toylang#76) and stops
/// expanding at, rather than looping. The arguments travel alongside the name so that a boxed
/// self-reference can be checked against the instantiation it reappears inside
/// (kantord/toylang#117): reusing them is the only way `resolve_named` avoids expanding one
/// forever, so a self-reference that reappears with different arguments -- `Nest<Vec<T>>` inside
/// `Nest<T>` -- has no honest placeholder to return and is refused instead.
///
/// `args` instantiates a generic declaration: payloads resolve with each parameter bound to
/// its argument, already resolved (an argument never re-enters this enum's `seen` chain, so
/// `Opt<Opt<Int>>` is two honest levels, not a false cycle). `None` builds the registry
/// template instead, each parameter standing for itself as `Type::Param` -- which is also the
/// eager pass that validates a declaration nothing uses.
pub(super) fn resolve_enum(
    decl: &EnumDecl,
    env: &TypeEnv,
    seen: &mut Vec<(String, Vec<Type>)>,
    args: Option<&[Type]>,
) -> Result<Type, Error> {
    for (i, (p, span)) in decl.params.iter().enumerate() {
        if !p.chars().next().is_some_and(char::is_uppercase) {
            return Err(Error::new(
                *span,
                format!(
                    "a type parameter starts with a capital letter, and `{p}` reads as a value"
                ),
            ));
        }
        // A parameter may shadow a declared enum or alias -- resolve_named consults the
        // parameter bindings first, so inside the declaration the name means the parameter,
        // unambiguously, the same scoping Rust gives struct Foo<E> beside an enum E. Refusing
        // the collision instead broke every program declaring enum E the moment the prelude
        // gained Result<T, E> (kantord/toylang#85). Builtins stay off limits: Vec-the-parameter
        // would make every Vec<...> in the payload mean the wrong thing at a distance.
        if ty::is_builtin_type_name(p) {
            return Err(Error::new(
                *span,
                format!("type parameter `{p}` takes the name of a built-in type"),
            ));
        }
        if decl.params[..i].iter().any(|(earlier, _)| earlier == p) {
            return Err(Error::new(
                *span,
                format!("type parameter `{p}` is declared twice in `{}`", decl.name),
            ));
        }
    }
    let bound: Vec<Type> = match args {
        Some(args) => args.to_vec(),
        None => decl
            .params
            .iter()
            .map(|(p, _)| Type::Param(p.clone()))
            .collect(),
    };
    let params: HashMap<&str, &Type> = decl
        .params
        .iter()
        .map(|(p, _)| p.as_str())
        .zip(bound.iter())
        .collect();
    seen.push((decl.name.clone(), bound.clone()));
    let mut variants = Vec::new();
    for v in &decl.variants {
        let payload = match &v.payload {
            Some(ty) => {
                // `boxed` starts false at the top of every variant: whether this one payload
                // reaches back to `decl.name` through a `Vec` is assessed fresh each time, so
                // `enum E { safe(Vec<E>), bad(E) }` accepts `safe` and still refuses `bad`.
                let resolved = resolve_bound(ty, env, seen, &params, false)?;
                // The record and Vec spellings refuse a stream inside themselves, but the
                // parens spelling can put one directly in payload position, so the ban has to
                // be stated here too: an enum value is storable, and a stream is not.
                if resolved.contains_stream() {
                    return Err(Error::new(
                        ty.span(),
                        format!(
                            "the payload of `{}` cannot hold a stream, which has nothing to store",
                            v.name
                        ),
                    ));
                }
                Some(resolved)
            }
            None => None,
        };
        variants.push((v.name.clone(), payload));
    }
    seen.pop();
    Ok(Type::Enum {
        name: decl.name.clone(),
        args: bound,
        variants,
    })
}

pub(super) fn alias_map(aliases: &[Alias]) -> Result<Aliases<'_>, Error> {
    let mut map: Aliases = HashMap::new();
    for a in aliases {
        if !a.name.chars().next().is_some_and(char::is_uppercase) {
            return Err(Error::new(
                a.span,
                format!(
                    "a type name starts with a capital letter, and `{}` reads as a value",
                    a.name
                ),
            ));
        }
        if ty::is_builtin_type_name(&a.name) {
            return Err(Error::new(
                a.span,
                format!("`{}` is a built-in type and cannot be redefined", a.name),
            ));
        }
        if map.insert(a.name.clone(), &a.ty).is_some() {
            return Err(Error::new(
                a.span,
                format!("type `{}` is defined twice", a.name),
            ));
        }
    }
    Ok(map)
}

pub(super) fn signatures(defs: &[Def], env: &TypeEnv) -> Result<HashMap<String, Sig>, Error> {
    let mut sigs = HashMap::new();
    for def in defs {
        value_name(&def.name, def.span, "function name")?;
        if let Some(param) = &def.param {
            value_name(&param.name, param.span, "parameter name")?;
        }
        if BUILTIN_NAMES.contains(&def.name.as_str()) {
            return Err(Error::new(
                def.span,
                format!("`{}` is a builtin and cannot be redefined", def.name),
            ));
        }
        if sigs.contains_key(&def.name) {
            return Err(Error::new(
                def.span,
                format!("`{}` is defined twice", def.name),
            ));
        }
        let sig = Sig {
            param: match &def.param {
                Some(param) => Some(resolve(&param.ty, env, &mut Vec::new())?),
                None => None,
            },
            ret: resolve(&def.ret, env, &mut Vec::new())?,
        };
        // A stream is born only at a source, so a function cannot conjure one: a stream result
        // flows in through a stream parameter, and the pipeline stays one chain fusion can
        // read. Refusing is the reversible direction.
        if matches!(sig.ret, Type::Stream(_)) && !matches!(sig.param, Some(Type::Stream(_))) {
            return Err(Error::new(
                def.span,
                format!(
                    "`{}` returns {} without taking a stream; a stream is born only at a source",
                    def.name, sig.ret
                ),
            ));
        }
        sigs.insert(def.name.clone(), sig);
    }
    Ok(sigs)
}

/// `seen` is the chain of names currently being expanded, each with the arguments it was
/// expanded with -- aliases and enums share it -- so a type written in terms of itself is
/// caught rather than expanded forever.
pub(super) fn resolve(
    ty: &TypeExpr,
    env: &TypeEnv,
    seen: &mut Vec<(String, Vec<Type>)>,
) -> Result<Type, Error> {
    resolve_bound(ty, env, seen, &HashMap::new(), false)
}

/// The arity-checked, resolved type arguments for `name`'s `<...>`, shared between an ordinary
/// instantiation and a self-reference caught below -- both need the same check against
/// `decl.params.len()`, one on the way to expanding the declaration and the other on the way to
/// a placeholder for it. An argument is a fresh type position, not a continuation of whatever
/// `Vec` this occurrence of `name` itself sat behind, so it resolves with `boxed` reset to
/// `false` rather than threading the caller's through.
fn resolve_args(
    name: &str,
    args: &[TypeExpr],
    decl_params: usize,
    span: Span,
    env: &TypeEnv,
    seen: &mut Vec<(String, Vec<Type>)>,
    params: &HashMap<&str, &Type>,
) -> Result<Vec<Type>, Error> {
    if args.len() != decl_params {
        let wants = match decl_params {
            0 => format!("`{name}` takes no type argument"),
            1 => format!("`{name}` takes one type argument"),
            n => format!("`{name}` takes {n} type arguments"),
        };
        let found = match args.len() {
            0 => String::new(),
            n => format!(", found {n}"),
        };
        return Err(Error::new(span, format!("{wants}{found}")));
    }
    let mut resolved_args = Vec::new();
    for arg in args {
        let resolved = resolve_bound(arg, env, seen, params, false)?;
        if resolved.contains_stream() {
            return Err(Error::new(
                arg.span(),
                format!("`{name}` cannot hold a stream, which has nothing to store"),
            ));
        }
        resolved_args.push(resolved);
    }
    Ok(resolved_args)
}

/// One named reference: a bound parameter, a built-in scalar, an alias, or an enum -- with
/// the arity of its `<...>` arguments held here, where the declaration is in hand.
///
/// `boxed` is whether this occurrence has already passed through a `Vec` since the innermost
/// enum currently being expanded (`seen`'s top) started -- reset at the top of each variant's
/// payload by `resolve_enum`, set by the `Vec` arm below, and otherwise carried through
/// unchanged, because once a value's storage is a heap indirection every field nested inside it
/// stays indirect too.
fn resolve_named(
    name: &str,
    args: &[TypeExpr],
    span: Span,
    env: &TypeEnv,
    seen: &mut Vec<(String, Vec<Type>)>,
    params: &HashMap<&str, &Type>,
    boxed: bool,
) -> Result<Type, Error> {
    if let Some(bound) = params.get(name) {
        if !args.is_empty() {
            return Err(Error::new(
                span,
                format!("`{name}` is a type parameter and takes no type argument"),
            ));
        }
        return Ok((*bound).clone());
    }
    if let Some(built_in) = Type::from_name(name) {
        if !args.is_empty() {
            return Err(Error::new(span, format!("`{name}` takes no type argument")));
        }
        return Ok(built_in);
    }
    if let Some(at) = seen.iter().position(|(s, _)| s == name) {
        // Behind a `Vec`, a self-reference is a heap indirection rather than an infinite
        // layout, the same reason `Vec<Json>` is an ordinary field and not a contradiction
        // (kantord/toylang#76: Json's array case). Nothing downstream can expand `name`'s own
        // variants without looping forever -- `name` is still on `seen` -- so this stops one
        // layer short and returns a placeholder instead; whichever consumer actually needs the
        // variants (`check::enum_variants`) re-derives them from the registry, one layer at a
        // time, exactly as deep as the program itself ever navigates.
        if boxed {
            let decl_params = env.enums.get(name).map_or(0, |d| d.params.len());
            let resolved_args = resolve_args(name, args, decl_params, span, env, seen, params)?;
            // The placeholder above stands in for `name`'s own variant list, re-derived later
            // from the registry at whatever arguments this occurrence carries. That only works
            // if those arguments are the ones already being expanded: `Nest<T>` referring to
            // itself as `Nest<Vec<T>>` names a second, larger instantiation the registry never
            // builds, and re-deriving from it recurses one `Vec` deeper every time -- forever,
            // since no two of `Nest<T>`, `Nest<Vec<T>>`, `Nest<Vec<Vec<T>>>`, ... ever coincide
            // (kantord/toylang#117). Requiring an exact repeat closes that off: a self-reference
            // is a cycle back to the instantiation already in progress, not a new one.
            if resolved_args != seen[at].1 {
                let expected = Type::Enum {
                    name: name.to_string(),
                    args: seen[at].1.clone(),
                    variants: Vec::new(),
                };
                return Err(Error::new(
                    span,
                    format!(
                        "`{name}` refers to itself with different type arguments; a \
                         self-reference must repeat `{expected}` unchanged"
                    ),
                ));
            }
            return Ok(Type::Enum {
                name: name.to_string(),
                args: resolved_args,
                variants: Vec::new(),
            });
        }
        // The names expanded since this one last appeared are the cycle, and naming them
        // is the difference between knowing there is one and finding it.
        let through: Vec<String> = seen[at + 1..]
            .iter()
            .map(|(s, _)| format!("`{s}`"))
            .collect();
        let path = if through.is_empty() {
            String::new()
        } else {
            format!(", through {}", through.join(" and "))
        };
        return Err(Error::new(
            span,
            format!("type `{name}` is written in terms of itself{path}"),
        ));
    }
    if let Some(written) = env.aliases.get(name) {
        if !args.is_empty() {
            return Err(Error::new(span, format!("`{name}` takes no type argument")));
        }
        seen.push((name.to_string(), Vec::new()));
        let expanded = resolve_bound(written, env, seen, params, boxed);
        seen.pop();
        return expanded;
    }
    if let Some(decl) = env.enums.get(name) {
        let resolved_args = resolve_args(name, args, decl.params.len(), span, env, seen, params)?;
        return resolve_enum(decl, env, seen, Some(&resolved_args));
    }
    Err(Error::new(span, format!("unknown type `{name}`")))
}

/// `resolve` with a generic enum's parameters in scope: inside `enum Opt<T>`'s payloads,
/// `T` names whatever the binding holds -- the argument at an instantiation, or the
/// parameter itself in the registry template. Everywhere else the binding map is empty.
fn resolve_bound(
    ty: &TypeExpr,
    env: &TypeEnv,
    seen: &mut Vec<(String, Vec<Type>)>,
    params: &HashMap<&str, &Type>,
    boxed: bool,
) -> Result<Type, Error> {
    match ty {
        TypeExpr::Named { name, args, span } => {
            resolve_named(name, args, *span, env, seen, params, boxed)
        }
        // The containment bans hold in the grammar itself, not just at value construction
        // sites: a stream is not a value, so no annotation may describe one as stored.
        TypeExpr::Vec { elem, .. } => {
            let inner = resolve_bound(elem, env, seen, params, true)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "a Vec cannot hold a stream, which has nothing to store".to_string(),
                ));
            }
            Ok(Type::Vec(Box::new(inner)))
        }
        TypeExpr::Stream { elem, .. } => {
            let inner = resolve_bound(elem, env, seen, params, boxed)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "a Stream cannot hold another stream; there is nothing it could yield"
                        .to_string(),
                ));
            }
            Ok(Type::Stream(Box::new(inner)))
        }
        TypeExpr::Record { fields, span } => {
            let mut out = Vec::new();
            for (name, ty) in fields {
                if out.iter().any(|(n, _): &(String, Type)| n == name) {
                    return Err(Error::new(
                        *span,
                        format!("field `{name}` is declared twice"),
                    ));
                }
                let field = resolve_bound(ty, env, seen, params, boxed)?;
                if field.contains_stream() {
                    return Err(Error::new(
                        ty.span(),
                        format!("`{name}` cannot hold a stream, which has nothing to store"),
                    ));
                }
                out.push((name.clone(), field));
            }
            Ok(Type::Record(out))
        }
    }
}
