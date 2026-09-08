# pipe_through

`pipe_through({cmd: Str, args: Vec<Str>, lines: Stream<Str>} -> Stream<PipeLine>`: streams the
lines of `lines` into a subprocess's stdin, and relays the subprocess's stdout and stderr lines back,
each tagged by origin as a `PipeLine` value. `PipeLine` is the prelude's closed-nominal enum:
`Stdout{text: Str}` for a line read from the child's stdout,and `Stderr{text: Str}` for one from its
stderr.

The `lines` field is the one place a stream is legal inside a record: it is consumed by the subprocess,
never stored. `cmd` and `args` are ordinary values; `cmd` is run against `PATH`, exactly the way the
host's own shell would spell it. The child's exit status is not an error: a filter like `grep` exits
nonzero on "no matches", which is a normal outcome for the shape this builtin exists to express.



As of the Rust backend, stdout lines stream out as they arrive,and stderr lines are drained concurrently
so neither pipe can fill up and stall the child. The two streams' relative order is deterministic: all
stdout lines come first, then all stderr lines, which is what keeps the tagged output reproducible. The
stream starts at `lines` and dies at `collect`, exactly the way any other stream does.





The one backend where the primitive exists today is Rust (`std::process::Command` with piped stdin,stdout,
and stderr). the other backends' `Builtin` match arms carry an unimplemented arm for it, so a program
using `pipe_through` refuses to compile there. Backend coverage is the row's business, not this page's.
