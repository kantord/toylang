# Native runtime in Rust: what comparable compilers do, and a migration plan

Research note for the board row `native-runtime-direction-decide` (2026-09-24). It builds on
[native-backend-rust-ergonomics-research.md](native-backend-rust-ergonomics-research.md), which
already established that the LLVM binding is inkwell and that the open question is the runtime:
its options were A (keep `runtime/toylang.c`), B (a Rust runtime crate built as a static
archive, linked by the existing `cc` call) and C (B plus one table that generates both the
Rust externs and the LLVM declarations). That note is not repeated here.

Conventions: every claim carries a source. "Measured" means I ran it in this session (scratch
work under the session scratchpad, nothing committed; commands are in section 10). "Unverified"
means I could not confirm it from a primary source in this pass. Line counts and "lines that
disappear" in section 4 are estimates: no port was written.

## 0. Summary and recommendation

**Choose B, then C as a follow-up.** Replace `runtime/toylang.c` with a Rust runtime crate built
as a `staticlib`, embedded in the compiler, and linked by the existing `cc` call. Keep the
exported C ABI (`tl_*` symbols, `tl_str`, `tl_vec` with its columns) exactly as it is, so
`src/emit_llvm.rs` does not change during the port. Use `std` for the first version. Add the
single declaration table (option C) after the port, as a `macro_rules!` table.

How strongly the evidence favours each option:

- **B over A: strong.** About 59% of the C (an estimate, table in section 4) disappears into
  `std` and three small crates, and the part that goes is the risky part: a hand-written JSON
  parser (about 625 lines), a hand-written Float printer (about 100 lines) and `qsort` comparators.
  Two memory-safety and parity defects were found in the C during this research (section 8), one
  confirmed with AddressSanitizer. The Float printer replacement was checked byte for byte against
  the C over 3,000,016 doubles and found identical and about 40 times faster. The cost is
  measurable and bounded: one nested cargo build (3 to 5 seconds clean), a 4.5 to 8.3 MB archive
  inside the compiler binary, and emitted programs that grow from 47 KB to 88 KB (`no_std`) or
  361 KB (`std`) stripped.
- **C over B: modest.** The three-way sync problem is real (58 `add_function` sites today), but
  Inko, the closest comparable project, lives with a hand-written enum and no sync check
  (section 2). Do it after the port, when the signatures live in Rust in one place.
- **Not recommended now: bitcode linking.** It works with the pinned toolchain (section 3.1) and
  would allow cross-module inlining, but it couples the runtime build to the LLVM version that
  inkwell links. The staticlib route has no such coupling.

The `std` versus `no_std` choice is reversible: both export the same C ABI, and both were built
and linked here (section 3.2).

## 1. Ground truth about toylang today

All measured or read locally.

- `runtime/toylang.c` is 1908 lines (the earlier note says 1602), with 72 non-static functions
  and 58 `module.add_function` sites in `src/emit_llvm.rs`.
- **The runtime is compiled without optimization.** `src/lib.rs` (`link`, around line 427) runs
  `cc program.o toylang.c -o out` with no `-O` flag, so today's native runtime is a `-O0` build.
- `Cargo.toml` pins `inkwell = { version = "0.10.0", features = ["llvm22-1"] }`. The installed
  rustc is 1.98.1 and bundles LLVM 22.1.8 (`rustc -vV`); `llvm-config --version` is 22.1.8. They
  match today. There is no `rust-toolchain` file, so nothing pins the rustc a contributor uses.
- The native backend targets the host only: `TargetMachine::get_default_triple()`
  (`src/emit_llvm.rs:3313`) and the host `cc`. There is no cross-compilation path to regress.
- `build.rs` exists but only regenerates the prelude; there is no `cc` crate and no nested build.
- Neither arithmetic nor value printing is in the C. 32-bit wrapping, division checks and the
  composition of printed values are emitted as IR by `emit_llvm.rs`; the C supplies primitives
  (`tl_int_to_str`, `tl_float_to_str`, `tl_quote`, `tl_str_join`, `tl_concat`, `tl_print`).
- Every value crosses the ABI as one 8-byte slot. `tl_str` is `{ const char *ptr; int64_t len }`
  and `tl_vec` is `{ int64_t len; int64_t ncols; int64_t **cols }`, struct of arrays, one column
  per record field (`runtime/toylang.c:37-59`, `203-253`). This layout is baked into the IR and
  is a fixed constraint: no crate can supply it, and the Rust port must reproduce it with
  `#[repr(C)]` structs.
- The memory posture is "nothing frees" (`runtime/toylang.c:7-10`). Temporary buffers are freed
  (10 `free` calls), values are leaked.
- libc functions used (counted with grep): `memcpy` 36, `snprintf` 13, `free` 10, `memcmp` 9,
  `write` 8, `exit` 7, `strlen` 5, `getline` 4, `qsort` 3, `read` 3, `pipe2` 3, `strtod` 2,
  `strcpy` 2, `fcntl` 2, `signal` 2, and one each of `malloc`, `realloc`, `memset`, `memchr`,
  `poll`, `posix_spawnp`, `waitpid`, `strtoll`, `strerror`, `isnan`, `isinf`, `fprintf`, `fputs`.
- The Rust source backend is separate: `link_rust` runs one `rustc` call on one self-contained
  file and its doc comment states the "nothing to install" reason (`src/lib.rs`, above
  `link_rust`). `emit_rs.rs` already carries its own Float printer built on Rust's `Display`.

## 2. What comparable projects do

| Project | Compiler | Runtime language | How the runtime reaches the program | Declarations in sync |
|---|---|---|---|---|
| Inko | Rust, LLVM | Rust | Static archive `libinko.a`, linked with `cc`; per-target archives for cross builds | Hand-written Rust enum, no check |
| Roc (Rust era) | Rust, LLVM | Zig builtins | Compiled to LLVM bitcode (`builtins-host.bc`); consumption by inkwell not confirmed | Not confirmed |
| Pony | C/C++, LLVM | libponyrt | `libponyrt.bc` merged into the program module (default on clang builds), else `libponyrt.a` | Not confirmed |
| Swift | C++, LLVM | C++ | Runtime library; IR declarations come from one x-macro file | Single source: `RuntimeFunctions.def` |
| Koka | Haskell, emits C | C (`kklib`) | Compiled with the generated C | n/a (C to C) |
| Nim | Nim, emits C | Nim `system` module | Compiled together with the program's C output | n/a |
| Crystal | Crystal, LLVM | Crystal stdlib plus C libraries | Links libgc, pcre2 and others | Not confirmed |
| OCaml | OCaml | C and per-architecture assembly | `runtime/` directory, C plus `.S` files | Not confirmed |
| Zig | Zig | Zig (`lib/compiler_rt`) | Not confirmed beyond the directory | Not confirmed |
| Julia | C++, LLVM | C and C++ (libjulia) | Intrinsics lowered to libjulia calls | Hand-written `JuliaFunction` table in `codegen.cpp` (from a search summary) |
| rustc `compiler-builtins` | Rust | Rust, a port of LLVM compiler-rt | Distributed as part of the sysroot | n/a |
| Gleam | Rust | none | Targets Erlang and JavaScript only | not comparable |
| Mun | Rust | not confirmed | not confirmed | not confirmed |

