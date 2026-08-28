import { useMemo } from "react"
import { Marked, type Token, type Tokens } from "marked"

import { Code } from "@/components/Code"
import type { Corpus } from "@/lib/corpus"
import { resolveLink, type Page } from "@/lib/docs"

/**
 * Renders one docs page. Everything except code fences goes through marked; the fences are the
 * fragment protocol the harness (tests/docs.rs) runs, so each kind gets its own presentation
 * here rather than appearing as an anonymous code block.
 */
export function Markdown({ page, corpus }: { page: Page; corpus: Corpus }) {
  const blocks = useMemo(() => split(page), [page])

  return (
    <article className="docs-prose min-w-0 max-w-2xl">
      {blocks.map((b, i) =>
        b.kind === "html" ? (
          <div key={i} dangerouslySetInnerHTML={{ __html: b.html }} />
        ) : (
          <Fence key={i} token={b.token} corpus={corpus} />
        ),
      )}
    </article>
  )
}

type Block = { kind: "html"; html: string } | { kind: "fence"; token: Tokens.Code }

/** The fence languages that belong to the fragment protocol; anything else is illustration. */
const FRAGMENT = new Set(["toylang", "input", "output", "refuses", "error", "case"])

function split(page: Page): Block[] {
  const md = new Marked({
    renderer: {
      // Relative markdown links are written for the repository; rendered, they should lead to
      // the matching page here, or to GitHub for files this site does not show.
      link(token) {
        const target = resolveLink(page, token.href) ?? token.href
        const text = this.parser.parseInline(token.tokens)
        const external = target.startsWith("http")
        return `<a href="${target}"${external ? ' target="_blank" rel="noreferrer"' : ""}>${text}</a>`
      },
    },
  })

  const blocks: Block[] = []
  let run: Token[] = []
  const flush = () => {
    if (run.length) blocks.push({ kind: "html", html: md.parser(run) })
    run = []
  }
  for (const token of md.lexer(page.markdown)) {
    if (token.type === "code" && FRAGMENT.has((token as Tokens.Code).lang ?? "")) {
      flush()
      blocks.push({ kind: "fence", token: token as Tokens.Code })
    } else {
      run.push(token)
    }
  }
  flush()
  return blocks
}

function Fence({ token, corpus }: { token: Tokens.Code; corpus: Corpus }) {
  switch (token.lang) {
    case "toylang":
      return <Code code={token.text} lang="toylang" />
    case "input":
      return <Labeled label="stdin" code={token.text} />
    case "output":
      return <Labeled label="prints" code={token.text} />
    case "error":
      return <Labeled label="the checker refuses this program" code={token.text} />
    case "refuses":
      return (
        <p className="rounded-md border border-destructive/40 bg-destructive/5 px-4 py-3 text-sm">
          Every backend refuses to run this. What each says while refusing is its own business,
          so there is no output to show.
        </p>
      )
    case "case":
      return <EmbeddedCase id={token.text.trim()} corpus={corpus} />
    default:
      return null
  }
}

function Labeled({ label, code }: { label: string; code: string }) {
  return (
    <div className="space-y-1">
      <div className="text-xs font-medium text-muted-foreground">{label}</div>
      <Code code={code} lang="text" />
    </div>
  )
}

/**
 * A corpus case shown in place of repeating its program: same program, input, and expectation
 * the corpus runs, plus the way into the Examples browser where its emitted code lives.
 */
function EmbeddedCase({ id, corpus }: { id: string; corpus: Corpus }) {
  const c = corpus.cases.find((x) => x.name === id)
  // The harness verifies every embedded id exists, so this renders only while corpus.json and
  // the docs are mid-edit; saying which id is better than rendering nothing.
  if (!c) {
    return <p className="text-sm text-destructive">corpus case `{id}` is not in corpus.json</p>
  }
  return (
    <div className="space-y-3 rounded-md border p-4">
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-sm font-medium">{c.name}</span>
        <a className="text-xs text-muted-foreground underline" href={`#/examples/${c.name}`}>
          open in Examples
        </a>
      </div>
      <Code code={c.program} lang="toylang" />
      {c.input !== null && <Labeled label="stdin" code={c.input} />}
      {c.expect.kind === "output" ? (
        <Labeled label="prints" code={c.expect.value} />
      ) : (
        <p className="text-sm text-muted-foreground">Every backend refuses to run this.</p>
      )}
    </div>
  )
}
