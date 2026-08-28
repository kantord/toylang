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
      // CommonMark always puts a blank-line gap before a comment that interrupts a paragraph, so
      // marked lexes that gap as its own "space" token -- a piece with nothing but whitespace for
      // `raw`. Two or more annotation comments can also follow the same paragraph in a row
      // (kantord/toylang#30 scope addition: the highlight-bleed bug). Either shape defeats
      // `pieces[i-1]` alone: it lands on the blank-space piece (whose `.html` is empty, so the
      // wash silently has nothing to attach a class to) or on an earlier comment's own already-
      // emptied placeholder, rather than the real paragraph or heading the comment is about.
      // Skipping past both blank and comment-only pieces on both sides fixes both.
      const isSkippable = (raw: string) => isCommentOnly(raw) || raw.trim() === ""
      for (let i = 0; i < pieces.length; i++) {
        if (!isCommentOnly(pieces[i].raw)) continue
        let target: (typeof pieces)[number] | undefined
        for (let j = i - 1; j >= 0 && !target; j--) {
          if (!isSkippable(pieces[j].raw)) target = pieces[j]
        }
        for (let j = i + 1; j < pieces.length && !target; j++) {
          if (!isSkippable(pieces[j].raw)) target = pieces[j]
        }
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

/** A block's rendered text, with markup stripped -- the same "original" text the edit-inbox
 *  record and the reply flow key on (kantord/toylang#30: the sidebar's immediate mark-as-read
 *  needs it too, to acknowledge a block without opening it). Browser-only, like the rest of
 *  annotations mode. */
export function blockPlainText(block: Block): string {
  if (block.kind !== "html") return ""
  const html = block.pieces.map((p) => p.html).join("")
  return new DOMParser().parseFromString(html, "text/html").body.textContent ?? ""
}

/** True when a token's whole source is nothing but one or more annotation comments, e.g. a
 *  `<!-- @review ... -->` line CommonMark split off of the paragraph above it. Only the
 *  `annotateRaw` folding below reads this; the annotation types themselves live in
 *  lib/annotations.ts, out of the production bundle's reach. */
function isCommentOnly(raw: string): boolean {
  return /^(?:\s*<!--\s*@(?:review|comment|fill)\b[\s\S]*?-->\s*)+$/.test(raw)
}
