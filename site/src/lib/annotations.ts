/**
 * The `@review`/`@comment`/`@fill` comment scan for annotations mode (kantord/toylang#23).
 * Split out of blocks.ts so this stays reachable only through a dynamic import: nothing in the
 * production bundle references it, and `vite build` tree-shakes it out accordingly.
 */

import { blockRaw, type Block } from "@/lib/blocks"
import type { Page } from "@/lib/docs"

export type AnnotationType = "review" | "comment" | "fill"

export interface Annotation {
  page: Page
  block: number
  type: AnnotationType
  note: string
}

const ANNOTATION_RE = /<!--\s*@(review|comment|fill)\b([\s\S]*?)-->/g

/** Every annotation comment on a page, one entry per match, keyed to the run it lives in (the
 *  edit-inbox unit); rendering pinpoints the exact piece via `annotationIn`. */
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
