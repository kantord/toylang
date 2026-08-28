import { useEffect, useMemo, useRef } from "react"
import type { Tokens } from "marked"

import { Code } from "@/components/Code"
import type { Annotation, AnnotationType, Piece } from "@/lib/blocks"
import { annotationIn, pageAnnotations, splitBlocks } from "@/lib/blocks"
import type { Corpus } from "@/lib/corpus"
import type { Page } from "@/lib/docs"
import { cn } from "@/lib/utils"

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
  const annotations = useMemo(() => (annotate ? pageAnnotations(page, blocks) : []), [annotate, page, blocks])

  return (
    <article className="docs-prose min-w-0 max-w-2xl">
      {blocks.map((b, i) =>
        b.kind === "html" ? (
          <ProseBlock
            key={i}
            pieces={b.pieces}
            editable={annotate}
            scrollTo={annotate && scrollToBlock === i}
            annotations={annotations.filter((a) => a.block === i)}
            onEdited={annotate ? (edited, original) => saveEdit(page, i, original, edited) : undefined}
          />
        ) : (
          <Fence key={i} token={b.token} corpus={corpus} />
        ),
      )}
    </article>
  )
}

const HIGHLIGHT: Record<AnnotationType, string> = {
  review: "bg-amber-300/25 dark:bg-amber-400/20",
  comment: "bg-sky-300/25 dark:bg-sky-400/20",
  fill: "bg-fuchsia-300/25 dark:bg-fuchsia-400/20",
}

const NOTE_LABEL: Record<AnnotationType, string> = {
  review: "review",
  comment: "comment",
  fill: "fill in",
}

const SAVE_DEBOUNCE_MS = 800

function saveEdit(page: Page, block: number, original: string, edited: string) {
  fetch("/__annotations/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ page: page.path, block, original, edited }),
  }).catch((e) => console.error("annotation autosave failed", e))
}

/** Injects a class onto a piece's own root tag rather than wrapping it, so an annotated
 *  paragraph or heading stays a direct sibling of its neighbors and the run's normal-mode
 *  spacing rules (`.docs-prose p + p`, etc.) still apply. */
function washPiece(html: string, type: AnnotationType | undefined): string {
  if (!type) return html
  return html.replace(/^(\s*<[a-zA-Z0-9]+)/, `$1 class="${HIGHLIGHT[type]}"`)
}

function ProseBlock({
  pieces,
  editable,
  scrollTo,
  annotations,
  onEdited,
}: {
  pieces: Piece[]
  editable: boolean
  scrollTo: boolean
  annotations: Annotation[]
  onEdited?: (edited: string, original: string) => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const timer = useRef<number | undefined>(undefined)
  const plainHtml = useMemo(() => pieces.map((p) => p.html).join(""), [pieces])
  const annotatedHtml = useMemo(
    () => pieces.map((p) => washPiece(p.html, annotationIn(p.annotateRaw))).join(""),
    [pieces],
  )
  // The pristine rendered text, captured once from the markdown-derived html rather than the
  // live (editable) DOM, so it stays the "before" side no matter how much later editing happens.
  const original = useMemo(
    () => new DOMParser().parseFromString(plainHtml, "text/html").body.textContent ?? "",
    [plainHtml],
  )

  useEffect(() => {
    if (scrollTo) ref.current?.scrollIntoView({ behavior: "smooth", block: "center" })
  }, [scrollTo])

  if (!editable) {
    return <div dangerouslySetInnerHTML={{ __html: plainHtml }} />
  }

  return (
    <div className={cn("rounded-sm", scrollTo && "ring-2 ring-primary")}>
      {annotations.map((a, i) => (
        <div key={i} className="mb-1 inline-block rounded px-1.5 py-0.5 text-[11px] font-medium text-foreground/70">
          {NOTE_LABEL[a.type]}: {a.note}
        </div>
      ))}
      <div
        ref={ref}
        contentEditable
        suppressContentEditableWarning
        className="outline-none focus:bg-background/60"
        onInput={() => {
          window.clearTimeout(timer.current)
          timer.current = window.setTimeout(() => {
            onEdited?.(ref.current?.innerText ?? "", original)
          }, SAVE_DEBOUNCE_MS)
        }}
        dangerouslySetInnerHTML={{ __html: annotatedHtml }}
      />
    </div>
  )
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
