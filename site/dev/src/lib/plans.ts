import { parse } from "yaml"

import { saveToInbox } from "@dev/lib/annotations"
import { parseIssue } from "@dev/lib/board"

/**
 * The plans under `plans/`, for the approval flow (kantord/toylang#110). A planning or research
 * task lands its output as `plans/<name>.md` with YAML frontmatter carrying a `status`; the
 * maintainer approves it or sends it back from the mail app, and the coordinator applies that
 * decision by rewriting the frontmatter. Loaded by glob at dev/build time the way board.ts loads
 * plans/board.yaml -- a plan is committed content, not something a server has to hand out.
 *
 * A plan file with no frontmatter status is not in the flow at all: most of `plans/` predates
 * this and is historical record, and back-filling a status onto a document nobody actually
 * ruled on would be inventing the ruling.
 */

/** `proposed` waits on the maintainer; `approved` is ready to be picked up as build work;
 *  `needs-changes` goes back for another planning phase. */
export type PlanStatus = "proposed" | "approved" | "needs-changes"

const STATUSES: PlanStatus[] = ["proposed", "approved", "needs-changes"]

export interface Plan {
  /** Repo-relative path (`plans/euler-ergonomics.md`) -- also the key a decision is filed
   *  under, so the coordinator opens exactly the file a record names. */
  path: string
  /** The first `# ` heading, same rule docs.ts uses. */
  title: string
  status: PlanStatus
  /** A `gh:N` issue in the frontmatter, when one commissioned the plan. */
  issue: number | null
  /** The document with its frontmatter stripped. */
  markdown: string
}

const raw = import.meta.glob("../../../../plans/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>

const FRONTMATTER = /^---\r?\n([\s\S]*?)\r?\n---\r?\n/

interface Loaded {
  plans: Plan[]
  /** Files that declare a status this build does not know. Surfaced in the board's plans panel
   *  rather than only logged: a plan dropped out of the approval queue in silence is the one
   *  way a maintainer decision can go missing without anyone noticing. */
  errors: string[]
}

function load(): Loaded {
  const plans: Plan[] = []
  const errors: string[] = []
  for (const [file, text] of Object.entries(raw)) {
    const path = file.replace(/^(\.\.\/)+/, "")
    const m = FRONTMATTER.exec(text)
    if (!m) continue
    let front: { status?: unknown; issue?: unknown } | null
    try {
      front = parse(m[1]) as { status?: unknown; issue?: unknown } | null
    } catch (e) {
      // One typo'd frontmatter must not throw at module init and blank every plan with it --
      // the same no-silent-drop rule the unknown-status branch below enforces.
      errors.push(`${path}: frontmatter does not parse: ${String(e)}`)
      continue
    }
    const status = front?.status
    if (status === undefined) continue
    if (typeof status !== "string" || !STATUSES.includes(status as PlanStatus)) {
      errors.push(`${path}: status is ${JSON.stringify(status)}, not one of ${STATUSES.join(", ")}`)
      continue
    }
    const markdown = text.slice(m[0].length)
    plans.push({
      path,
      title: /^# (.+)$/m.exec(markdown)?.[1] ?? path,
      status: status as PlanStatus,
      issue: parseIssue(front?.issue),
      markdown,
    })
  }
  plans.sort((a, b) => a.path.localeCompare(b.path))
  return { plans, errors }
}

const loaded = load()

export const PLANS: Plan[] = loaded.plans
export const PLAN_ERRORS: string[] = loaded.errors

export type Decision = "approve" | "needs-changes"

/** Delivers a plan decision through the #30 inbox machinery, the same door a grilling round's
 *  answers go through (lib/grill.ts): `page` is the plan file itself and `block` is always 0 --
 *  a plan carries one decision, not a list of them -- so the coordinator's existing poll of
 *  `docs/.annotations/inbox.json` picks it up with no second store to read. `edited` is JSON for
 *  the same reason a round's is: the coordinator maps it mechanically rather than reading prose.
 */
export function submitPlanDecision(plan: Plan, decision: Decision, notes: string): Promise<void> {
  return saveToInbox(
    {
      page: plan.path,
      block: 0,
      original: plan.title,
      edited: JSON.stringify({ decision, notes: notes.trim() || null }),
    },
    "plan decision failed",
  )
}

/** The `page:block` key a plan's decision is stored under, shared by the submit above and the
 *  mail app's read-state check. */
export function planKey(plan: Plan): string {
  return `${plan.path}:0`
}
