import { Check } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { FLOW_FOR_TYPE, MessageCard } from "@dev/components/MessageCard"
import { Button } from "@/components/ui/button"
import { annotationsIn, pageAnnotations, type Annotation, type AnnotationType } from "@dev/lib/annotations"
import type { Piece } from "@/lib/blocks"
import type { Page } from "@/lib/docs"
import { cn } from "@/lib/utils"

/**
 * The editable half of annotations mode (kantord/toylang#23): the contenteditable prose block,
 * its marker-pen wash, and the autosave into the edit inbox. Reachable only from
 * dev/src/components/DevDocsPage.tsx (kantord/toylang#50) -- the production build never opens
 * dev/ at all, so none of it reaches that bundle.
 *
 * Two distinct areas live here (kantord/toylang#30): the INBOX is the coordinator's own
 * `@review`/`@comment`/`@fill` annotations, read out of the markdown source and answered by
 * editing the block's text in place (the flow already built for #23/#28, below). AUTHORING is
 * the reverse channel -- the maintainer selects any span of text on any page and leaves a note
 * for the coordinator, with no edit to the page involved. It has its own store
 * (`/__annotations/note`, `docs/.annotations/notes.json`) so a note never gets mistaken for an
 * edited reply.
 */

export { pageAnnotations }

const HIGHLIGHT: Record<AnnotationType, string> = {
  review: "bg-amber-300/25 dark:bg-amber-400/20",
  comment: "bg-sky-300/25 dark:bg-sky-400/20",
  fill: "bg-fuchsia-300/25 dark:bg-fuchsia-400/20",
}

/** The maintainer's own AUTHORING wash: a different color from any INBOX type, so a note the
 *  maintainer left is never mistaken for a question the coordinator asked. */
const NOTE_HIGHLIGHT = "bg-emerald-300/25 dark:bg-emerald-400/20"

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
 *  browser. Its mere presence also means the block has been answered (kantord/toylang#30): the
 *  reply-by-edit flow below is how an INBOX item gets marked read. */
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

interface NoteRecord {
  page: string
  block: number
  anchor: string
  note: string
}

/** The AUTHORING side (kantord/toylang#30): notes the maintainer has already left on this
 *  block, plus a way to add one. Kept separate from `useInboxRecord` above -- a note is not a
 *  reply, and posting one never touches the edit-inbox store. */
function useBlockNotes(page: Page, block: number): [NoteRecord[], (anchor: string, note: string) => void] {
  const [notes, setNotes] = useState<NoteRecord[]>([])
  useEffect(() => {
    let cancelled = false
    const query = new URLSearchParams({ page: page.path, block: String(block) })
    fetch(`/__annotations/note?${query}`)
      .then((r) => (r.ok ? (r.json() as Promise<{ records: NoteRecord[] }>) : { records: [] }))
      .then(({ records }) => {
        if (!cancelled) setNotes(records)
      })
      .catch(() => {
        // No inbox endpoint reachable -- annotate mode still works, just with no saved notes.
      })
    return () => {
      cancelled = true
    }
  }, [page.path, block])

  const addNote = useCallback(
    (anchor: string, note: string) => {
      const record: NoteRecord = { page: page.path, block, anchor, note }
      setNotes((prev) => [...prev.filter((r) => r.anchor !== anchor), record])
      fetch("/__annotations/note", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(record),
        keepalive: true,
      }).catch((e) => console.error("annotation note save failed", e))
    },
    [page.path, block],
  )

  return [notes, addNote]
}

interface Highlight {
  anchor?: string
  className: string
}

/** Washes one piece's rendered HTML with its highlights. An anchored highlight wraps only the
 *  first occurrence of its exact text, walking the piece's own text nodes so the mark lands on
 *  the right words even through nested inline markup (kantord/toylang#30); an anchor that no
 *  longer occurs verbatim (the source moved since it was captured) is dropped rather than
 *  falling back to the whole piece, which would mislabel unrelated text. An unanchored highlight
 *  -- a coordinator comment on the whole piece, still allowed -- injects a class onto the
 *  piece's own root tag instead, so an annotated paragraph or heading stays a direct sibling of
 *  its neighbors and the run's normal-mode spacing rules (`.docs-prose p + p`, etc.) still
 *  apply. */
