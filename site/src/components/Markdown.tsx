import { useEffect, useMemo, useState } from "react"
import type { Tokens } from "marked"

import { Code } from "@/components/Code"
import type { Piece } from "@/lib/blocks"
import { splitBlocks } from "@/lib/blocks"
import type { Corpus } from "@/lib/corpus"
import type { Page } from "@/lib/docs"

type AnnotateModule = typeof import("@/components/AnnotateMode")

/** Loads the annotate-mode module only once that mode is actually on, and only in dev: the
 *  `import.meta.env.DEV` check is a dynamic-import guard, not just a render guard, so
 *  `vite build` never puts AnnotateMode.tsx (or the annotation scan it pulls in) in the
 *  production bundle (kantord/toylang#23). */
function useAnnotateMode(active: boolean): AnnotateModule | null {
  const [mod, setMod] = useState<AnnotateModule | null>(null)
  useEffect(() => {
    if (active && import.meta.env.DEV) {
      import("@/components/AnnotateMode").then(setMod)
    }
  }, [active])
  return active ? mod : null
}

/**
 * Renders one docs page. Everything except code fences goes through marked; the fences are the
 * fragment protocol the harness (tests/docs.rs) runs, so each kind gets its own presentation
 * here rather than appearing as an anonymous code block.
 *
 * `annotate` is the dev-only annotations mode (kantord/toylang#23): prose blocks get a
 * marker-pen wash where the coordinator left a review/comment/fill note, and become editable so
 * a reply autosaves to the edit inbox. It never runs outside `import.meta.env.DEV`.
 */
export function Markdown({
  page,
  corpus,
  annotate = false,
  scrollToBlock,
}: {
  page: Page
  corpus: Corpus
  annotate?: boolean
  scrollToBlock?: number
}) {
  const blocks = useMemo(() => splitBlocks(page), [page])
  const annotateMode = useAnnotateMode(annotate)
  const annotations = useMemo(
    () => (annotateMode ? annotateMode.pageAnnotations(page, blocks) : []),
    [annotateMode, page, blocks],
  )

  return (
    <article className="docs-prose min-w-0 max-w-2xl">
      {blocks.map((b, i) => {
        if (b.kind === "fence") return <Fence key={i} token={b.token} corpus={corpus} />
        if (annotateMode) {
          const { AnnotatedProseBlock } = annotateMode
          return (
            <AnnotatedProseBlock
              key={i}
              pieces={b.pieces}
              scrollTo={scrollToBlock === i}
              annotations={annotations.filter((a) => a.block === i)}
              onEdited={(edited, original) => annotateMode.saveEdit(page, i, original, edited)}
            />
          )
        }
        return <ProseBlock key={i} pieces={b.pieces} />
      })}
    </article>
  )
}

function ProseBlock({ pieces }: { pieces: Piece[] }) {
  const html = useMemo(() => pieces.map((p) => p.html).join(""), [pieces])
  return <div dangerouslySetInnerHTML={{ __html: html }} />
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
