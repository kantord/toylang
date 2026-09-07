# HTTP query sugar: what the vocabulary already supports

Research spike ahead of the `http-query-sugar-design` round (board row
`http-query-sugar-research`, gh:171). The maintainer note asks for the minimal
HTTP query surface (requests-like: get/post? headers? body?) as "sugar over
whatever backend already provides", and for a survey of what request/response building
blocks already exist per backend before scoping a design row. This writes up
(a) what a source/sink looks like per backend today, (b) what each backend runtime
could supply for HTTP,(c) requests' minimal surface as the sugar reference, and
(d) three syntax candidates grounded in the existing vocabulary, each as real
toylang code, so the design round can ask a concrete question instead of a
green-field one. None of the candidate programs compile against today's compiler:
the point is what the sugar should read as, for the round to rule on, not a corpus
case (a program that does not compile is not a corpus case).

## What the vocabulary already is

A toylang program is one expression: there is no sequencing in the grammar, and
[the stdout/stderr effect-model research](stdout-stderr-effect-model-research.md)
makes that the load-bearing fact for effects as well. Input enters through
sources, and there is exactly one real stdin, read at most once, one of four ways
([sources/stdin.md](../docs/reference/sources/stdin.md), [sources/dsv.md](../docs/reference/sources/dsv.md)):

- `parse(stdin)`: one JSON value of the checked type, validated before the program
  runs; `stdin | map(parse(.))`: one JSON value per line, a `Stream<T>`;
- `stdin` (spelled `lines` in the IR): raw lines, a `Stream<Str>`;
- `dsv(delim)` / `csv` / `tsv`: raw lines split on the delimiter, `Vec<Vec<Str>>`.

Output leaves one of two ways: the top-level printer renders the program's value by its
type, or [`jsonlines(v)`](../docs/reference/builtins/jsonlines.md) writes each entry of a
`Vec<T>` / `Stream<T>` as its own JSON line. `jsonlines` is a sink: legal only as the
program's outermost expression or in a `Sink`-returning function body, never a value.

Two more facts matter for HTTP. First, functions are unary and a record is how
several arguments travel ([draft.md:899](draft.md), [draft.md:972](draft.md)), with parens
optional when the argument is a record literal (`circle{r: 1}`). Second, there is no
Map type: records are closed, typed field sets, so arbitrary string-to-string headers
have no existing representation: a `Vec<{name: Str, value: Str}>` or a closed
record of known header names are the expressible shapes.

Failures are one of two kinds: runtime stops (division by zero, a stdin parse
failure)and absence answers as `Opt` (indexing past the end, `tail`, `first`, `max`).
Nothing carries a status code today.

## What a source/sink looks like per backend today

Every backend reads the same real stdin and writes the same real stdout, uniformly, and
no backend has any network primitive. Verified three ways: no socket/http/fetch/
urlopen/reqwest string anywhere in `src/`, `runtime/`, `tests/`, or `benches/`; each
emitter's I/O is stdin-reading/stdout-writing code only; and a toolchain check confirms
the substrate gaps below.

 Backend | one JSON value | one per line (JSON lines) | raw lines | DSV | sink
 Lua | `t_input` | `t_inputs` | `tl_collect_lines()` | `tl_split_lines(tl_collect_lines(), d)` | `tl_jsonlines` helper (fused loop via `tl_next_input`)
 JS | `t_input` | `t_inputs` | `tl_collect_lines()` | `tl_collect_lines().map(l -> l.split(d))` | `tl_jsonlines` helper
 Native | runtime slot via `tl_read_value` | `tl_read_inputs` | `tl_collect_lines` | `tl_collect_lines` + `tl_split_lines` | runtime `tl_jsonlines`
 jq | `$t_input` | `$t_inputs` | `[inputs]` | `[inputs | if length == 0 then [""] else split(d) end]` | tojson filter pipeline
 Go | `t_input` | `t_inputs` | `tlCollectLines()` | `tlDsv(tlCollectLines(), d)` | `tlJsonlines` helper
 Py | `t_input` | `t_inputs` | `tl_collect_lines()` | `[l.split(d) for l in tl_collect_lines()]` | `tl_jsonlines` helper
 Rust | `tl_read_value()` | `tl_read_values()` | `tl_read_lines()` | `tl_dsv(&tl_read_lines(), d)` | `tl_jsonlines` helper

