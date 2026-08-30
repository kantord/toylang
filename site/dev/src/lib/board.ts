import { parse } from "yaml"

/**
 * `plans/board.yaml` and `plans/board-archive.yaml`, loaded at build/dev time (docs.ts loads
 * the docs tree the same way): the board is committed data, so the graph page ships in the
 * public build with no server behind it. Done rows live only in the archive (issue #113); a
 * `needs`/`soft` id absent from the live board is satisfied by that same rule, so `BOARD` alone
 * is already the not-done view.
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
  /** True for a `todo` task whose needs have all landed (absent from the live board) -- the
   *  unblocked frontier, shown by both views. */
  unblocked: boolean
  /** True for a `todo` task with at least one need still on the live board -- the graph's
   *  primary highlight (kantord/toylang#33). Mutually exclusive with `unblocked`; both are
   *  false for `delegated` and `done` tasks. */
  blocked: boolean
}

const rawBoard = import.meta.glob("../../../../plans/board.yaml", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>

const rawArchive = import.meta.glob("../../../../plans/board-archive.yaml", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>

/** A `gh:N` reference to its issue number. Exported for lib/plans.ts, whose frontmatter uses
 *  the same spelling as a board row's `issue` field. */
export function parseIssue(issue: unknown): number | null {
  if (typeof issue !== "string") return null
  const m = /^gh:(\d+)$/.exec(issue)
  return m ? Number(m[1]) : null
}

interface RawRow {
  id: string
  issue?: string
  title: string
  kind: Kind
  needs: string[]
  status: Status
}

function parseRows(raw: Record<string, string>): RawRow[] {
  const [text] = Object.values(raw)
  return parse(text) as RawRow[]
}

function load(): { board: Task[]; archive: Task[] } {
  const boardRows = parseRows(rawBoard)
  const archiveRows = parseRows(rawArchive)

  // Only the live board can gate anything; an id missing from it is satisfied -- it landed
  // and moved to the archive (issue #113).
  const liveIds = new Set(boardRows.map((r) => r.id))
  const toTask = (r: RawRow): Task => {
    const needsAllDone = r.needs.every((id) => !liveIds.has(id))
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
  }

  return { board: boardRows.map(toTask), archive: archiveRows.map(toTask) }
}

const { board, archive } = load()

/** The live board: never contains a `done` row (issue #113). */
export const BOARD: Task[] = board

/** Landed rows, for history views only -- never consulted to decide whether something is
 *  blocked. */
export const ARCHIVE: Task[] = archive

/** The full history view -- live rows first, landed rows after, each in file order. */
export const ALL_TASKS: Task[] = [...board, ...archive]

