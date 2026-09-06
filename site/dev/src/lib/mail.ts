/**
 * The mail app's data (kantord/toylang#41): the maintainer's span notes and free-form compose
 * messages, both served by the `/__annotations/note` and `/__annotations/compose` endpoints
 * (vite-plugins/annotations-inbox.ts), plus the `MailItem` shape and the pure mappers that turn
 * annotations, notes, compose messages, grill rounds, and plans awaiting approval into it.
 * Split out of the component so this stays testable without React, matching lib/grill.ts.
 */

import type { FlowType } from "@dev/components/MessageCard"
import { FLOW_FOR_TYPE } from "@dev/components/MessageCard"
import type { Annotation } from "@dev/lib/annotations"
import type { Round } from "@dev/lib/grill"
import type { Plan } from "@dev/lib/plans"

export interface NoteRecord {
  page: string
  block: number
  anchor: string
  note: string
}

/** A message composed with no page or span behind it -- the mail app's Compose button. */
export interface ComposeRecord {
  id: string
  subject: string
  note: string
  created: string
}

export type Folder = "inbox" | "notes" | "archive"

/** A snippet of an item's underlying page or span, for the "preview with highlights" the issue
 *  asks for -- `anchor`, when present, is the exact quoted words to mark inside `text`. */
export interface Preview {
  text: string
  anchor?: string
}

/** One row's worth of normalized display data, whichever of the five underlying records it
 *  came from -- so the list and reading pane render off one shape instead of branching five
 *  ways at every call site. */
export interface MailItem {
  key: string
  folder: Folder
  sender: string
  subject: string
  note: string
  flow: FlowType
  preview: Preview | null
  annotation?: Annotation
  round?: { topic: string; round: Round }
  /** A round file that exists on disk but cannot be served (kantord/toylang#164): the topic names
   *  the file and the error is the server's reason, kept on the row so a bad file is noticed
   *  instead of silently hiding mail. */
  roundError?: { topic: string; error: string }
  plan?: Plan
}

const SUBJECT_MAX_LEN = 60
const NOTE_ANCHOR_MAX_LEN = 40

function truncate(s: string, maxLen: number): string {
  return s.length > maxLen ? `${s.slice(0, maxLen)}...` : s
}

export function annotationItem(a: Annotation, answered: boolean): MailItem {
  return {
    key: `annotation:${a.page.path}:${a.block}`,
    folder: answered ? "archive" : "inbox",
    sender: a.page.title,
    subject: truncate(a.note, SUBJECT_MAX_LEN),
    note: a.note,
    flow: FLOW_FOR_TYPE[a.type],
    preview: { text: a.original, anchor: a.anchor },
    annotation: a,
  }
}

export function noteItem(n: NoteRecord, index: number): MailItem {
  return {
    key: `note:${n.page}:${n.block}:${index}`,
    folder: "notes",
    sender: "You",
    subject: `re: "${truncate(n.anchor, NOTE_ANCHOR_MAX_LEN)}"`,
    note: n.note,
    flow: "reply",
    preview: { text: n.anchor, anchor: n.anchor },
  }
}

export function composeItem(c: ComposeRecord): MailItem {
  return {
    key: `compose:${c.id}`,
    folder: "notes",
    sender: "You",
    subject: c.subject.trim() || "(no subject)",
    note: c.note,
    flow: "reply",
    preview: null,
  }
}

export function grillItem(topic: string, round: Round, answered: boolean): MailItem {
  return {
    key: `grill:${topic}`,
    folder: answered ? "archive" : "inbox",
    sender: "Grilling round",
    subject: topic,
    note: round.intro ?? round.questions[0]?.question ?? "",
    flow: "round",
    preview: null,
    round: { topic, round },
  }
}

/** A round that cannot be rendered: the list keeps its row so the file's existence is visible
 *  even though its content never parsed or served (kantord/toylang#164). Stays a `round`-flow
 *  inbox item -- it is a round the maintainer must fix, so it needs attention like any other. */
export function grillErrorItem(topic: string, error: string): MailItem {
  return {
    key: `grill-error:${topic}`,
    folder: "inbox",
    sender: "Grilling round",
    subject: topic,
    note: error,
    flow: "round",
    preview: { text: error },
    roundError: { topic, error },
  }
}

/** A plan is inbox mail only while it is `proposed` and undecided (kantord/toylang#110). The two
 *  conditions are separate on purpose: `answered` covers the gap between the maintainer clicking
 *  and the coordinator rewriting the frontmatter, and the frontmatter covers the case where the
 *  coordinator has since cleared the inbox record the click left behind. */
export function planItem(plan: Plan, answered: boolean): MailItem {
  const pending = plan.status === "proposed" && !answered
  return {
    key: `plan:${plan.path}`,
    folder: pending ? "inbox" : "archive",
    sender: "Plan",
    subject: plan.title,
    note: pending
      ? "Read it, then approve it or send it back for another planning phase."
      : `Already decided: ${plan.status}.`,
    flow: "plan",
    preview: { text: firstProse(plan.markdown) },
    plan,
  }
}

/** A plan's opening prose, for the list row's one-line preview: the first paragraph that isn't
 *  a heading, since the title is already the row's subject. */
function firstProse(markdown: string): string {
  const para = markdown.split(/\n\s*\n/).find((p) => p.trim() && !p.trim().startsWith("#"))
  return (para ?? "").replace(/\s+/g, " ").trim()
}

export function fetchNotesAndComposed(): Promise<{ notes: NoteRecord[]; composed: ComposeRecord[] }> {
  return fetch("/__annotations/notes-all")
    .then((r) => (r.ok ? (r.json() as Promise<{ records: NoteRecord[]; composed: ComposeRecord[] }>) : { records: [], composed: [] }))
    .then(({ records, composed }) => ({ notes: records, composed }))
    .catch(() => ({ notes: [], composed: [] }))
}

/** Sends a compose message and returns the record as the server persisted it -- the id and
 *  `created` timestamp are the server's, not invented locally, so the mail app's local list
 *  never diverges from notes.json. */
export function sendCompose(subject: string, note: string): Promise<ComposeRecord> {
  return fetch("/__annotations/compose", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ subject, note }),
  }).then(async (res) => {
    if (!res.ok) throw new Error(`compose failed: ${res.statusText}`)
    const { record } = (await res.json()) as { record: ComposeRecord }
    return record
  })
}