Notable spellings: jq's `lines` is `[inputs]` under `-n -R` raw-input mode, and
its `dsv` forces an empty line to one empty field to match every other backend;
Python's `t_inputs` is a list comprehension over `sys.stdin`; Go and Rust decode
via a streaming `json.Decoder` / the emitted `tl_read_values`.

## What each backend runtime could supply

The sugar substrate, per backend runtime (verified against the installed toolchain:
go 1.24.4, node v20.19.2, python 3.13.5, lua 5.4.7, jq
1.8.2, cc 14.2, rustc absent):

- **Go**: `net/http`, https natively, zero new dependencies.
- **JS**: global `fetch` (a function on node v20;available since node 18), zero new dependencies. The response shape is `status`, `headers`, `text()` / `json()`.
- **Python**: `urllib.request` (installed; https via stdlib `ssl`). `requests` itself is NOT installed: the py backend would sugar over urllib, the way "requests-like" names the UX, not the target.

- **Lua**: nothing. No `socket` module on the installed 5.4;`socket.http` is the external LuaSocket C library, a new dependency.
- **jq**: nothing. jq 1.8.2 has no `http` builtin; base jq ships no network primitives.

- **Rust backend**: nothing without breaking a constraint:the emitted file is self-contained with no external crate ([emit_rs.rs:10](../src/emit_rs.rs)), and Rust stdlib has no HTTP or TLS. rustc isn't even installed here.

- **Native (C)**: nothing. C stdlib has no HTTP or TLS; a hand-rolled plain-TCP client could do `http://` only, and only without TLS, which no real API serves.


The TLS split is the survey's sharpest finding:three of seven backends (Go, JS,
Py) can do `https://` with zero new dependencies, and four (Lua, jq, Rust,
native)cannot without a dependency decision (LuaSocket, a reqwest-style crate, a
TLS lib for C) or by scoping HTTP to plain `http://`. And the corpus harness requires
every backend to agree on a program's output (or to all refuse it, per the working
agreement's corpus rules),so an HTTP corpus case would either take the whole class off
every backend, or force the dependency decision on the four. "Which backend(s) it
targets" is the maintainer's own open question, and the survey's answer is the
three-TLS-backends split, or a deps decision, ruled before any syntax.

## requests' minimal surface, the sugar reference

`requests` is not installed here, so what follows is its documented API
(requests.readthedocs.io), the 2.x surface),not run:

- `requests.get(url, params=None, headers=None, timeout=None, ...) -> Response`
- `requests.post(url, data=None, json=None, headers=None, timeout=None, ...) -> Response`
- `Response`: `.status_code` -> Int, `.headers` -> case-insensitive str-to-str
  mapping, `.text` -> decoded body Str, `.json()` -> parsed JSON body (raises on
  an unparseable body), `.content` -> bytes.
- Headers and params are dicts of str-to-str;the body is either `data` (form-encoded:
  a Str or a dict of fields)or `json` (serialized JSON body).

The minimal surface the task names (get/post, headers, body, response shape)is
exactly `requests.get(url, headers={...})`, `requests.post(url, json={...},
headers={...})`, and `resp.status_code` / `resp.headers` / `resp.json()`. The pieces
toylang would need to express: a URL (a Str literal), an optional method, optional
headers, an optional body (a JSON value or a form Str),and an optional status/headers
read. Params (the query string)is sugar over the URL itself, which toylang can
already build with `+` on Str, so it is not a separate surface item for the minimal cut.



## Candidate A: `http(url)`, the body-as-value source

The minimal cut, and the most grounded: GET the URL, parse the response body as one
JSON value of the checked type, exactly as `parse(stdin)` does for stdin. A non-2xx
status or an unparseable body stops the program, the way a stdin parse failure does.




```toylang
http("https://api.example.com/users") | select(.age >= 18) | .[].name
```



For the cases that need more than a bare GET, the record-argument form carries
method, headers, and body, since a record is how several arguments travel:

```toylang
http({
    url: "https://api.example.com/users",
    method: "POST",
    headers: [{name: "Content-Type", value: "application/json"}],
    body: {name: "Ada"},
})
```



Grounding: `dsv(delim)` is the parameterized member of the sources family, so a
source with an argument already exists;`parse(stdin)` establishes
body-as-checked-typed-value; records-are-how-arguments-travel extends the argument
from `dsv`'s string literal to a config record. The result composes with everything
downstream unchanged: it IS a source, so `map`, `select`, `collect`, field access,
and `jsonlines` all work over it as they do over stdin. The JSON-lines endpoint
shape (`stdin | map(parse(.))`) is the natural streaming analogue for a body that is
one value per line, though the minimal cut above is one-value.

