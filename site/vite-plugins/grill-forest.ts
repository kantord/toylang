import { readdir, readFile } from "node:fs/promises"
import path from "node:path"

import type { Plugin } from "vite"
import { parse } from "yaml"

/**
 * Dev-only endpoints backing the grill-forest chain UI (kantord/toylang#grill-forest): a forest
 * is a YAML file the coordinator writes to `docs/.grill/<topic>.forest.yaml` while the dev server
 * is already running, read fresh on every request (same reasoning as grill-rounds.ts: a coordinator
 * write mid-session must be visible without a restart). `apply: "serve"` keeps this, and the
 * `docs/.grill/`/`docs/.annotations/` directories it touches, out of `vite build` entirely.
 *
 * No shared helper with grill-rounds.ts: the two formats (flat wizard questions vs. a node tree)
 * may still diverge, and sharing code between two young, possibly-diverging things is the wrong
 * abstraction to build first.
 */

interface RawAnswer {
  sourceOption?: unknown
  content?: unknown
  wasEdited?: unknown
  answeredAt?: unknown
}

interface RawNode {
  id?: unknown
  parent?: unknown
  status?: unknown
  supersededNote?: unknown
  title?: unknown
  flow?: unknown
  background?: unknown
  thesis?: unknown
  question?: unknown
  options?: unknown
  answer?: unknown
}

interface RawActivityEntry {
  parent?: unknown
  note?: unknown
  since?: unknown
}

interface RawForest {
  topic?: unknown
  activity?: unknown
  nodes?: unknown
}

const STATUSES = ["draft", "live", "answered", "superseded"] as const

/** Validates the whole file (drafts included -- a broken draft is still an authoring bug worth
 *  surfacing) before any filtering happens. Returns a message naming the specific problem, or
 *  `null` when the file is servable -- mirrors grill-rounds.ts's own "reject a round missing
 *  `questions`, or a question missing `question`" discipline, extended for the extra ways a node
 *  tree can be broken that a flat list can't. Every field the client renders unconditionally has
 *  to be checked here, not just the ones a first pass happened to think of: a field this validator
 *  misses still parses and caches cleanly, then crashes `GrillChain` at render with no server-side
 *  signal at all -- `title` and `answer.sourceOption`/`wasEdited` were both missed this way before
 *  a review round caught it, so treat "does the client render this without a guard" as the actual
 *  checklist, not the fields that happened to matter for an earlier bug. */
function validateForest(parsed: unknown): string | null {
  if (typeof parsed !== "object" || parsed === null) return "must be a YAML mapping"
  const nodes = (parsed as RawForest).nodes
  if (!Array.isArray(nodes) || nodes.length === 0) return `needs a non-empty top-level "nodes" list`

  const byId = new Map<string, RawNode>()
  for (const raw of nodes as RawNode[]) {
    if (typeof raw.id !== "string" || raw.id === "") return `every node needs a string "id"`
    if (byId.has(raw.id)) return `duplicate node id "${raw.id}"`
    byId.set(raw.id, raw)
  }
  const liveChildrenOf = new Map<string | null, string>() // parentId -> first non-draft child id seen
  for (const raw of nodes as RawNode[]) {
    if (raw.parent !== null && raw.parent !== undefined) {
      if (raw.parent === raw.id) return `node "${raw.id as string}": "parent" cannot be its own id`
      if (typeof raw.parent !== "string" || !byId.has(raw.parent)) {
        return `node "${raw.id as string}": "parent" does not resolve to any node in this file`
      }
      // A non-draft node's parent must be non-draft too -- otherwise the child is unreachable
      // (not a root, and not any served node's child once `filterDrafts` strips its draft
      // parent), disappearing silently instead of erroring the way this validator otherwise
      // promises to.
      if (raw.status !== "draft" && byId.get(raw.parent)?.status === "draft") {
        return `node "${raw.id as string}": parent "${raw.parent}" is still a draft -- a non-draft node cannot have a draft parent`
      }
    }
    if (typeof raw.status !== "string" || !(STATUSES as readonly string[]).includes(raw.status)) {
      return `node "${raw.id as string}": unrecognized "status" (must be one of ${STATUSES.join(", ")})`
    }
    if (typeof raw.title !== "string" || raw.title === "") return `node "${raw.id as string}": needs a "title" string`
    if (typeof raw.question !== "string" || raw.question === "") {
      return `node "${raw.id as string}": needs a "question" string`
    }
    if (raw.status === "superseded" && (typeof raw.supersededNote !== "string" || raw.supersededNote === "")) {
      return `node "${raw.id as string}": status "superseded" needs a "supersededNote"`
    }
    if (raw.status === "answered") {
      const answer = raw.answer as RawAnswer | undefined
      if (typeof answer !== "object" || answer === null || typeof answer.content !== "string" || answer.content === "") {
        return `node "${raw.id as string}": status "answered" needs an "answer" with a "content" string`
      }
      if (answer.sourceOption !== null && typeof answer.sourceOption !== "string") {
        return `node "${raw.id as string}": "answer.sourceOption" must be a string or null`
      }
      if (typeof answer.wasEdited !== "boolean") {
        return `node "${raw.id as string}": "answer.wasEdited" must be a boolean`
      }
    }
    if (raw.options !== undefined) {
      if (!Array.isArray(raw.options)) return `node "${raw.id as string}": "options" must be a list`
      const labels = new Set<string>()
      for (const opt of raw.options as unknown[]) {
        const o = opt as { label?: unknown; content?: unknown } | null
        if (
          typeof o !== "object" ||
          o === null ||
          typeof o.label !== "string" ||
          o.label === "" ||
          typeof o.content !== "string"
        ) {
          return `node "${raw.id as string}": every option needs a non-empty "label" and a "content" string`
        }
        // Labels double as the answer's `sourceOption` and the composer's React key -- a
        // duplicate would make two different options indistinguishable in the submitted answer.
        if (labels.has(o.label)) return `node "${raw.id as string}": duplicate option label "${o.label}"`
        labels.add(o.label)
      }
    }
    // The agent should only ever promote one branch per answer (documented, not built as a UI
    // constraint -- see the design plan's Deferred section) -- but two `live`/`answered` nodes
    // sharing a parent by mistake would otherwise just silently vanish one of them from the chain
    // with nothing on disk to say why, the same "no signal to notice by" failure class supersede's
    // own sibling rule exists to avoid. `superseded` is deliberately excluded from this check: a
    // superseded node coexisting with a `live` replacement under the SAME parent is the correct,
    // intended shape (see the supersede-and-replace note above), not a conflict.
    if ((raw.status === "live" || raw.status === "answered") && raw.parent !== undefined) {
      const parentKey = (raw.parent ?? null) as string | null
      const existing = liveChildrenOf.get(parentKey)
      if (existing) {
        return `node "${raw.id as string}" and "${existing}" are both ${raw.status} under the same parent -- promote only one branch`
      }
      liveChildrenOf.set(parentKey, raw.id as string)
    }
  }

  if ((parsed as RawForest).activity !== undefined) {
    const activity = (parsed as RawForest).activity
    if (!Array.isArray(activity)) return `"activity" must be a list`
    for (const raw of activity as RawActivityEntry[]) {
      if (raw.parent !== null && (typeof raw.parent !== "string" || !byId.has(raw.parent))) {
        return `an "activity" entry's "parent" must be null or resolve to a node in this file`
      }
      if (typeof raw.note !== "string" || raw.note === "") return `an "activity" entry needs a non-empty "note" string`
      if (typeof raw.since !== "string" || raw.since === "") return `an "activity" entry needs a "since" string`
    }
  }
  return null
}

