import { useMemo } from "react"
import type { Tokens } from "marked"

import { Code } from "@/components/Code"
import type { Piece } from "@/lib/blocks"
import { splitBlocks } from "@/lib/blocks"
import type { EmbeddedCaseData } from "@/lib/pageData"
import type { Page } from "@/lib/docs"
import { exampleHref } from "@/lib/pageData"
import { withBase } from "@/lib/route"

/**
 * Renders one docs page. Everything except code fences goes through marked; the fences are the
 * fragment protocol the harness (tests/docs.rs) runs, so each kind gets its own presentation
 * here rather than appearing as an anonymous code block.
 *
 * `cases` is the small slice of the corpus this exact page's `<case>` fences reference
 * (lib/pageData.ts), not the whole corpus -- the static build embeds only that per page.
 */
export function Markdown({ page, cases }: { page: Page; cases: Record<string, EmbeddedCaseData> }) {
  const blocks = useMemo(() => splitBlocks(page), [page])

  return (
    <article className="docs-prose min-w-0 max-w-2xl">
      {blocks.map((b, i) =>
        b.kind === "fence" ? (
          <Fence key={i} token={b.token} cases={cases} />
        ) : (
          <ProseBlock key={i} pieces={b.pieces} />
        ),
      )}
    </article>
  )
}

function ProseBlock({ pieces }: { pieces: Piece[] }) {
  const html = useMemo(() => pieces.map((p) => p.html).join(""), [pieces])
  return <div dangerouslySetInnerHTML={{ __html: html }} />
}

export function Fence({ token, cases }: { token: Tokens.Code; cases: Record<string, EmbeddedCaseData> }) {
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
      return <EmbeddedCase id={token.text.trim()} cases={cases} />
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
function EmbeddedCase({ id, cases }: { id: string; cases: Record<string, EmbeddedCaseData> }) {
  const c = cases[id]
  // The harness verifies every embedded id exists, so this renders only while corpus.json and
  // the docs are mid-edit; saying which id is better than rendering nothing.
  if (!c) {
    return <p className="text-sm text-destructive">corpus case `{id}` is not in corpus.json</p>
  }
  return (
    <div className="space-y-3 rounded-md border p-4">
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-sm font-medium">{c.name}</span>
        <a className="text-xs text-muted-foreground underline" href={withBase(exampleHref(c.name))}>
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
