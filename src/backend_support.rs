//! Which backends can emit the builtins that are still landing one backend at a time.
//!
//! `sort_by`, `max_by`, `transpose`, `pipe_through`, `sqrt`, and `float` each landed on some
//! backends first, with the rest to follow as their own board rows. Until this table existed,
//! the other emitters carried an `unreachable!("not yet implemented for this backend")` arm
//! for them, so a program using one on the wrong backend did not get a refusal, it got a
//! compiler panic -- and the reference pages could not say truthfully what happens. The
//! refusal now happens here, before any emitter runs, in one place that a landing row updates
//! when it adds an arm; the tests in `tests/unbuilt_arms.rs` hold this table to what the
//! emitters actually do in both directions. `sqrt` and `float` now run on every backend;
//! `pipe_through` is the one still short a landing (Jq and native).

use crate::Backend;
use crate::tir::{self, Builtin, Kind, Program, Tir};

/// A builtin partway through its per-backend landing.
pub struct Landing {
    pub name: &'static str,
    pub built_on: &'static [Backend],
}

pub const LANDINGS: &[Landing] = &[
    Landing {
        name: "sort_by",
        built_on: &[
            Backend::Go,
            Backend::Rust,
            Backend::Lua,
            Backend::Js,
            Backend::Py,
            Backend::Jq,
        ],
    },
    Landing {
        name: "max_by",
        built_on: &[
            Backend::Go,
            Backend::Rust,
            Backend::Lua,
            Backend::Js,
            Backend::Py,
            Backend::Jq,
        ],
    },
    Landing {
        name: "transpose",
        built_on: &[
            Backend::Go,
            Backend::Rust,
            Backend::Js,
            Backend::Py,
            Backend::Lua,
            Backend::Jq,
            Backend::Native,
        ],
    },
    Landing {
        name: "pipe_through",
        built_on: &[
            Backend::Go,
            Backend::Rust,
            Backend::Py,
            Backend::Js,
            Backend::Lua,
        ],
    },
    Landing {
        name: "sqrt",
        built_on: &[
            Backend::Go,
            Backend::Rust,
            Backend::Py,
            Backend::Js,
            Backend::Lua,
            Backend::Jq,
            Backend::Native,
        ],
    },
    Landing {
        name: "float",
        built_on: &[
            Backend::Go,
            Backend::Rust,
            Backend::Py,
            Backend::Js,
            Backend::Lua,
            Backend::Jq,
            Backend::Native,
        ],
    },
    // Closures (closures-first-class-functions-design, 2026-09-23): landed on Rust as
    // `Rc<dyn Fn>` and on Go as a bare func type. The other five backends have no
    // representation for a stored closure value yet -- unlike `sqrt`/`float`, this is not
    // "the same arm, five more times," since a dynamically-typed target's story for "call
    // whatever function this value happens to be" differs entirely from a statically-typed
    // one's, and jq has no function values at all.
    Landing {
        name: "closure",
        built_on: &[Backend::Rust, Backend::Go],
    },
];

fn landing_named(name: &str) -> &'static Landing {
    LANDINGS
        .iter()
        .find(|l| l.name == name)
        .expect("every name looked up here is in LANDINGS")
}

fn landing_of(t: &Tir) -> Option<&'static Landing> {
    match &t.kind {
        Kind::SortBy { .. } => Some(landing_named("sort_by")),
        Kind::MaxBy { .. } => Some(landing_named("max_by")),
        Kind::Builtin {
            which: Builtin::Transpose,
            ..
        } => Some(landing_named("transpose")),
        Kind::Builtin {
            which: Builtin::PipeThrough,
            ..
        } => Some(landing_named("pipe_through")),
        Kind::Builtin {
            which: Builtin::Sqrt,
            ..
        } => Some(landing_named("sqrt")),
        Kind::Builtin {
            which: Builtin::FloatOf,
            ..
        } => Some(landing_named("float")),
        Kind::Closure { .. } | Kind::ApplyClosure { .. } => Some(landing_named("closure")),
        _ => None,
    }
}

/// Refuses `program` for `backend` if it uses a builtin that backend has no arm for yet,
/// naming the builtin and the backends that do run it. `Ok` means every emitter arm the
/// program will reach exists.
pub fn refuse_unbuilt(backend: Backend, program: &Program) -> Result<(), String> {
    let mut missing: Option<&Landing> = None;
    let mut visit = |t: &Tir| {
        if missing.is_none()
            && let Some(l) = landing_of(t)
            && !l.built_on.contains(&backend)
        {
            missing = Some(l);
        }
    };
    for f in &program.funcs {
        tir::each_node(&f.body, &mut visit);
    }
    tir::each_node(&program.body, &mut visit);
    match missing {
        None => Ok(()),
        Some(l) => {
            let runs_on: Vec<&str> = l.built_on.iter().map(|b| b.name()).collect();
            Err(format!(
                "`{}` has no {} backend yet; today it runs on {}",
                l.name,
                backend.name(),
                runs_on.join(" and ")
            ))
        }
    }
}
