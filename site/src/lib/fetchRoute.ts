/**
 * Resolves a same-origin navigation target the same way entry-client.tsx resolves the page it
 * boots on: by reading the globals scripts/prerender.mjs embedded in that page's own document
 * (kantord/toylang#50), never by fetching the full corpus. clientNav.tsx uses this to turn a
 * click into a route without a real navigation (kantord/toylang#55).
 */
import type { Case, CaseSummary } from "@/lib/corpus"
import { pageForPath, type AppRoute, type EmbeddedCaseData } from "@/lib/pageData"
import { stripBase } from "@/lib/route"

type CaseIndex = { backends: string[]; cases: CaseSummary[] }

// Seeded from the booting page's own globals when it happens to be an examples page, so the
// first cross-navigation into `/examples/*` doesn't re-fetch what is already on hand.
let caseIndex: CaseIndex | null =
  typeof window !== "undefined" && window.__CASE_INDEX__ ? window.__CASE_INDEX__ : null

function readAssign<T>(doc: Document, name: string): T | null {
  const prefix = `window.${name}=`
  for (const script of doc.querySelectorAll("script:not([src])")) {
    const text = script.textContent ?? ""
    if (text.startsWith(prefix)) return JSON.parse(text.slice(prefix.length)) as T
  }
  return null
}

async function loadCaseIndex(doc: Document): Promise<CaseIndex | null> {
  if (caseIndex) return caseIndex
  const script = [...doc.querySelectorAll("script[src]")].find((s) =>
    s.getAttribute("src")?.includes("case-index.js"),
  )
  const src = script?.getAttribute("src")
  if (!src) return null
  const text = await fetch(src).then((r) => r.text())
  const prefix = "window.__CASE_INDEX__="
  if (!text.startsWith(prefix)) return null
  caseIndex = JSON.parse(text.slice(prefix.length)) as CaseIndex
  return caseIndex
}

async function resolve(pathname: string): Promise<{ route: AppRoute; title: string } | null> {
  const res = await fetch(pathname)
  if (!res.ok) return null
  const doc = new DOMParser().parseFromString(await res.text(), "text/html")
  const title = doc.title

  const cases = readAssign<Record<string, EmbeddedCaseData>>(doc, "__EMBEDDED_CASES__")
  if (cases) {
    const page = pageForPath(stripBase(pathname))
    // A bare section root (`/guides/`) mirrors that section's first page but isn't reachable
    // through `pageForPath` -- caller falls back to a real navigation, which serves that same
    // file directly.
    if (!page) return null
    return { route: { kind: "docs", page, cases }, title }
  }

  const current = readAssign<Case>(doc, "__CASE__")
  if (current) {
    const index = await loadCaseIndex(doc)
    if (!index) return null
    return { route: { kind: "examples", current, index: index.cases, backends: index.backends }, title }
  }

  return null
}

const cache = new Map<string, Promise<{ route: AppRoute; title: string } | null>>()

/** Fetches and resolves `pathname`, caching by pathname so a hover/viewport prefetch and the
 *  click that follows it share one fetch (kantord/toylang#55). Returns null for anything that
 *  isn't a prerendered page in this shape: a `pnpm dev` load (never embeds these globals), a
 *  bare section root, or a plain 404 -- the caller's job is to fall back to a real navigation. */
export function loadRoute(pathname: string): Promise<{ route: AppRoute; title: string } | null> {
  let entry = cache.get(pathname)
  if (!entry) {
    entry = resolve(pathname)
    cache.set(pathname, entry)
  }
  return entry
}
