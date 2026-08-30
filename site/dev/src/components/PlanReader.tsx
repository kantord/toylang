import { useState } from "react"

import { DevMarkdown } from "@dev/components/DevMarkdown"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import { clearDraft, loadDraft, saveDraft } from "@dev/lib/draft"
import { submitPlanDecision, type Decision, type Plan, type PlanStatus } from "@dev/lib/plans"
import { cn } from "@/lib/utils"

/** One badge per plan status, shared with the board's plans panel so the two surfaces name and
 *  color a status the same way. */
export const PLAN_STATUS: Record<PlanStatus, { label: string; badge: string }> = {
  proposed: { label: "proposed", badge: "bg-rose-500/15 text-rose-700 dark:text-rose-300" },
  approved: { label: "approved", badge: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300" },
  "needs-changes": { label: "needs changes", badge: "bg-amber-500/15 text-amber-700 dark:text-amber-300" },
}

function draftKey(plan: Plan): string {
  return `toylang-plan-draft:${plan.path}`
}

/**
 * A proposed plan's reading-pane body (kantord/toylang#110): the plan rendered in full, then
 * Approve or Needs changes, the same shape a grilling round has -- read the real thing in the
 * page, decide in the page, no terminal round-trip. The decision goes through the annotations
 * inbox (lib/plans.ts) and the coordinator applies it by rewriting the plan's frontmatter, so
 * nothing here writes to `plans/` itself.
 *
 * Editing the plan is deliberately not a control here. A plan is a committed markdown file, so
 * the maintainer's own editor is a better editor than a textarea in a dev server would be; the
 * notes box is for what an edit can't say ("approved except section 4").
 */
export function PlanReader({
  plan,
  answered,
  onDecided,
}: {
  plan: Plan
  /** A decision for this plan is already sitting in the coordinator's inbox, unapplied. */
  answered: boolean
  onDecided: () => void
}) {
  const [notes, setNotes] = useState(() => loadDraft(draftKey(plan), ""))
  const [sending, setSending] = useState<Decision | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [sent, setSent] = useState<Decision | null>(null)

  const setDraft = (text: string) => {
    setNotes(text)
    saveDraft(draftKey(plan), text)
  }

  const decide = async (decision: Decision) => {
    setSending(decision)
    setError(null)
    try {
      await submitPlanDecision(plan, decision, notes)
      clearDraft(draftKey(plan))
      setSent(decision)
      onDecided()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSending(null)
    }
  }

  const status = PLAN_STATUS[plan.status]
  const decided = sent !== null

  return (
    <div className="max-w-2xl space-y-4">
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <Badge className={cn(status.badge, "border-0")} variant="outline">
          {status.label}
        </Badge>
        <code className="rounded bg-muted px-1 py-0.5">{plan.path}</code>
        {plan.issue && (
          <a
            className="underline"
            href={`https://github.com/kantord/toylang/issues/${plan.issue}`}
            target="_blank"
            rel="noreferrer"
          >
            #{plan.issue}
          </a>
        )}
      </div>

      <DevMarkdown text={plan.markdown} basePath={plan.path} />

      <Separator />

      {decided ? (
        <div className="space-y-2 rounded-md border border-emerald-500/40 bg-emerald-500/5 p-4 text-sm">
          <p className="font-semibold">
            {sent === "approve" ? "Approved." : "Sent back for changes."}
          </p>
          <p className="text-muted-foreground">
            It is in the coordinator's inbox, keyed to {plan.path}. The status on the board flips
            when the coordinator applies it.
          </p>
        </div>
      ) : plan.status !== "proposed" ? (
        <p className="text-sm text-muted-foreground">
          This plan is already {status.label}. Change the call by editing {plan.path} or telling
          the coordinator.
        </p>
      ) : answered ? (
        <p className="text-sm text-muted-foreground">
          A decision for this plan is already waiting in the coordinator's inbox.
        </p>
      ) : (
        <div className="space-y-3">
          <div className="space-y-1">
            <div className="text-sm font-bold">
              Approve it, or send it back for another planning phase.
            </div>
            <p className="text-xs text-muted-foreground">
              Changes you want made are best written into {plan.path} directly -- this box is for
              everything an edit can't say.
            </p>
          </div>
          <Textarea
            value={notes}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="Notes for the coordinator (optional)"
          />
          {error && (
            <p className="text-sm text-destructive">
              Delivery failed: {error}. Your notes are kept -- retry.
            </p>
          )}
          <div className="flex gap-2">
            <Button onClick={() => decide("approve")} disabled={sending !== null}>
              {sending === "approve" ? "Approving..." : "Approve"}
            </Button>
            <Button
              variant="outline"
              onClick={() => decide("needs-changes")}
              disabled={sending !== null}
            >
              {sending === "needs-changes" ? "Sending..." : "Needs changes"}
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
