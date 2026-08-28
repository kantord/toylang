# Typing wire data with enums

<!-- @comment Coordinator, replying to your note: correct -- variants are decided to be
capitalized types (Ping, Quit, Text; draft.md, the variants-are-types revision), but that
flip is not built yet, and this guide tracks the shipped implementation so its fragments
keep compiling under the fence harness. The full doc migration is folded into the
variant-types-flip build (board row, fable tier); this page flips the moment the language
does. -->

The task: stdin carries JSON whose shape varies by kind -- status strings, tagged messages,
mixed events -- and the program must handle every kind, provably.

## Name the set

Wire formats spell "one of a known set" two ways: a bare string (`"active"`), or a
single-key wrapper (`{"text": {"body": "hi"}}`). An enum declaration covers both at once,
because that pair is exactly the enum encoding: unit variants are strings, payload variants
are wrappers.

```toylang
enum Msg { ping, quit, text{body: Str} }

fn render(m: Msg) -> Str = m | ping -> "*ping*" or quit -> "*quit*" or text -> .body

render text{body: "hi"}
```

```output
hi
```

Declare the variants after the strings and wrappers the wire actually carries, and the type
system takes it from there: `flip input` below is a complete program that reads a status
and answers with the other one.

```case
enum_input
```

## Let validation refuse the garbage

Because the enum declares a closed set, input validation has something to check against. A
string that names no variant, a wrapper whose payload misses the declared type -- both are
refused before the program runs, on every backend, rather than flowing through as
unmatched data:

```case
enum_scalar_input_reject
```

This is the point of typing the wire: the match inside the program never meets a value
outside the set, which is what lets the checker demand every variant be handled and nothing
else.

## Payloads are any single type

A record payload (`text{body: Str}`) is declared in braces and destructured in the arm. A
scalar or `Vec` payload is declared in parens, constructed like a call, and bound whole to
`.` in its arm:

```case
enum_vec_payload
```

## The whole shape at stream scale

Enums compose with streams: `inputs` typed as `Stream<Msg>` validates each line against the
set as it arrives, one exhaustive match inside `map` proves coverage, and the sink prints
as it goes:

```case
jsonlines_enum_stream
```

A default arm (`any() -> ...`) trades some of that proof away for brevity; reach for it
when new variants should fall through rather than break the build. A guard arm trades none
of it away and cannot substitute for `any()` either -- the [matching guide](matching.md)
covers why. The [match reference](../reference/operators/match.md) has the arm shapes.
