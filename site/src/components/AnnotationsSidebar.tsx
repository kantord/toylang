import { useMemo } from "react"

import { pageAnnotations, type AnnotationType } from "@/lib/annotations"
import { splitBlocks } from "@/lib/blocks"
import { href, PAGES, type Page } from "@/lib/docs"
import { cn } from "@/lib/utils"

const DOT: Record<AnnotationType, string> = {
  review: "bg-amber-500",
  comment: "bg-sky-500",
  fill: "bg-fuchsia-500",
}

/** Replaces the section nav in annotations mode: every review/comment/fill note across all
 *  docs pages, in one flat list, since the coordinator leaves them wherever they land. */
export function AnnotationsSidebar({ current }: { current: Page }) {
  const annotations = useMemo(() => PAGES.flatMap((p) => pageAnnotations(p, splitBlocks(p))), [])

  if (annotations.length === 0) {
    return <p className="text-sm text-muted-foreground">No annotations yet.</p>
  }

  return (
    <nav className="space-y-1 text-sm">
      {annotations.map((a, i) => (
        <a
          key={i}
          href={`${href(a.page)}?b=${a.block}`}
          className={cn(
            "flex items-start gap-2 rounded px-2 py-1.5 hover:bg-muted",
            a.page === current && "bg-muted/60",
          )}
        >
          <span className={cn("mt-1.5 size-1.5 shrink-0 rounded-full", DOT[a.type])} />
          <span className="min-w-0">
            <span className="block truncate font-medium text-foreground">{a.page.title}</span>
            <span className="block truncate text-muted-foreground">{a.note || a.type}</span>
          </span>
        </a>
      ))}
    </nav>
  )
}
