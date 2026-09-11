import { useEffect, useReducer, useRef } from "react"

import { AnswerComposer } from "@dev/components/AnswerComposer"
import { Section } from "@dev/components/GrillWizard"
import { MessageCard } from "@dev/components/MessageCard"
import { activePath, clearPendingAnswer, loadPendingAnswer, type ForestRound } from "@dev/lib/grillForest"

const EXCERPT_MAX_LEN = 140

function truncate(s: string, maxLen: number): string {
  const clean = s.replace(/\s+/g, " ").trim()
  return clean.length > maxLen ? `${clean.slice(0, maxLen)}...` : clean
}

/**
 * Renders `activePath(...)` top-to-bottom as an actual back-and-forth (kantord/toylang#grill-forest):
 * agent bubbles align left (`mr-auto`), the human's own answers align right (`ml-auto`) -- the
 * single most literal reading of "an actual back-and-forth" over a uniform stack of identical
 * cards, and pure layout, no schema change. A child's card quotes a short excerpt of its parent's
 * own answer, so a branch reads as a reply rather than an unrelated new card -- `path[i-1]` is
 * always that parent, since `activePath` only ever appends a node's own child next.
 */
export function GrillChain({ topic, round, scrollToNodeId }: { topic: string; round: ForestRound; scrollToNodeId?: string }) {
  const path = activePath(round.nodes, round.activity)
  const scrollRef = useRef<HTMLDivElement>(null)
  const wasNearBottom = useRef(true)
  const didInitialScroll = useRef(false)
  // Submitting an answer changes only localStorage (the pending record), not `round` -- this
  // forces the one re-render needed to pick that up immediately instead of waiting for the next
  // poll tick to change `round` itself.
  const [, refresh] = useReducer((n: number) => n + 1, 0)

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    // A reload lands back on the exact node the URL names (kantord/toylang#grill-forest) --
    // cheap specifically because the server holds no per-connection state to resume otherwise.
    // Tried once per mount; after that, ordinary near-bottom auto-scroll takes over.
    if (!didInitialScroll.current) {
      didInitialScroll.current = true
      const target = scrollToNodeId && el.querySelector<HTMLElement>(`[data-node-id="${CSS.escape(scrollToNodeId)}"]`)
      if (target) {
        target.scrollIntoView({ block: "center" })
        return
      }
    }
    if (wasNearBottom.current) el.scrollTop = el.scrollHeight
  }, [path.length, scrollToNodeId])

  // Once the coordinator has durably processed an answer (the node itself now says `answered`),
  // the local pending record has served its purpose -- clear it rather than leaving it to survive
  // a future refresh with nothing left to reconcile it against.
  useEffect(() => {
    for (const entry of path) {
      if (entry.kind === "node" && entry.node.status === "answered") clearPendingAnswer(topic, entry.node.id)
    }
  }, [topic, path])

  const onScroll = () => {
    const el = scrollRef.current
    if (!el) return
    // Auto-scroll only when already near the bottom -- an unconditional scroll would yank the
    // human away from reading ancestor context on every poll tick.
    wasNearBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80
  }

  if (path.length === 0) {
    return <p className="text-sm text-muted-foreground">Nothing here yet.</p>
  }

  return (
    <div ref={scrollRef} onScroll={onScroll} className="max-h-full space-y-4 overflow-y-auto">
      {path.map((entry, i) => {
        if (entry.kind === "activity") {
          return <ThinkingBubble key={`activity-${i}`} note={entry.note} />
        }

        const node = entry.node
        const prevEntry = i > 0 ? path[i - 1] : null
        const parent = prevEntry?.kind === "node" ? prevEntry.node : null
        const pending = node.status === "live" ? loadPendingAnswer(topic, node.id) : null

        return (
          <div key={node.id} data-node-id={node.id} className="space-y-3">
            <div className="mr-auto max-w-2xl space-y-2 rounded-lg border bg-card p-3">
              {parent?.answer && (
                <blockquote className="border-l-2 pl-2 text-xs text-muted-foreground">
                  {truncate(parent.answer.content, EXCERPT_MAX_LEN)}
                </blockquote>
              )}
              <MessageCard flow={node.flow ?? "question"} note={node.title} />
              {node.background && <Section label="Background" markdown={node.background} />}
              {node.thesis && <Section label="Thesis" markdown={node.thesis} />}
              <Section label="Question" markdown={node.question} />
            </div>

            {node.status === "superseded" && (
              <div className="mr-auto max-w-2xl space-y-1 rounded-lg border border-dashed p-3 opacity-60">
                <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Superseded</div>
                <p className="text-sm text-muted-foreground">{node.supersededNote}</p>
              </div>
            )}

            {node.status === "answered" && node.answer && (
              <AnswerBubble
                content={node.answer.content}
                sourceOption={node.answer.sourceOption}
                edited={node.answer.wasEdited}
                sending={false}
              />
            )}

            {node.status === "live" && pending && (
              <AnswerBubble content={pending.content} sourceOption={pending.sourceOption} edited={pending.wasEdited} sending />
            )}

            {node.status === "live" && !pending && i === path.length - 1 && (
              <AnswerComposer topic={topic} node={node} onSubmitted={refresh} />
            )}
          </div>
        )
      })}
    </div>
  )
}

function AnswerBubble({
  content,
  sourceOption,
  edited,
  sending,
}: {
  content: string
  sourceOption: string | null
  edited: boolean
  sending: boolean
}) {
  return (
    <div className="ml-auto max-w-2xl space-y-1 rounded-lg bg-primary/10 p-3">
      {edited && sourceOption && <div className="text-xs font-medium text-muted-foreground">Edited from {sourceOption}</div>}
      <p className="whitespace-pre-wrap text-sm">{content}</p>
      {sending && <div className="text-xs text-muted-foreground">Sending...</div>}
    </div>
  )
}

function ThinkingBubble({ note }: { note: string }) {
  return (
    <div className="mr-auto flex max-w-2xl items-center gap-2 rounded-lg border bg-card p-3">
      <div className="flex shrink-0 items-center gap-1">
        <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground [animation-delay:0ms]" />
        <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground [animation-delay:150ms]" />
        <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground [animation-delay:300ms]" />
      </div>
      {note && <span className="text-xs text-muted-foreground">{note}</span>}
    </div>
  )
}
