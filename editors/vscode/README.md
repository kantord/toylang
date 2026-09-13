# toylang for VS Code

Syntax highlighting only. `syntaxes/toylang.tmLanguage.json` is generated -- run
`cargo run --bin gen_syntax` from the repo root after changing `src/parse.rs`'s token set, not by
hand. `language-configuration.json` (comment leader, bracket pairs) is the one hand-written file
here; it is small and stable enough that a generator would cost more than it saves.

Try it locally without publishing:

```sh
ln -s "$(pwd)/editors/vscode" ~/.vscode/extensions/toylang-dev
```

Restart VS Code. Package for distribution with `npx vsce package` from this directory.