/** Strips every `draft` node -- the plan-ahead surface a draft is never served, initial fetch or
 *  otherwise. Activity entries pass through unchanged; they never carry draft content themselves.
 *  Deliberately doesn't return the file's own internal `topic:` field -- the response always uses
 *  the query-derived topic instead (see the caller), same trust-the-URL convention
 *  grill-rounds.ts uses, since a renamed file's internal field could go stale. */
function filterDrafts(parsed: RawForest): { activity: unknown; nodes: RawNode[] } {
  const nodes = (parsed.nodes as RawNode[]).filter((n) => n.status !== "draft")
  return { activity: parsed.activity ?? [], nodes }
}

export function grillForest(): Plugin {
  const dir = path.resolve(import.meta.dirname, "..", "..", "docs", ".grill")
  // Last good filtered response per topic, so a read racing a coordinator write (a parse failure
  // or a momentarily-invalid mid-edit file) serves stale-but-valid content instead of a 500 --
  // there's no persistent connection here to fall back to, just the next poll.
  const lastGood = new Map<string, unknown>()

  return {
    name: "grill-forest",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__grill-forest/topics", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        let topics: string[] = []
        try {
          topics = (await readdir(dir))
            .filter((f) => f.endsWith(".forest.yaml"))
            .map((f) => f.slice(0, -".forest.yaml".length))
            .sort()
        } catch {
          // No docs/.grill directory yet -- no topics, not an error.
        }
        res.statusCode = 200
        res.setHeader("Content-Type", "application/json")
        res.end(JSON.stringify({ topics }))
      })

      server.middlewares.use("/__grill-forest/round", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        const query = new URLSearchParams((req.url ?? "").split("?")[1] ?? "")
        const topic = query.get("topic") ?? ""
        // A topic is a URL query parameter with no other access control in front of it -- resolve
        // through path.join and check it stays under `dir`, same guard grill-rounds.ts uses.
        const file = path.join(dir, `${topic}.forest.yaml`)
        if (!topic || path.dirname(file) !== dir) {
          res.statusCode = 400
          res.end("invalid topic")
          return
        }
        let text: string
        try {
          text = await readFile(file, "utf8")
        } catch {
          res.statusCode = 404
          res.end(`no forest for topic "${topic}"`)
          return
        }
        let parsed: unknown
        try {
          parsed = parse(text)
        } catch (e) {
          const cached = lastGood.get(topic)
          if (cached) {
            res.statusCode = 200
            res.setHeader("Content-Type", "application/json")
            res.end(JSON.stringify(cached))
            return
          }
          res.statusCode = 400
          res.end(`${file} is not valid YAML: ${e instanceof Error ? e.message : String(e)}`)
          return
        }
        // Unlike a YAML parse failure (above), a validation failure means the file parsed fine
        // but its content doesn't match the schema -- a real authoring mistake, not a momentary
        // mid-write race. Falling back to `lastGood` here was tried and is wrong: a coordinator
        // writing an `answer` in the wrong shape (a bare string instead of the
        // `{sourceOption, content, wasEdited, answeredAt}` object) parsed as valid YAML, so this
        // branch ran, found a cached "live" response, and silently kept serving THAT forever --
        // the client never saw the node as answered, never saw an error either, and the human's
        // own already-submitted local pending answer just sat showing "Sending..." with nothing
        // on screen to explain why. A parseable-but-invalid file gets a real error every time,
        // not a silent stale substitute.
        const error = validateForest(parsed)
        if (error) {
          res.statusCode = 400
          res.end(`${file}: ${error}`)
          return
        }
        const body = { topic, ...filterDrafts(parsed as RawForest) }
        lastGood.set(topic, body)
        res.statusCode = 200
        res.setHeader("Content-Type", "application/json")
        res.end(JSON.stringify(body))
      })
    },
  }
}
