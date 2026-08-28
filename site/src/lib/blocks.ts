import { Marked, type Token, type Tokens } from "marked"

import { resolveLink, type Page } from "@/lib/docs"

/** The fence languages that belong to the fragment protocol; anything else is illustration. */
export const FRAGMENT = new Set(["toylang", "input", "output", "refuses", "error", "case"])

/** One markdown token's rendered HTML alongside its own source, so annotations mode can tell
 *  which piece of a run a comment landed in without re-lexing. */
export interface Piece {
  html: string
  raw: string
  /** `raw`, with a comment-only sibling token folded in. CommonMark lets an HTML comment on
   *  its own line interrupt a paragraph, so a trailing `<!-- @review -->` becomes its own
   *  (invisible) token rather than staying part of the paragraph it is meant to mark; this is
   *  what a piece's highlight attribution reads instead of `raw`, so the wash lands on the
   *  text the comment follows. */
  annotateRaw: string
}

export type Block = { kind: "html"; pieces: Piece[] } | { kind: "fence"; token: Tokens.Code }

function renderer(page: Page) {
  return new Marked({
    renderer: {
      // Relative markdown links are written for the repository; rendered, they should lead to
      // the matching page here, or to GitHub for files this site does not show.
      link(token) {
        const target = resolveLink(page, token.href) ?? token.href
        const text = this.parser.parseInline(token.tokens)
        const external = target.startsWith("http")
        return `<a href="${target}"${external ? ' target="_blank" rel="noreferrer"' : ""}>${text}</a>`
      },
    },
  })
}

/**
 * Splits a page into the runs marked renders as prose and the fences the fragment protocol
 * owns. Shared by the reader and annotations mode so a block's index means the same thing to
 * both -- it is what an edit-inbox record and a jump-to link both key on. Each run keeps its
 * per-token pieces (rather than one joined HTML string) so a comment inside one paragraph can
 * be told apart from its neighbors in the same run.
 */
export function splitBlocks(page: Page): Block[] {
  const md = renderer(page)
  const blocks: Block[] = []
  let run: Token[] = []
  const flush = () => {
    if (run.length) {
      const pieces = run.map((t) => ({ html: md.parser([t]), raw: t.raw, annotateRaw: t.raw }))
      for (let i = 0; i < pieces.length; i++) {
        if (!isCommentOnly(pieces[i].raw)) continue
        const target = pieces[i - 1] ?? pieces[i + 1]
        if (!target) continue
        target.annotateRaw += pieces[i].annotateRaw
        pieces[i].annotateRaw = ""
      }
      blocks.push({ kind: "html", pieces })
    }
    run = []
  }
  for (const token of md.lexer(page.markdown)) {
    if (token.type === "code" && FRAGMENT.has((token as Tokens.Code).lang ?? "")) {
      flush()
      blocks.push({ kind: "fence", token: token as Tokens.Code })
    } else {
      run.push(token)
    }
  }
  flush()
  return blocks
}

/** A block's full source, joining its pieces back up; this is what the edit inbox keys the
 *  block on and what the annotation scan runs over. */
export function blockRaw(block: Block): string {
  return block.kind === "html" ? block.pieces.map((p) => p.raw).join("") : ""
}

export type AnnotationType = "review" | "comment" | "fill"

export interface Annotation {
  page: Page
  block: number
  type: AnnotationType
  note: string
}

const ANNOTATION_RE = /<!--\s*@(review|comment|fill)\b([\s\S]*?)-->/g

/** True when a token's whole source is nothing but one or more annotation comments, e.g. a
 *  `<!-- @review ... -->` line CommonMark split off of the paragraph above it. */
function isCommentOnly(raw: string): boolean {
  return /^(?:\s*<!--\s*@(?:review|comment|fill)\b[\s\S]*?-->\s*)+$/.test(raw)
}

/** Every annotation comment on a page, one entry per match, keyed to the run it lives in (the
 *  edit-inbox unit); rendering pinpoints the exact piece via `annotationsIn`. */
export function pageAnnotations(page: Page, blocks: Block[]): Annotation[] {
  const found: Annotation[] = []
  blocks.forEach((block, index) => {
    for (const m of blockRaw(block).matchAll(ANNOTATION_RE)) {
      found.push({ page, block: index, type: m[1] as AnnotationType, note: m[2].trim() })
    }
  })
  return found
}

/** The first annotation type left inside one piece's own source, if any -- this is what a
 *  marker-pen wash attaches to, so it covers only the commented element and not its run-mates. */
export function annotationIn(raw: string): AnnotationType | undefined {
  const m = ANNOTATION_RE.exec(raw)
  ANNOTATION_RE.lastIndex = 0
  return m ? (m[1] as AnnotationType) : undefined
}