Notes, each tied to what was actually read:

- **Inko is the closest match** (Rust compiler, LLVM, Rust runtime). Its `rt` crate has
  `crate-type` of static library plus Rust library, and depends on `libc`, `backtrace`,
  `unicode-segmentation`, `rustls` and others, so it is a `std` runtime
  ([rt/Cargo.toml](https://raw.githubusercontent.com/inko-lang/inko/main/rt/Cargo.toml)). The
  linker passes `libinko.a` from the configured runtime directory, and for cross-compilation looks
  in a `runtimes/<target>` directory and fails with "no runtime is available for target"
  ([compiler/src/linker.rs](https://raw.githubusercontent.com/inko-lang/inko/main/compiler/src/linker.rs)).
  It needs `cc` or `clang` to link ([installation](https://docs.inko-lang.org/manual/latest/setup/installation/)).
  Declarations are a hand-written `RuntimeFunction` enum with an LLVM signature per variant and, as
  read, no generation or check against the crate
  ([runtime_function.rs](https://raw.githubusercontent.com/inko-lang/inko/main/compiler/src/llvm/runtime_function.rs)).
  On memory, its manual says it moved to the system allocator after abandoning an Immix-based
  design because the implementation "was quite complex and difficult to debug"
  ([runtime design](https://docs.inko-lang.org/manual/latest/design/runtime/)).
- **Roc** shows the bitcode route and its cost. Its Rust-era builtins were Zig compiled by
  `zig build-obj ... -OReleaseFast` to `builtins-host.bc`
  ([roc#6514](https://github.com/roc-lang/roc/issues/6514),
  [zig#16076](https://github.com/ziglang/zig/issues/16076)). Roc has since rewritten its compiler
  from Rust to Zig; the write-up cites compile times, and separately describes emitting LLVM
  bitcode directly because "LLVM has strong backwards-compatibility on its bitcode but not on its
  public-facing API"
  ([gist](https://gist.github.com/rtfeldman/77fb430ee57b42f5f2ca973a3992532f.pibb)). That is
  evidence that LLVM API churn is a real maintenance cost for Rust LLVM bindings, and it applies
  to inkwell whichever runtime option is chosen.
- **Pony** is the strongest evidence for bitcode: `libponyrt.bc` is merged into the program module
  with `LLVMLinkModules2` by default on clang builds, and the linker falls back to `libponyrt.a`
  when the bitcode is absent ([ponyc#6137](https://github.com/ponylang/ponyc/pull/6137)). The same
  thread describes inlining and allocation-promotion passes tuned for the merged runtime. The
  runtime's implementation language was not confirmed from the directory listing (unverified).
- **Swift** keeps the runtime function list in one x-macro file. Its header says the file "defines
  x-macros used for metaprogramming with the set of runtime functions", and each entry carries the
  symbol, calling convention, return and argument types and attributes
  ([RuntimeFunctions.def](https://raw.githubusercontent.com/swiftlang/swift/main/include/swift/Runtime/RuntimeFunctions.def)).
  This is the precedent for option C.
- **Koka and Nim** emit C, so their runtime is compiled with the program and there is no
  signature-sync problem: `kklib` is a C library with a `mimalloc` submodule
  ([kklib](https://github.com/koka-lang/koka/tree/master/kklib)), and Nim's `nimc` doc says the
  runtime (system module and memory manager, default `orc`) is compiled with the program
  ([nimc](https://nim-lang.org/docs/nimc.html)). Not directly transferable to an IR emitter.
- **Crystal** links Boehm GC and several C libraries alongside a Crystal-written standard library
  ([required libraries](https://github.com/crystal-lang/crystal/wiki/All-required-libraries),
  [src/](https://github.com/crystal-lang/crystal/tree/master/src)).
- **OCaml** keeps a C runtime with per-architecture assembly
  ([runtime/](https://github.com/ocaml/ocaml/tree/trunk/runtime)).
- **compiler-builtins** is "largely a port of LLVM's compiler-rt", distributed with the sysroot and
  licensed MIT and Apache-2.0 with the LLVM exception
  ([README](https://raw.githubusercontent.com/rust-lang/compiler-builtins/master/compiler-builtins/README.md)).
  Its relevance is that no_std Rust replacing C runtime code is an established practice; it says
  nothing about how a user compiler should ship one.
- **Zig** has `lib/compiler_rt/*.zig` ([tree](https://github.com/ziglang/zig/tree/master/lib/compiler_rt));
  how it is built and linked was not confirmed (unverified).
- **Julia** documents that intrinsics are lowered to calls into libjulia
  ([devdocs/llvm](https://docs.julialang.org/en/v1/devdocs/llvm/)); the `JuliaFunction` table
  detail comes from a search summary of `codegen.cpp` and is unverified.
- **Gleam** compiles to Erlang and JavaScript only ([gleam.run](https://gleam.run/)), so it is not
  a comparison. **Mun**'s runtime story could not be confirmed from its README (unverified).

What the survey says about the decision:

1. Projects with a Rust compiler and an LLVM backend that wrote their own runtime picked Rust
   (Inko) or accepted a second language (Roc, Zig), and none of them were found regretting it in
   the sources read. No source read states a regret about the runtime language either way.
2. The linkage forms in use are a static archive (Inko, Pony's fallback) and merged bitcode (Pony
   default, Roc). Inko needs per-target archives for cross builds.
3. Only Swift is confirmed to generate declarations from one source. Inko does not, and ships.

## 3. Mechanics for toylang

### 3.1 How the runtime gets built and reaches the linker

| Mechanism | Status | Measured or sourced |
|---|---|---|
| Workspace crate, `crate-type = ["staticlib"]`, built by a nested `cargo build` in `build.rs`, embedded with `include_bytes!`, written to the temp dir at link time like `toylang.c` is today | Works on stable | Clean build 2.2 s (no deps), 4.8 s (with `ryu-js` and `serde_json`), 2.8 s (`no_std`); archive 8.3 to 8.5 MB (`std`), 4.5 to 4.6 MB (`no_std`) |
| Artifact dependencies (`-Z bindeps`) | Nightly only | The cargo book states "Still unstable/nightly-only" ([unstable](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html)); reject |
| `rustc --emit=llvm-bc` with fat LTO, embedded and merged with inkwell `link_in_module` | Works with today's pinned versions | Measured: `cargo rustc -- --emit=llvm-bc -C lto=fat` on a `no_std` crate gives one 373 KB module whose only externals are `malloc`, `calloc`, `realloc`, `free`, `abort`, `memcmp` and LLVM intrinsics, exporting only the 5 `tl_*` symbols. A scratch program using the pinned `inkwell 0.10.0 / llvm22-1` parsed it, linked it into a program module, emitted an object, and `cc prog.o -o prog` produced a binary that ran (exit 5 as expected), with no runtime archive |
| The `cc` crate compiling C | Works | Keeps the C; not a way off it |

Cargo does not build the `staticlib` of a plain dependency, only its rlib, which is why a nested
cargo call (or bindeps) is needed for the archive route.

**Why not bitcode now.** LLVM's policy is one-directional: "The current LLVM version supports
loading any bitcode since version 3.0", and no forward compatibility is promised
([DeveloperPolicy](https://llvm.org/docs/DeveloperPolicy.html)). The rustc book's advice for
cross-language LTO is an LLVM at least as new as the newest compiler involved, ideally the exact
same version ([linker-plugin-lto](https://doc.rust-lang.org/rustc/linker-plugin-lto.html)).
Here rustc 1.98.1 (LLVM 22.1.8) and inkwell `llvm22-1` (system LLVM 22.1.8) agree, but a
contributor with a newer rustc than system LLVM would produce bitcode the compiler cannot read,
and the failure would appear at build time of the compiler. The staticlib route has no LLVM
coupling: `cc` links machine code. The inlining benefit is unmeasured for toylang (it needs a
benchmark against `benches/programs`), so it is a later optimization, not a reason to start there.

**Linking a std staticlib.** The Rust reference says a `staticlib` "contains ... all upstream
dependencies", including the standard library, exports all public symbols, and that "unused
sections can be removed ... (`--gc-sections`)" and that multiple Rust staticlibs "are likely to
conflict" ([linkage](https://doc.rust-lang.org/reference/linkage.html)). Consequences here:

- Link with `-Wl,--gc-sections`; it took the `std` case from 8.3 MB archive to a 2.26 MB linked
  binary (361 KB stripped).
- Exactly one Rust staticlib may be linked into a program. That rules out sharing the runtime with
  a second Rust staticlib and is one reason section 3.5 recommends against sharing with `emit_rs.rs`.
- The link line needs what `--print native-static-libs` reports: measured `-lgcc_s -lutil -lrt
  -lpthread -lm -ldl -lc` for `std`, and an empty list for the `no_std` build (only libc and libm).
- Step 1 re-measured the link line on this host (glibc 2.44, GNU ld 2.47, cc 16.2.1) with a
  `std` archive that spawns a process, runs a thread, reads stdin and formats: it links with no
  extra libraries at all, dynamic or `-static`, and `--gc-sections` saved only about 1 KB of a
  2.28 MB unstripped binary (the archive's std is already thin after lto). `-lm` adds a `libm.so.6`
  dependency nothing uses. The committed link line still passes the rustc-reported libraries,
  because glibc older than 2.34 keeps pthread and dl in separate libraries; that case is unmeasured.
- Both variants also link fully static with `cc -static` against glibc (measured: 3.33 MB `std`,
  0.87 MB `no_std`). A musl target needs the `x86_64-unknown-linux-musl` std component, which is
  installed here; other targets need their own archive, as Inko does. Not tested beyond the host.

### 3.2 `std` or `no_std`, panic strategy, allocator

Measured, all with `panic = "abort"`, `lto = true`, `codegen-units = 1`, `opt-level = 2`, on
stable Rust 1.98.1, linked with `cc -Wl,--gc-sections` against a minimal `main`, then stripped:

| Runtime | Linked binary | Stripped |
|---|---|---|
| C `toylang.c`, `cc -O0` (as `link` does today) | 53,848 | 47,536 |
| C `toylang.c`, `cc -O2` | 53,424 | 47,520 |
| Rust `std`, stable sort + utf8 + `process::Command` + `ryu-js` + `serde_json` (`float_roundtrip`) | 2,258,864 | 361,336 |
| Rust `std`, `opt-level = "z"` | 2,018,320 | 377,736 |
| Rust `no_std + alloc`, base (stable sort, utf8, Vec) | | 18,616 |
| ... plus `ryu-js` | | 35,000 |
| ... plus `serde_json` (`alloc`, `float_roundtrip`) | | 71,960 |
| ... plus both | 106,528 | 88,336 |
| bitcode merged via inkwell, no archive (`no_std`) | 105,968 | 88,184 |

These use a tiny `main`, so a real program links more of the runtime; treat them as a floor. The
`std` floor is dominated by the standard library itself (the dependencies add nothing measurable
there because `--gc-sections` drops what is unused).

- **`std` costs about 314 KB over C, `no_std` about 40 KB.** If emitted programs must stay
  small, `no_std + alloc` is viable and works on stable: a `#[global_allocator]` that forwards to
  libc `malloc`/`realloc`/`free`, a `#[panic_handler]` that calls `abort`, and `panic = "abort"`
  in the profile. It cannot use `std::process` or `std::io`, so `pipe_through` and stdin reading
  would be ports of the existing C onto the `libc` crate (about the same line count as the C).
- **Recommendation: start with `std`.** It is what Inko ships (section 2), it makes the pipe and
  stdin code shorter (about 45 lines instead of about 130), and the ABI is identical, so moving
  to `no_std` later is a contained change if binary size becomes a requirement.
- Allocator: keep libc `malloc` (the `std` default on Linux). `bumpalo` (MIT OR Apache-2.0,
  `no_std`, [crates.io](https://crates.io/api/v1/crates/bumpalo)) would fit the "nothing frees"
  posture, but that posture belongs to the mutation-model work (`runtime/toylang.c:7-10`), so
  the port should leak with `Vec::leak` / `mem::forget` and change nothing about it.
- Out-of-memory: C prints `toylang: out of memory` and exits 1. Rust's default allocation failure
  aborts. Reproducing the message needs an explicit `try_reserve` wrapper or accepting a change
  (a risk, section 7).

### 3.3 Crates for what the C hand-writes

See the table in section 4 for the per-responsibility mapping. The decisions that needed
evidence:

- **Float to string: `ryu-js`.** It is "a fork of the ryu crate adjusted to comply to the
  ECMAScript number-to-string algorithm", `#![no_std]`, categories include `no-std::no-alloc`,
  licence `Apache-2.0 OR BSL-1.0`, current version 1.0.3 updated 2026-07-10
  ([repo](https://github.com/boa-dev/ryu-js),
  [crates.io](https://crates.io/api/v1/crates/ryu-js)). The C printer is `tl_shortest_digits`
  plus `tl_float_to_str`, an ECMA-262 layout over a `snprintf("%.*e")` / `strtod` retry loop
  (`runtime/toylang.c:70-172`). Measured: 3,000,016 doubles (random bit patterns, decimals such
  as `m * 10^e`, dyadic fractions, plus NaN, -0, both infinities, `1e21`, `1e-7`, `1e-6`,
  `123456789012345680000`, `5e-324`, the largest and smallest normal values) fed to the C
  `tl_float_to_str` (the real file, `#include`d) and to `ryu_js::Buffer::format`: output
  **identical for every input**, for both ryu-js 0.2.2 and 1.0.3. (Repeated at step 2 on a different 3,403,219-double sample, this did not hold: the C printed 17 digits
  where Node and ryu-js print 16 on 449 inputs. See the board row `lua-float-print-not-shortest`.) Speed: 12.4 s versus 0.30 s
  including I/O, about 40 times faster. Rust's own `Display` is not a replacement on its own:
  `emit_rs.rs:74` notes it is shortest-round-trip but holds fixed notation across the range,
  which is why that backend has a wrapper.
- **Float from string: `str::parse::<f64>`.** Measured against libc `strtod` on 1,000,000
  random decimals (1 to 25 significant digits, optional exponent): identical bit patterns for
  all. It accepts more than JSON does (`+1`, `1.`, `.5`, `inf`, `nan`), so the grammar check must
  stay ahead of it, exactly as the C scans a NUL-terminated buffer before `strtod`.
- **JSON: `serde_json`, but only with `float_roundtrip`.** Measured on 919,210 of those strings
  that are valid JSON numbers: `serde_json` 1.0.151 with default features returned a different
  value from the correctly-rounded parse for **136,225 (14.8%)**, for example
  `8525388853933633.0` and `91186252760.18955`; with the `float_roundtrip` feature (present in
  its manifest) the count was **0**. The sample is weighted toward long digit strings, so 14.8% is
  not a rate for typical input, but any Float path through `serde_json` must enable the feature.
  `serde_json` 1.0.151 is `MIT OR Apache-2.0`, supports `alloc` without `std`, and depends on
  `itoa`, `memchr`, `serde_core` and `zmij` (read from the crate's `Cargo.toml` in the local
  registry). Toylang's `Cargo.lock` already resolves it.
- **Sorting: `slice::sort_by_key` (stable, needs `alloc`).** The std docs say the stable sorts
  require `alloc` while `sort_unstable*` are in `core`
  ([slice](https://doc.rust-lang.org/std/primitive.slice.html)). This deletes `qsort`, the
  `const void *` comparators and the `(key, row)` tie-break struct: sorting an index vector by
  key is stable by construction, and the column permute stays as it is.
- **String search and split: `str::split`.** For a non-empty delimiter it gives every field
  including empty and trailing ones, the shape `tl_split` documents. The empty-delimiter case
  differs (`"rust".split("")` yields `["", "r", "u", "s", "t", ""]`, per the std docs
  ([str](https://doc.rust-lang.org/std/primitive.str.html))) but is unreachable: measured, the
  checker rejects `dsv("")` on every backend ("`dsv`'s delimiter cannot be empty"). The C loop
  would never advance on an empty delimiter, so this is a latent hazard the port removes.
- **UTF-8: `core::str::from_utf8`** (in `core`) and `str::chars()`. `simdutf8` (MIT OR
  Apache-2.0, `no_std`, 0.1.5, last updated 2024-09-22,
  [crates.io](https://crates.io/api/v1/crates/simdutf8)) is faster on long inputs but no measurement
  here shows UTF-8 validation matters; not needed.
- **Subprocess: `std::process::Command`.** `wait_with_output` closes the child's stdin before
  waiting and reads stdout and stderr together
  ([Child](https://doc.rust-lang.org/std/process/struct.Child.html)), but it does not feed stdin
  concurrently, so `pipe_through` needs a writer thread (measured: a 12-line version worked in the
  staticlib). The C poll loop (165 lines) becomes about 45. The existing
  `tests/pipe_through.rs` covers the deadlock, early-exit and large-output cases.
- **String quoting must stay hand-written.** Measured: `serde_json::to_string` differs from
  `tl_quote` on exactly two of the 128 ASCII characters: 0x08 and 0x0c, where serde_json writes
  `\b` and `\f` and the C writes `\u0008` and `\u000c`. See section 8: the backends already
  disagree about this, so the port must not pick a side by accident.
- **Integer to string:** `itoa` (`no_std`, MIT OR Apache-2.0) or `core::fmt`. Either works; the
  C is 8 lines and this is not a decision that matters.

### 3.4 One declaration table (option C)

Today a function is declared in three places (section "The cost of one more runtime function" of
the earlier note). With the runtime in Rust there are two: the `extern "C"` definition and the
`add_function` call. A `macro_rules!` table can generate the first and expose the second's data:

```rust
// in a small shared crate, used by the runtime crate and by the compiler
runtime_fns! {
    fn tl_vec_len(v: ptr) -> i64;
    fn tl_vec_sort_by(v: ptr, keys: ptr, is_str: i32) -> ptr;
}
```

The macro expands to `#[unsafe(no_mangle)] pub extern "C" fn` definitions in the runtime, and to a
`const SIGNATURES: &[Sig]` the compiler iterates to call `add_function`. This is the same shape as
Swift's x-macro file (section 2). Not evaluated: a proc macro, or generating the table from a
header with `cbindgen` (unverified). The 58 declarations are a moderate cost, not an emergency:
Inko has a similar hand-written list and no sync check. Do it after the port.

### 3.5 Should the Rust source backend share the runtime?

No. Three reasons, all from the code and sources above:

1. `emit_rs.rs` emits typed Rust (its own Vec and record shapes), not the columnar layout the
   runtime serves, so it would use almost none of it.
2. Its constraint is one `rustc` call on one self-contained file with no external crate
   (`src/lib.rs`, `link_rust` doc), and the Rust reference warns that multiple Rust staticlibs
   conflict; linking the runtime archive would break the first and risk the second.
3. What it could share is small, pure code, and that is source rather than a library: the Float
   printer is the one duplicated item, and a dependency-free program cannot use `ryu-js`.

The `native-backend-rust-ergonomics-research.md` note reached the same view on the first point.

## 4. Inventory of `runtime/toylang.c` by responsibility (widened scope)

Line ranges are from the current file. "Replacement" names the std facility or crate; "Rust
lines" is a rough estimate of what remains as hand-written Rust for that section. "C gone" is
the C in the range minus that estimate, floored at zero. These are estimates, not measurements.

| # | C lines | Responsibility | Replacement | Rust lines | C gone |
|---|---|---|---|---|---|
| 1 | 1-52 | Includes, `tl_str`, `tl_alloc`, OOM message, `tl_str_new` | `#[repr(C)] TlStr`; std allocator; leak with `Vec::leak` | 25 | 27 |
| 2 | 54-68 | `tl_concat`, `tl_int_to_str` | `[a, b].concat()`; `itoa` or `core::fmt` | 8 | 7 |
| 3 | 70-172 | Float printing (`tl_shortest_digits`, `tl_float_to_str`) | `ryu-js` (verified identical) | 10 | 93 |
| 4 | 174-201 | `tl_str_eq`, `tl_str_cmp`, `tl_print` | slice `==` and `cmp` (byte order, same as `memcmp`); `write_all` | 15 | 13 |
| 5 | 203-297 | Columnar Vec core, column, mask, records (`tl_vec_new` ... `tl_rec_set`) | Hand-written; the layout is the ABI | 80 | 15 |
| 6 | 299-345 | `tl_quote`, `tl_str_join` | `quote` stays hand-written (see 3.3); `join` via `[&str]::join` | 35 | 12 |
| 7 | 346-497 | JSON infrastructure: descriptor grammar, `tl_fail`, `tl_list`, whitespace, `tl_type_skip` | `Vec`; descriptor skip stays; `tl_fail` becomes `eprintln` and `exit(1)` | 50 | 100 |
| 8 | 497-972 | JSON parse body: `\uXXXX`, UTF-8 encode, strings, skip, records, enums, vecs, numbers | `serde_json` `Value` plus a descriptor walker keeping the path-specific messages | 120 | 355 |
| 9 | 974-1123 | `tl_read_input`, `tl_parse_str`, `tl_read_inputs`, `tl_read_one_input` | `read_to_end`; `serde_json` `StreamDeserializer` for one value per line | 50 | 100 |
| 10 | 1125-1253 | `tl_utf8_valid`, stdin lines (buffered, streaming), `tl_split` | `from_utf8`; `BufRead::read_until(b'\n')`; `str::split` | 45 | 85 |
| 11 | 1262-1403 | `rec_from_vec`, Opt, tail, first, any, all, flatten, concat, transpose | Slice methods over the columns | 100 | 42 |
| 12 | 1405-1507 | Comparators, `sort`, `sort_by` (with row tie-break), `max_by` | `sort_by_key` on an index vector (stable); `max_by` stays a hand loop with a strict `>`, because `Iterator::max_by_key` returns the LAST of equal maxima (measured: `[(1,"a"),(2,"c"),(2,"b")]` gives `(2, "b")`) and the language rule is first wins | 45 | 58 |
| 13 | 1509-1560 | `reverse`, `sum` (32-bit narrowing), `max` | `slice::reverse`, `wrapping_add` with `as i32 as i64`, `iter().max()` | 25 | 27 |
| 14 | 1557-1690 | `tl_at`, `tl_sel_len`, `tl_sel_at`, `tl_vec_slice`, `tl_unwrap` | Hand-written slice logic | 105 | 29 |
| 15 | 1692-1741 | `tl_range`, `tl_chars` | `0..n`; `str::chars()` | 20 | 30 |
| 16 | 1743-1908 | `pipe_through` (poll loop, posix_spawn, signals) | `std::process::Command` plus a writer thread | 45 | 121 |

Totals: about 1,890 C lines, about 780 lines of Rust, about 1,110 C lines (59%) gone. Of the Rust
that remains, roughly 600 (rows 1, 5, 6, 11, 14) is glue that reproduces the columnar `tl_vec`
and `tl_str` ABI and that no crate can supply. If the process code stays on `no_std` plus the
`libc` crate, row 16 saves about 25 lines instead of 121 and the total drops to about 52%.

Not in the C and not affected: arithmetic, printing composition, comparison of composites (all IR).
The JSON descriptor grammar is a private protocol between `emit_llvm.rs` and the C parser and
survives the port unchanged.

### Crate facts

Licence and MSRV are read from each crate's `Cargo.toml` in the local registry; dates and
download counts from the crates.io API pages cited. Toylang has no `LICENSE` file at the repo
root, so compatibility with the project's own licence cannot be assessed here; all of these are
permissive.

| Crate | Use | `no_std` | Licence | Newest / updated | Footprint |
|---|---|---|---|---|---|
| `std` / `core` / `alloc` | Str, sort, process, io, `f64::parse`, `from_utf8`, `chars` | `core` and `alloc` yes | MIT OR Apache-2.0 | ships with rustc | `std` floor is the 361 KB stripped above |
| `ryu-js` | Float printing | yes (`#![no_std]`, `no-alloc`) | Apache-2.0 OR BSL-1.0 | 1.0.3, 2026-07-10 | +16 KB stripped (`no_std` build); MSRV 1.71 |
| `serde_json` | JSON reading | with `alloc` | MIT OR Apache-2.0 | 1.0.151, 2026-07-20; about 1.3 billion downloads | +53 KB stripped (`no_std` build); needs `float_roundtrip`; MSRV 1.71 |
| `memchr` | Byte search, only needed in `no_std` | yes | Unlicense OR MIT | 2.8.3, 2026-07-08 | Already a dependency of `serde_json` |
| `itoa` | Int printing (optional) | yes | MIT OR Apache-2.0 | 1.0.18 | Tiny; already a dependency of `serde_json` |
| `libc` | Only for a `no_std` port of the pipe code | yes | MIT OR Apache-2.0 | 0.2.189 in the registry | Bindings only |
| `bumpalo` | Optional arena for the leak posture | with `alloc` | MIT OR Apache-2.0 | 3.20.3, 2026-05-22 | Not needed for the port |
| `simdutf8` | Faster UTF-8 validation | yes | MIT OR Apache-2.0 | 0.1.5, 2024-09-22 | Not needed |

## 5. Which option, and by how much (restated)

- A (keep C): supported only if the goal is zero change. It keeps the defects in section 8,
  the `-O0` runtime, the hand parser and the three-way sync, and it grows with every builtin (the
  parity work of the last two days added `tl_vec_sort_by`, `tl_vec_max_by`, closure envs and
  `tl_pipe_through` to it).
- B (Rust staticlib): favoured strongly, for the reasons in section 0.
- C (B plus a table): favoured mildly, as a step after B.

## 6. Migration plan

Every step ends with `just check` green (the corpus requires every backend to agree, and
`native_agrees_where_it_compiles` in `tests/backend_llvm.rs` reports any program native stops
compiling), the C file shrinking, and no change to `src/emit_llvm.rs` until step 9.

0. **Preconditions, no runtime change.** Fix the `strcpy` overflows (section 8), and add a corpus
   case for control characters printed inside a Vec (needs the ruling in section 8 first).
   Record a baseline of `benches/programs` run times on native so a slowdown is visible.
1. **Scaffold.** Add a workspace member (for example `runtime-rs`, package `toylang-rt`,
   `crate-type = ["staticlib"]`, `panic = "abort"`, `lto = true`). Add a `build.rs` step that runs
   a nested `cargo build --release -p toylang-rt` into a separate target directory under
   `OUT_DIR`, and embed the archive with `include_bytes!`. Change `link` in `src/lib.rs` to write
   both `toylang.c` and the archive and run
   `cc program.o toylang.c libtoylang_rt.a -Wl,--gc-sections -lpthread -ldl -lm`. The crate
   exports nothing yet except a smoke function; the test is that the link still works.
   Measured link line needs are in section 3.1.
2. **Float printing.** Port `tl_float_to_str` on `ryu-js`. Delete C lines 70-172. Run the float
   corpus cases and repeat the one-off differential (3,000,016 doubles, section 10) once more.
3. **Str primitives.** `TlStr` as `#[repr(C)]`, then concat, int to string, eq, cmp, print, quote,
   join. C code that still calls these keeps compiling, because C can call Rust-defined `tl_*`
   symbols through the same prototypes it already has. A symbol must be defined in exactly one of
   the two, so each port deletes the C body in the same commit.
4. **Vec, record and Opt core.** Port `tl_vec_*` construction, `tl_rec_*`, masks, `tl_opt_*` with
   `#[repr(C)] TlVec { len, ncols, cols }` and unsafe accessors. Add a layout test on both sides
   (`offsetof` in a C test, `mem::offset_of!` in Rust) while any C code still reads a `tl_vec`.
5. **Vector operations, in groups,** each its own commit: sorts and `sort_by`/`max_by` (stable
   sort; the corpus cases `sort_by_stable`, `max_by_first_of_ties` pin the tie behaviour);
   reverse, flatten, concat, transpose; sum, max, `tl_at`, slices, unwrap, `tl_sel_*`; range and
   `chars`.
6. **Input.** Lines, stdin and split first (small), then the JSON path with `serde_json` and
   `float_roundtrip`, keeping the path-specific error strings (`tl_fail` messages appear in
   tests). For the JSON step, run the existing corpus inputs and the error tests, and add a
   differential run of both parsers over the corpus inputs before deleting the C parser.
7. **`pipe_through`** on `std::process` with a writer thread; `tests/pipe_through.rs` is the net.
8. **Delete `runtime/toylang.c`,** the `RUNTIME_C` constant and the two-file `cc` call; the link
   is `cc program.o libtoylang_rt.a ...`.
9. **Optional follow-ups,** independently: the single declaration table (3.4); measuring bitcode
   merging for cross-module inlining of the tiny accessors (`tl_vec_get`, `tl_vec_set`), which
   the `-O0` C never inlined either; a `no_std` build if program size matters.

## 7. Risks

1. **Nested cargo in `build.rs`.** It doubles some build work (3 to 5 seconds clean, measured) and
   needs the runtime's dependencies resolvable offline where the compiler is built. Mitigation: a
   workspace with one lockfile, `--locked`, and a separate target directory to avoid the build
   lock. Not tested inside the real repository (the experiments ran in a scratch project).
2. **Emitted program size.** 47 KB now, 361 KB with `std` (a floor, measured with a tiny main),
   88 KB with `no_std`. Acceptable for a CLI tool; a real constraint only if programs are
   deployed somewhere size matters. Reversible (section 3.2).
3. **Behaviour parity at the edges.** The corpus pins most of it, but these are not obviously
   pinned: OOM message and exit code (C exits 1 with a message, Rust aborts); SIGPIPE, because
   the generated `main` bypasses Rust's `lang_start` and so its signal setup, and the C code
   currently ignores SIGPIPE only around the spawn; JSON error text at each path; `\b`/`\f`
   escaping (section 8). Each needs a test before its C is deleted.
4. **Float parsing through `serde_json` without `float_roundtrip`** is wrong for real inputs
   (section 3.3). A lint or a test that parses a long-digit Float must guard the feature flag.
5. **Toolchain coupling if bitcode is adopted later** (section 3.1).
6. **New dependencies in every compiled program:** `serde_json` and `ryu-js` are well maintained
   by the dates above, but they are now part of the runtime's supply chain. `ryu-js` is a fork
   of `ryu` maintained under `boa-dev`; its release history was not reviewed in depth.
7. **Two toolchains at compiler build time is not new** (cargo already builds the compiler), but
   `cc` is still required at user compile time to link, as with Inko.
8. **Unmeasured:** runtime performance of the Rust runtime against the `-O0` C on real programs.
   The Float printer alone is about 40 times faster in a harness that includes I/O.

## 8. Findings outside the brief

Found while measuring; reported, not fixed (this note changes no code).

1. **Heap buffer overflow in `tl_float_to_str`** (`runtime/toylang.c:115` and `123`).
   `strcpy(tl_alloc(3), "NaN")` writes 4 bytes into 3, and `strcpy(tl_alloc(1), "0")` writes 2
   bytes into 1 (the terminating NUL). Confirmed with AddressSanitizer on input 0.0:
   "heap-buffer-overflow ... WRITE of size 2 ... 0 bytes after 1-byte region". gcc also warns at
   `-O2` (`-Wstringop-overflow`). It is harmless in practice today because malloc rounds up, but
   it is undefined behaviour in the shipped runtime, and a Rust port removes it. Fixed at step 0
   (`memcpy` of the exact length), with a test that links the runtime under AddressSanitizer
   (`tests/native_asan.rs`).
2. **The backends disagree on how a control character prints inside a Vec or record.**
   Measured with a `Str` containing 0x08 and 0x0c printed as `[s]`: JS and jq print
   `["a\bb\fc"]`; Rust, Go, Python, Lua and native print `["a\u0008b\u000cc"]`. No corpus case
   pins it (`tests/corpus/escapes.yaml` covers `"`, backslash and tab only;
   `control_char_escapes.yaml` checks input decoding, and its expected output prints the raw
   string, not a quoted one). This is a parity gap the differential sweep of the parity goal did
   not cover. It needs a language ruling (JSON's short forms `\b`/`\f`, as JS, jq and
   `serde_json` do, or `\u00xx` for both) and a corpus case. It also decides whether the Rust
   port can use `serde_json` for quoting.
3. **The runtime is built without optimization** (section 1): `cc program.o toylang.c` has no `-O`.

## 9. What would change the recommendation

- A hard requirement that emitted native programs stay under about 100 KB: choose the `no_std`
  variant from day one, and accept porting the process and stdin code onto the `libc` crate.
- A requirement to cross-compile the native backend to several targets: B still works, but the
  archive must be built per target and shipped or located per target (Inko does this); if that
  cost is unacceptable, staying on a C runtime that `cc` compiles per target is the alternative.
- A benchmark showing cross-module inlining of the small accessors is a large win: move step 9's
  bitcode item forward, and pin the toolchain (`rust-toolchain`) and inkwell's LLVM together.
- A decision that emitted programs may depend on a system library or an installed runtime: then a
  shared library or an installed archive replaces `include_bytes!`. Not recommended: it removes
  "nothing to install".
- If the mutation-model work decides on reference counting or tracing soon, port after it, or make
  the allocation a single seam first. The port does not need that decision but must not preempt it.
- Evidence that any listed crate is unmaintained or license-incompatible with the project once its
  licence is chosen.

## 10. How the measurements were made

Scratch only, under the session scratchpad (`rtexp/`), all against the repository's own
`runtime/toylang.c` and its pinned versions; nothing was committed.

- Float: a C harness `#include`s `runtime/toylang.c` and prints `tl_float_to_str` for each
  64-bit pattern read from a file; a Rust binary prints `ryu_js::Buffer::format` for the same
  patterns; `cmp` on the outputs. 3,000,016 inputs. Repeated for ryu-js 0.2.2 (offline) and 1.0.3.
- JSON floats: 1,000,000 generated decimal strings; compared `serde_json::from_str::<f64>`
  (default, then with `float_roundtrip`) to `str::parse::<f64>`, and `strtod` to `str::parse`.
- Staticlib: a scratch crate exposing a sort, a UTF-8 check, `ryu-js`, `serde_json` and a
  `Command`-based pipe, built in release with the profile in section 3.2, linked with a small C
  `main`, sizes taken before and after `strip`. A `no_std + alloc` twin with a libc `malloc`
  global allocator. The bitcode was produced with `cargo rustc --release -- --emit=llvm-bc -C
  lto=fat -C embed-bitcode=yes` and consumed by a scratch program using
  `inkwell = "=0.10.0"` with `llvm22-1`.
- Quote: all 128 ASCII characters through `tl_quote` and `serde_json::to_string`.
- Differential parity: `toylang run FILE <backend>` on each of seven backends for the control
  character program.

## 11. Unverified in this pass

- Roc's consumption of its bitcode through inkwell (only the Zig build outputs were confirmed).
- Pony's runtime implementation language; how Zig builds and links `compiler_rt`; how Crystal
  declares its runtime calls; Mun's runtime; Julia's `JuliaFunction` table beyond a search summary.
- Whether `cbindgen` or a proc macro would be better than `macro_rules!` for section 3.4.
- Performance of the Rust runtime against the C on real programs; the inlining benefit of bitcode.
- That an OOM abort versus exit(1) difference, or SIGPIPE handling under `std` without
  `lang_start`, matters to any existing test.
- The project's own licence (no `LICENSE` file found at the repo root).
- That `str` ordering equals byte order (the C already uses `memcmp` and the corpus pins the
  result across backends; the std doc page fetched did not quote the `Ord` impl).

## Sources

Inko: <https://raw.githubusercontent.com/inko-lang/inko/main/rt/Cargo.toml>,
<https://raw.githubusercontent.com/inko-lang/inko/main/compiler/src/linker.rs>,
<https://raw.githubusercontent.com/inko-lang/inko/main/compiler/src/llvm/runtime_function.rs>,
<https://docs.inko-lang.org/manual/latest/design/runtime/>,
<https://docs.inko-lang.org/manual/latest/setup/installation/>.
Roc: <https://github.com/roc-lang/roc/issues/6514>, <https://github.com/ziglang/zig/issues/16076>,
<https://gist.github.com/rtfeldman/77fb430ee57b42f5f2ca973a3992532f.pibb>,
<https://raw.githubusercontent.com/roc-lang/roc/main/build.zig>.
Pony: <https://github.com/ponylang/ponyc/pull/6137>,
<https://github.com/ponylang/ponyc/tree/main/src/libponyrt>.
Swift: <https://raw.githubusercontent.com/swiftlang/swift/main/include/swift/Runtime/RuntimeFunctions.def>.
Koka: <https://github.com/koka-lang/koka/tree/master/kklib>. Nim: <https://nim-lang.org/docs/nimc.html>.
Crystal: <https://github.com/crystal-lang/crystal/wiki/All-required-libraries>,
<https://github.com/crystal-lang/crystal/tree/master/src>.
OCaml: <https://github.com/ocaml/ocaml/tree/trunk/runtime>.
Zig: <https://github.com/ziglang/zig/tree/master/lib/compiler_rt>.
Julia: <https://docs.julialang.org/en/v1/devdocs/llvm/>.
compiler-builtins: <https://raw.githubusercontent.com/rust-lang/compiler-builtins/master/compiler-builtins/README.md>.
Gleam: <https://gleam.run/>. Mun: <https://github.com/mun-lang/mun>.
Rust and cargo: <https://doc.rust-lang.org/nightly/cargo/reference/unstable.html>,
<https://doc.rust-lang.org/reference/linkage.html>,
<https://doc.rust-lang.org/rustc/linker-plugin-lto.html>,
<https://doc.rust-lang.org/std/primitive.slice.html>,
<https://doc.rust-lang.org/std/primitive.str.html>,
<https://doc.rust-lang.org/std/process/struct.Child.html>.
LLVM: <https://llvm.org/docs/DeveloperPolicy.html>.
Crates: <https://github.com/boa-dev/ryu-js>, <https://crates.io/api/v1/crates/ryu-js>,
<https://crates.io/api/v1/crates/serde_json>, <https://crates.io/api/v1/crates/memchr>,
<https://crates.io/api/v1/crates/bumpalo>, <https://crates.io/api/v1/crates/simdutf8>.
Local: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/lib.rs`, `src/emit_llvm.rs`,
`runtime/toylang.c`, `tests/corpus/`, `~/.cargo/registry` crate manifests.

## 12. Native run-time baseline before the port

Recorded on 2026-09-24 at step 0, with the C runtime as it stood after the `strcpy` fix in
section 8. Step 8 compares against this table. Host: 12th Gen Intel Core i7-12700KF, 20 threads,
Linux 7.2.4-zen2, gcc 16.2.1, rustc 1.98.1. The machine was not idle: the load average was about
3.6 throughout, because other sessions were running, so the small workloads carry real noise.

Each program was linked with `toylang build FILE native` (the `cc program.o toylang.c` line of
section 1, no `-O`) and timed with `hyperfine -N --warmup 5 --runs 30`, feeding the single input
from `benches/inputs/`. Milliseconds, wall clock.

| program | input | median | mean | stddev | min | max |
| --- | --- | --- | --- | --- | --- | --- |
| binary-trees | 14 | 3.8 | 5.1 | 3.1 | 2.7 | 13.5 |
| fasta | 500 | 3.8 | 3.8 | 1.1 | 2.4 | 6.6 |
| mandelbrot | 200 | 42.2 | 43.3 | 3.1 | 40.8 | 52.7 |
| n-body | 10000 | 99.6 | 100.0 | 1.4 | 98.4 | 105.5 |
| spectral-norm | 100 | 41.6 | 42.4 | 2.0 | 40.8 | 49.0 |

`fannkuch-redux` has no row: it does not compile on any backend at this commit
(`toylang: fannkuch-redux.toy: 'step' is already a plain function; a trait method cannot share
its name (at byte 619)`), so `just bench fannkuch-redux` fails as well. That break predates this
step and is not fixed here.

Read binary-trees and fasta by the minimum and the median, not the mean: at 3 to 4 ms they are
dominated by process start-up and the mean is pulled up by a few slow runs. The three others are
steady enough that a slowdown beyond about 5 ms (mandelbrot, spectral-norm) or 3 ms (n-body)
would be outside what this measurement can hide. None of these programs is heavy on printing
Floats or on the JSON reader, so they say little about the ports of steps 2 and 6 on their own;
step 8 should read them as a guard against a general slowdown, not as evidence for any one port.

### Check after step 5 (Vec operations in Rust)

Recorded on 2026-09-24 with the load average at about 4. The table above is not reused here,
because other sessions made it noisy; instead the same six programs were built once with the
runtime as it stood before step 5 (Vec operations still in C) and once after, and timed
interleaved with `hyperfine -N --warmup 5 --runs 30` on the same inputs. Every native output is
byte-identical to the Rust backend's on the same input. Medians in milliseconds, before / after:

| program | before | after |
| --- | --- | --- |
| binary-trees | 3.4 | 3.7 |
| fasta | 2.6 | 2.3 |
| mandelbrot | 38.8 | 39.0 |
| n-body | 95.3 | 93.3 |
| spectral-norm | 39.1 | 39.2 |
| fannkuch-redux | 52.4 | 52.5 |

No slowdown outside the noise. fannkuch-redux compiles again at this commit, so it has a row
now (there is no step 0 baseline for it). These programs spend their time in generated code and
in Vec construction and element access, which were already Rust at the "before" build, so they
are a weak test of the ported sorts and reshapes; the unit tests and the corpus carry those.

### Check after step 6 (input in Rust)

Recorded on 2026-09-24 with the load average at about 3.5. The runtime before step 6 (C lines,
split and JSON parser) against after, the same six programs built by a compiler from each
commit and timed with `hyperfine -N --warmup 5 --runs 30` on the `benches/inputs/` files. Every
output is byte-identical between the two builds. Medians would be more honest than means at
these sizes; the means, in milliseconds, before / after:

| program | before | after |
| --- | --- | --- |
| binary-trees | 3.5 | 3.4 |
| fannkuch-redux | 51.6 | 51.2 |
| fasta | 2.3 | 2.3 (a first run read 8.2 with a stddev of 2.4; two reruns of 100 gave 2.3 and 2.5) |
| mandelbrot | 34.2 | 33.0 |
| n-body | 90.4 | 89.7 |
| spectral-norm | 35.0 | 35.5 |

No slowdown outside the noise. These programs read a single small number, so they say nothing
about the reader itself. That was measured on generated input instead (not committed; a
26 MB document with 300,000 records, the same records as 26 MB of JSON lines, 500,000 floats,
500,000 text lines), C before and Rust after:

| input | before | after |
| --- | --- | --- |
| `parse stdin`, `{ users: Vec<{ name, age, ok }> }` | 3447 ms | 185 ms |
| `parse stdin`, `Vec<Float>` | 119 ms | 61 ms |
| `collect` over JSON lines | 273 ms | 174 ms |
| `jsonlines` over JSON lines | 324 ms | 252 ms |
| `collect stdin` (lines) | 34 ms | 29 ms |
| `dsv(",")` | 172 ms | 140 ms |

The 3.4 s row is the C parser allocating a buffer the size of the rest of the input for every
string it read (`tl_alloc(j->end - j->p)` in `tl_parse_string`), 2.8 s of it in the kernel.

### Findings from step 6

- **The compiler's own serde_json misreads Floats.** `toylang` itself depends on serde_json
  without `float_roundtrip`, and `run_on` reads every input into a `serde_json::Value` before
  any backend sees it. Built as `cargo build -p toylang`, `printf 91186252760.18955 | toylang
  run f.toy <backend>` (with `fn id(x: Float) -> Float = x; id(parse stdin)`) prints
  `91186252760.18956` on all five backends that run it; the correctly rounded double prints
  `91186252760.18954`. In a workspace build (`cargo build --workspace`, `just check`) cargo
  unifies runtime-rs's `float_roundtrip` onto the compiler and the same run prints the right
  digits, so the test suite cannot see it. Board row `compiler-float-input-roundtrip`.
- **The C `tl_utf8_valid` accepted overlong forms, surrogates and code points past U+10FFFF.**
  3,153 of 185,792 sequences tried. Rust's `from_utf8` refuses them.
- **The descriptor grammar's comment in src/emit_llvm.rs (`descriptor`) still points at
  runtime/toylang.c.** The grammar now lives at the top of runtime-rs/src/json.rs. Left for step
  8, which deletes the C file and has to fix that pointer and the ones in tests/corpus/.

### Check after step 7 (`pipe_through` in Rust)

Recorded on 2026-09-24 with the load average at about 4.5. Compilers built from the parent
commit and from this one, the same six programs built with each and timed interleaved with
`hyperfine -N --warmup 5 --runs 30` on the `benches/inputs/` files. Every output is
byte-identical between the two builds. Medians in milliseconds, before / after:

| program | before | after |
| --- | --- | --- |
| binary-trees | 2.9 | 3.4 (minimum 2.5 on both) |
| fannkuch-redux | 52.4 | 53.7 |
| fasta | 2.5 | 2.5 |
| mandelbrot | 34.0 | 33.5 |
| n-body | 93.6 | 93.8 |
| spectral-norm | 37.5 | 37.9 |

No slowdown outside the noise. None of these programs calls `pipe_through`, so this only says
that carrying `std::process` and `libc` did not slow start-up. Stripped, the n-body binary went
from 398,216 to 406,408 bytes (+8 KB) with the process and thread code linked in.

### Findings from step 7

- **`tl_pipe_through` was the last C behaviour.** With it and its helpers (`tl_alloc`,
  `tl_list_push`, `tl_buf_append`, `tl_pipe_lines`, `tl_pipe_close`) gone, `runtime/toylang.c`
  is a header comment. It is still compiled and linked; step 8 deletes it. The Rust symbols that
  existed only for C to call (`tl_fail`, `tl_utf8_valid`, `tl_str_new`) went with it.
- **SIGPIPE.** The generated `main` bypasses `lang_start`, so a compiled program has SIGPIPE at
  its default action, and a Rust test binary does not (its `main` ignores it), which is why the
  behaviour is pinned in `tests/native_pipe_through.rs` on the built binary and not in the crate's
  unit tests. Writing to a child that has already exited would kill the program, so the writer
  thread blocks SIGPIPE for itself with `pthread_sigmask` (the crate now depends on `libc` for
  that). The C ignored the signal for the whole process around its poll loop and restored it;
  the thread mask never touches the program's own disposition. Removing the mask makes
  `writing_to_a_child_that_already_exited_does_not_kill_the_program` fail. The program's own
  stdout closed early still ends it with SIGPIPE, before and after a `pipe_through`, the way it
  did with the C.
- **The child's signals.** `std::process::Command` puts SIGPIPE back to its default action in
  the child and clears the signal mask, whether or not the program itself was started ignoring
  SIGPIPE. Checked with a child running `yes | head -n 1` under a shell that had run
  `trap '' PIPE`: silent under the port, and `yes: standard output: Broken pipe` when SIGPIPE stays
  ignored.
- **One difference left, on purpose.** An argument or command containing a NUL byte was cut at
  the NUL by the C. `Command` refuses it, so it is now `cannot spawn subprocess ...: nul byte
  found in provided data`.
