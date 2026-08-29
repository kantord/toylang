import { useEffect, useState } from "react"

import { BoardPage } from "@/components/BoardPage"
import { ExamplesPage } from "@/components/ExamplesPage"
import { Markdown } from "@/components/Markdown"
import { loadCorpus, type Corpus } from "@/lib/corpus"
import { href, PAGES, type Page, type Section } from "@/lib/docs"
import { cn } from "@/lib/utils"

/** The four sections, each with one job (plans/docs-site.md): a linear course, task-oriented
 *  feature pages, the complete reference, and the corpus browser -- plus Board, the project
 *  status view (kantord/toylang#31), which isn't part of that Diataxis split. */
const SECTIONS: { key: Section | "examples" | "board"; label: string }[] = [
  { key: "tutorial", label: "Tutorial" },
  { key: "guides", label: "Guides" },
  { key: "reference", label: "Reference" },
  { key: "examples", label: "Examples" },
  { key: "board", label: "Board" },
]

function useHash(): string {
  const [hash, setHash] = useState(location.hash)
  useEffect(() => {
    const on = () => setHash(location.hash)
    window.addEventListener("hashchange", on)
    return () => window.removeEventListener("hashchange", on)
  }, [])
  return hash
}

export default function App() {
  const [corpus, setCorpus] = useState<Corpus | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [backend, setBackend] = useState("js")
  const [annotate, setAnnotate] = useState(false)
  const hash = useHash()

  useEffect(() => {
    loadCorpus()
      .then(setCorpus)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
  }, [])

  if (error) {
    return (
      <main className="mx-auto max-w-2xl p-10">
        <h1 className="text-lg font-semibold">The corpus did not load</h1>
        <p className="mt-2 text-sm text-muted-foreground">{error}</p>
      </main>
    )
  }
  if (!corpus) {
    return <main className="p-10 text-sm text-muted-foreground">Loading...</main>
  }

  // Annotations-mode jump links append `?b=<block>` to a normal route; it never appears
  // outside that mode, so plain reading routes are untouched by the split.
  const [hashPath, hashQuery] = hash.replace(/^#\/?/, "").split("?")
  const segments = hashPath.split("/").filter(Boolean)
  const jumpBlock = hashQuery ? Number(new URLSearchParams(hashQuery).get("b")) : undefined
  const scrollToBlock = jumpBlock !== undefined && Number.isFinite(jumpBlock) ? jumpBlock : undefined

  // Pre-Diataxis links were bare case names (`#greet`); they keep working under Examples.
  if (segments.length === 1 && corpus.cases.some((c) => c.name === segments[0])) {
    location.hash = `#/examples/${segments[0]}`
    return null
  }

  const section = segments[0] ?? ""
  let body
  if (section === "board") {
    body = <BoardPage />
  } else if (section === "grill-wizard") {
    // The structured wizard (kantord/toylang#34), a different beast from the "grill" section
    // below: that one renders free-form markdown rounds through the normal docs pipeline, this
    // one has its own screen-by-screen UI over a YAML round file with no page/prose model at all.
    body = <GrillWizardRoute topic={segments[1]} />
  } else if (section === "mail") {
    // The mail app (kantord/toylang#41): a dedicated route rather than the old embedded sidebar,
    // since a three-pane mail client needs the whole area, not a 220px aside column.
    body = <MailAppRoute onOpenPage={() => setAnnotate(true)} />
  } else if (section === "examples" && segments[1] === "euler") {
    // The Euler stress-test stream: markdown pages under docs/examples/euler, out of the
    // corpus browser's flat case-name routing (kantord/toylang#35).
    body = (
      <DocsSection
        section="examples"
        segments={segments.slice(1)}
        corpus={corpus}
        annotate={annotate}
        scrollToBlock={scrollToBlock}
      />
    )
  } else if (section === "examples") {
    body = (
      <ExamplesPage
        corpus={corpus}
        selected={segments[1] ?? corpus.cases[0].name}
        onSelect={(name) => (location.hash = `#/examples/${encodeURIComponent(name)}`)}
        backend={backend}
        onBackend={setBackend}
      />
    )
  } else if (
    section === "tutorial" ||
    section === "guides" ||
    section === "reference" ||
    section === "grill"
  ) {
    body = (
      <DocsSection
        section={section}
        segments={segments.slice(1)}
        corpus={corpus}
        annotate={annotate}
        scrollToBlock={scrollToBlock}
      />
    )
  } else {
    // The landing route: the tutorial is where a new reader starts; before it has chapters,
    // the browser everyone already links to.
    const first = PAGES.find((p) => p.section === "tutorial")
    location.hash = first ? href(first) : "#/examples"
    return null
  }

  return (
    <div className="mx-auto flex min-h-screen max-w-[1500px] flex-col gap-6 p-6">
      <header className="flex flex-wrap items-baseline gap-x-6 gap-y-2">
        <h1 className="text-xl font-semibold tracking-tight">toylang</h1>
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
        {import.meta.env.DEV && (
          <a
            href="#/grill-wizard"
            className={cn(
              "ml-auto rounded-md border px-2 py-1 text-xs font-medium",
              section === "grill-wizard"
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-muted-foreground hover:text-foreground",
            )}
          >
            Grill
          </a>
        )}
        {import.meta.env.DEV && (
          <a
            href="#/mail"
            className={cn(
              "rounded-md border px-2 py-1 text-xs font-medium",
              section === "mail"
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-muted-foreground hover:text-foreground",
            )}
          >
            Mail
          </a>
        )}
        {import.meta.env.DEV && (
          <button
            type="button"
            onClick={() => setAnnotate((v) => !v)}
            className={cn(
              "rounded-md border px-2 py-1 text-xs font-medium",
              annotate
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-muted-foreground hover:text-foreground",
            )}
          >
            Annotations{annotate ? " (on)" : ""}
          </button>
        )}
      </header>

      {body}
    </div>
  )
}

function DocsSection({
  section,
  segments,
  corpus,
  annotate = false,
  scrollToBlock,
}: {
  section: Section
  segments: string[]
  corpus: Corpus
  annotate?: boolean
  scrollToBlock?: number
}) {
  const pages = PAGES.filter((p) => p.section === section)
  if (pages.length === 0) {
    return <p className="text-sm text-muted-foreground">Nothing here yet.</p>
  }

  const [group, slug] = segments.length > 1 ? segments : ["", segments[0]]
  const current = pages.find((p) => p.group === group && p.slug === slug) ?? pages[0]

  // The reference is grouped by its subdirectories (builtins, types, ...); the flat sections
  // have a single unlabeled group.
  const groups = [...new Set(pages.map((p) => p.group))].map((g) => ({
    name: g,
    pages: pages.filter((p) => p.group === g),
  }))

  return (
    <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[220px_minmax(0,1fr)]">
      <aside className="lg:sticky lg:top-6 lg:self-start">
        <nav className="space-y-4 text-sm">
          {groups.map((g) => (
            <div key={g.name} className="space-y-1">
              {g.name && (
                <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  {g.name}
                </div>
              )}
              {g.pages.map((p) => (
                <a
                  key={p.path}
                  href={href(p)}
                  className={cn(
                    "block rounded px-2 py-1 text-muted-foreground hover:bg-muted hover:text-foreground",
                    p === current && "bg-muted font-medium text-foreground",
                  )}
                >
                  {p.title}
                </a>
              ))}
            </div>
          ))}
        </nav>
      </aside>

      <main className="min-w-0">
        <Markdown page={current} corpus={corpus} annotate={annotate} scrollToBlock={scrollToBlock} />
        <PagerLinks pages={pages} current={current} />
      </main>
    </div>
  )
}

/** The wizard's dev-only route (kantord/toylang#34): a dynamic import behind
 *  `import.meta.env.DEV`, exactly like `MailAppRoute` below, so `vite build` never puts
 *  GrillWizard.tsx (or the round-fetching it pulls in) in the production bundle. */
function GrillWizardRoute({ topic }: { topic?: string }) {
  const [mod, setMod] = useState<typeof import("@/components/GrillWizard") | null>(null)
  useEffect(() => {
    if (import.meta.env.DEV) import("@/components/GrillWizard").then(setMod)
  }, [])
  if (!import.meta.env.DEV) {
    return <p className="text-sm text-muted-foreground">The grilling wizard is dev-only.</p>
  }
  if (!mod) return <p className="text-sm text-muted-foreground">Loading...</p>
  // `key` remounts the wizard when the topic changes in place (back/forward between two
  // rounds), so a shorter round can never inherit a step index pointing past its end.
  return topic ? <mod.GrillWizardPage key={topic} topic={topic} /> : <mod.GrillIndexPage />
}

/** The mail app's dev-only route (kantord/toylang#41): same dynamic-import tree-shaking pattern
 *  as the wizard above, so `vite build` never puts MailApp.tsx (or the annotation scan it pulls
 *  in) in the production bundle. `onOpenPage` flips the docs page's own annotate toggle on, so a
 *  "open on page" link lands with the block already editable rather than a plain read. */
function MailAppRoute({ onOpenPage }: { onOpenPage: () => void }) {
  const [mod, setMod] = useState<typeof import("@/components/MailApp") | null>(null)
  useEffect(() => {
    if (import.meta.env.DEV) import("@/components/MailApp").then(setMod)
  }, [])
  if (!import.meta.env.DEV) {
    return <p className="text-sm text-muted-foreground">The mail app is dev-only.</p>
  }
  if (!mod) return <p className="text-sm text-muted-foreground">Loading...</p>
  return <mod.MailApp onOpenPage={onOpenPage} />
}

/** Previous/next within the section, so the tutorial reads as the linear course it is. */
function PagerLinks({ pages, current }: { pages: Page[]; current: Page }) {
  const i = pages.indexOf(current)
  const prev = i > 0 ? pages[i - 1] : null
  const next = i < pages.length - 1 ? pages[i + 1] : null
  if (!prev && !next) return null
  return (
    <div className="mt-10 flex max-w-2xl justify-between border-t pt-4 text-sm">
      <span>{prev && <a href={href(prev)} className="underline">&larr; {prev.title}</a>}</span>
      <span>{next && <a href={href(next)} className="underline">{next.title} &rarr;</a>}</span>
    </div>
  )
}
