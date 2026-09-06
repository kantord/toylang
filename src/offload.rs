//! The `--explain-offload` diagnostic: which sub-expressions became vectorized kernels and which
//! fell back to per-element processing, and why.
//!
//! The predicate is cardinality, the same one the design's kernel-admissibility mapping draws
//! (draft.md, "Cardinality is the kernel-admissibility predicate"): a `Vec` has known extent, so
//! a `map`/`select` over one compiles to a loop that can be vectorized; a `Stream` is unbounded
//! and arrives one entry at a time, so its per-element stages are never kernels. The one other
//! compile-time decision worth reporting is `tir::fusion`: whether the program's `jsonlines` sink
//! ran as a fused read-one/transform-one/write-one loop or eagerly over an already-materialized
//! value.

use crate::tir::{self, Kind, Program, Source, Tir};
use crate::ty::Type;

/// The offload report, one line per decision, ending with a newline. The fusion decision first,
/// then one line per `map`/`select` node in source order.
pub fn explain(program: &Program) -> String {
    let mut out = String::new();
    fusion_line(program, &mut out);
    let mut report = |t: &Tir| match &t.kind {
        Kind::Map { source, .. } => kernel_line("map", &source.ty, &mut out),
        Kind::Select { source, .. } => kernel_line("select", &source.ty, &mut out),
        _ => {}
    };
    for f in &program.funcs {
        tir::each_node(&f.body, &mut report);
    }
    tir::each_node(&program.body, &mut report);
    out
}

/// The program-level decision `tir::fusion` makes, reported as the first line: whether the sink
/// ran as a fused loop, and when it did not, what the argument's type says instead.
fn fusion_line(program: &Program, out: &mut String) {
    if let Some(fusion) = tir::fusion(program) {
        let source = match fusion.source {
            Source::Inputs => "`inputs`",
            Source::Lines => "`lines`",
            Source::Range(_) => "`range`",
        };
        out.push_str(&format!(
            "the `jsonlines` pipeline fused into a read-one/transform-one/write-one loop over {source}\n"
        ));
    } else {
        // `jsonlines` takes a Vec or a stream (checked); a stream argument always fuses above, so
        // what is left here is the materialized case.
        let Kind::Builtin {
            which: tir::Builtin::JsonLines,
            arg,
        } = &program.body.kind
        else {
            out.push_str("no `jsonlines` sink: nothing to fuse\n");
            return;
        };
        let Type::Vec(elem) = &arg.ty else {
            unreachable!("a stream-typed `jsonlines` argument always fuses")
        };
        out.push_str(&format!(
            "`jsonlines` ran eagerly over a materialized Vec<{elem}>: known extent, so the sink had nothing to stream\n"
        ));
    }
}

/// One per-element operation's verdict, per its source type's cardinality: over a `Vec` the
/// operation is a kernel over an already-materialized dimension; over a `Stream` it is a
/// per-element stage, processed one entry at a time as it arrives, which is the one shape the
/// kernel thesis excludes.
fn kernel_line(name: &str, source: &Type, out: &mut String) {
    match source {
        Type::Vec(elem) => {
            let kernel = if name == "map" {
                format!("an elementwise map kernel (One<{elem}>: exactly one output per input")
            } else {
                format!("a compaction kernel (Opt<{elem}>: zero or one kept per entry")
            };
            out.push_str(&format!(
                "{name} over Vec<{elem}>: became {kernel}, known extent, vectorizable)\n"
            ));
        }
        Type::Stream(elem) => {
            out.push_str(&format!(
                "{name} over Stream<{elem}>: fell back to per-element streaming (Stream's extent is unbounded and unknown, so no vectorizable kernel is possible)\n"
            ));
        }
        other => unreachable!("a `map`/`select` source is a Vec or a Stream by the checker, found {other}"),
    }
}
