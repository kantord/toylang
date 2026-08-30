/**
 * The `@review`/`@comment`/`@fill` comment scan for annotations mode (kantord/toylang#23):
 * the INBOX, the coordinator's own annotations to the maintainer. The AUTHORING side --
 * the maintainer's own notes back to the coordinator (kantord/toylang#30) -- doesn't live in
 * the markdown source at all; see AnnotateMode.tsx and the `/__annotations/note` endpoint.
 * Split out of blocks.ts (which the production bundle does use) and moved under dev/
 * (kantord/toylang#50): the production build never opens this directory, so nothing here
 * reaches that bundle.
 */

import { blockPlainText, blockRaw, type Block } from "@/lib/blocks"
import type { Page } from "@/lib/docs"

export type AnnotationType = "review" | "comment" | "fill"

export interface Annotation {
  page: Page
  block: number
  type: AnnotationType
  /** The exact rendered text the annotation concerns (kantord/toylang#30), when the coordinator
   *  quoted a span rather than commenting on the whole piece. */
  anchor?: string
  note: string
  /** The block's own rendered text (kantord/toylang#30): the sidebar's immediate mark-as-read
   *  needs this to acknowledge a block without opening its page first. */
  original: string
}

// A quoted span comes right after the type, before the free-text note: `@review "the exact
// words" the rest is the note`. No escaping inside the quotes -- an anchor is copied verbatim
// from rendered prose, and prose containing a literal `"` is rare enough not to plan for.
const ANNOTATION_RE = /<!--\s*@(review|comment|fill)\b(?:\s+"([^"]*)")?([\s\S]*?)-->/g

/** Every annotation comment on a page, one entry per match, keyed to the run it lives in (the
 *  edit-inbox unit); rendering pinpoints the exact piece (and span) via `annotationsIn`. */
export function pageAnnotations(page: Page, blocks: Block[]): Annotation[] {
  const found: Annotation[] = []
  blocks.forEach((block, index) => {
    for (const m of blockRaw(block).matchAll(ANNOTATION_RE)) {
      found.push({
        page,
        block: index,
        type: m[1] as AnnotationType,
        anchor: m[2] || undefined,
        note: m[3].trim(),
        original: blockPlainText(block),
      })
    }
  })
  return found
}

/** Every annotation left inside one piece's own source -- what a marker-pen wash attaches to, so
 *  it covers only the commented element (and, with an anchor, only the quoted words) rather than
 *  its run-mates. */
export function annotationsIn(raw: string): { type: AnnotationType; anchor?: string; note: string }[] {
  return [...raw.matchAll(ANNOTATION_RE)].map((m) => ({
    type: m[1] as AnnotationType,
    anchor: m[2] || undefined,
    note: m[3].trim(),
  }))
}

/** POSTs one record through the #30 inbox door (`/__annotations/save`) -- the shared shape under
 *  a grilling round's answers (lib/grill.ts) and a plan decision (lib/plans.ts). `label` names
 *  the record in the failure message, since the two callers' users are looking at different
 *  things when a submit fails. */
export function saveToInbox(
  record: { page: string; block: number; original: string; edited: string },
  label: string,
): Promise<void> {
  return fetch("/__annotations/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(record),
  }).then((res) => {
    if (!res.ok) throw new Error(`${label}: ${res.status} ${res.statusText}`)
  })
}
