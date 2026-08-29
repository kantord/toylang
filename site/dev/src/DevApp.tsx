import { useEffect, useState } from "react"

import { BoardPage } from "@dev/components/BoardPage"
import { DevDocsPage } from "@dev/components/DevDocsPage"
import { GrillIndexPage, GrillWizardPage } from "@dev/components/GrillWizard"
import { MailApp } from "@dev/components/MailApp"
import { loadCorpus, type Corpus } from "@/lib/corpus"
import { cn } from "@/lib/utils"

const SECTIONS = [
  { key: "mail", label: "Mail" },
  { key: "grill-wizard", label: "Grill" },
  { key: "board", label: "Board" },
  { key: "annotate", label: "Annotations" },
] as const

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
 * The dev-only tooling shell (kantord/toylang#50): mail app, grill wizard, board, and the
 * annotations overlay onto the docs pages, all under this one entry (dev/index.html) so a
 * production build -- which only ever opens ../index.html -- has no path into any of it.
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
  if (section === "grill-wizard") {
    body = segments[1] ? <GrillWizardPage key={segments[1]} topic={segments[1]} /> : <GrillIndexPage />
  } else if (section === "board") {
    body = <BoardPage />
  } else if (section === "annotate") {
    if (!corpus) {
      body = <p className="text-sm text-muted-foreground">Loading...</p>
    } else {
      body = <DevDocsPage corpus={corpus} segments={segments.slice(1)} scrollToBlock={scrollToBlock} />
    }
  } else {
    body = <MailApp />
  }

  return (
    // Mail wants the full viewport for its panes (the maintainer's compose ask, b1d3edc);
    // the reading-measure cap stays for everything else.
    <div className={cn("mx-auto flex min-h-screen flex-col gap-6 p-6", section !== "mail" && "max-w-[1500px]")}>
      <header className="flex flex-wrap items-baseline gap-x-6 gap-y-2">
        <h1 className="text-xl font-semibold tracking-tight">toylang dev</h1>
        <nav className="flex gap-4 text-sm">
          {SECTIONS.map((s) => (
            <a
              key={s.key}
              href={`#/${s.key}`}
              className={cn(
                "text-muted-foreground hover:text-foreground",
                section === s.key && "font-medium text-foreground",
              )}
            >
              {s.label}
            </a>
          ))}
        </nav>
      </header>

      {body}
    </div>
  )
}
