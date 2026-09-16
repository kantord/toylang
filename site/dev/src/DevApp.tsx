import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { BoardPage } from "@dev/components/BoardPage"
import { DevDocsPage } from "@dev/components/DevDocsPage"
import { GrillForestApp } from "@dev/components/GrillForestApp"
import { RunPage } from "@dev/components/RunPage"
import { useOpenGrillCount } from "@dev/lib/grillForest"
import { loadCorpus, type Corpus } from "@/lib/corpus"
import { cn } from "@/lib/utils"

const SECTIONS = [
  { key: "board", label: "Board" },
  { key: "annotate", label: "Annotations" },
  { key: "grill", label: "Grill" },
] as const

// Must match `server.port` in vite.tools.config.ts. Duplicated rather than shared -- it's one
// number, read from two places (a Node-side Vite config and browser code) that would otherwise
// need a whole shared module for it.
const TOOLS_PORT = "5180"

/** Nothing stops `dev/index.html` from still loading under the plain docs dev server (`pnpm
 *  dev`), which no longer serves `/__annotations/*`/`/__grill*` at all since the dev-server split
 *  -- every fetch here is written to soft-fail ("no rounds"/empty list), so without this the tab
 *  would misleadingly read as "nothing to grill" rather than "wrong server." */
function WrongServerBanner() {
  if (location.port === TOOLS_PORT) return null
  return (
    <p className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-1.5 text-xs text-amber-700 dark:text-amber-300">
      This looks like the docs dev server (port {location.port || "80"}) -- run{" "}
      <code className="font-mono">pnpm dev:tools</code> instead (port {TOOLS_PORT}).
    </p>
  )
}

const queryClient = new QueryClient()

function GrillNavBadge() {
  const count = useOpenGrillCount()
  if (count === 0) return null
  return <span className="ml-1 rounded-full bg-primary/15 px-1.5 text-xs text-primary">{count}</span>
}

function useHash(): string {
  const [hash, setHash] = useState(location.hash)
  useEffect(() => {
    const on = () => setHash(location.hash)
    window.addEventListener("hashchange", on)
    return () => window.removeEventListener("hashchange", on)
  }, [])
  return hash
}

/**
 * The dev-only tooling shell (kantord/toylang#50): the board, the annotations overlay onto the
 * docs pages, and the grill-forest rounds, each a top-level section of this one entry
 * (dev/index.html) so a production build -- which only ever opens ../index.html -- has no path
 * into any of it. Grilling lives only in the Grill tab's forest rounds; the Board is its own
 * tab (`#/board`) and hosts plan approval.
 */
export function DevApp() {
  const [corpus, setCorpus] = useState<Corpus | null>(null)
  const hash = useHash()

  useEffect(() => {
    loadCorpus().then(setCorpus)
  }, [])

  const [hashPath, hashQuery] = hash.replace(/^#\/?/, "").split("?")
  const segments = hashPath.split("/").filter(Boolean)
  const jumpBlock = hashQuery ? Number(new URLSearchParams(hashQuery).get("b")) : undefined
  const scrollToBlock = jumpBlock !== undefined && Number.isFinite(jumpBlock) ? jumpBlock : undefined

  const section = segments[0] ?? "board"
  let body
  if (section === "annotate") {
    if (!corpus) {
      body = <p className="text-sm text-muted-foreground">Loading...</p>
    } else {
      body = <DevDocsPage corpus={corpus} segments={segments.slice(1)} scrollToBlock={scrollToBlock} />
    }
  } else if (section === "grill") {
    // A `SECTIONS` entry alone would still fall through to the default below -- this branch is
    // what actually routes `#/grill` anywhere.
    body = <GrillForestApp segments={segments.slice(1)} />
  } else if (section === "board") {
    body = <BoardPage />
  } else if (section === "run" && segments.length === 3) {
    // Reached from a delegated card's run link, not from the nav: a run page is a detail view of
    // one board row, so it does not get a SECTIONS entry of its own.
    body = <RunPage rowId={segments[1]} runId={segments[2]} />
  } else {
    body = <BoardPage />
  }

  return (
    <QueryClientProvider client={queryClient}>
      <div className={cn("mx-auto flex min-h-screen max-w-[1500px] flex-col gap-6 p-6")}>
        <header className="flex flex-wrap items-baseline gap-x-6 gap-y-2">
          <h1 className="text-xl font-semibold tracking-tight">toylang dev</h1>
          <nav className="flex gap-4 text-sm">
            {SECTIONS.map((s) => (
              <a
                key={s.key}
                href={`#/${s.key}`}
                className={cn(
                  "flex items-center text-muted-foreground hover:text-foreground",
                  section === s.key && "font-medium text-foreground",
                )}
              >
                {s.label}
                {s.key === "grill" && <GrillNavBadge />}
              </a>
            ))}
          </nav>
          <WrongServerBanner />
        </header>

        {body}
      </div>
    </QueryClientProvider>
  )
}