function washPiece(html: string, highlights: Highlight[]): { html: string; placed: Set<string> } {
  const placed = new Set<string>()
  if (highlights.length === 0) return { html, placed }
  let out = html
  const spans = highlights.filter((h): h is { anchor: string; className: string } => !!h.anchor)
  if (spans.length > 0) {
    const root = document.createElement("div")
    root.innerHTML = out
    for (const { anchor, className } of spans) {
      // Skip text already inside a mark this same wash placed: two annotations can quote
      // overlapping or identical text (kantord/toylang#30, the highlight-bleed bug), and without
      // this a later anchor would wrap itself around an earlier mark's own text, nesting two
      // highlight colors into one blob rather than each annotation owning its own span.
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
        acceptNode: (n) => (n.parentElement?.closest("mark") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
      })
      let node: Text | null
      while ((node = walker.nextNode() as Text | null)) {
        const idx = node.data.indexOf(anchor)
        if (idx === -1) continue
        const match = node.splitText(idx)
        match.splitText(anchor.length)
        const mark = document.createElement("mark")
        mark.className = className
        mark.appendChild(match.cloneNode(true))
        match.replaceWith(mark)
        placed.add(anchor)
        break
      }
    }
    out = root.innerHTML
  }
  const wholePiece = highlights.find((h) => !h.anchor)
  if (wholePiece) out = out.replace(/^(\s*<[a-zA-Z0-9]+)/, `$1 class="${wholePiece.className}"`)
  return { html: out, placed }
}

/** The live text selection, if it's a non-empty range inside `root` -- what an AUTHORING note
 *  anchors to. `rect` positions the "+ note" affordance next to it. */
