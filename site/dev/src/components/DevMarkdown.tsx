import { Marked, type Tokens } from "marked"
import { useMemo } from "react"

import { Code } from "@/components/Code"
import { annotateHref } from "@dev/lib/nav"
import { PAGES } from "@/lib/docs"
import { cn } from "@/lib/utils"

/** The plain renderer, shared by every caller that has no `basePath` to resolve links against. */
const plain = new Marked()

/** Where a repository file lives when this app has no page of its own for it. Written out
 *  rather than shared with lib/docs.ts's `resolveLink`, which resolves against a docs page and
 *  returns production routes; the dev app's own links go to the annotations overlay instead. */
const GITHUB_BLOB = "https://github.com/kantord/toylang/blob/main/"

/**
 * Markdown with real code fences, for the dev app's own message bodies -- a grilling round's
 * sections (kantord/toylang#34: "full code blocks") and a plan under review
 * (kantord/toylang#110). A fence gets `Code`'s shiki highlighting, everything else goes through
 * marked's own HTML, the same split `lib/blocks.ts` makes for a docs page.
 *
 * No fragment protocol here: neither round files nor `plans/*.md` are checked by the fence
 * harness (tests/docs.rs walks `docs/`), so a language tag is illustration and nothing claims
 * otherwise.
 *
 * `basePath` is the repo-relative path of the file the markdown came from. With it, relative
 * links resolve the way they would if you opened the file in the repository: to the docs page
 * in this app when one renders that file, to GitHub otherwise. A plan is full of links like
 * `../docs/reference/builtins/sort.md`, which without this land on a dev-server 404.
 */
export function DevMarkdown({
  text,
  className,
  basePath,
}: {
  text: string
  className?: string
  basePath?: string
}) {
  const marked = useMemo(() => (basePath ? repoLinkRenderer(basePath) : plain), [basePath])
  const tokens = useMemo(() => marked.lexer(text), [marked, text])
  return (
    <div className={cn("docs-prose", className)}>
      {tokens.map((t, i) =>
        t.type === "code" ? (
          <Code key={i} code={(t as Tokens.Code).text} lang={(t as Tokens.Code).lang || "text"} />
        ) : (
          <div key={i} dangerouslySetInnerHTML={{ __html: marked.parser([t]) }} />
        ),
      )}
    </div>
  )
}

function repoLinkRenderer(basePath: string): Marked {
  return new Marked({
    renderer: {
      link(token) {
        const target = resolveRepoLink(basePath, token.href) ?? token.href
        const external = target.startsWith("http")
        const text = this.parser.parseInline(token.tokens)
        // Attribute-escaped by hand because this template bypasses marked's own link renderer,
        // which would have done it; without it a quote in an href breaks out of the attribute.
        const href = target.replace(/&/g, "&amp;").replace(/"/g, "&quot;")
        return `<a href="${href}"${external ? ' target="_blank" rel="noreferrer"' : ""}>${text}</a>`
      },
    },
  })
}

function resolveRepoLink(basePath: string, href: string): string | null {
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("#") || href.startsWith("/")) return null
  const segments = basePath.split("/").slice(0, -1)
  for (const seg of href.replace(/#.*$/, "").split("/")) {
    if (seg === "..") segments.pop()
    else if (seg !== ".") segments.push(seg)
  }
  const path = segments.join("/")
  const page = PAGES.find((p) => p.path === path)
  return page ? annotateHref(page) : GITHUB_BLOB + path
}
