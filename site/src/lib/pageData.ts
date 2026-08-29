/**
 * What one production route needs to render, and nothing more (kantord/toylang#50): the static
 * build embeds exactly this per page rather than the whole corpus (1.3MB, mostly seven backends'
 * worth of emitted code no single page shows), and the same functions run the prerender script
 * (server) and the dev-mode fallback (client, when there is no prerendered data to read).
 */
import type { Case, CaseSummary, Corpus } from "@/lib/corpus"
import { summarize } from "@/lib/corpus"
import { splitBlocks } from "@/lib/blocks"
import { href, PAGES, type Page } from "@/lib/docs"
import { stripBase } from "@/lib/route"

/** The `<case>` fence's-worth of a corpus case: enough for `EmbeddedCase` (Markdown.tsx), never
 *  the `emitted` field, which no docs page shows. */
export type EmbeddedCaseData = Pick<Case, "name" | "program" | "input" | "expect">

export type AppRoute =
  | { kind: "docs"; page: Page; cases: Record<string, EmbeddedCaseData> }
  | { kind: "examples"; current: Case; index: CaseSummary[]; backends: string[] }

/** The `<case>` fences a page's markdown actually embeds, looked up in `corpus` -- usually
 *  none or one, so this stays small even though `corpus` itself is not. */
export function embeddedCasesFor(page: Page, corpus: Corpus): Record<string, EmbeddedCaseData> {
  const out: Record<string, EmbeddedCaseData> = {}
  for (const block of splitBlocks(page)) {
    if (block.kind !== "fence" || block.token.lang !== "case") continue
    const id = block.token.text.trim()
    const c = corpus.cases.find((x) => x.name === id)
    if (c) out[id] = { name: c.name, program: c.program, input: c.input, expect: c.expect }
  }
  return out
}

/** The case tree's index -- every case, without `emitted` -- shared by every example page since
 *  it does not change from one to the next (kantord/toylang#50: shipped once as `case-index.js`,
 *  not re-embedded per page). */
export function caseIndexFor(corpus: Corpus): CaseSummary[] {
  return corpus.cases.map(summarize)
}

export function docsRoute(page: Page, corpus: Corpus): AppRoute {
  return { kind: "docs", page, cases: embeddedCasesFor(page, corpus) }
}

export function examplesRoute(current: Case, corpus: Corpus): AppRoute {
  return { kind: "examples", current, index: caseIndexFor(corpus), backends: corpus.backends }
}

/** The site-relative path an example case's page lives at. */
export function exampleHref(name: string): string {
  return `/examples/${encodeURIComponent(name)}/`
}

/** Which `Page` a site-relative path names: `/` for the tutorial's first chapter, otherwise an
 *  exact match on `href`. Shared with fetchRoute.ts's fetch-based resolution
 *  (kantord/toylang#55), which never has a `Corpus` to call `resolveRoute` with. A bare section
 *  root (`/guides/`, the top nav's own links) doesn't match here even though
 *  scripts/prerender.mjs writes a file there too, mirroring that section's first page under a
 *  different URL -- fetchRoute.ts falls back to a real navigation for it rather than
 *  special-casing a path no page's `href` actually produces. */
export function pageForPath(path: string): Page | null {
  if (path === "/") return firstPage()
  return PAGES.find((p) => href(p) === path) ?? null
}

/**
 * A real URL back to the route it names, given the full corpus -- only ever called once, at
 * client hydration boot, for a `pnpm dev` load that has no prerendered globals to read instead.
 * A production load already has them embedded and skips this entirely; a client-side navigation
 * after boot (kantord/toylang#55) resolves through fetchRoute.ts instead, which never has a
 * `Corpus` to call this with.
 */
export function resolveRoute(pathname: string, corpus: Corpus): AppRoute {
  const path = stripBase(pathname)
  const page = pageForPath(path)
  if (page) return docsRoute(page, corpus)

  const m = /^\/examples\/([^/]*)\/?$/.exec(path)
  if (m) {
    const name = m[1] ? decodeURIComponent(m[1]) : corpus.cases[0].name
    const current = corpus.cases.find((c) => c.name === name) ?? corpus.cases[0]
    return examplesRoute(current, corpus)
  }

  return docsRoute(firstPage(), corpus)
}

/** The page `/` (and any unresolved path) mirrors -- the tutorial's first chapter, since that
 *  is where a new reader starts (plans/docs-site.md). Exported so entry-client.tsx's hydration
 *  boot picks the exact same page the prerender script rendered there, rather than falling
 *  back to `PAGES[0]` (alphabetically first by section, which is Examples' euler stream). */
export function firstPage(): Page {
  return PAGES.find((p) => p.section === "tutorial") ?? PAGES[0]
}
