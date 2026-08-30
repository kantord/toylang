import { ARCHIVE, BOARD, type Status, type Task } from "@dev/lib/board"
import { TaskCard } from "@dev/components/board/TaskCard"

const COLUMNS: { status: Status; label: string }[] = [
  { status: "todo", label: "Todo" },
  { status: "delegated", label: "Delegated" },
  { status: "done", label: "Done" },
]

/**
 * The board as a plain kanban: one column per status, no edges -- todo, delegated, done, each in
 * its source file's own order (the file's priority ordering). Done pulls from board-archive.yaml
 * (issue #113): the live board never holds a done row. The dependency graph lives on the Graph
 * tab instead.
 */
export function KanbanView() {
  const byStatus = new Map<Status, Task[]>(COLUMNS.map((c) => [c.status, []]))
  for (const t of [...BOARD, ...ARCHIVE]) byStatus.get(t.status)?.push(t)

  return (
    <div className="grid gap-4 sm:grid-cols-3">
      {COLUMNS.map((c) => {
        const tasks = byStatus.get(c.status) ?? []
        return (
          <div key={c.status} className="flex flex-col gap-2 rounded-lg border border-border bg-muted/20 p-3">
            <div className="flex items-center justify-between text-xs font-medium uppercase tracking-wide text-muted-foreground">
              <span>{c.label}</span>
              <span>{tasks.length}</span>
            </div>
            <div className="flex flex-col gap-2">
              {tasks.map((t) => (
                <TaskCard key={t.id} task={t} />
              ))}
            </div>
          </div>
        )
      })}
    </div>
  )
}