function selectionIn(root: HTMLElement): { text: string; rect: DOMRect } | null {
  const sel = window.getSelection()
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null
  const range = sel.getRangeAt(0)
  if (!root.contains(range.commonAncestorContainer)) return null
  const text = sel.toString().trim()
  return text ? { text, rect: range.getBoundingClientRect() } : null
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
  const [notes, addNote] = useBlockNotes(page, block)
  // Each note's anchor is claimed by the first piece it's found in, via `washPiece`'s own
  // text-node search, so the same selected text never lights up twice across a run.
  const annotatedHtml = useMemo(() => {
    const remaining = new Map(notes.map((n) => [n.anchor, n]))
    const htmls = pieces.map((p) => {
      const highlights: Highlight[] = [
        ...annotationsIn(p.annotateRaw).map((a) => ({ anchor: a.anchor, className: HIGHLIGHT[a.type] })),
        ...[...remaining.values()].map((n) => ({ anchor: n.anchor, className: NOTE_HIGHLIGHT })),
      ]
      const { html, placed } = washPiece(p.html, highlights)
      placed.forEach((a) => remaining.delete(a))
      return html
    })
    return htmls.join("")
  }, [pieces, notes])
  // The pristine rendered text, captured once from the markdown-derived html rather than the
  // live (editable) DOM, so it stays the "before" side no matter how much later editing happens.
  const original = useMemo(
    () => new DOMParser().parseFromString(plainHtml, "text/html").body.textContent ?? "",
    [plainHtml],
  )

  const [pendingSelection, setPendingSelection] = useState<{ text: string; x: number; y: number } | null>(null)
  const [noteFormOpen, setNoteFormOpen] = useState(false)
  const [noteDraft, setNoteDraft] = useState("")
  const widgetRef = useRef<HTMLDivElement>(null)
  // An inbox record means the coordinator's own delivery already picked up this block's reply;
  // `justAnswered` covers the gap before that round-trip, both for an ordinary typed reply and
  // for "mark as read" below (kantord/toylang#30 scope addition: read state must flip instantly,
  // not only once the fetch that built `inboxRecord` on mount happens to catch up).
  const [justAnswered, setJustAnswered] = useState(false)

  // Selecting text is how the maintainer starts an AUTHORING note (kantord/toylang#30) --
  // separate from typing, which is a reply to the coordinator's own INBOX annotation. A mouseup
  // on the "+ note" affordance or the form itself is not a change of mind about the selection,
  // even though the browser's own selection can read back empty by then (a re-render can rebuild
  // the prose block's DOM between the original mouseup and this one, and a Selection whose
  // anchor node no longer exists just reads empty) -- so those clicks are ignored here rather
  // than trusted to clear `pendingSelection`.
  useEffect(() => {
    const onSelect = (e: MouseEvent) => {
      if (widgetRef.current?.contains(e.target as Node)) return
      if (noteFormOpen || !ref.current) return
      const sel = selectionIn(ref.current)
      if (!sel) {
        setPendingSelection(null)
        return
      }
      setPendingSelection({ text: sel.text, x: sel.rect.left + sel.rect.width / 2, y: sel.rect.top })
    }
    document.addEventListener("mouseup", onSelect)
    return () => document.removeEventListener("mouseup", onSelect)
  }, [noteFormOpen])

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

  const answered = inboxRecord !== null || justAnswered

  // The email send-button equivalent (kantord/toylang#30): commits the reply right away,
  // skipping the 800ms debounce that exists to coalesce keystrokes, not to gate an explicit
  // "I'm done" click. With nothing typed yet, it sends the pristine text back as its own
  // reply -- an acknowledgment with no edit, which is a legitimate answer to a review/comment
  // annotation that doesn't call for a text change.
  const markAsRead = useCallback(() => {
    window.clearTimeout(timer.current)
    onEdited(editedRef.current ?? original, original)
    setJustAnswered(true)
  }, [onEdited, original])

  return (
    <div className={cn("relative rounded-sm", scrollTo && "ring-2 ring-primary")}>
      {annotations.length > 0 && (
        <div className="mb-1 flex flex-wrap items-start gap-1.5">
          <div className="flex-1 space-y-1">
            {annotations.map((a, i) => (
              <MessageCard key={i} flow={FLOW_FOR_TYPE[a.type]} note={a.note} dense muted={answered} />
            ))}
          </div>
          {!answered && (
            <Button type="button" variant="outline" size="xs" onClick={markAsRead}>
              <Check /> mark as read
            </Button>
          )}
        </div>
      )}
      {notes.map((n, i) => (
        <div key={i} className="mb-1">
          <MessageCard flow="reply" note={n.note} dense />
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
          // The maintainer typing a reply is answering, full stop (kantord/toylang#30): read
          // state flips the moment that's true, not only once the debounced network save lands.
          setJustAnswered(true)
          window.clearTimeout(timer.current)
          timer.current = window.setTimeout(flush, SAVE_DEBOUNCE_MS)
        }}
        {...contentProps}
      />
      {pendingSelection && (
        <div
          ref={widgetRef}
          className="fixed z-50 -translate-x-1/2 -translate-y-full"
          style={{ left: pendingSelection.x, top: pendingSelection.y - 6 }}
        >
          {!noteFormOpen ? (
            <button
              type="button"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => setNoteFormOpen(true)}
              className="rounded border bg-background px-2 py-1 text-[11px] font-medium shadow-sm hover:bg-muted"
            >
              + note
            </button>
          ) : (
            <div className="flex w-64 flex-col gap-1.5 rounded border bg-background p-2 shadow-sm">
              <p className="truncate text-[11px] text-muted-foreground">"{pendingSelection.text}"</p>
              <textarea
                autoFocus
                value={noteDraft}
                onChange={(e) => setNoteDraft(e.target.value)}
                placeholder="Note to the coordinator"
                className="min-h-16 resize-none rounded border px-1.5 py-1 text-xs outline-none"
              />
              <div className="flex justify-end gap-1">
                <button
                  type="button"
                  className="rounded px-2 py-0.5 text-[11px] text-muted-foreground hover:text-foreground"
                  onClick={() => {
                    setNoteFormOpen(false)
                    setPendingSelection(null)
                    setNoteDraft("")
                  }}
                >
                  cancel
                </button>
                <button
                  type="button"
                  disabled={!noteDraft.trim()}
                  className="rounded bg-primary px-2 py-0.5 text-[11px] font-medium text-primary-foreground disabled:opacity-50"
                  onClick={() => {
                    addNote(pendingSelection.text, noteDraft.trim())
                    setNoteFormOpen(false)
                    setPendingSelection(null)
                    setNoteDraft("")
                  }}
                >
                  save
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
