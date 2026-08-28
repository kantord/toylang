# lines

`lines` is stdin as a `Stream<Str>` of raw lines: no parsing, no quoting, each line handed
over as the text it is. The source for input that is lines of text rather than JSON.

```case
lines_cat
```

The line terminator is not part of the entry, an unterminated final line still arrives, and
a `\r` before the `\n` is preserved rather than stripped: the entries are the bytes between
terminators, not a platform's opinion of them.

Being a [Stream](../types/stream.md) source, `lines` follows the stream rules: read at most
once per program (there is only ever one real stdin, so a second `lines` is refused rather
than silently handed nothing), consumed exactly once, incompatible with
[`input`](input.md) and [`inputs`](inputs.md), and not readable inside a `map` or `select`
body, which runs once per entry.

```case
jsonlines_of_lines
```
