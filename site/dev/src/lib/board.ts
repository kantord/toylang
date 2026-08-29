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
  /** True for a `todo` task whose needs are all `done` -- the unblocked frontier, shown by both views. */
  unblocked: boolean
  /** True for a `todo` task with at least one need not yet `done` -- the graph's primary
   *  highlight (kantord/toylang#33). Mutually exclusive with `unblocked`; both are false for
   *  `delegated` and `done` tasks. */
  blocked: boolean
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
  return rows.map((r) => {
    const needsAllDone = r.needs.every((id) => statusOf.get(id) === "done")
    return {
      id: r.id,
      issue: parseIssue(r.issue),
      title: r.title,
      kind: r.kind,
      needs: r.needs,
      status: r.status,
      unblocked: r.status === "todo" && needsAllDone,
      blocked: r.status === "todo" && !needsAllDone,
    }
  })
}

export const BOARD: Task[] = load()

