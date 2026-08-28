import { useMemo } from "react"

import { Badge } from "@/components/ui/badge"
import { BOARD, rankOf, type Task } from "@/lib/board"
import { cn } from "@/lib/utils"

const NODE_W = 236
const NODE_H = 112
const GAP_X = 72
const GAP_Y = 16

interface Placed extends Task {
  x: number
  y: number
}

/** Longest-path layering (board.ts), columns left to right; within a column, board order --
 *  the file's own priority ordering (plans/board.yaml's header: "position is priority"). */
function layout(tasks: Task[]): { placed: Placed[]; width: number; height: number } {
  const rank = rankOf(tasks)
  const byRank = new Map<number, Task[]>()
  for (const t of tasks) {
    const r = rank.get(t.id) ?? 0
    const bucket = byRank.get(r)
    if (bucket) bucket.push(t)
    else byRank.set(r, [t])
  }

  const placed: Placed[] = []
  let maxRank = 0
  let maxRows = 1
  for (const [r, bucket] of byRank) {
    maxRank = Math.max(maxRank, r)
    maxRows = Math.max(maxRows, bucket.length)
    bucket.forEach((t, i) => {
      placed.push({ ...t, x: r * (NODE_W + GAP_X), y: i * (NODE_H + GAP_Y) })
    })
  }

  return {
    placed,
    width: (maxRank + 1) * NODE_W + maxRank * GAP_X,
    height: maxRows * NODE_H + (maxRows - 1) * GAP_Y,
  }
}

const STATUS_STYLE: Record<Task["status"], string> = {
  done: "border-emerald-600/40 bg-emerald-500/10 dark:border-emerald-400/40",
  delegated: "border-sky-500/50 bg-sky-500/10 dark:border-sky-400/50",
  todo: "border-border bg-card",
}

/**
 * The board as a dependency graph: one node per plans/board.yaml row, edges from `needs`,
 * color by status, the unblocked frontier (todo with every need done) picked out with a ring.
 * Read-only -- the only interaction is following a node's issue link.
 */
export function BoardPage() {
  const { placed, width, height } = useMemo(() => layout(BOARD), [])
  const byId = useMemo(() => new Map(placed.map((t) => [t.id, t])), [placed])

  const edges = useMemo(
    () =>
      placed.flatMap((t) =>
        t.needs.map((needId) => {
          const from = byId.get(needId)
          if (!from) return null
          return { id: `${needId}->${t.id}`, from, to: t }
        }),
      ).filter((e): e is { id: string; from: Placed; to: Placed } => e !== null),
    [placed, byId],
  )

  return (
    <div className="space-y-4">
      <p className="max-w-2xl text-sm text-muted-foreground">
        <code className="rounded bg-muted px-1 py-0.5 text-xs">plans/board.yaml</code> as a
        dependency graph: what is done, what is being worked on, and what is next in line.
      </p>
      <Legend />
      <div className="overflow-auto rounded-lg border border-border p-6">
        <div className="relative" style={{ width, height }}>
          <svg
            className="pointer-events-none absolute inset-0 overflow-visible"
            width={width}
            height={height}
          >
            {edges.map((e) => (
              <Edge key={e.id} from={e.from} to={e.to} />
            ))}
          </svg>
          {placed.map((t) => (
            <Node key={t.id} task={t} />
          ))}
        </div>
      </div>
    </div>
  )
}

function Edge({ from, to }: { from: Placed; to: Placed }) {
  const x1 = from.x + NODE_W
  const y1 = from.y + NODE_H / 2
  const x2 = to.x
  const y2 = to.y + NODE_H / 2
  const midX = (x1 + x2) / 2
  return (
    <path
      d={`M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`}
      fill="none"
      stroke="var(--border)"
      strokeWidth={1.5}
    />
  )
}

function Node({ task }: { task: Placed }) {
  const inner = (
    <div
      className={cn(
        "flex h-full flex-col gap-1.5 rounded-lg border p-3 text-left transition-colors",
        STATUS_STYLE[task.status],
        task.unblocked && "ring-2 ring-primary ring-offset-2 ring-offset-background",
        task.issue && "hover:border-foreground/40",
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

  return (
    <div className="absolute" style={{ left: task.x, top: task.y, width: NODE_W, height: NODE_H }}>
      {task.issue ? (
        <a
          href={`https://github.com/kantord/toylang/issues/${task.issue}`}
          target="_blank"
          rel="noreferrer"
          className="block h-full"
        >
          {inner}
        </a>
      ) : (
        inner
      )}
    </div>
  )
}

function Legend() {
  return (
    <div className="flex flex-wrap gap-x-5 gap-y-2 text-xs text-muted-foreground">
      <LegendSwatch className={STATUS_STYLE.todo} label="todo" />
      <LegendSwatch className={STATUS_STYLE.delegated} label="delegated" />
      <LegendSwatch className={STATUS_STYLE.done} label="done" />
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
