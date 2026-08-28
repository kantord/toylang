import { useCallback, useEffect, useMemo, useRef, useState } from "react"

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
  // keepalive lets this survive a beforeunload/visibilitychange flush, where the tab can be
  // gone before an ordinary fetch would finish (kantord/toylang#28).
  fetch("/__annotations/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ page: page.path, block, original, edited }),
    keepalive: true,
  }).catch((e) => console.error("annotation autosave failed", e))
}

interface InboxRecord {
  page: string
  block: number
  original: string
  edited: string
}

/** localStorage draft for one block, written on every keystroke rather than debounced, so an
 *  HMR re-render or a reload before the network save fires never loses text that's already been
 *  typed (kantord/toylang#28). Keyed on `original` too: once the source the draft was taken
 *  against no longer matches the live page, the draft is either applied or superseded, and
 *  either way it stops being this block's pending state. */
interface Draft {
  original: string
  edited: string
}

function draftKey(page: Page, block: number): string {
  return `toylang-annotate-draft:${page.path}:${block}`
}

function loadDraft(page: Page, block: number): Draft | null {
  try {
    const raw = localStorage.getItem(draftKey(page, block))
    return raw ? (JSON.parse(raw) as Draft) : null
  } catch {
    return null
  }
}

function saveDraft(page: Page, block: number, original: string, edited: string) {
  try {
    localStorage.setItem(draftKey(page, block), JSON.stringify({ original, edited } satisfies Draft))
  } catch {
    // Private browsing or a full quota -- the debounced network save still covers this block.
  }
}

function clearDraft(page: Page, block: number) {
  try {
    localStorage.removeItem(draftKey(page, block))
  } catch {
    // Nothing to undo if the read/write above never worked either.
  }
}

/** The inbox record for one block, if the coordinator hasn't consumed it yet. Read once per
 *  mount; a fresh reload is exactly when this needs to be current, and localStorage (written on
 *  every keystroke within a session) is what stays current between reloads on the same
 *  browser. */
function useInboxRecord(page: Page, block: number): InboxRecord | null {
  const [record, setRecord] = useState<InboxRecord | null>(null)
  useEffect(() => {
    let cancelled = false
    const query = new URLSearchParams({ page: page.path, block: String(block) })
    fetch(`/__annotations/inbox?${query}`)
      .then((r) => (r.ok ? (r.json() as Promise<{ record: InboxRecord | null }>) : { record: null }))
      .then(({ record }) => {
        if (!cancelled) setRecord(record)
      })
      .catch(() => {
        // No inbox endpoint reachable -- the block just renders pristine, same as before #28.
      })
    return () => {
      cancelled = true
    }
  }, [page.path, block])
  return record
}

/** Injects a class onto a piece's own root tag rather than wrapping it, so an annotated
 *  paragraph or heading stays a direct sibling of its neighbors and the run's normal-mode
 *  spacing rules (`.docs-prose p + p`, etc.) still apply. */
function washPiece(html: string, type: AnnotationType | undefined): string {
  if (!type) return html
  return html.replace(/^(\s*<[a-zA-Z0-9]+)/, `$1 class="${HIGHLIGHT[type]}"`)
}

export function AnnotatedProseBlock({
  page,
  block,
  pieces,
  scrollTo,
  annotations,
  onEdited,
}: {
  page: Page
  block: number
  pieces: Piece[]
  scrollTo: boolean
  annotations: Annotation[]
  onEdited: (edited: string, original: string) => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const timer = useRef<number | undefined>(undefined)
  const editedRef = useRef<string | null>(null)
  const hasTypedRef = useRef(false)
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

  // Read once per (page, block, original): a re-render triggered by the source itself changing
  // (an HMR update to the underlying markdown, or a fresh mount after a full reload) is exactly
  // when the draft needs to be re-checked against the new pristine text -- an ordinary re-render
  // in between (e.g. a sibling toggling) must not re-read localStorage and clobber a keystroke
  // that hasn't made it there yet (kantord/toylang#28).
  const localDraft = useMemo(() => loadDraft(page, block), [page, block, original])
  const inboxRecord = useInboxRecord(page, block)

  // A draft or inbox record only counts as this block's pending state while it was taken
  // against the text currently on screen. Once the coordinator applies it (or the source moves
  // for any other reason), `original` no longer matches, and the block falls back to showing
  // whatever is now pristine -- the pending mark drops exactly when the content it described
  // does (#28c).
  // The inbox fetch is async and can resolve after the user has already started typing (there
  // was no localStorage draft yet to shadow it): once that happens, a late-arriving inbox record
  // must not stomp on live keystrokes just because it wins the race (#28, "nothing typed may be
  // lost").
  const pending = useMemo(() => {
    if (localDraft && localDraft.original === original) return localDraft.edited
    if (!hasTypedRef.current && inboxRecord && inboxRecord.original === original) return inboxRecord.edited
    return null
  }, [localDraft, inboxRecord, original])

  useEffect(() => {
    if (localDraft && localDraft.original !== original) clearDraft(page, block)
  }, [localDraft, original, page, block])

  const flush = useCallback(() => {
    window.clearTimeout(timer.current)
    if (editedRef.current === null) return
    onEdited(editedRef.current, original)
  }, [onEdited, original])

  useEffect(() => {
    if (scrollTo) ref.current?.scrollIntoView({ behavior: "smooth", block: "center" })
  }, [scrollTo])

  // A tab close/navigation or switch-away can land mid-debounce; both flush the pending network
  // save immediately rather than losing it to the 800ms wait (#28a). The localStorage draft
  // itself needs no flush -- onInput already writes it synchronously on every keystroke.
  useEffect(() => {
    const onVisibility = () => {
      if (document.visibilityState === "hidden") flush()
    }
    window.addEventListener("beforeunload", flush)
    document.addEventListener("visibilitychange", onVisibility)
    return () => {
      window.removeEventListener("beforeunload", flush)
      document.removeEventListener("visibilitychange", onVisibility)
    }
  }, [flush])

  const contentProps =
    pending !== null ? { children: pending } : { dangerouslySetInnerHTML: { __html: annotatedHtml } }

  return (
    <div className={cn("rounded-sm", scrollTo && "ring-2 ring-primary")}>
      {annotations.map((a, i) => (
        <div key={i} className="mb-1 inline-block rounded px-1.5 py-0.5 text-[11px] font-medium text-foreground/70">
          {NOTE_LABEL[a.type]}: {a.note}
        </div>
      ))}
      {pending !== null && (
        <div className="mb-1 inline-block rounded border border-dashed px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">
          pending -- not yet applied
        </div>
      )}
      <div
        ref={ref}
        contentEditable
        suppressContentEditableWarning
        className={cn("outline-none focus:bg-background/60", pending !== null && "whitespace-pre-wrap")}
        onInput={() => {
          hasTypedRef.current = true
          const edited = ref.current?.innerText ?? ""
          editedRef.current = edited
          saveDraft(page, block, original, edited)
          window.clearTimeout(timer.current)
          timer.current = window.setTimeout(flush, SAVE_DEBOUNCE_MS)
        }}
        {...contentProps}
      />
    </div>
  )
}
