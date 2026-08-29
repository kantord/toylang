// Turns the client bundle `vite build` already produced into real per-page documents
// (kantord/toylang#50): one `index.html` per docs page and per corpus case, GitHub Pages'
// asset-hashed `dist/index.html` reused as the shell every one of them shares.
//
// Runs through Vite's own dev-mode module graph (`ssrLoadModule`) rather than a second SSR
// build: entry-server.tsx already needs nothing a plain `vite build --ssr` pass would buy that
// this doesn't also get, and it is one fewer build step to keep in sync.
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"

import { createServer } from "vite"

const root = path.resolve(import.meta.dirname, "..")
const dist = path.join(root, "dist")

const corpus = JSON.parse(await readFile(path.join(root, "public/corpus.json"), "utf8"))
const template = await readFile(path.join(dist, "index.html"), "utf8")

const vite = await createServer({
  root,
  server: { middlewareMode: true },
  appType: "custom",
})
const base = vite.config.base

/** @type {import("../src/entry-server.tsx")} */
const { PAGES, href, docsRoute, examplesRoute, exampleHref, firstPage, renderRoute } =
  await vite.ssrLoadModule("/src/entry-server.tsx")
const { summarize } = await vite.ssrLoadModule("/src/lib/corpus.ts")

function withBase(p) {
  return base.replace(/\/$/, "") + p
}

// A corpus program is arbitrary text; escaping `<` keeps a stray `</script>` inside one from
// closing the tag its JSON is embedded in.
function embed(value) {
  return JSON.stringify(value).replace(/</g, "\\u003c")
}

function shell({ title, bodyHtml, headExtra }) {
  return template
    .replace("<title>toylang</title>", `<title>${title} - toylang</title>${headExtra ?? ""}`)
    .replace('<div id="root"></div>', `<div id="root">${bodyHtml}</div>`)
}

async function writeAt(routePath, html) {
  const dir = path.join(dist, routePath)
  await mkdir(dir, { recursive: true })
  await writeFile(path.join(dir, "index.html"), html)
}

// Docs pages: tutorial, guides, reference, grill, and the euler stream under examples/.
// Prev/next within a section (App.tsx's PagerLinks) are also the prefetch hint, since they are
// the two places a reader most likely goes next.
const bySection = new Map()
for (const p of PAGES) {
  if (!bySection.has(p.section)) bySection.set(p.section, [])
  bySection.get(p.section).push(p)
}

for (const page of PAGES) {
  const route = docsRoute(page, corpus)
  const siblings = bySection.get(page.section)
  const i = siblings.indexOf(page)
  const neighbors = [siblings[i - 1], siblings[i + 1]].filter(Boolean)
  const prefetch = neighbors.map((n) => `<link rel="prefetch" href="${withBase(href(n))}">`).join("")
  const doc = shell({
    title: page.title,
    bodyHtml: renderRoute(route),
    headExtra: prefetch + `<script>window.__EMBEDDED_CASES__=${embed(route.cases)}</script>`,
  })
  await writeAt(href(page), doc)
}

// The landing page mirrors the first tutorial chapter -- a real document at `/`, not a
// client-side redirect to one.
{
  const page = firstPage()
  const route = docsRoute(page, corpus)
  const doc = shell({
    title: page.title,
    bodyHtml: renderRoute(route),
    headExtra: `<script>window.__EMBEDDED_CASES__=${embed(route.cases)}</script>`,
  })
  await writeAt("/", doc)
}

// The case index: name, type, and node tags for every case, shared by every example page as one
// cacheable asset instead of being re-embedded per page (kantord/toylang#50) -- the full corpus
// is 1.3MB, almost all of it the seven backends' emitted code no sidebar entry shows.
const caseIndex = { backends: corpus.backends, cases: corpus.cases.map(summarize) }
await writeFile(path.join(dist, "case-index.js"), `window.__CASE_INDEX__=${embed(caseIndex)}`)
const caseIndexScript = `<script src="${withBase("/case-index.js")}"></script>`

for (const c of corpus.cases) {
  const route = examplesRoute(c, corpus)
  const html = renderRoute(route)
  const doc = shell({
    title: c.name,
    bodyHtml: html,
    headExtra: caseIndexScript + `<script>window.__CASE__=${embed(c)}</script>`,
  })
  await writeAt(exampleHref(c.name), doc)
}

// `/examples/` itself, same as the nav link -- defaults to the first case, matching the old
// hash router's fallback.
{
  const first = corpus.cases[0]
  const route = examplesRoute(first, corpus)
  const doc = shell({
    title: "Examples",
    bodyHtml: renderRoute(route),
    headExtra: caseIndexScript + `<script>window.__CASE__=${embed(first)}</script>`,
  })
  await writeAt("/examples/", doc)
}

await vite.close()

console.log(`prerendered ${PAGES.length} docs pages and ${corpus.cases.length} examples`)
