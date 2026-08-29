import { Archive, Check, Inbox as InboxIcon, Pencil, StickyNote, X } from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"

import { FLOW_FOR_TYPE, MessageCard, type FlowType } from "@dev/components/MessageCard"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import { pageAnnotations, type Annotation } from "@dev/lib/annotations"
import { annotateHref } from "@dev/lib/nav"
import { splitBlocks } from "@/lib/blocks"
import { PAGES } from "@/lib/docs"
import { fetchNotesAndComposed, sendCompose, type ComposeRecord, type NoteRecord } from "@dev/lib/mail"
import { cn } from "@/lib/utils"

/**
 * The annotations area as a mail client (kantord/toylang#41), replacing AnnotationsSidebar's
 * cramped per-page nav list with a dedicated three-pane app: a folder rail, a message list, and
 * a reading pane, plus a floating Compose button for a message with no page or span behind it.
 * The actual answering mechanism (contenteditable reply, mark-as-read, span-note authoring)
 * stays where it was built, in AnnotateMode.tsx on the doc page itself -- this app is the
 * triage layer on top of it, matching how a mail client's list is a view over messages that
 * live, and get replied to, somewhere else.
 */

interface InboxRecord {
  page: string
  block: number
}

type Folder = "inbox" | "notes" | "archive"

const FOLDERS: { key: Folder; label: string; icon: typeof InboxIcon }[] = [
  { key: "inbox", label: "Inbox", icon: InboxIcon },
  { key: "notes", label: "Your notes & composed", icon: StickyNote },
  { key: "archive", label: "Archive", icon: Archive },
]

/** One row's worth of normalized display data, whichever of the three underlying records it
 *  came from -- so the list and reading pane render off one shape instead of branching three
 *  ways at every call site. */
interface MailItem {
  key: string
  folder: Folder
  sender: string
  subject: string
  note: string
  flow: FlowType
  annotation?: Annotation
}

function annotationItem(a: Annotation, answered: boolean): MailItem {
  return {
    key: `annotation:${a.page.path}:${a.block}`,
    folder: answered ? "archive" : "inbox",
    sender: a.page.title,
    subject: a.note.length > 60 ? `${a.note.slice(0, 60)}...` : a.note,
    note: a.note,
    flow: FLOW_FOR_TYPE[a.type],
    annotation: a,
  }
}

function noteItem(n: NoteRecord, index: number): MailItem {
  return {
    key: `note:${n.page}:${n.block}:${index}`,
    folder: "notes",
    sender: "You",
    subject: `re: "${n.anchor.length > 40 ? `${n.anchor.slice(0, 40)}...` : n.anchor}"`,
    note: n.note,
    flow: "reply",
  }
}

function composeItem(c: ComposeRecord): MailItem {
  return {
    key: `compose:${c.id}`,
    folder: "notes",
    sender: "You",
    subject: c.subject.trim() || "(no subject)",
    note: c.note,
    flow: "reply",
  }
}

/** Every reply record, for sorting inbox items into Inbox vs. Archive -- same read/unread split
 *  AnnotationsSidebar used (kantord/toylang#30), tracked locally so mark-as-read moves an item
 *  the instant it's clicked rather than waiting on a refetch. */
function useAnsweredKeys(): [Set<string>, (key: string) => void] {
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

function markAsRead(a: Annotation) {
  fetch("/__annotations/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ page: a.page.path, block: a.block, original: a.original, edited: a.original }),
    keepalive: true,
  }).catch((e) => console.error("annotation mark-as-read failed", e))
}

