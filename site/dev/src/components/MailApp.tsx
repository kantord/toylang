import { Archive, Check, Inbox as InboxIcon, LayoutDashboard, Pencil, StickyNote, X } from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"

import { BoardPage } from "@dev/components/BoardPage"
import { GrillRoundReader } from "@dev/components/GrillWizard"
import { FLOW, MessageCard } from "@dev/components/MessageCard"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import { pageAnnotations, type Annotation } from "@dev/lib/annotations"
import { fetchRound, fetchRoundTopics, roundPagePath, type Round } from "@dev/lib/grill"
import { annotateHref } from "@dev/lib/nav"
import { splitBlocks } from "@/lib/blocks"
import { PAGES } from "@/lib/docs"
import {
  annotationItem,
  composeItem,
  fetchNotesAndComposed,
  grillItem,
  noteItem,
  sendCompose,
  type ComposeRecord,
  type Folder,
  type MailItem,
  type NoteRecord,
  type Preview,
} from "@dev/lib/mail"
import { cn } from "@/lib/utils"

/**
 * Everything is email (kantord/toylang#52): the annotations inbox, the maintainer's own notes
 * and composed messages, grilling rounds, and the board all live in this one app now, gmail-
 * style -- a folder rail, a table-ish message list stacked *over* the reading pane (not beside
 * it), and a floating Compose button. A round arrives as mail and is answered right there in the
 * reading pane; the board is a tab of the same rail rather than a route of its own. Replaces the
 * three-pane side-by-side layout from kantord/toylang#41, which put the list and reading pane
 * next to each other instead of stacked.
 */

interface InboxRecord {
  page: string
  block: number
}

type Tab = Folder | "board"

const TABS: { key: Tab; label: string; icon: typeof InboxIcon }[] = [
  { key: "inbox", label: "Inbox", icon: InboxIcon },
  { key: "notes", label: "Your notes & composed", icon: StickyNote },
  { key: "archive", label: "Archive", icon: Archive },
  { key: "board", label: "Board", icon: LayoutDashboard },
]

/** Subfolders inside Inbox (gh:57): "action" is the default view and isn't a flow of its own --
 *  it's every flow except `status`, the wash of acknowledgements and FYIs that never need a
 *  reply. The other four line up with the flows an inbox item can actually carry (never
 *  `reply`, which only annotation items in the Notes folder use). */
type InboxCategory = "action" | "escalation" | "question" | "round" | "status"

const INBOX_CATEGORIES: { key: InboxCategory; label: string }[] = [
  { key: "action", label: "Needs response" },
  { key: "escalation", label: FLOW.escalation.label },
  { key: "question", label: FLOW.question.label },
  { key: "round", label: "Grill rounds" },
  { key: "status", label: FLOW.status.label },
]

/** Every answered `page:block` key, for sorting inbox items into Inbox vs. Archive -- same
 *  read/unread split AnnotationsSidebar used (kantord/toylang#30), and reused as-is for grilling
 *  rounds (kantord/toylang#52): a round's answers land in this exact store, keyed by the round
 *  file's path and each question's index, so one fetch and one local set covers both. Tracked
 *  locally so answering an item moves it the instant it happens rather than waiting on a
 *  refetch. */