Cost: none of the response envelope is reachable: a program cannot tell a 404
from a 500, and cannot read response headers. For requests' `resp.json()`-style use
this is complete; for any status/header inspection, it is not.



## Candidate B: `fetch(url))`, the response record

The requests-Response shape rendered as a toylang record: status, headers, body,
all reachable by ordinary field access:



```toylang
fetch("https://api.example.com/users").status
fetch("https://api.example.com/users").body | .[].name
```



Headers have no Map type to ride on, so they would be a `Vec<{name: Str, value: Str}>`
(or a closed record of known header names),and the body field has the same "no type of
its own" problem `input` has, but `input`'s trick (check the whole expression against an
expected type) does not reach through a record field access, so the body field's type
has no existing rule to be checked against.



Grounding: records are how several things travel, and field access is the
vocabulary's way in;this is the only candidate where status and headers are data a
program can branch on. Cost: two things the vocabulary does not have (a value-typed
response record, and a header representation that is either a list-of-pairs or a closed
record of known names),and the un-typable body field needs a new checking rule or an
explicit per-call type annotation, both new. It is the least "thin sugar" of the three.



## Candidate C: two entry points, `http(url)` and `http_resp(url)`

The sources family already ships two spellings of the same capability (`input` and
`inputs` read the same stdin two ways;`csv`/`tsv` are `dsv` with the delimiter fixed),so
"same capability, two entry points" is an existing pattern. `http(url)` keeps
candidate A's body-as-value source for the common case, and `http_resp(url)` (or
`fetch(url)`,the name is for the round to pick) surfaces candidate B's envelope
only when a program asks for it:

```toylang
http("https://api.example.com/users") | .[].name
http_resp("https://api.example.com/users").status
```

Grounding:the dsv family's "same operation, several spellings" pattern,with the
common spelling parameterized the least. Cost: two builtins to document and learn, and
candidate B's typing costs are still paid on the rare path, so this candidate only pays
them when the envelope is genuinely asked for. The design round's real question,
sharpened: is the envelope (status/headers as data) worth a new response type, a
header representation, and a body-field typing rule at all, or is candidate A (status
only as a failure message) enough?



## What the design round now has to rule on

- **The backend split**, before any syntax: only Go, JS, Py can do https with zero
  new deps, and the corpus requires every backend to agree on every program's output. Either
  HTTP is scoped to the three capable backends with the other four refusing this whole class,
  or Lua/jq/Rust/native get a dependency decision (LuaSocket, a reqwest-style crate, a
  TLS lib for C),or HTTP is plain-http-only everywhere(no real API serves)..
 The maintainer's
  "which backend(s) it targets" is the first question.


- **Status: data or failure?** Candidate A stops on non-2xx, the way a stdin parse
  failure does;candidates B and C make status a field a program can branch on. Nothing
  in the vocabulary carries a status code today, so making it data invents a response
  shape the language has no other instance of.


- **Headers: how are they represented at all**, since there is no Map type:
  `Vec<{name, value}>` or a closed record of known header names. This has to be answered
   before either B or C's envelope works, and before the POST config in A's record form works
   either.


- **The body field's typing** (for B and C:the "no type of its own" trick does not
  reach through a record field access;the round has to pick a rule (an explicit type
  annotation on the call, or a checked-only field class extended),or drop the envelope.



Derived:the source/sink vocabulary from
[docs/reference/sources/stdin.md](../docs/reference/sources/stdin.md),
[docs/reference/sources/dsv.md](../docs/reference/sources/dsv.md),
[docs/reference/builtins/parse.md](../docs/reference/builtins/parse.md), and
[docs/reference/builtins/jsonlines.md](../docs/reference/builtins/jsonlines.md);the
unary/records decisions from [draft.md](draft.md);the corpus agreement rule from the
working agreement;the backend toolchain facts from the installed environment (go
1.24.4, node v20.19.2, python 3.13.5, lua 5.4.7, jq 1.8.2,
cc 14.2, rustc absent, `requests` absent, `socket` absent, global `fetch`
present);the requests surface from its documented 2.x API, not run. Agent-invented:
the three candidate shapes themselves, and the TLS-split framing for the round's first
question.