export function MailApp() {
  const annotations = useMemo(() => PAGES.flatMap((p) => pageAnnotations(p, splitBlocks(p))), [])
  const [answeredKeys, markReadLocally] = useAnsweredKeys()
  const [notes, setNotes] = useState<NoteRecord[]>([])
  const [composed, setComposed] = useState<ComposeRecord[]>([])
  const [folder, setFolder] = useState<Folder>("inbox")
  const [focusIndex, setFocusIndex] = useState(0)
  const [openKey, setOpenKey] = useState<string | null>(null)
  const [composeOpen, setComposeOpen] = useState(false)

  useEffect(() => {
    fetchNotesAndComposed().then(({ notes, composed }) => {
      setNotes(notes)
      setComposed(composed)
    })
  }, [])

  const items = useMemo<MailItem[]>(() => {
    const fromAnnotations = annotations.map((a) => annotationItem(a, answeredKeys.has(`${a.page.path}:${a.block}`)))
    const fromComposed = [...composed]
      .sort((a, b) => b.created.localeCompare(a.created))
      .map(composeItem)
    const fromNotes = notes.map(noteItem)
    return [...fromAnnotations, ...fromComposed, ...fromNotes]
  }, [annotations, answeredKeys, notes, composed])

  const visible = items.filter((i) => i.folder === folder)
  const counts: Record<Folder, number> = {
    inbox: items.filter((i) => i.folder === "inbox").length,
    notes: items.filter((i) => i.folder === "notes").length,
    archive: items.filter((i) => i.folder === "archive").length,
  }
  // Looked up from `items`, not `visible`: marking an inbox item read moves it into Archive
  // mid-session, and the reading pane should keep showing it rather than going blank just
  // because it left the folder still on screen.
  const open = items.find((i) => i.key === openKey) ?? null

  const selectFolder = (f: Folder) => {
    setFolder(f)
    setFocusIndex(0)
    setOpenKey(null)
  }

  // j/k and the arrow keys move a cursor through the current folder's list, matching a mail
  // client's keyboard flow; Enter opens whatever the cursor is on into the reading pane. Ignored
  // while a text field (the compose panel, most likely) has focus, so typing "j" into a message
  // body never gets eaten as a list-navigation key.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName
      if (tag === "INPUT" || tag === "TEXTAREA") return
      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault()
        setFocusIndex((i) => Math.min(i + 1, visible.length - 1))
      } else if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault()
        setFocusIndex((i) => Math.max(i - 1, 0))
      } else if (e.key === "Enter") {
        const item = visible[focusIndex]
        if (item) setOpenKey(item.key)
      }
    }
    document.addEventListener("keydown", onKey)
    return () => document.removeEventListener("keydown", onKey)
  }, [visible, focusIndex])

  const onMarkRead = (a: Annotation) => {
    markReadLocally(`${a.page.path}:${a.block}`)
    markAsRead(a)
  }

  const onSendCompose = async (subject: string, body: string) => {
    await sendCompose(subject, body)
    setComposed((prev) => [...prev, { id: crypto.randomUUID(), subject, note: body, created: new Date().toISOString() }])
    setComposeOpen(false)
    selectFolder("notes")
  }

  return (
    <div className="relative grid min-h-0 flex-1 gap-4 lg:grid-cols-[180px_280px_minmax(0,1fr)]">
      <aside className="space-y-1">
        {FOLDERS.map((f) => (
          <button
            key={f.key}
            type="button"
            onClick={() => selectFolder(f.key)}
            className={cn(
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm",
              folder === f.key ? "bg-muted font-medium text-foreground" : "text-muted-foreground hover:bg-muted/60",
            )}
          >
            <f.icon className="size-4 shrink-0" />
            <span className="flex-1 truncate">{f.label}</span>
            <span className="text-xs text-muted-foreground">{counts[f.key]}</span>
          </button>
        ))}
      </aside>

      <div className="min-h-0 overflow-y-auto rounded-md border">
        {visible.length === 0 ? (
          <p className="p-3 text-sm text-muted-foreground">Nothing here.</p>
        ) : (
          visible.map((item, i) => (
            <MailRow
              key={item.key}
              item={item}
              focused={i === focusIndex}
              open={item.key === openKey}
              onClick={() => {
                setFocusIndex(i)
                setOpenKey(item.key)
              }}
            />
          ))
        )}
      </div>

      <main className="min-h-0 overflow-y-auto rounded-md border p-4">
        {open ? (
          <ReadingPane item={open} onMarkRead={onMarkRead} />
        ) : (
          <p className="text-sm text-muted-foreground">Select a message.</p>
        )}
      </main>

      <div className="fixed bottom-6 right-6">
        {composeOpen ? (
          <ComposePanel onSend={onSendCompose} onCancel={() => setComposeOpen(false)} />
        ) : (
          <Button size="lg" className="shadow-lg" onClick={() => setComposeOpen(true)}>
            <Pencil /> Compose
          </Button>
        )}
      </div>
    </div>
  )
}

