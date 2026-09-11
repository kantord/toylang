import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { DevDocsPage } from "@dev/components/DevDocsPage"
import { GrillForestApp } from "@dev/components/GrillForestApp"
import { MailApp } from "@dev/components/MailApp"
import { useOpenGrillCount } from "@dev/lib/grillForest"
import { loadCorpus, type Corpus } from "@/lib/corpus"
import { cn } from "@/lib/utils"

const SECTIONS = [
  { key: "mail", label: "Mail" },
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
 * The dev-only tooling shell (kantord/toylang#50): the mail app -- which now also holds the
 * grill rounds and the board as tabs of its own (kantord/toylang#52) -- and the annotations
 * overlay onto the docs pages, both under this one entry (dev/index.html) so a production build
 * -- which only ever opens ../index.html -- has no path into any of it.
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

  const section = segments[0] ?? "mail"
  let body
  if (section === "annotate") {
    if (!corpus) {
      body = <p className="text-sm text-muted-foreground">Loading...</p>
    } else {
      body = <DevDocsPage corpus={corpus} segments={segments.slice(1)} scrollToBlock={scrollToBlock} />
    }
  } else if (section === "grill") {
    // A `SECTIONS` entry alone would still fall through to `MailApp` below -- this branch is
    // what actually routes `#/grill` anywhere.
    body = <GrillForestApp segments={segments.slice(1)} />
  } else {
    body = <MailApp />
  }

  return (
    <QueryClientProvider client={queryClient}>
      {/* Mail wants the full viewport for its panes (the maintainer's compose ask, b1d3edc);
          the reading-measure cap stays for everything else. */}
      <div className={cn("mx-auto flex min-h-screen flex-col gap-6 p-6", section !== "mail" && "max-w-[1500px]")}>
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
