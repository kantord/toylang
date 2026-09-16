import { useEffect, useState } from "react"

import { PlanReader, PLAN_STATUS } from "@dev/components/PlanReader"
import { Badge } from "@/components/ui/badge"
import { PLAN_ERRORS, PLANS, planKey, type Plan } from "@dev/lib/plans"
import { cn } from "@/lib/utils"

interface InboxRecord {
  page: string
  block: number | string
}

/** Every answered `page:block` key already sitting in the coordinator's inbox, so a proposed
 *  plan knows whether a decision is already waiting (kantord/toylang#110) -- the same read-state
 *  the mail app fetched for its plan folder, moved here with the approval controls themselves.
 *  Tracked locally so deciding a plan flips its row the instant it happens rather than waiting on
 *  a refetch. */
function useAnsweredKeys(): [Set<string>, (key: string) => void] {
  const [answered, setAnswered] = useState<Set<string>>(new Set())
  useEffect(() => {
    fetch("/__annotations/inbox-all")
      .then((r) => (r.ok ? (r.json() as Promise<{ records: InboxRecord[] }>) : { records: [] }))
      .then(({ records }) => setAnswered(new Set(records.map((r) => `${r.page}:${r.block}`))))
      .catch(() => {
        // No inbox endpoint reachable -- every plan just reads as undecided.
      })
  }, [])
  const markRead = (key: string) => setAnswered((prev) => new Set(prev).add(key))
  return [answered, markRead]
}

/**
 * Where every plan under `plans/` stands (kantord/toylang#110): proposed ones are waiting on the
 * maintainer, approved ones are ready to be picked up as build work, and one sent back is in
 * another planning phase. The status cards stay read-only, like the rest of the board; the
 * decision is made here too -- a proposed plan renders its full reading pane (PlanReader) with
 * Approve / Needs changes controls, which deliver through the same inbox door the mail app's plan
 * approval used (lib/plans.ts's `submitPlanDecision`).
 *
 * Renders nothing when no plan declares a status, which is the state of a repository whose plans
 * all predate this flow.
 */
export function PlansPanel() {
  const [answeredKeys, markReadLocally] = useAnsweredKeys()
  if (PLANS.length === 0 && PLAN_ERRORS.length === 0) return null
  const proposed = PLANS.filter((p) => p.status === "proposed")
  return (
    <div className="space-y-4 rounded-lg border border-border bg-muted/20 p-3">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">Plans</div>
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {PLANS.map((p) => (
          <PlanRow key={p.path} plan={p} />
        ))}
      </div>
      {PLAN_ERRORS.map((e) => (
        <p key={e} className="text-xs text-destructive">
          {e}
        </p>
      ))}
      {proposed.length > 0 && (
        <div className="space-y-4">
          {proposed.map((p) => (
            <PlanReader
              key={p.path}
              plan={p}
              answered={answeredKeys.has(planKey(p))}
              onDecided={() => markReadLocally(planKey(p))}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function PlanRow({ plan }: { plan: Plan }) {
  const status = PLAN_STATUS[plan.status]
  return (
    <div className="flex flex-col gap-1.5 rounded-lg border border-border bg-card p-3">
      <div className="flex items-center gap-1.5">
        <Badge className={cn(status.badge, "border-0 text-[10px]")} variant="outline">
          {status.label}
        </Badge>
        {plan.issue && (
          <a
            className="ml-auto shrink-0 text-[10px] text-muted-foreground hover:underline"
            href={`https://github.com/kantord/toylang/issues/${plan.issue}`}
            target="_blank"
            rel="noreferrer"
          >
            #{plan.issue}
          </a>
        )}
      </div>
      <p className="line-clamp-2 text-xs leading-snug text-foreground" title={plan.title}>
        {plan.title}
      </p>
      <code className="truncate text-[10px] text-muted-foreground">{plan.path}</code>
    </div>
  )
}
