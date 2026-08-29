import { useMemo, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { CaseSummary } from "@/lib/corpus"
import { cn } from "@/lib/utils"

/** One segment of a dotted node-type path (`arith` in `arith.add`). A case sits at every
 * node whose path it has a tag for, so `record_valued_field` (tagged `projection` and
 * `selection.collapse`) is a leaf under both `projection` and `selection/collapse`. */
interface TreeNode {
  children: Map<string, TreeNode>
  cases: CaseSummary[]
}

function emptyNode(): TreeNode {
  return { children: new Map(), cases: [] }
}

function buildTree(cases: CaseSummary[]): TreeNode {
  const root = emptyNode()
  for (const c of cases) {
    for (const tag of c.nodeTypes) {
      let node = root
      for (const segment of tag.split(".")) {
        let next = node.children.get(segment)
        if (!next) {
          next = emptyNode()
          node.children.set(segment, next)
        }
        node = next
      }
      node.cases.push(c)
    }
  }
  return root
}

function countCases(node: TreeNode): number {
  let n = node.cases.length
  for (const child of node.children.values()) n += countCases(child)
  return n
}

function CaseLeaf({
  c,
  depth,
  selected,
  hrefFor,
}: {
  c: CaseSummary
  depth: number
  selected: string
  hrefFor: (name: string) => string
}) {
  return (
    <li>
      <a
        href={hrefFor(c.name)}
        style={{ paddingLeft: `${depth * 14 + 28}px` }}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-sm py-1 pr-2 text-left text-sm",
          c.name === selected
            ? "bg-accent text-accent-foreground"
            : "hover:bg-accent/50",
        )}
      >
        <span className="truncate font-mono text-[13px]">{c.name}</span>
        {c.expectKind === "refusal" ? (
          <Badge variant="destructive" className="shrink-0 text-[10px]">
            refuses
          </Badge>
        ) : (
          <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
            {c.resultType}
          </span>
        )}
      </a>
    </li>
  )
}

function Folder({
  name,
  node,
  path,
  depth,
  expanded,
  forceOpen,
  onToggle,
  selected,
  hrefFor,
}: {
  name: string
  node: TreeNode
  path: string
  depth: number
  expanded: Set<string>
  forceOpen: boolean
  onToggle: (path: string) => void
  selected: string
  hrefFor: (name: string) => string
}) {
  const open = forceOpen || expanded.has(path)
  const childFolders = [...node.children.entries()].sort(([a], [b]) => a.localeCompare(b))
  const childCases = [...node.cases].sort((a, b) => a.name.localeCompare(b.name))

  return (
    <li>
      <button
        type="button"
        onClick={() => onToggle(path)}
        style={{ paddingLeft: `${depth * 14 + 8}px` }}
        className="flex w-full items-center gap-1.5 rounded-sm py-1 pr-2 text-left text-sm hover:bg-accent/50"
      >
        <span className="w-3 shrink-0 text-center text-muted-foreground">
          {open ? "⌄" : "›"}
        </span>
        <span className="truncate font-mono text-[13px]">{name}</span>
        <span className="ml-auto shrink-0 font-mono text-[10px] text-muted-foreground">
          {countCases(node)}
        </span>
      </button>
      {open && (
        <ul>
          {childFolders.map(([segment, child]) => (
            <Folder
              key={segment}
              name={segment}
              node={child}
              path={`${path}.${segment}`}
              depth={depth + 1}
              expanded={expanded}
              forceOpen={forceOpen}
              onToggle={onToggle}
              selected={selected}
              hrefFor={hrefFor}
            />
          ))}
          {childCases.map((c) => (
            <CaseLeaf key={c.name} c={c} depth={depth + 1} selected={selected} hrefFor={hrefFor} />
          ))}
        </ul>
      )}
    </li>
  )
}

export function CaseTree({
  cases,
  selected,
  hrefFor,
}: {
  cases: CaseSummary[]
  selected: string
  hrefFor: (name: string) => string
}) {
  const [query, setQuery] = useState("")
  const [expanded, setExpanded] = useState<Set<string>>(new Set())

  const q = query.trim().toLowerCase()
  const shown = useMemo(() => {
    if (!q) return cases
    // Searching the program too, so "select" or "unlines" finds the cases that use it rather
    // than only the ones named after it.
    return cases.filter((c) => c.name.includes(q) || c.program.toLowerCase().includes(q))
  }, [cases, q])

  const tree = useMemo(() => buildTree(shown), [shown])
  const topLevel = useMemo(
    () => [...tree.children.entries()].sort(([a], [b]) => a.localeCompare(b)),
    [tree],
  )

  function toggle(path: string) {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

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
          {topLevel.map(([segment, node]) => (
            <Folder
              key={segment}
              name={segment}
              node={node}
              path={segment}
              depth={0}
              expanded={expanded}
              forceOpen={q.length > 0}
              onToggle={toggle}
              selected={selected}
              hrefFor={hrefFor}
            />
          ))}
          {topLevel.length === 0 && (
            <li className="px-3 py-6 text-center text-sm text-muted-foreground">
              Nothing matches that.
            </li>
          )}
        </ul>
      </ScrollArea>
    </div>
  )
}
