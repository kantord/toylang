/**
 * The documentation pages, loaded from `docs/` at the repository root. The markdown lives with
 * the code rather than in the site so the fence harness (tests/docs.rs) can run every fragment;
 * the site is only a renderer of files something else already verified.
 */

export type Section = "tutorial" | "guides" | "reference" | "grill" | "examples"

export interface Page {
  section: Section
  /** The reference's subdirectory (`builtins`), empty for the flat sections. */
  group: string
  /** The path segment used in routes; a tutorial page's ordering prefix is stripped. */
  slug: string
  /** The first `# ` heading, which every page has. */
  title: string
  /** Repo-relative path (`docs/reference/builtins/map.md`), the base for resolving links. */
  path: string
  markdown: string
}

// Eager: the nav needs every title up front, and the pages are prose, small next to the
// grammars and the corpus the site already ships.
const files = import.meta.glob("../../../docs/**/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>

// A dot directory, so it needs its own glob: bare `**` patterns don't descend into dotfiles,
// and this one is gitignored (kantord/toylang#23) rather than part of the repo's docs tree.
const grillFiles = import.meta.glob("../../../docs/.grill/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>

function parse(): Page[] {
  const pages: Page[] = []
  for (const [file, markdown] of Object.entries({ ...files, ...grillFiles })) {
    const path = file.replace(/^(\.\.\/)+/, "")
    const rel = path.replace(/^docs\//, "")
    const parts = rel.split("/")
    // The grill directory holds session documents (grilling rounds): not one of the public
    // sections, and not filtered out either -- it rides the same renderer under its own section
    // so the annotations sidebar can jump to it.
    const section = parts[0] === ".grill" ? "grill" : parts[0]
    // ADRs are decision records, not presentation content; the drafts directory does not exist
    // yet but the same reasoning would apply.
    if (
      section !== "tutorial" &&
      section !== "guides" &&
      section !== "reference" &&
      section !== "grill" &&
      section !== "examples"
    ) {
      continue
    }
    const stem = parts[parts.length - 1].replace(/\.md$/, "")
    pages.push({
      section,
      group: section === "grill" ? "" : parts.length > 2 ? parts[1] : "",
      slug: stem.replace(/^\d+-/, ""),
      title: /^# (.+)$/m.exec(markdown)?.[1] ?? stem,
      path,
      markdown,
    })
  }
  // Filenames order the nav: tutorial chapters by their numeric prefix, everything else
  // alphabetically, which for the reference is the order a reader scans a list of names in.
  pages.sort((a, b) => a.path.localeCompare(b.path))
  return pages
}

export const PAGES: Page[] = parse()

export function page(section: Section, group: string, slug: string): Page | null {
  return (
    PAGES.find((p) => p.section === section && p.group === group && p.slug === slug) ?? null
  )
}

/** A docs page's real route (kantord/toylang#50): the site-relative path GitHub Pages serves
 *  the page's own static file at, directory-style so the URL needs no `.html`. Callers wrap
 *  it with `withBase` (lib/route.ts) before putting it in an `href`. */
export function href(p: Page): string {
  return p.group ? `/${p.section}/${p.group}/${p.slug}/` : `/${p.section}/${p.slug}/`
}

/**
 * Where a relative markdown link inside `from` leads: the matching page's route when the
 * target is a rendered docs page, the file on GitHub when it is anywhere else in the repo
 * (CONTEXT.md, draft.md, an ADR), since those are part of the record but not of this site.
 */
export function resolveLink(from: Page, target: string): string | null {
  if (!/^[^:]*\.md(#.*)?$/.test(target)) return null
  const clean = target.replace(/#.*$/, "")
  const base = from.path.split("/").slice(0, -1)
  for (const seg of clean.split("/")) {
    if (seg === "..") base.pop()
    else if (seg !== ".") base.push(seg)
  }
  const path = base.join("/")
  const hit = PAGES.find((p) => p.path === path)
  if (hit) return href(hit)
  return `https://github.com/kantord/toylang/blob/main/${path}`
}
