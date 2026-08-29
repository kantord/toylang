import { Badge } from "@/components/ui/badge"
import type { AnnotationType } from "@dev/lib/annotations"
import { cn } from "@/lib/utils"

/**
 * Shared shape for every agentic message surface (kantord/toylang#30 design-system addition):
 * an inbox annotation, an AUTHORING note, or a reply all render through this so the maintainer
 * never has to parse which kind of thing they're looking at from typography alone -- a thick
 * left border and a badge carry that instead. Four flow types, not the three annotation
 * types: `review`/`fill`/`comment` describe what the coordinator's comment syntax marks, but
 * `reply` has no comment-syntax equivalent -- it's the maintainer's own note going the other
 * way, so it needs a flow of its own the wash types don't have.
 */
export type FlowType = "question" | "escalation" | "status" | "reply"

/** How a coordinator annotation's comment-syntax type maps to a flow: `@review` is something
 *  that needs the maintainer's ratification (escalation), `@fill` is a direct question with an
 *  expected short answer, and `@comment` is FYI/status ("resolved per your answer", "correct").
 *  Matches how the existing corpus of annotations actually reads (docs/reference/types/record.md,
 *  docs/reference/builtins/concat.md). */
export const FLOW_FOR_TYPE: Record<AnnotationType, FlowType> = {
  review: "escalation",
  fill: "question",
  comment: "status",
}

const FLOW: Record<FlowType, { label: string; border: string; badge: string }> = {
  question: { label: "Question", border: "border-fuchsia-500", badge: "bg-fuchsia-500/15 text-fuchsia-700 dark:text-fuchsia-300" },
  escalation: { label: "Escalation", border: "border-amber-500", badge: "bg-amber-500/15 text-amber-700 dark:text-amber-300" },
  status: { label: "Status", border: "border-sky-500", badge: "bg-sky-500/15 text-sky-700 dark:text-sky-300" },
  reply: { label: "Reply", border: "border-emerald-500", badge: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300" },
}

interface Section {
  label: string | null
  text: string
  action: boolean
}

const LABEL_LINE = /^([A-Za-z][A-Za-z ]{1,20}):\s(.*)$/
const ACTION_LABEL = /^(question|action)$/i
const ACTION_LEAD = /^(Edit|Schedule|Read|Reply|Confirm|Choose|Pick|Ratify)\b/

/**
 * Most existing annotations are one unstructured paragraph (they predate this format), so
 * structure is opt-in: a note written as `Background: ...` / `Thesis: ...` / `Question: ...`
 * lines renders each as its own section, and the Question/Action one is the bold line. A note
 * with no labels gets its trailing sentence checked for the same thing -- a question mark, or
 * a leading imperative like "Edit this note with your call" -- and split out as the action line
 * if it looks like one, so old content still gets a bold line pointing at what it's asking for.
 */
function splitSections(note: string): Section[] {
  const lines = note
    .split(/\n+/)
    .map((l) => l.trim())
    .filter(Boolean)
  if (lines.length > 1 && lines.every((l) => LABEL_LINE.test(l))) {
    return lines.map((l) => {
      const m = l.match(LABEL_LINE)
      const label = m?.[1] ?? null
      const text = m?.[2] ?? l
      return { label, text, action: ACTION_LABEL.test(label ?? "") }
    })
  }
  const sentences = note.split(/(?<=[.?!])\s+/).filter(Boolean)
  const last = sentences[sentences.length - 1] ?? ""
  const looksLikeAction = /\?\s*$/.test(last) || ACTION_LEAD.test(last)
  if (looksLikeAction && sentences.length > 1) {
    return [
      { label: null, text: sentences.slice(0, -1).join(" "), action: false },
      { label: null, text: last, action: true },
    ]
  }
  return [{ label: null, text: note, action: looksLikeAction }]
}

export function MessageCard({
  flow,
  note,
  dense,
  muted,
}: {
  flow: FlowType
  note: string
  /** Compact spacing for list rows and inline block labels, vs. the fuller card elsewhere. */
  dense?: boolean
  /** An already-answered/read item: same structure, quieter color so it sinks visually. */
  muted?: boolean
}) {
  const f = FLOW[flow]
  const sections = splitSections(note)
  return (
    <div
      className={cn(
        "space-y-0.5 rounded-sm border-l-4 py-1 pl-2",
        f.border,
        muted && "opacity-60",
        dense ? "text-[11px]" : "text-xs",
      )}
    >
      {!dense && (
        <Badge className={cn(f.badge, "border-0")} variant="outline">
          {f.label}
        </Badge>
      )}
      {sections.map((s, i) => (
        <p key={i} className={cn(s.action ? "font-semibold text-foreground" : "text-muted-foreground")}>
          {s.label && <span className="font-medium text-foreground">{s.label}: </span>}
          {s.text}
        </p>
      ))}
    </div>
  )
}