function useAnsweredKeys(): [Set<string>, (key: string) => void] {
  const [answered, setAnswered] = useState<Set<string>>(new Set())
  useEffect(() => {
    fetch("/__annotations/inbox-all")
      .then((r) => (r.ok ? (r.json() as Promise<{ records: InboxRecord[] }>) : { records: [] }))
      .then(({ records }) => setAnswered(new Set(records.map((r) => `${r.page}:${r.block}`))))
      .catch(() => {
        // No inbox endpoint reachable -- every item just reads as unanswered.
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

/** Every grilling round waiting in `docs/.grill/` (kantord/toylang#34), fetched once up front so
 *  the inbox list has each round's question count and content ready before the maintainer opens
 *  one -- the reading pane never shows its own loading state. Soft-fails to no rounds, matching
 *  every other list here: a dev server with no `/__grill` endpoint just means nothing to grill. */
function useGrillRounds(): { topic: string; round: Round }[] {
  const [rounds, setRounds] = useState<{ topic: string; round: Round }[]>([])
  useEffect(() => {
    fetchRoundTopics()
      .then((topics) => Promise.all(topics.map((t) => fetchRound(t).then((round) => ({ topic: t, round })))))
      .then(setRounds)
      .catch(() => {
        // No grill endpoint reachable -- the inbox just has no rounds.
      })
  }, [])
  return rounds
}

export function MailApp() {
  const annotations = useMemo(() => PAGES.flatMap((p) => pageAnnotations(p, splitBlocks(p))), [])
  const grillRounds = useGrillRounds()
  const [answeredKeys, markReadLocally] = useAnsweredKeys()
  const [notes, setNotes] = useState<NoteRecord[]>([])
  const [composed, setComposed] = useState<ComposeRecord[]>([])
  const [tab, setTab] = useState<Tab>("inbox")
  const [inboxCategory, setInboxCategory] = useState<InboxCategory>("action")
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
    const fromGrill = grillRounds.map(({ topic, round }) => {
      const page = roundPagePath(topic)
      const answered = round.questions.every((_, i) => answeredKeys.has(`${page}:${i}`))
      return grillItem(topic, round, answered)
    })
    const fromComposed = [...composed]
      .sort((a, b) => b.created.localeCompare(a.created))
      .map(composeItem)
    const fromNotes = notes.map(noteItem)
    return [...fromAnnotations, ...fromGrill, ...fromComposed, ...fromNotes]
  }, [annotations, grillRounds, answeredKeys, notes, composed])

  const inboxItems = useMemo(() => items.filter((i) => i.folder === "inbox"), [items])
  const folderItems = useMemo(
    () => (tab === "board" ? [] : items.filter((i) => i.folder === tab)),
    [items, tab],
  )
  // The action-first inbox (gh:57): the default "Needs response" subfolder drops every `status`
  // item -- acknowledgements and FYIs -- so the list is only things that actually want the
  // maintainer to do something. The other subfolders are one flow each, `status` included, for
  // when the maintainer wants to see the noise on purpose.
  const visible = useMemo(
    () =>
      tab === "inbox"
        ? folderItems.filter((i) => (inboxCategory === "action" ? i.flow !== "status" : i.flow === inboxCategory))
        : folderItems,
    [tab, folderItems, inboxCategory],
  )
  const categoryCounts = useMemo<Record<InboxCategory, number>>(
    () => ({
      action: inboxItems.filter((i) => i.flow !== "status").length,
      escalation: inboxItems.filter((i) => i.flow === "escalation").length,
      question: inboxItems.filter((i) => i.flow === "question").length,
      round: inboxItems.filter((i) => i.flow === "round").length,
      status: inboxItems.filter((i) => i.flow === "status").length,
    }),
    [inboxItems],
  )
  // The Inbox tab's own badge counts the same "needs response" set as its default view, not
  // every item sitting in the folder -- otherwise the number itself would be status noise.
  const counts = useMemo<Record<Folder, number>>(
    () => ({
      inbox: categoryCounts.action,
      notes: items.filter((i) => i.folder === "notes").length,
      archive: items.filter((i) => i.folder === "archive").length,
    }),
    [categoryCounts, items],
  )
  // Looked up from `items`, not `visible`: answering an inbox item moves it into Archive
  // mid-session, and the reading pane should keep showing it rather than going blank just
  // because it left the folder still on screen.
  const open = items.find((i) => i.key === openKey) ?? null

  // `visible` can shrink out from under a stale focusIndex -- an item answered mid-session
  // leaves the folder without going through j/k or selectCategory/selectTab, so Enter would
  // silently no-op against an out-of-range index until the next arrow key.
  useEffect(() => {
    setFocusIndex((i) => Math.min(i, Math.max(0, visible.length - 1)))
  }, [visible.length])

  const selectTab = (t: Tab) => {
    setTab(t)
    setInboxCategory("action")
    setFocusIndex(0)
    setOpenKey(null)
  }

  const selectCategory = (c: InboxCategory) => {
    setInboxCategory(c)
    setFocusIndex(0)
  }

  // j/k and the arrow keys move a cursor through the current folder's list, matching a mail
  // client's keyboard flow; Enter opens whatever the cursor is on into the reading pane. Ignored
  // while a text field (the compose panel, most likely) has focus, so typing "j" into a message
  // body never gets eaten as a list-navigation key, and skipped entirely on the Board tab, which
  // has no list to move a cursor through.
  useEffect(() => {
    if (tab === "board") return
    const onKey = (e: KeyboardEvent) => {
      const t = (e.target as HTMLElement | null)?.tagName
      if (t === "INPUT" || t === "TEXTAREA") return
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
  }, [tab, visible, focusIndex])

  const onMarkRead = (a: Annotation) => {
    markReadLocally(`${a.page.path}:${a.block}`)
    markAsRead(a)
  }

  const onRoundAnswered = (topic: string, questionCount: number) => {
    const page = roundPagePath(topic)
    for (let i = 0; i < questionCount; i++) markReadLocally(`${page}:${i}`)
  }

  const onSendCompose = async (subject: string, body: string) => {
    const record = await sendCompose(subject, body)
    setComposed((prev) => [...prev, record])
    setComposeOpen(false)
    selectTab("notes")
  }

  return (
    <div className="relative grid min-h-0 flex-1 gap-4 lg:grid-cols-[180px_minmax(0,1fr)]">
      <aside className="space-y-1">
        {TABS.map((t) => (
          <div key={t.key}>
            <button
              type="button"
              onClick={() => selectTab(t.key)}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm",
                tab === t.key ? "bg-muted font-medium text-foreground" : "text-muted-foreground hover:bg-muted/60",
              )}
            >
              <t.icon className="size-4 shrink-0" />
              <span className="flex-1 truncate">{t.label}</span>
              {t.key !== "board" && <span className="text-xs text-muted-foreground">{counts[t.key]}</span>}
            </button>
            {t.key === "inbox" && tab === "inbox" && (
              <div className="ml-6 space-y-0.5 border-l pl-2">
                {INBOX_CATEGORIES.map((c) => (
                  <button
                    key={c.key}
                    type="button"
                    onClick={() => selectCategory(c.key)}
                    className={cn(
                      "flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-xs",
                      inboxCategory === c.key
                        ? "bg-muted font-medium text-foreground"
                        : c.key === "status"
                          ? "text-muted-foreground/60 hover:bg-muted/60"
                          : "text-muted-foreground hover:bg-muted/60",
                    )}
                  >
                    <span className="flex-1 truncate">{c.label}</span>
                    <span>{categoryCounts[c.key]}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        ))}
      </aside>

      {tab === "board" ? (
        <main className="min-h-0 overflow-y-auto rounded-md border p-4">
          <BoardPage />
        </main>
      ) : (
        // Gmail-like over-under (kantord/toylang#52), not the three-pane side-by-side #41 shipped:
        // the list gets a fixed share of the height, the reading pane takes the rest.
        <div className="grid min-h-0 grid-rows-[minmax(0,45%)_minmax(0,1fr)] gap-4">
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
              <ReadingPane item={open} onMarkRead={onMarkRead} onRoundAnswered={onRoundAnswered} />
            ) : (
              <p className="text-sm text-muted-foreground">Select a message.</p>
            )}
          </main>
        </div>
      )}

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

/** Splits `text` around `anchor` into a highlightable three-part snippet, capped at `maxLen` and
 *  centered on the match -- the "preview with the important parts highlighted" the issue asks
 *  for. With no anchor, or one that no longer occurs verbatim, it's just a plain truncation. */
function buildSnippet(text: string, anchor: string | undefined, maxLen = 140): { pre: string; mark: string; post: string } {
  const clean = text.replace(/\s+/g, " ").trim()
  const cleanAnchor = anchor?.replace(/\s+/g, " ").trim()
  const plain = { pre: clean.length > maxLen ? `${clean.slice(0, maxLen)}...` : clean, mark: "", post: "" }
  if (!cleanAnchor) return plain
  const idx = clean.indexOf(cleanAnchor)
  if (idx === -1) return plain

  const budget = Math.max(0, maxLen - cleanAnchor.length)
  const before = Math.floor(budget / 2)
  const after = budget - before
  const start = Math.max(0, idx - before)
  const end = Math.min(clean.length, idx + cleanAnchor.length + after)
  return {
    pre: (start > 0 ? "..." : "") + clean.slice(start, idx),
    mark: cleanAnchor,
    post: clean.slice(idx + cleanAnchor.length, end) + (end < clean.length ? "..." : ""),
  }
}

function Snippet({ preview }: { preview: Preview }) {
  const s = buildSnippet(preview.text, preview.anchor)
  return (
    <>
      {s.pre}
      {s.mark && <mark className="rounded-sm bg-primary/20 px-0.5 text-foreground">{s.mark}</mark>}
      {s.post}
    </>
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
  const flow = FLOW[item.flow]
  // An acknowledgement is never a request for engagement (gh:57): it sinks visually the same
  // way an already-read item does, whether or not it's actually been read yet.
  const ackMuted = item.flow === "status"
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "grid w-full grid-cols-[120px_1fr_14px] items-start gap-3 border-b px-3 py-2 text-left last:border-b-0",
        open ? "bg-muted" : focused ? "bg-muted/50" : "hover:bg-muted/30",
        ackMuted && "opacity-60",
      )}
    >
      <span className="truncate pt-0.5 text-xs font-semibold">{item.sender}</span>
      <span className="min-w-0">
        <span className="flex items-center gap-1.5">
          <Badge variant="outline" className={cn(flow.badge, "shrink-0 border-0 text-[10px]")}>
            {flow.label}
          </Badge>
          <span className="truncate text-xs font-medium">{item.subject}</span>
        </span>
        {item.preview && (
          <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
            <Snippet preview={item.preview} />
          </span>
        )}
      </span>
      <span className="pt-1">
        {item.folder === "inbox" && (
          <span className={cn("block size-1.5 rounded-full", ackMuted ? "bg-muted-foreground/50" : "bg-primary")} />
        )}
      </span>
    </button>
  )
}

function ReadingPane({
  item,
  onMarkRead,
  onRoundAnswered,
}: {
  item: MailItem
  onMarkRead: (a: Annotation) => void
  onRoundAnswered: (topic: string, questionCount: number) => void
}) {
  if (item.round) {
    const { topic, round } = item.round
    return (
      <div className="space-y-3">
        <div className="space-y-1">
          <div className="text-xs text-muted-foreground">From: {item.sender}</div>
          <div className="text-sm font-semibold">{item.subject}</div>
        </div>
        <Separator />
        <GrillRoundReader
          key={topic}
          topic={topic}
          round={round}
          onAllAnswered={() => onRoundAnswered(topic, round.questions.length)}
        />
      </div>
    )
  }

  const a = item.annotation
  return (
    <div className="max-w-2xl space-y-3">
      <div className="space-y-1">
        <div className="text-xs text-muted-foreground">From: {item.sender}</div>
        <div className="text-sm font-semibold">{item.subject}</div>
      </div>
      <Separator />
      {item.preview && (
        <div className="rounded-md bg-muted/40 p-2 text-xs text-muted-foreground">
          <Snippet preview={item.preview} />
        </div>
      )}
      <MessageCard flow={item.flow} note={item.note} muted={item.flow === "status"} />
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
