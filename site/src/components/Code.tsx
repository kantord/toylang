import { useHighlighted } from "@/lib/highlight"
import { cn } from "@/lib/utils"

/**
 * A code block. Falls back to unhighlighted text until the grammars arrive, and stays there for
 * toylang itself, which has no grammar to load.
 */
export function Code({ code, lang, className }: { code: string; lang: string; className?: string }) {
  const html = useHighlighted(code, lang)
  const shared = "overflow-x-auto rounded-md border bg-muted/40 p-4 text-[13px] leading-relaxed"

  if (!html) {
    return (
      <pre className={cn(shared, className)}>
        <code>{code}</code>
      </pre>
    )
  }
  return (
    <div
      className={cn(shared, "shiki-host", className)}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}
