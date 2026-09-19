# toylang.conf.yaml

The compiler's one config file. It is found by walking upward from the current directory and
taking the first `toylang.conf.yaml` on the way to the root, so a file in a project's top
directory covers every subdirectory; no file anywhere means the defaults. Only the JS backend
reads it, on every `run`, `emit`, and `build` with `js`: the other backends never open it, and
an empty file is the same as none.

It has two keys. `target` is `node` (the default) or `web`. `web` holds up to three JavaScript
substitutes, `input`, `lines`, and `read_line`, for the stdin readers the emitted code would
otherwise get from node's `fs`. Anything else is an error.

## `target: web`

Node reads stdin through `require("fs")`; a browser has neither. For a program that reads no
stdin the two targets emit byte-identical code (checked with `diff` on `str(1 + 2)`). For a
program that does, the web target refuses at compile time rather than emitting a script that
would throw on its first line in a browser:

```
$ cat toylang.conf.yaml
target: web
$ toylang emit lines.toy js
toylang: lines.toy: the web target has no stdin: `stdin`, `dsv`, and stream-typed pipelines all read through node's `fs`
```

## The `web` substitutes

Each `web` key is JavaScript text pasted verbatim at the top of the emitted file, in place of
the node reader the program's shape calls. Nothing checks the text: it has to define the
function the emitted code will call, and `lines: 3` was accepted and emitted a bare `3`.
Which reader a program calls depends on how it reads stdin, and only the matching substitute
lifts the refusal.

`input` defines `tl_read_input()`, returning the whole of stdin as one string. It is what
`parse(stdin)` calls, and what an eager `collect(stdin | map(parse(.)))` calls too, since the
two share the read and differ only in what follows it.

```toylang
fn twice(x: Int) -> Int = x * 2

twice(parse stdin)
```

```input
21
```

```output
42
```

```
$ cat toylang.conf.yaml
target: web
web:
  input: |
    function tl_read_input() { return "21"; }
$ toylang emit input.toy js
function tl_read_input() { return "21"; }
// `|0` is ToInt32: it wraps to 32 bits and truncates toward zero, and V8 folds it away once it
// knows the value is already a Smi.
function tl_div(a, b) {
  if (b === 0) { throw new Error("toylang: divided by zero"); }
  return (a / b) | 0;
}
function tl_rem(a, b) {
  if (b === 0) { throw new Error("toylang: divided by zero"); }
  return (a % b) | 0;
}
function v_twice(v_x) {
  return Math.imul(v_x, 2);
}
const t_input = JSON.parse(tl_read_input());
console.log(String(v_twice(t_input)));
$ toylang emit input.toy js > input.js && node input.js
42
```

`lines` defines `tl_collect_lines()`, returning an array of raw lines. It is what
`collect(stdin)` and a [`dsv`](../sources/dsv.md) read call: the shapes that need every line
in hand before the body runs.

```toylang
collect stdin
```

```input
ada
bo
```

```output
["ada","bo"]
```

```
$ cat toylang.conf.yaml
target: web
web:
  lines: |
    function tl_collect_lines() { return ["ada", "bo", "cy"]; }
$ toylang emit lines.toy js
function tl_join(v, f) {
  const parts = [];
  for (let i = 0; i < v.length; i++) parts.push(f(v[i]));
  return "[" + parts.join(",") + "]";
}
function tl_collect_lines() { return ["ada", "bo", "cy"]; }
console.log(tl_join(tl_collect_lines(), (e0) => JSON.stringify(e0)));
```

`read_line` defines `tl_read_line()`, returning the next raw line or `null` at the end. It is
what a fused pipeline calls: a stream-typed `stdin` feeding [`jsonlines`](../builtins/jsonlines.md),
which every backend compiles into a read-one/transform-one/write-one loop, and the fused form
of `stdin | map(parse(.))` likewise. The substitute can carry state of its own, since the text
is pasted whole:

```toylang
fn shout(names: Stream<Str>) -> Stream<Str> = names | map(. + "!")

jsonlines(shout stdin)
```

```input
ada
bo
```

```output
"ada!"
"bo!"
```

```
$ cat toylang.conf.yaml
target: web
web:
  read_line: |
    let tl_lines = ["ada", "bo"];
    function tl_read_line() { return tl_lines.length ? tl_lines.shift() : null; }
$ toylang emit stream.toy js | tail -7
let tl_lines = ["ada", "bo"];
function tl_read_line() { return tl_lines.length ? tl_lines.shift() : null; }
for (;;) {
  const t_line_raw = tl_read_line();
  if (t_line_raw === null) break;
  const t_1 = t_line_raw;
  console.log(JSON.stringify((t_1 + "!")));
}
```

A substitute for a reader the program does not call leaves the refusal in place: the `lines`
config above, applied to this fused `shout` program, is refused with the same message as no
config at all, because the fused loop never calls `tl_collect_lines`. The same goes for
`lines` against the `parse(stdin)` program. Match the substitute to the shape, not to the
source name.

Under `target: node`, or with no `target` line, the `web` keys are ignored entirely: the emitted
code was byte-identical with and without them. And `toylang run FILE js` under a web config
runs the substituted script through node, so what the program sees is the substitute's data,
not the terminal's:

```
$ echo from-stdin | toylang run lines.toy js
["ada","bo","cy"]
```

## A malformed file

A file that does not parse fails every JS command with the file's absolute path (shortened to
`/work` below), the key, and the position, and leaves the other backends alone. An unknown key
is an error rather than a silent no-op, so a typo cannot pass as a working config:

```
$ printf 'targt: web\n' > toylang.conf.yaml
$ toylang emit plain.toy js
toylang: plain.toy: /work/toylang.conf.yaml: unknown field `targt`, expected `target` or `web`
$ printf 'web:\n  foo: 1\n' > toylang.conf.yaml
$ toylang emit plain.toy js
toylang: plain.toy: /work/toylang.conf.yaml: web: unknown field `foo`, expected one of `input`, `lines`, `read_line` at line 2 column 3
$ printf 'target: browser\n' > toylang.conf.yaml
$ toylang emit plain.toy js
toylang: plain.toy: /work/toylang.conf.yaml: target: unknown variant `browser`, expected `node` or `web` at line 1 column 9
$ toylang run plain.toy
3
```

The last line is the Lua default, which never read the file. `toylang build plain.toy js`
fails the same way and writes neither file.
