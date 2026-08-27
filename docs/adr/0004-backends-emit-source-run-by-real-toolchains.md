---
status: accepted
---

# Backends emit source run by real toolchains; the compiler is never a runtime dependency

Recorded 2026-08-27, after the fact, from draft.md's streaming sections and the research log.

Every backend emits source text (or LLVM IR) that the target's own real toolchain runs -- `node`,
`python3`, `jq`, `go`, `rustc`, the LLVM pipeline -- as a separate OS process with stdio
connected through, `Stdio::inherit()` for live runs. The toylang compiler produces the artifact
and is never a dependency of running it. The one exception is deliberate and named: Lua runs
embedded in the harness process via `mlua`, so "reading stdin" there is a function call, not
IPC, and fixtures reach it by a different mechanism (a file, with the global `io.lines`
repointed at it).

Two reasons, both observed doing work rather than assumed:

- The emitted program has to be the thing tested. Corpus fixtures are piped in verbatim so
  each backend does its own real line splitting, "which is the only way the corpus can prove
  all backends' splitting genuinely agrees rather than testing a Rust-side reimplementation
  instead" (draft.md). Go's stdlib scanner disagreeing about `\r` was found exactly this way
  ([a sixth instance of the backend having rules the checker does not](../../research-log/a-sixth-backend-rule-the-checker-did-not-know.md)).
- Cross-process pipelining comes free. Not pre-reading stdin into the harness before a
  subprocess runs gives `grep foo | wc -l`-style overlap from the kernel, an architectural
  fact rather than a language feature -- and it is what later let streaming programs go live
  without the harness in the data path.

The alternative a reader might assume -- one interpreter or VM with backends as libraries --
would have made every "does the target really behave this way" question unanswerable by
construction, which is the point of having the backends at all
([backends as falsifiers](0002-backends-as-falsifiers.md)).
