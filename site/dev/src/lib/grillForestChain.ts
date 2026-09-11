/**
 * The pure grill-forest chain logic (kantord/toylang#grill-forest): types for a `.forest.yaml`
 * file and the `activePath`/`threadStatus` walk over them. Kept free of `@/`/`@dev/` alias imports
 * (matching flow.ts's own discipline) so `node --test` can load this directly, no bundler in the
 * way -- grillForest.ts, which does need the alias-resolved React Query hooks and the inbox
 * client, re-exports everything from here.
 */

import type { FlowType } from "./flow.ts"

export type ForestNodeStatus = "live" | "answered" | "superseded"

export interface ForestOption {
  label: string
  content: string
}

export interface ForestAnswer {
  sourceOption: string | null
  content: string
  wasEdited: boolean
  answeredAt: string
}

export interface ForestNode {
  id: string
  parent: string | null
  status: ForestNodeStatus
  supersededNote?: string
  title: string
  flow?: FlowType
  background?: string
  thesis?: string
  question: string
  options?: ForestOption[]
  answer?: ForestAnswer
}

export interface ForestActivityEntry {
  parent: string | null
  note: string
  since: string
}

export interface ForestRound {
  topic: string
  activity: ForestActivityEntry[]
  nodes: ForestNode[]
}

export type ChainEntry = { kind: "node"; node: ForestNode } | { kind: "activity"; note: string }

/**
 * Walks the first root the file contains (v1 scope: one file is one thread -- a root-switcher for
 * the rarer multi-root case isn't built) down through answered children to whichever comes next:
 * a trailing `live` question awaiting the human, a `superseded` dead end, or an `activity` entry
 * if the agent is between an answer and its next live node.
 *
 * If more than one non-draft child sits under one node -- the schema doesn't forbid it, though
 * the agent should only ever promote one branch per answer -- the lowest-id child wins,
 * consistently, so this should-never-happen case fails as a stable, boring choice rather than
 * flickering between renders.
 */
export function activePath(nodes: ForestNode[], activity: ForestActivityEntry[]): ChainEntry[] {
  const root = nodes.find((n) => n.parent === null) ?? null
  if (!root) {
    // No root node exists yet -- the only thing that can be showing is a brand-new thread's very
    // first question being drafted.
    const starting = activity.find((a) => a.parent === null)
    return starting ? [{ kind: "activity", note: starting.note }] : []
  }

  const path: ChainEntry[] = []
  let current: ForestNode | undefined = root
  while (current) {
    path.push({ kind: "node", node: current })
    if (current.status !== "answered") break
    const children = nodes.filter((n) => n.parent === current!.id).sort((a, b) => a.id.localeCompare(b.id))
    if (children.length > 0) {
      current = children[0]
      continue
    }
    const waiting = activity.find((a) => a.parent === current!.id)
    if (waiting) path.push({ kind: "activity", note: waiting.note })
    current = undefined
  }
  return path
}

export type ThreadStatus = "waiting" | "thinking" | "idle"

/** Where a topic's one thread (v1 scope) currently stands, for the topic rail's per-topic dot and
 *  the nav tab's aggregate count -- derived from the same `activePath` walk the chain view uses,
 *  so there's no separate notion of "open" to keep in sync with it. */
export function threadStatus(round: ForestRound): ThreadStatus {
  const path = activePath(round.nodes, round.activity)
  const last = path[path.length - 1]
  if (!last) return "idle"
  if (last.kind === "activity") return "thinking"
  return last.node.status === "live" ? "waiting" : "idle"
}
