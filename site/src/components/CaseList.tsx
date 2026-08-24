import { useMemo, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { Case } from "@/lib/corpus"
import { cn } from "@/lib/utils"

export function CaseList({
  cases,
  selected,
  onSelect,
}: {
  cases: Case[]
  selected: string
  onSelect: (name: string) => void
}) {
  const [query, setQuery] = useState("")

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return cases
    // Searching the program too, so "select" or "unlines" finds the cases that use it rather
    // than only the ones named after it.
    return cases.filter(
      (c) => c.name.includes(q) || c.program.toLowerCase().includes(q),
    )
  }, [cases, query])

  return (
    <div className="flex h-full flex-col gap-3">
      <Input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Filter by name or source"
        aria-label="Filter cases"
      />
      <div className="text-xs text-muted-foreground">
        {shown.length} of {cases.length} cases
      </div>
      <ScrollArea className="min-h-0 flex-1 rounded-md border">
        <ul className="p-1">
          {shown.map((c) => (
            <li key={c.name}>
              <button
                type="button"
                onClick={() => onSelect(c.name)}
                className={cn(
                  "flex w-full items-center justify-between gap-2 rounded-sm px-3 py-2 text-left text-sm",
                  c.name === selected
                    ? "bg-accent text-accent-foreground"
                    : "hover:bg-accent/50",
                )}
              >
                <span className="truncate font-mono text-[13px]">{c.name}</span>
                {c.expect.kind === "refusal" ? (
                  <Badge variant="destructive" className="shrink-0 text-[10px]">
                    refuses
                  </Badge>
                ) : (
                  <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                    {c.resultType}
                  </span>
                )}
              </button>
            </li>
          ))}
          {shown.length === 0 && (
            <li className="px-3 py-6 text-center text-sm text-muted-foreground">
              Nothing matches that.
            </li>
          )}
        </ul>
      </ScrollArea>
    </div>
  )
}
