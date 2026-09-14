import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import type { Task } from "@dev/lib/board"
import { dispatchSummary, runHref, useDispatchLatest, type LatestRun } from "@dev/lib/dispatch"

/** Border/fill by status, shared by the graph node and the kanban card so the two views read as
 *  one visual language (kantord/toylang#33). */
export const STATUS_STYLE: Record<Task["status"], string> = {
  done: "border-emerald-600/40 bg-emerald-500/10 dark:border-emerald-400/40",
  delegated: "border-sky-500/50 bg-sky-500/10 dark:border-sky-400/50",
  todo: "border-border bg-card",
}

/** What a `delegated` row is actually doing, from `/__dispatch/latest`. board.yaml alone cannot
 *  say: it marked twelve rows delegated for days after their runs had all ended STUCK
 *  (plans/dispatch-self-healing-plan.md), and this card used to read every one as "in progress". */
function DispatchState({ task }: { task: Task }) {
  const { data, error } = useDispatchLatest()
  if (error) return <span className="text-[10px] text-muted-foreground">dispatch state unavailable</span>
  if (!data) return null
  const latest: LatestRun | undefined = data[task.id]
  const text = dispatchSummary(latest)
  if (latest?.live) {
    return (
      <span className="flex items-center gap-1 text-[10px] font-medium text-sky-600 dark:text-sky-400">
        <span className="size-1.5 animate-pulse rounded-full bg-sky-500" />
        {text}
      </span>
    )
  }
  return (
    <span
      className={cn(
        "text-[10px] font-medium",
        !latest && "text-muted-foreground",
        latest?.status === "GREEN" && "text-emerald-600 dark:text-emerald-400",
        latest && latest.status !== "GREEN" && "text-amber-600 dark:text-amber-400",
      )}
    >
      {text}
    </span>
  )
}

/** One task, rendered the same way in the graph and the kanban board: status color, kind badge,
 *  the delegated row's real dispatch state, the blocked/next-up signal, and the issue
 *  click-through. `className` lets each host size it -- the graph node fixes width and height,
 *  the kanban card just fills its column. */
export function TaskCard({ task, className }: { task: Task; className?: string }) {
  const inner = (
    <div
      className={cn(
        "flex h-full flex-col gap-1.5 rounded-lg border p-3 text-left transition-colors",
        STATUS_STYLE[task.status],
        // Blocked is the PRIMARY signal (issue #33's wording), so it carries the heavier
        // ring; next-up stays visible but quieter.
        task.blocked && "border-destructive ring-2 ring-destructive ring-offset-2 ring-offset-background",
        task.unblocked && "ring-1 ring-primary/70",
        task.issue && "hover:border-foreground/40",
      )}
    >
      <div className="flex items-center gap-1.5">
        <Badge variant="outline" className="text-[10px] uppercase tracking-wide">
          {task.kind}
        </Badge>
        {task.status === "delegated" && <DispatchState task={task} />}
        {task.blocked && <span className="text-[10px] font-medium text-destructive">blocked</span>}
        {task.unblocked && (
          <span className="text-[10px] font-medium text-primary">next up</span>
        )}
        {task.issue && (
          <span className="ml-auto shrink-0 text-[10px] text-muted-foreground">#{task.issue}</span>
        )}
      </div>
      <p className="line-clamp-3 text-xs leading-snug text-foreground" title={task.title}>
        {task.title}
      </p>
    </div>
  )

  const linked = task.issue ? (
    <a
      href={`https://github.com/kantord/toylang/issues/${task.issue}`}
      target="_blank"
      rel="noreferrer"
      className="block min-h-0 flex-1"
    >
      {inner}
    </a>
  ) : (
    <div className="min-h-0 flex-1">{inner}</div>
  )

  // The run link sits beside the card rather than inside it: the card body is already the
  // issue anchor, and an anchor inside an anchor is not HTML.
  return (
    <div className={cn("flex flex-col gap-1", className)}>
      {linked}
      {task.status === "delegated" && <RunLink task={task} />}
    </div>
  )
}

function RunLink({ task }: { task: Task }) {
  const { data } = useDispatchLatest()
  const latest = data?.[task.id]
  if (!latest) return null
  return (
    <a href={runHref(task.id, latest.run_id)} className="self-end font-mono text-[10px] text-primary hover:underline">
      run {latest.run_id}
    </a>
  )
}

export function TaskLegend() {
  return (
    <div className="flex flex-wrap gap-x-5 gap-y-2 text-xs text-muted-foreground">
      <LegendSwatch className={STATUS_STYLE.todo} label="todo" />
      <LegendSwatch className={STATUS_STYLE.delegated} label="delegated" />
      <LegendSwatch className={STATUS_STYLE.done} label="done" />
      <span className="flex items-center gap-1.5">
        <span className="size-3 rounded border border-destructive/50 ring-1 ring-destructive/40" />
        blocked (needs not all done)
      </span>
      <span className="flex items-center gap-1.5">
        <span className="size-3 rounded border border-primary ring-2 ring-primary ring-offset-1 ring-offset-background" />
        unblocked (next up)
      </span>
    </div>
  )
}

function LegendSwatch({ className, label }: { className: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className={cn("size-3 rounded border", className)} />
      {label}
    </span>
  )
}
