# Step 1: walking skeleton

```
"hello world"
```

prints `hello world`.

The point is not the program, which is trivial, but that every stage exists and sits on the call
path: read file, lex, parse, check, lower, emit Lua, run through `mlua`, write stdout. Nothing is
hardcoded past the parser, but the language is one bare string literal and nothing else.

## Done when

- `cargo run -- run examples/hello.toy` prints `hello world`
- `cargo test` is green with one snapshot case
- the emitted Lua is visible via a flag, because reading it is how the next four steps get
  debugged

## Shape

Modules, each with something real in it rather than a placeholder: `lex`, `ast`, `parse`, `ty`,
`check`, `ir`, `lower`, `emit_lua`, `main`.

The IR is worth having even now, when it carries one node. Emitting Lua straight from the checked
AST would work at this size and would have to be undone at step 4, when composition starts
lowering to loops.

## Not in this step

Any operator. Any type other than `Str`. Error reporting beyond a message and a non-zero exit;
spans are recorded in the AST but nothing reads them yet.
