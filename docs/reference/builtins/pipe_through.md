# pipe_through

`pipe_through({cmd: Str, args: Vec<Str>, lines: Stream<Str>}) -> Stream<PipeLine>`: streams the
lines of `lines` into a subprocess's stdin, and relays the subprocess's stdout and stderr lines back,
each tagged by origin as a `PipeLine` value. `PipeLine` is the prelude's closed-nominal enum:
`Stdout{text: Str}` for a line read from the child's stdout, and `Stderr{text: Str}` for one from its
stderr.

The `lines` field is the one place a stream is legal inside a record: it is consumed by the subprocess,
never stored. `cmd` and `args` are ordinary values; `cmd` is run against `PATH`, exactly the way the
host's own shell would spell it. The child's exit status is not an error: a filter like `grep` exits
nonzero on "no matches", which is a normal outcome for the shape this builtin exists to express.



As of the Rust backend, stdout lines stream out as they arrive, and stderr lines are drained concurrently
so neither pipe can fill up and stall the child. The two streams' relative order is deterministic: all
stdout lines come first, then all stderr lines, which is what keeps the tagged output reproducible. The
stream starts at `lines` and dies at `collect`, exactly the way any other stream does.





Runs on every backend except jq, which refuses it permanently: a jq program cannot spawn a
process, so `pipe_through` is a host-capability primitive that jq has no way to express. The
refusal happens before anything is emitted, and says why rather than "yet":

```
`pipe_through` has no jq backend, and never will: jq cannot spawn a process
```

Lua has no bidirectional-pipe primitive and no separate `lua` process to hand pipes to --
`toylang run --backend lua` runs the emitted chunk embedded in the compiler's own process via
`mlua` -- so stdin and stderr go through temp files rather than pipes.

The native backend uses `posix_spawnp` and one `poll` loop in its C runtime that writes stdin
and reads stdout and stderr together, so it needs no threads. It buffers all of stdout and stderr
before yielding lines, where the Rust backend streams stdout as it arrives; the output is the same.
A child that closes its stdin early (`head`) or never reads it does not stall the program.

Lines split on `\n` only and keep a `\r`, on every backend.
