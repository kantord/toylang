---
status: accepted
---

# LLVM via inkwell for the native backend

Recorded 2026-08-27, after the fact, from draft.md's Q15 entry and the build as it exists.

The native backend lowers through LLVM using the `inkwell` bindings (Cargo.toml pins
`llvm22-1`). Cranelift was the considered alternative and lost on the ground that matters
here: it never auto-vectorizes, and the design's whole performance argument rests on handing a
vectorizer loops it likes. It also emits nothing for the web, so a WebAssembly target would
need a second unrelated backend anyway. Cranelift wins decisively on build simplicity and
compile speed, which is why the rustc-style answer -- Cranelift for debug builds, LLVM for
release -- was recorded as a real option and remains open rather than rejected; what is
decided is that LLVM is the release path and the one that exists.

The lock-in is the usual LLVM kind: a heavyweight build dependency pinned to an LLVM major
version, paid for with access to its optimizer. draft.md's Q15 entry records the reasoning;
its status line records the outcome ("built and running").
