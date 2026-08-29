import { StrictMode } from "react"
import { createRoot, hydrateRoot, type Root } from "react-dom/client"

import { App } from "./App"
import "./index.css"
import type { Case, CaseSummary } from "@/lib/corpus"
import { loadCorpus } from "@/lib/corpus"
import { href, PAGES } from "@/lib/docs"
import { installClientNav } from "@/lib/clientNav"
import { firstPage, resolveRoute, type AppRoute, type EmbeddedCaseData } from "@/lib/pageData"
import { stripBase } from "@/lib/route"

declare global {
  interface Window {
    /** Set by the prerender script (scripts/prerender.mjs) on every docs page, keyed by the
     *  `<case>` fence ids that page actually embeds -- usually none or one. Its presence is
     *  also how the client tells a prerendered page apart from a plain `pnpm dev` load, which
     *  has nothing to hydrate against and falls back to fetching the corpus itself. */
    __EMBEDDED_CASES__?: Record<string, EmbeddedCaseData>
    /** Set on every `/examples/*` page: the one full case this page shows. */
    __CASE__?: Case
    /** Set by the shared `case-index.js` asset every `/examples/*` page loads: the sidebar's
     *  name/type summary of every case, and the backend list, without the emitted code that
     *  makes the full corpus 1.3MB (kantord/toylang#50). */
    __CASE_INDEX__?: { backends: string[]; cases: CaseSummary[] }
  }
}

async function resolvePrerenderedOrDev(): Promise<AppRoute> {
  if (window.__EMBEDDED_CASES__) {
    const path = stripBase(location.pathname)
    const page = PAGES.find((p) => href(p) === path) ?? firstPage()
    return { kind: "docs", page, cases: window.__EMBEDDED_CASES__ }
  }
  if (window.__CASE__ && window.__CASE_INDEX__) {
    return { kind: "examples", current: window.__CASE__, index: window.__CASE_INDEX__.cases, backends: window.__CASE_INDEX__.backends }
  }
  // No embedded data: a `pnpm dev` load, which never ran the prerender step. Fetch the corpus
  // once and derive the same route a build would have baked in.
  const corpus = await loadCorpus()
  return resolveRoute(location.pathname, corpus)
}

const rootEl = document.getElementById("root")!

resolvePrerenderedOrDev().then((route) => {
  const app = (
    <StrictMode>
      <App route={route} />
    </StrictMode>
  )
  let root: Root
  if (rootEl.hasChildNodes()) {
    root = hydrateRoot(rootEl, app)
  } else {
    root = createRoot(rootEl)
    root.render(app)
  }
  installClientNav(root)
})
