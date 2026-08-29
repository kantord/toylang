/**
 * Site-relative paths (lib/docs.ts's `href`, lib/pageData.ts's example-case routes) don't carry
 * the GitHub Pages base (`/toylang/`); this is the one place that adds it, so a page never
 * hardcodes the prefix a reader's browser needs.
 */
export function withBase(path: string): string {
  const base = import.meta.env.BASE_URL.replace(/\/$/, "")
  return `${base}${path}`
}

/** The inverse: a `location.pathname` back to the site-relative form `href` produces, so the
 *  client can find which `Page` (or example case) a real URL landed on. */
export function stripBase(pathname: string): string {
  const base = import.meta.env.BASE_URL.replace(/\/$/, "")
  const rel = pathname.startsWith(base) ? pathname.slice(base.length) : pathname
  return rel.startsWith("/") ? rel : `/${rel}`
}
