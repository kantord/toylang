import { parse } from "yaml"

/**
 * `plans/board.yaml`, loaded at build/dev time (docs.ts loads the docs tree the same way): the
 * board is committed data, so the graph page ships in the public build with no server behind it.
 */
export type Kind = "build" | "decide"
export type Status = "todo" | "delegated" | "done"

export interface Task {
  id: string
  /** A `gh:N` issue number, when one carries the spec. */
  issue: number | null
  title: string
  kind: Kind
  needs: string[]
  status: Status
  /** True for a `todo` task whose needs are all `done` -- the kanban's unblocked frontier. */
  unblocked: boolean
}

const raw = import.meta.glob("../../../../plans/board.yaml", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>

function parseIssue(issue: unknown): number | null {
  if (typeof issue !== "string") return null
  const m = /^gh:(\d+)$/.exec(issue)
  return m ? Number(m[1]) : null
}

function load(): Task[] {
  const [text] = Object.values(raw)
  const rows = parse(text) as {
    id: string
    issue?: string
    title: string
    kind: Kind
    needs: string[]
    status: Status
  }[]

  const statusOf = new Map(rows.map((r) => [r.id, r.status]))
  return rows.map((r) => ({
    id: r.id,
    issue: parseIssue(r.issue),
    title: r.title,
    kind: r.kind,
    needs: r.needs,
    status: r.status,
    unblocked: r.status === "todo" && r.needs.every((id) => statusOf.get(id) === "done"),
  }))
}

export const BOARD: Task[] = load()

/** Longest-path layer from a root (a task with no needs), for a left-to-right DAG layout. */
export function rankOf(tasks: Task[]): Map<string, number> {
  const byId = new Map(tasks.map((t) => [t.id, t]))
  const rank = new Map<string, number>()
  function resolve(id: string, path: Set<string>): number {
    const cached = rank.get(id)
    if (cached !== undefined) return cached
    if (path.has(id)) {
      // A needs-cycle is a board bug (plans/board.yaml's own header calls this a deadlock);
      // breaking it at 0 keeps the page rendering instead of recursing forever.
      rank.set(id, 0)
      return 0
    }
    const task = byId.get(id)
    const r = !task || task.needs.length === 0
      ? 0
      : 1 + Math.max(...task.needs.map((n) => resolve(n, new Set(path).add(id))))
    rank.set(id, r)
    return r
  }
  for (const t of tasks) resolve(t.id, new Set())
  return rank
}
