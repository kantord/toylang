//! The checker's half of `@(path)` module routing (gh:167); `crate::modules` is the loader that
//! resolved the path and merged the module's declarations before checking began.

use crate::ast::{Origin, Span};
use crate::error::Error;
use crate::tir::{Kind, Tir};

use super::{Ctx, conform};

/// `@(path)`: a call of the routed module's `handle` with `.` as its argument. Checked as
/// strictly as any call -- `.`'s type must be exactly `handle`'s declared parameter type, no
/// coercion (the dispatch-strictness ruling) -- but the messages name the route, since the
/// program never spelled the function it is calling.
pub(super) fn route_call(ctx: &Ctx, path: &str, span: Span) -> Result<Tir, Error> {
    let Some(entry) = ctx.routes.get(&(ctx.file.clone(), path.to_string())) else {
        // Only reachable by checking a parsed file without running `modules::inject` on it.
        return Err(Error::new(
            span,
            format!("module `{path}` was not loaded before checking"),
        ));
    };
    let sig = ctx
        .sigs
        .get(entry)
        .expect("the loader renamed a real `handle` to this name");
    let arg = match &sig.param {
        None => None,
        Some(param_ty) => {
            let Some((ty, local)) = &ctx.subject else {
                return Err(Error::new(
                    span,
                    format!(
                        "`@(\"{path}\")` applies the module's `handle` to `.`, but `.` is not \
                         bound here"
                    ),
                ));
            };
            if ty != param_ty {
                return Err(Error::new(
                    span,
                    format!("module `{path}`'s `handle` takes {param_ty}, but `.` here is {ty}"),
                ));
            }
            // Same as an ordinary call's argument: equal record types can still differ in
            // field order, which the positional backends read by.
            Some(Box::new(conform(
                ctx,
                Tir::new(ty.clone(), Kind::Local(*local)),
                param_ty,
            )))
        }
    };
    Ok(Tir::new(
        sig.ret.clone(),
        Kind::Call {
            func: entry.clone(),
            arg,
        },
    ))
}

/// Prefixes an error found inside a routed module's definition with the module's path: the
/// span is a byte offset into that module's file, and without the name the offset would be read
/// against the program.
pub(super) fn in_file(e: Error, origin: &Origin) -> Error {
    match origin {
        Origin::Module(path) => {
            Error::new(e.span, format!("in module `{}`: {}", path.display(), e.msg))
        }
        Origin::Program | Origin::Prelude => e,
    }
}
