/**
 * Loaded by scripts/prerender.mjs through Vite's `ssrLoadModule`, in Node, once per production
 * build (kantord/toylang#50) -- no separate SSR bundle, since that script already gets a
 * dev-mode module graph with `import.meta.glob` and JSX handled for free.
 */
import { renderToString } from "react-dom/server"

import { App } from "./App"
import type { AppRoute } from "@/lib/pageData"

export { PAGES, href } from "@/lib/docs"
export { docsRoute, examplesRoute, exampleHref, firstPage } from "@/lib/pageData"

export function renderRoute(route: AppRoute): string {
  return renderToString(<App route={route} />)
}
