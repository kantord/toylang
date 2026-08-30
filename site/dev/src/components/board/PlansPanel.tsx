import { PLAN_STATUS } from "@dev/components/PlanReader"
import { Badge } from "@/components/ui/badge"
import { PLAN_ERRORS, PLANS, type Plan } from "@dev/lib/plans"
import { cn } from "@/lib/utils"

/**
 * Where every plan under `plans/` stands (kantord/toylang#110): proposed ones are waiting on the
 * maintainer in the mail app's Plan approvals folder, approved ones are ready to be picked up as
 * build work, and one sent back is in another planning phase. Read-only, like the rest of the
 * board -- the decision is made in the mail app, where the plan is actually readable.
 *
 * Renders nothing when no plan declares a status, which is the state of a repository whose plans
 * all predate this flow.
 */
export function PlansPanel() {
  if (PLANS.length === 0 && PLAN_ERRORS.length === 0) return null
  return (
    <div className="space-y-2 rounded-lg border border-border bg-muted/20 p-3">
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
