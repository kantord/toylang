import { useEffect, useMemo, useRef } from "react"

import { annotationIn, pageAnnotations, type Annotation, type AnnotationType } from "@/lib/annotations"
import type { Piece } from "@/lib/blocks"
import type { Page } from "@/lib/docs"
import { cn } from "@/lib/utils"

/**
 * The editable half of annotations mode (kantord/toylang#23): the contenteditable prose block,
 * its marker-pen wash, and the autosave into the edit inbox. Loaded only through a dynamic
 * `import.meta.env.DEV` import from Markdown.tsx and App.tsx, so none of it reaches the
 * production bundle -- only the toggle button that would ever turn this mode on does, and that
 * button itself is compiled out of production builds.
 */

export { pageAnnotations }

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

export function saveEdit(page: Page, block: number, original: string, edited: string) {
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

export function AnnotatedProseBlock({
  pieces,
  scrollTo,
  annotations,
  onEdited,
}: {
  pieces: Piece[]
  scrollTo: boolean
  annotations: Annotation[]
  onEdited: (edited: string, original: string) => void
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
            onEdited(ref.current?.innerText ?? "", original)
          }, SAVE_DEBOUNCE_MS)
        }}
        dangerouslySetInnerHTML={{ __html: annotatedHtml }}
      />
    </div>
  )
}
