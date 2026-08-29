import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import type { Task } from "@dev/lib/board"

/** Border/fill by status, shared by the graph node and the kanban card so the two views read as
 *  one visual language (kantord/toylang#33). */
export const STATUS_STYLE: Record<Task["status"], string> = {
  done: "border-emerald-600/40 bg-emerald-500/10 dark:border-emerald-400/40",
  delegated: "border-sky-500/50 bg-sky-500/10 dark:border-sky-400/50",
  todo: "border-border bg-card",
}

/** One task, rendered the same way in the graph and the kanban board: status color, kind badge,
 *  the delegated pulse, the blocked/next-up signal, and the issue click-through. `className` lets
 *  each host size it -- the graph node fixes width and height, the kanban card just fills its
 *  column. */
export function TaskCard({ task, className }: { task: Task; className?: string }) {
  const inner = (
    <div
      className={cn(
        "flex flex-col gap-1.5 rounded-lg border p-3 text-left transition-colors",
        STATUS_STYLE[task.status],
        // Blocked is the PRIMARY signal (issue #33's wording), so it carries the heavier
        // ring; next-up stays visible but quieter.
        task.blocked && "border-destructive ring-2 ring-destructive ring-offset-2 ring-offset-background",
        task.unblocked && "ring-1 ring-primary/70",
        task.issue && "hover:border-foreground/40",
        className,
      )}
    >
      <div className="flex items-center gap-1.5">
        <Badge variant="outline" className="text-[10px] uppercase tracking-wide">
          {task.kind}
        </Badge>
        {task.status === "delegated" && (
          <span className="flex items-center gap-1 text-[10px] font-medium text-sky-600 dark:text-sky-400">
            <span className="size-1.5 animate-pulse rounded-full bg-sky-500" />
            in progress
          </span>
        )}
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

  if (!task.issue) return inner
  return (
    <a
      href={`https://github.com/kantord/toylang/issues/${task.issue}`}
      target="_blank"
      rel="noreferrer"
      className="block h-full"
    >
      {inner}
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
