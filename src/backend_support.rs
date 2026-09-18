//! Which backends can emit the builtins that are still landing one backend at a time.
//!
//! `sort_by`, `max_by`, `transpose`, and `pipe_through` each landed on one or two backends
//! first, with the rest to follow as their own board rows. Until this table existed, the
//! other emitters carried an `unreachable!("not yet implemented for this backend")` arm for
//! them, so a program using one on the wrong backend did not get a refusal, it got a
//! compiler panic -- and the reference pages could not say truthfully what happens. The
//! refusal now happens here, before any emitter runs, in one place that a landing row
//! updates when it adds an arm; the tests in `tests/unbuilt_arms.rs` hold this table to what
//! the emitters actually do in both directions.

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
        built_on: &[Backend::Go, Backend::Rust],
    },
    Landing {
        name: "max_by",
        built_on: &[Backend::Go, Backend::Rust],
    },
    Landing {
        name: "transpose",
        built_on: &[Backend::Go, Backend::Rust],
    },
    Landing {
        name: "pipe_through",
        built_on: &[Backend::Rust],
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
