/**
 * The grill-forest data (kantord/toylang#grill-forest): fetch helpers, React Query hooks, and the
 * local-only "pending answer" record that stands in for a submitted-but-not-yet-processed answer.
 * The pure types and the `activePath`/`threadStatus` walk live in grillForestChain.ts instead of
 * here, re-exported below -- that file has no `@/`/`@dev/` alias imports (matching flow.ts's own
 * discipline) so `node --test` can load it directly; this one needs React Query and the inbox
 * client, which a plain node run can't resolve without a bundler.
 *
 * The coordinator's own processing cadence, not a browser poll interval, is what confirms an
 * answer (it writes the answer into the node and clears the inbox record together, in that
 * order -- see the plan/skill docs) -- that can reasonably take minutes, so a submitted answer is
 * shown from local state, not a query-cache optimistic write that a stale in-flight poll could
 * silently overwrite before real confirmation ever arrives.
 */

import { useQueries, useQuery } from "@tanstack/react-query"

import { saveToInbox } from "@dev/lib/annotations"
import { clearDraft, loadDraft, saveDraft } from "@dev/lib/draft"

import { threadStatus, type ForestRound, type ForestNode } from "./grillForestChain.ts"

export {
  activePath,
  threadStatus,
  type ForestNodeStatus,
  type ForestOption,
  type ForestAnswer,
  type ForestNode,
  type ForestActivityEntry,
  type ForestRound,
  type ChainEntry,
  type ThreadStatus,
} from "./grillForestChain.ts"

/** How often the tools app polls `/__grill-forest/*` (kantord/toylang#grill-forest): the plan's
 *  own "~1-1.5s" target for feeling live, comfortably under what a plain stateless GET/React Query
 *  poll can sustain for a single local user. */
const POLL_INTERVAL_MS = 1200

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) throw new Error(await res.text())
  return res.json() as Promise<T>
}

export function fetchForestTopics(): Promise<string[]> {
  return fetch("/__grill-forest/topics")
    .then((r) => json<{ topics: string[] }>(r))
    .then((r) => r.topics)
}

export function fetchForestRound(topic: string): Promise<ForestRound> {
  return fetch(`/__grill-forest/round?topic=${encodeURIComponent(topic)}`).then((r) => json<ForestRound>(r))
}

/** The inbox page identity a forest's answers are saved under -- mirrors `roundPagePath` in
 *  lib/grill.ts. */
export function forestPagePath(topic: string): string {
  return `docs/.grill/${topic}.forest.yaml`
}

/** Every `.forest.yaml` topic on disk, polled -- a new file the coordinator writes shows up in the
 *  rail without a reload. Default `refetchOnWindowFocus` stays on: a stateless polled GET has no
 *  per-connection lifecycle, so switching back to this tab (or opening the same URL elsewhere on
 *  the LAN) resyncs immediately instead of waiting out the interval. */
export function useForestTopics() {
  return useQuery({ queryKey: ["grill-forest", "topics"], queryFn: fetchForestTopics, refetchInterval: POLL_INTERVAL_MS })
}

/** One topic's forest, polled the same way. The sole steady-state data source for the chain view
 *  -- there is no separate inbox query on the client (see the module doc). */
export function useForestRound(topic: string) {
  return useQuery({
    queryKey: ["grill-forest", "round", topic],
    queryFn: () => fetchForestRound(topic),
    refetchInterval: POLL_INTERVAL_MS,
    enabled: topic !== "",
  })
}

/** The nav tab's "N open" badge (kantord/toylang#grill-forest): costs no new plumbing since it's
 *  the same per-topic round payload the topic rail already polls, just fetched here too via
 *  `useQueries` (a dynamic-length list of topics can't drive a fixed number of `useQuery` calls,
 *  which the rules of hooks require). Lives in `DevApp.tsx`'s nav, which is always mounted, so the
 *  `QueryClientProvider` above it needs to wrap all of `DevApp`, not just the grill section --
 *  otherwise this hook would have nothing to read from while a different section is open. */
export function useOpenGrillCount(): number {
  const topics = useForestTopics().data ?? []
  const rounds = useQueries({
    queries: topics.map((topic) => ({
      queryKey: ["grill-forest", "round", topic],
      queryFn: () => fetchForestRound(topic),
      refetchInterval: POLL_INTERVAL_MS,
    })),
  })
  return rounds.filter((r) => r.data && threadStatus(r.data) === "waiting").length
}

/** A human's own submitted answer, kept purely client-side until a real poll of
 *  `/__grill-forest/round` shows the coordinator has durably processed it (`status: answered`) --
 *  see the module doc for why this isn't a query-cache optimistic write. Keyed `${topic}:${nodeId}`
 *  since node ids are only unique within one topic file. */
export interface PendingAnswer {
  sourceOption: string | null
  content: string
  wasEdited: boolean
  submittedAt: string
}

function pendingKey(topic: string, nodeId: string): string {
  return `toylang-grill-forest-pending:${topic}:${nodeId}`
}

export function loadPendingAnswer(topic: string, nodeId: string): PendingAnswer | null {
  return loadDraft<PendingAnswer | null>(pendingKey(topic, nodeId), null)
}

export function clearPendingAnswer(topic: string, nodeId: string) {
  clearDraft(pendingKey(topic, nodeId))
}

/** Writes the local pending record synchronously (so the answer bubble appears before the network
 *  round-trip even starts), then posts to the inbox -- the same delivery door wizard rounds use.
 *  A failed POST leaves the pending record in place; the caller re-shows an error and a retry
 *  re-attempts the same POST (idempotent: the inbox dedups by page+block). */
export async function submitForestAnswer(
  topic: string,
  node: ForestNode,
  answer: { sourceOption: string | null; content: string; wasEdited: boolean },
): Promise<void> {
  saveDraft(pendingKey(topic, node.id), { ...answer, submittedAt: new Date().toISOString() } satisfies PendingAnswer)
  await saveToInbox(
    {
      page: forestPagePath(topic),
      block: node.id,
      original: node.title,
      edited: JSON.stringify(answer),
    },
    `submit failed for "${node.title}"`,
  )
}
