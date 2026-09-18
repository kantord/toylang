# The command line

One binary, `toylang`, with four commands. Each names one file to read, except the
project-wide `fmt` walk. What a command produces goes to stdout; errors and diagnostics go to
stderr, prefixed with `toylang: FILE:`, and the exit code is nonzero when anything failed.

```
usage: toylang <run|emit> FILE [lua|js|jq|go|py|rust|llvm]
       toylang build FILE [js]
       toylang fmt FILE
       toylang fmt [--write]
       toylang --explain-offload <run|emit|build> FILE [lua|js|jq|go|py|rust|llvm]
```

`toylang run FILE [backend]` compiles the program, emits it for one backend, and runs the
result through that backend's own toolchain (lua, node, jq, go, python, rustc, or clang for
`llvm`). Lua is the default. stdin is read only when the program reads it, so a program that
does not is not left waiting on a terminal.

`toylang emit FILE [backend]` prints the emitted source instead of running it (LLVM IR for
`llvm`), with the same default. This is the way to see what a backend makes of a program.

`toylang build FILE [js]` writes a file next to where it was invoked, named after the source
file's stem: a linked native executable with no backend named, or the emitted script and its
`.d.ts` with `js`. Every other backend is refused, because nothing about it needs a build step
that `emit` does not already do. [toylang build](build.md) has the details.

`toylang fmt FILE` prints the file in canonical form and changes nothing on disk. `toylang
fmt` with no file walks down from the current directory, lists every `.toy` file that is not
canonical, and exits nonzero if any is; `toylang fmt --write` makes the same walk and rewrites
them.

Two things change what these commands do without appearing on the command line. A
[toylang.conf.yaml](config.md) anywhere up the directory tree switches the JS backend from
node to a browser target. The leading [--explain-offload](explain-offload.md) flag adds a
report on stderr of which stages became vectorizable kernels.
