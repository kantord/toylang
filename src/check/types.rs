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

/// Resolve one enum declaration to its type. `seen` carries the names being expanded, exactly
/// as for aliases, so an enum whose payload mentions itself is refused rather than expanded
/// forever -- there is no indirection for a recursive payload to hide behind yet.
pub(super) fn resolve_enum(
    decl: &EnumDecl,
    env: &TypeEnv,
    seen: &mut Vec<String>,
) -> Result<Type, Error> {
    seen.push(decl.name.clone());
    let mut variants = Vec::new();
    for v in &decl.variants {
        let payload = match &v.payload {
            Some(ty) => {
                let resolved = resolve(ty, env, seen)?;
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

/// `seen` is the chain of names currently being expanded -- aliases and enums share it -- so a
/// type written in terms of itself is caught rather than expanded forever.
pub(super) fn resolve(ty: &TypeExpr, env: &TypeEnv, seen: &mut Vec<String>) -> Result<Type, Error> {
    match ty {
        TypeExpr::Named { name, span } => {
            if let Some(built_in) = Type::from_name(name) {
                return Ok(built_in);
            }
            if let Some(at) = seen.iter().position(|s| s == name) {
                // The names expanded since this one last appeared are the cycle, and naming them
                // is the difference between knowing there is one and finding it.
                let through: Vec<String> =
                    seen[at + 1..].iter().map(|s| format!("`{s}`")).collect();
                let path = if through.is_empty() {
                    String::new()
                } else {
                    format!(", through {}", through.join(" and "))
                };
                return Err(Error::new(
                    *span,
                    format!("type `{name}` is written in terms of itself{path}"),
                ));
            }
            if let Some(written) = env.aliases.get(name) {
                seen.push(name.clone());
                let expanded = resolve(written, env, seen)?;
                seen.pop();
                return Ok(expanded);
            }
            if let Some(decl) = env.enums.get(name) {
                return resolve_enum(decl, env, seen);
            }
            Err(Error::new(*span, format!("unknown type `{name}`")))
        }
        // The containment bans hold in the grammar itself, not just at value construction
        // sites: a stream is not a value, so no annotation may describe one as stored.
        TypeExpr::Vec { elem, .. } => {
            let inner = resolve(elem, env, seen)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "a Vec cannot hold a stream, which has nothing to store".to_string(),
                ));
            }
            Ok(Type::Vec(Box::new(inner)))
        }
        TypeExpr::Stream { elem, .. } => {
            let inner = resolve(elem, env, seen)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "a Stream cannot hold another stream; there is nothing it could yield"
                        .to_string(),
                ));
            }
            Ok(Type::Stream(Box::new(inner)))
        }
        TypeExpr::Opt { elem, .. } => {
            let inner = resolve(elem, env, seen)?;
            if inner.contains_stream() {
                return Err(Error::new(
                    elem.span(),
                    "an Opt cannot hold a stream, which has nothing to store".to_string(),
                ));
            }
            Ok(Type::Opt(Box::new(inner)))
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
                let field = resolve(ty, env, seen)?;
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
