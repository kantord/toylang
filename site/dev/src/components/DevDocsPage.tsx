import { useMemo } from "react"

import { AnnotatedProseBlock, pageAnnotations, saveEdit } from "@dev/components/AnnotateMode"
import { annotateHref } from "@dev/lib/nav"
import { Fence } from "@/components/Markdown"
import { splitBlocks } from "@/lib/blocks"
import type { Corpus } from "@/lib/corpus"
import { PAGES, type Page, type Section } from "@/lib/docs"
import { embeddedCasesFor } from "@/lib/pageData"
import { cn } from "@/lib/utils"

const SECTIONS: Section[] = ["overview", "tutorial", "guides", "reference", "grill", "examples"]

/**
 * The dev-only annotations mode (kantord/toylang#23), now its own page rather than a toggle on
 * the production docs viewer: annotate mode is always on here, and this is the only place
 * `AnnotateMode.tsx` is reachable from, so `vite build` never has a reason to look at it
 * (kantord/toylang#50 -- see vite.config.ts's comment on why that is now structural).
 */
export function DevDocsPage({
  corpus,
  segments,
  scrollToBlock,
}: {
  corpus: Corpus
  segments: string[]
  scrollToBlock?: number
}) {
  const section = (segments[0] as Section) ?? "tutorial"
  const rest = segments.slice(1)
  const pages = PAGES.filter((p) => p.section === section)
  if (pages.length === 0) {
    return <p className="text-sm text-muted-foreground">Nothing here yet.</p>
  }

  const [group, slug] = rest.length > 1 ? rest : ["", rest[0]]
  const current = pages.find((p) => p.group === group && p.slug === slug) ?? pages[0]

  const groups = [...new Set(pages.map((p) => p.group))].map((g) => ({
    name: g,
    pages: pages.filter((p) => p.group === g),
  }))

  return (
    <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[220px_minmax(0,1fr)]">
      <aside className="space-y-4 lg:sticky lg:top-6 lg:self-start">
        <nav className="flex flex-wrap gap-2 text-xs">
          {SECTIONS.map((s) => (
            <a
              key={s}
              href={`#/annotate/${s}`}
              className={cn(
                "rounded border px-2 py-1",
                s === section ? "border-primary text-primary" : "text-muted-foreground",
              )}
            >
              {s}
            </a>
          ))}
        </nav>
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
                  href={annotateHref(p)}
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
        <AnnotatedMarkdown page={current} corpus={corpus} scrollToBlock={scrollToBlock} />
      </main>
    </div>
  )
}

function AnnotatedMarkdown({
  page,
  corpus,
  scrollToBlock,
}: {
  page: Page
  corpus: Corpus
  scrollToBlock?: number
}) {
  const blocks = useMemo(() => splitBlocks(page), [page])
  const cases = useMemo(() => embeddedCasesFor(page, corpus), [page, corpus])
  const annotations = useMemo(() => pageAnnotations(page, blocks), [page, blocks])

  return (
    <article className="docs-prose min-w-0 max-w-2xl">
      {blocks.map((b, i) =>
        b.kind === "fence" ? (
          <Fence key={i} token={b.token} cases={cases} />
        ) : (
          <AnnotatedProseBlock
            key={i}
            page={page}
            block={i}
            pieces={b.pieces}
            scrollTo={scrollToBlock === i}
            annotations={annotations.filter((a) => a.block === i)}
            onEdited={(edited, original) => saveEdit(page, i, original, edited)}
          />
        ),
      )}
    </article>
  )
}
