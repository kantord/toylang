import { Check } from "lucide-react"
import { useEffect, useMemo, useState } from "react"

import { FLOW_FOR_TYPE, MessageCard } from "@/components/MessageCard"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { pageAnnotations, type Annotation } from "@/lib/annotations"
import { splitBlocks } from "@/lib/blocks"
import { href, PAGES, type Page } from "@/lib/docs"
import { cn } from "@/lib/utils"

interface InboxRecord {
  page: string
  block: number
}

interface NoteRecord {
  page: string
  block: number
  anchor: string
  note: string
}

/** The blocks the maintainer has already answered (kantord/toylang#30): any inbox reply record
 *  for a block marks every INBOX annotation in it as read, matching the block-granular reply
 *  flow in AnnotateMode.tsx. Fetched once, then kept in sync locally by `markRead` below --
 *  this mode is for a live dev session, not something that needs to poll another tab. */
function useAnsweredBlocks(): [Set<string>, (key: string) => void] {
  const [answered, setAnswered] = useState<Set<string>>(new Set())
  useEffect(() => {
    fetch("/__annotations/inbox-all")
      .then((r) => (r.ok ? (r.json() as Promise<{ records: InboxRecord[] }>) : { records: [] }))
      .then(({ records }) => setAnswered(new Set(records.map((r) => `${r.page}:${r.block}`))))
      .catch(() => {
        // No inbox endpoint reachable -- every annotation just reads as unanswered.
      })
  }, [])
  const markRead = (key: string) => setAnswered((prev) => new Set(prev).add(key))
  return [answered, markRead]
}

/** Every AUTHORING note the maintainer has left, across all pages -- the reverse channel from
 *  the INBOX list below (kantord/toylang#30). */
function useAllNotes(): NoteRecord[] {
  const [notes, setNotes] = useState<NoteRecord[]>([])
  useEffect(() => {
    fetch("/__annotations/notes-all")
      .then((r) => (r.ok ? (r.json() as Promise<{ records: NoteRecord[] }>) : { records: [] }))
      .then(({ records }) => setNotes(records))
      .catch(() => {
        // No inbox endpoint reachable -- nothing sent shows up, same as a fresh checkout.
      })
  }, [])
  return notes
}

/** Commits an acknowledgment straight from the list, no page visit required (kantord/toylang#30,
 *  "the email send-button equivalent"): `edited` equal to `original` is a legitimate answer, the
 *  same no-change acknowledgment AnnotatedProseBlock's own mark-as-read sends. */
function markAsRead(a: Annotation) {
  fetch("/__annotations/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ page: a.page.path, block: a.block, original: a.original, edited: a.original }),
    keepalive: true,
  }).catch((e) => console.error("annotation mark-as-read failed", e))
}

/** Replaces the section nav in annotations mode: the coordinator's INBOX, unanswered first like
 *  unread mail (kantord/toylang#30) with answered items sunk to the bottom, followed by the
 *  maintainer's own AUTHORING notes -- a wholly separate list, since a note is not a reply. Each
 *  row reads like an email client's: sender (page), subject-line note, unread in bold, read
 *  quiet and out of the way. */
export function AnnotationsSidebar({ current }: { current: Page }) {
  const annotations = useMemo(() => PAGES.flatMap((p) => pageAnnotations(p, splitBlocks(p))), [])
  const [answeredBlocks, markRead] = useAnsweredBlocks()
  const notes = useAllNotes()

  const unanswered = annotations.filter((a) => !answeredBlocks.has(`${a.page.path}:${a.block}`))
  const answered = annotations.filter((a) => answeredBlocks.has(`${a.page.path}:${a.block}`))

  const onMarkRead = (a: Annotation) => {
    markRead(`${a.page.path}:${a.block}`)
    markAsRead(a)
  }

  return (
    <div className="space-y-4 text-sm">
      <div>
        <div className="mb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">Inbox</div>
        {unanswered.length === 0 && answered.length === 0 ? (
          <p className="text-sm text-muted-foreground">No annotations yet.</p>
        ) : (
          <nav className="space-y-1">
            {unanswered.map((a, i) => (
              <InboxRow key={i} a={a} current={current} answered={false} onMarkRead={onMarkRead} />
            ))}
            {answered.length > 0 && (
              <>
                {unanswered.length > 0 && <Separator className="my-2" />}
                <div className="pb-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  Answered
                </div>
                {answered.map((a, i) => (
                  <InboxRow key={i} a={a} current={current} answered onMarkRead={onMarkRead} />
                ))}
              </>
            )}
          </nav>
        )}
      </div>

      {notes.length > 0 && (
        <div>
          <div className="mb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">Your notes</div>
          <nav className="space-y-1.5">
            {notes.map((n, i) => {
              const page = PAGES.find((p) => p.path === n.page)
              if (!page) return null
              return (
                <a key={i} href={`${href(page)}?b=${n.block}`} className="block rounded hover:bg-muted">
                  <div className={cn("truncate px-2 pt-1.5 text-xs font-semibold", page === current && "text-primary")}>
                    {page.title}
                  </div>
                  <MessageCard flow="reply" note={n.note} dense />
                </a>
              )
            })}
          </nav>
        </div>
      )}
    </div>
  )
}

/** One inbox row: the sender line (page title, bold while unread) plus the message itself,
 *  and -- unread only -- the mark-as-read affordance, so a row can be cleared without leaving
 *  the list (kantord/toylang#30 scope addition: "the email send-button equivalent"). */
function InboxRow({
  a,
  current,
  answered,
  onMarkRead,
}: {
  a: Annotation
  current: Page
  answered: boolean
  onMarkRead: (a: Annotation) => void
}) {
  return (
    <div className={cn("group rounded", a.page === current && "bg-muted/60")}>
      <a href={`${href(a.page)}?b=${a.block}`} className="block hover:bg-muted">
        <div className="flex items-start justify-between gap-1 px-2 pt-1.5">
          <span className={cn("truncate text-xs", answered ? "font-normal text-muted-foreground" : "font-bold")}>
            {a.page.title}
          </span>
          {!answered && <span className="mt-0.5 size-1.5 shrink-0 rounded-full bg-primary" />}
        </div>
        <MessageCard flow={FLOW_FOR_TYPE[a.type]} note={a.note || a.type} dense muted={answered} />
      </a>
      {!answered && (
        <div className="px-2 pb-1.5">
          <Button
            type="button"
            variant="ghost"
            size="xs"
            className="h-5 px-1.5 text-[11px] text-muted-foreground opacity-0 group-hover:opacity-100"
            onClick={(e) => {
              e.preventDefault()
              onMarkRead(a)
            }}
          >
            <Check /> mark as read
          </Button>
        </div>
      )}
    </div>
  )
}
