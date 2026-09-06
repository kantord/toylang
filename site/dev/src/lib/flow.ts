/**
 * Shared shape for every agentic message surface (kantord/toylang#30 design-system addition):
 * an inbox annotation, an AUTHORING note, a reply, or a grilling round all render through this
 * so the maintainer never has to parse which kind of thing they're looking at from typography
 * alone -- a thick left border and a badge carry that instead. Six flow types, not the three
 * annotation types: `review`/`fill`/`comment` describe what the coordinator's comment syntax
 * marks, but `reply` has no comment-syntax equivalent -- it's the maintainer's own note going
 * the other way -- and `round` is a grilling round arriving as mail (kantord/toylang#52), so
 * each needs a flow of its own the wash types don't have. `plan` is the sixth: a plan document
 * waiting on approve / needs-changes (kantord/toylang#110), which is neither a question with an
 * answer to type nor a round with screens to walk.
 *
 * Kept free of `@/`/`@dev/` alias imports so `node --test` can load it directly (no bundler
 * in the way): FLOW_FOR_TYPE spells the annotation-type keys inline rather than importing
 * `AnnotationType` from annotations.ts, which would drag `@/lib/blocks` in with it.
 */
export type FlowType = "question" | "escalation" | "status" | "reply" | "round" | "plan"

/** How a coordinator annotation's comment-syntax type maps to a flow: `@review` is something
 *  that needs the maintainer's ratification (escalation), `@fill` is a direct question with an
 *  expected short answer, and `@comment` is FYI/status ("resolved per your answer", "correct").
 *  Matches how the existing corpus of annotations actually reads (docs/reference/types/record.md,
 *  docs/reference/builtins/concat.md). The keys must stay in step with `AnnotationType`
 *  (dev/src/lib/annotations.ts), spelled out here to keep this module dependency-free. */
export const FLOW_FOR_TYPE: Record<"review" | "comment" | "fill", FlowType> = {
  review: "escalation",
  fill: "question",
  comment: "status",
}

/** Exported so the mail list can tag a row with the same badge the reading pane shows for the
 *  message it opens into (kantord/toylang#52: "badges as tags on rows"). */
export const FLOW: Record<FlowType, { label: string; border: string; badge: string }> = {
  question: { label: "Question", border: "border-fuchsia-500", badge: "bg-fuchsia-500/15 text-fuchsia-700 dark:text-fuchsia-300" },
  escalation: { label: "Escalation", border: "border-amber-500", badge: "bg-amber-500/15 text-amber-700 dark:text-amber-300" },
  status: { label: "Status", border: "border-sky-500", badge: "bg-sky-500/15 text-sky-700 dark:text-sky-300" },
  reply: { label: "Reply", border: "border-emerald-500", badge: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300" },
  round: { label: "Grill round", border: "border-violet-500", badge: "bg-violet-500/15 text-violet-700 dark:text-violet-300" },
  plan: { label: "Plan", border: "border-rose-500", badge: "bg-rose-500/15 text-rose-700 dark:text-rose-300" },
}

/** Fallback for a flow value the wash table doesn't know (live incident: round files carried
 *  `flow: decide`), so an unrecognized value degrades to a neutral card instead of throwing
 *  on the `f.border` access and blanking the whole mail/grill UI. */
export const NEUTRAL: { label: string; border: string; badge: string } = {
  label: "Message",
  border: "border-muted-foreground/40",
  badge: "bg-muted text-muted-foreground",
}

export function flowStyle(flow: string): { label: string; border: string; badge: string } {
  return FLOW[flow as FlowType] ?? NEUTRAL
}