function MailRow({
  item,
  focused,
  open,
  onClick,
}: {
  item: MailItem
  focused: boolean
  open: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "block w-full border-b px-3 py-2 text-left last:border-b-0",
        open ? "bg-muted" : focused ? "bg-muted/50" : "hover:bg-muted/30",
      )}
    >
      <div className="flex items-baseline justify-between gap-2">
        <span className="truncate text-xs font-semibold">{item.sender}</span>
        {item.folder === "inbox" && <span className="size-1.5 shrink-0 rounded-full bg-primary" />}
      </div>
      <div className="truncate text-xs text-muted-foreground">{item.subject}</div>
    </button>
  )
}

function ReadingPane({
  item,
  onMarkRead,
}: {
  item: MailItem
  onMarkRead: (a: Annotation) => void
}) {
  const a = item.annotation
  return (
    <div className="max-w-2xl space-y-3">
      <div className="space-y-1">
        <div className="text-xs text-muted-foreground">From: {item.sender}</div>
        <div className="text-sm font-semibold">{item.subject}</div>
      </div>
      <Separator />
      <MessageCard flow={item.flow} note={item.note} />
      {a && (
        <div className="flex gap-2 pt-2">
          <a href={`${annotateHref(a.page)}?b=${a.block}`}>
            <Button type="button" variant="outline" size="sm">
              Open on page
            </Button>
          </a>
          {item.folder === "inbox" && (
            <Button type="button" variant="outline" size="sm" onClick={() => onMarkRead(a)}>
              <Check /> Mark as read
            </Button>
          )}
        </div>
      )}
    </div>
  )
}

function ComposePanel({
  onSend,
  onCancel,
}: {
  onSend: (subject: string, body: string) => Promise<void>
  onCancel: () => void
}) {
  const [subject, setSubject] = useState("")
  const [body, setBody] = useState("")
  const [sending, setSending] = useState(false)
  const [sendError, setSendError] = useState<string | null>(null)
  const bodyRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    bodyRef.current?.focus()
  }, [])

  // A failed delivery must never eat the message or wedge the button (kantord/toylang#46):
  // the draft stays in the textarea, the button re-arms, and the failure is said out loud.
  const send = async () => {
    if (!body.trim()) return
    setSending(true)
    setSendError(null)
    try {
      await onSend(subject.trim(), body.trim())
    } catch (e) {
      setSending(false)
      setSendError(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <Card className="w-80 shadow-xl">
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>New message</CardTitle>
        <button type="button" onClick={onCancel} className="text-muted-foreground hover:text-foreground">
          <X className="size-4" />
        </button>
      </CardHeader>
      <CardContent className="space-y-2">
        <Input placeholder="Subject (optional)" value={subject} onChange={(e) => setSubject(e.target.value)} />
        <Textarea
          ref={bodyRef}
          placeholder="Write to the coordinator (markdown)"
          value={body}
          onChange={(e) => setBody(e.target.value)}
          className="min-h-32"
        />
      </CardContent>
      <CardFooter className="justify-end gap-2 border-t-0 bg-transparent pt-(--card-spacing)">
        {sendError && <p className="mr-auto text-xs text-destructive">Delivery failed: {sendError}. Your draft is kept -- retry.</p>}
        <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="button" size="sm" disabled={!body.trim() || sending} onClick={send}>
          {sending ? "Sending..." : "Send"}
        </Button>
      </CardFooter>
    </Card>
  )
}
