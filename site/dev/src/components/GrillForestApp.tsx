import { useEffect, useRef } from "react"

import { GrillChain } from "@dev/components/GrillChain"
import { activePath, threadStatus, useForestRound, useForestTopics, type ThreadStatus } from "@dev/lib/grillForest"
import { cn } from "@/lib/utils"

const STATUS_DOT: Record<ThreadStatus, string> = {
  waiting: "bg-primary",
  thinking: "bg-amber-500 animate-pulse",
  idle: "bg-muted-foreground/40",
}

function TopicRow({ topic, active }: { topic: string; active: boolean }) {
  const { data } = useForestRound(topic)
  const status = data ? threadStatus(data) : "idle"
  return (
    <a
      href={`#/grill/${topic}`}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm",
        active ? "bg-muted font-medium text-foreground" : "text-muted-foreground hover:bg-muted/60",
      )}
    >
      <span className={cn("size-1.5 shrink-0 rounded-full", STATUS_DOT[status])} />
      <span className="flex-1 truncate">{topic}</span>
    </a>
  )
}

/**
 * The grill-forest view (kantord/toylang#grill-forest): a topic rail (one entry per
 * `docs/.grill/*.forest.yaml` file) plus the selected topic's chain. v1 scope treats one file as
 * one thread -- there is no root-switcher for the rarer multi-root case. `MailApp.tsx` is
 * untouched; this is its own section of `DevApp.tsx`, not a tab inside the mail app.
 */
export function GrillForestApp({ segments }: { segments: string[] }) {
  const { data: topics = [], isLoading: topicsLoading } = useForestTopics()
  const topicFromUrl = segments[0]
  const nodeFromUrl = segments[1]
  const selected = topicFromUrl && topics.includes(topicFromUrl) ? topicFromUrl : (topics[0] ?? null)

  const { data: round, error, isLoading } = useForestRound(selected ?? "")

  // Keeps the URL naming the current node, not just the topic, without a full navigation --
  // `replaceState` (not `location.hash =`) so this never re-triggers DevApp's own hashchange
  // listener and loops. This only runs at all when the page was opened WITHOUT a specific node in
  // the URL: a link that names one (a bookmark, a share, `nodeFromUrl`) is a request to look at
  // that exact point, and has to stay put in the address bar for the rest of this page's
  // lifetime -- the chain itself still renders everything since, live updates included, only the
  // URL stops following it. Captured once on mount so navigating between topics via the rail
  // (which links to a bare `#/grill/<topic>`) still gets ordinary tail-following for the newly
  // selected topic.
  const pinnedToExplicitNode = useRef(!!nodeFromUrl)
  // Seeded from the URL's own initial topic segment, not `null` -- otherwise the very first time
  // `round` data arrives (any real topic differing from `null`) would look exactly like a topic
  // switch and immediately clear the pin this effect exists to hold.
  const lastSyncedTopic = useRef<string | null>(topicFromUrl ?? null)
  useEffect(() => {
    if (!round) return
    // A topic switch (via the rail) resets the pin for the newly selected topic -- `nodeFromUrl`
    // was only ever meaningful for the topic the page first loaded with. This has to run BEFORE
    // the pin check below, not after: `GrillForestApp` isn't remounted on a hash change (`DevApp`
    // renders it unkeyed), so once pinned, a check ordered the other way around can never reach
    // the code that would unpin it -- dead code that looked like it worked because nothing
    // exercised a topic switch while testing the pin itself.
    if (round.topic !== lastSyncedTopic.current) {
      pinnedToExplicitNode.current = false
      lastSyncedTopic.current = round.topic
    }
    if (pinnedToExplicitNode.current) return
    const path = activePath(round.nodes, round.activity)
    const tail = [...path].reverse().find((e) => e.kind === "node")
    const nextHash = tail?.kind === "node" ? `#/grill/${round.topic}/${tail.node.id}` : `#/grill/${round.topic}`
    if (location.hash !== nextHash) history.replaceState(null, "", nextHash)
  }, [round])

  return (
    <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[200px_minmax(0,1fr)]">
      <aside className="space-y-1">
        {!topicsLoading && topics.length === 0 && <p className="px-2 text-xs text-muted-foreground">No grilling topics yet.</p>}
        {topics.map((topic) => (
          <TopicRow key={topic} topic={topic} active={topic === selected} />
        ))}
      </aside>
      <main className="min-h-0 overflow-y-auto rounded-md border p-4">
        {!selected && !topicsLoading && <p className="text-sm text-muted-foreground">Select a topic.</p>}
        {selected && isLoading && <p className="text-sm text-muted-foreground">Loading...</p>}
        {selected && error && (
          <div className="rounded-sm border-l-4 border-destructive py-1 pl-2 text-xs text-destructive">
            {error instanceof Error ? error.message : String(error)}
          </div>
        )}
        {selected && round && <GrillChain topic={selected} round={round} scrollToNodeId={nodeFromUrl} />}
      </main>
    </div>
  )
}
