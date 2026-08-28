import { useEffect, useState } from "react"
import type { HighlighterCore } from "shiki/core"

/**
 * Named one by one rather than through shiki's default entry, which registers every grammar it
 * ships and leaves the build with 11MB of lazily-loaded languages nobody asks for.
 *
 * jq and toylang are absent on purpose. Shiki has no jq grammar, and toylang does not have one
 * anywhere; both render as plain text, which is honest about what is known.
 */
const LANGS = new Set(["go", "javascript", "json", "llvm", "lua", "python", "rust"])

let pending: Promise<HighlighterCore> | null = null

function highlighter(): Promise<HighlighterCore> {
  // One instance for the page: the grammars are the bulk of the cost and would otherwise be
  // recompiled on every tab switch. Imported dynamically so they are a chunk of their own and
  // the corpus is readable before they arrive, rather than after 200kB of grammar.
  pending ??= (async () => {
    const [core, engine, light, dark, go, javascript, json, llvm, lua, python, rust] =
      await Promise.all([
        import("shiki/core"),
        import("shiki/engine/javascript"),
        import("shiki/themes/github-light.mjs"),
        import("shiki/themes/github-dark.mjs"),
        import("shiki/langs/go.mjs"),
        import("shiki/langs/javascript.mjs"),
        import("shiki/langs/json.mjs"),
        import("shiki/langs/llvm.mjs"),
        import("shiki/langs/lua.mjs"),
        import("shiki/langs/python.mjs"),
        import("shiki/langs/rust.mjs"),
      ])
    return core.createHighlighterCore({
      themes: [light.default, dark.default],
      langs: [go.default, javascript.default, json.default, llvm.default, lua.default, python.default, rust.default],
      // The JavaScript engine rather than Oniguruma, which would pull in a 600kB wasm blob for
      // six grammars this simple.
      engine: engine.createJavaScriptRegexEngine(),
    })
  })()
  return pending
}

/** Highlighted HTML, or null while the grammars load and for languages without one. */
export function useHighlighted(code: string, lang: string): string | null {
  const [html, setHtml] = useState<string | null>(null)
  useEffect(() => {
    if (!LANGS.has(lang)) {
      setHtml(null)
      return
    }
    let live = true
    highlighter()
      .then((h) =>
        h.codeToHtml(code, {
          lang,
          themes: { light: "github-light", dark: "github-dark" },
          defaultColor: false,
        }),
      )
      .then((out) => live && setHtml(out))
      .catch(() => live && setHtml(null))
    return () => {
      live = false
    }
  }, [code, lang])
  return html
}
