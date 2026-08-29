import type { Page } from "@/lib/docs"

/**
 * The dev app's own hash route to a docs page, opened in the always-on annotations overlay
 * (DevDocsPage.tsx). Distinct from lib/docs.ts's `href`, which is the page's real production
 * URL (kantord/toylang#50) -- the two apps navigate differently and never share a router.
 */
export function annotateHref(p: Page): string {
  return p.group ? `#/annotate/${p.section}/${p.group}/${p.slug}` : `#/annotate/${p.section}/${p.slug}`
}
