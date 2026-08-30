import { ExamplesPage } from "@/components/ExamplesPage"
import { Markdown } from "@/components/Markdown"
import { GROUP_LABELS, href, PAGES, type Page, type Section } from "@/lib/docs"
import type { AppRoute, EmbeddedCaseData } from "@/lib/pageData"
import { withBase } from "@/lib/route"
import { cn } from "@/lib/utils"

/** The four sections, each with one job (plans/docs-site.md): a linear course, task-oriented
 *  feature pages, the complete reference, and the corpus browser. Board, Mail, and the grill
 *  wizard are dev-only tooling (kantord/toylang#50) and live entirely under dev/, which this
 *  file never imports. */
const SECTIONS: { key: Section | "examples"; label: string }[] = [
  { key: "overview", label: "Overview" },
  { key: "tutorial", label: "Tutorial" },
  { key: "guides", label: "Guides" },
  { key: "reference", label: "Reference" },
  { key: "examples", label: "Examples" },
]

/**
 * One page, given exactly what it needs to render (lib/pageData.ts computes `route`): no
 * routing, no data fetching, no knowledge of how it got here. A first load reaches it through
 * entry-client.tsx reading the page's own prerendered globals (kantord/toylang#50); a click on
 * one of the links below reaches it again through clientNav.tsx fetching the target page's
 * globals the same way and swapping `route` in place (kantord/toylang#55), without a real
 * navigation. Either way, remounting on every route is what keeps this component from needing
 * to reconcile one page's local state with the next.
 */
export function App({ route }: { route: AppRoute }) {
  const activeSection = route.kind === "examples" ? "examples" : route.page.section

  const body =
    route.kind === "examples" ? (
      <ExamplesPage current={route.current} index={route.index} backends={route.backends} />
    ) : (
      <DocsSection section={route.page.section} current={route.page} cases={route.cases} />
    )

  return (
    <div className="mx-auto flex min-h-screen max-w-[1500px] flex-col gap-6 p-6">
      <header className="flex flex-wrap items-baseline gap-x-6 gap-y-2">
        <h1 className="text-xl font-semibold tracking-tight">toylang</h1>
        <nav className="flex gap-4 text-sm">
          {SECTIONS.map((s) => (
            <a
              key={s.key}
              href={withBase(s.key === "examples" ? "/examples/" : `/${s.key}/`)}
              className={cn(
                "text-muted-foreground hover:text-foreground",
                activeSection === s.key && "font-medium text-foreground",
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

/** How a docs group's raw directory name reads in the sidebar. Only the Euler stream needs a
 *  friendlier label (kantord/toylang#70): everything else already spells its directory name
 *  the way a reader would say it. */

function DocsSection({
  section,
  current,
  cases,
}: {
  section: Section
  current: Page
  cases: Record<string, EmbeddedCaseData>
}) {
  const pages = PAGES.filter((p) => p.section === section)

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
          {section === "examples" && (
            <div className="space-y-1">
              <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Test cases
              </div>
              <a
                href={withBase("/examples/")}
                className="block rounded px-2 py-1 text-muted-foreground hover:bg-muted hover:text-foreground"
              >
                Browse the corpus
              </a>
            </div>
          )}
          {groups.map((g) => (
            <div key={g.name} className="space-y-1">
              {g.name && (
                <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  {GROUP_LABELS[g.name] ?? g.name}
                </div>
              )}
              {g.pages.map((p) => (
                <a
                  key={p.path}
                  href={withBase(href(p))}
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
        <Markdown page={current} cases={cases} />
        <PagerLinks pages={pages} current={current} />
      </main>
    </div>
  )
}

/** Previous/next within the section, so the tutorial reads as the linear course it is. Also
 *  the pair the prerender script (scripts/prerender.mjs) hints as `<link rel="prefetch">` on
 *  this exact page, since they are the two places a reader is most likely to go next. */
export function PagerLinks({ pages, current }: { pages: Page[]; current: Page }) {
  const i = pages.indexOf(current)
  const prev = i > 0 ? pages[i - 1] : null
  const next = i < pages.length - 1 ? pages[i + 1] : null
  if (!prev && !next) return null
  return (
    <div className="mt-10 flex max-w-2xl justify-between border-t pt-4 text-sm">
      <span>
        {prev && (
          <a href={withBase(href(prev))} className="underline">
            &larr; {prev.title}
          </a>
        )}
      </span>
      <span>
        {next && (
          <a href={withBase(href(next))} className="underline">
            {next.title} &rarr;
          </a>
        )}
      </span>
    </div>
  )
}
