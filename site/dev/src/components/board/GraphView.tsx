import { useMemo, useState } from "react"
import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react"
import "@xyflow/react/dist/style.css"

import { ALL_TASKS, BOARD, type Task } from "@dev/lib/board"
import { TaskCard, TaskLegend } from "@dev/components/board/TaskCard"
import { Button } from "@/components/ui/button"

const NODE_W = 236
const NODE_H = 112

type TaskNode = Node<{ task: Task }, "task">

/** Deterministic string hash for a stable initial position -- same board, same layout across
 *  reloads, without a seeded PRNG library. */
function hash(s: string): number {
  let h = 0
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0
  return h
}

/** Ideal edge length: two card half-diagonals plus breathing room, so a resting spring roughly
 *  matches the visual gap the old column layout used between ranks. */
const SPRING_LEN = Math.hypot(NODE_W, NODE_H) + 70
const REPEL_K = 320
const SPRING_K = 0.05
const GRAVITY_K = 0.04
const DIR_K = 0.02
const FORCE_ITER = 500
const COLLISION_PAD = 28
const COLLISION_PASSES = 300

/**
 * Force-directed layout tuned for a dependency DAG (kantord/toylang#59): the old rankOf column
 * layout put every task at the same longest-path depth into one column, and columns with many
 * tasks became unusably tall. This spreads nodes by simulated physics instead --
 *
 * - inverse-square repulsion keeps every pair apart,
 * - a spring per `needs` edge pulls dependents toward their dependencies,
 * - a weak rightward bias on each edge keeps the arrows generally pointing the way dependencies
 *   flow, so the graph still reads left-to-right without being forced into discrete columns,
 * - gravity toward the centroid keeps disconnected tasks from drifting off into empty space.
 *
 * A final collision-relaxation pass (plain AABB push-apart, run to convergence) cleans up any
 * card overlap the physics alone didn't resolve -- cards are large rectangles, not points, and
 * pure force equilibria don't guarantee a real gap between them.
 */
function layout(tasks: Task[], edges: Edge[]): TaskNode[] {
  const ids = tasks.map((t) => t.id)
  const n = ids.length
  const edgePairs = edges.map((e) => [e.source, e.target] as const)

  const pos = new Map<string, { x: number; y: number }>()
  for (const id of ids) {
    const angle = ((hash(id) % 1000) / 1000) * Math.PI * 2
    const radius = 150 + (hash(id + "r") % 700)
    pos.set(id, { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius })
  }

  const startTemp = SPRING_LEN * 1.2
  for (let iter = 0; iter < FORCE_ITER; iter++) {
    const temp = startTemp * (1 - iter / FORCE_ITER)
    const disp = new Map(ids.map((id) => [id, { x: 0, y: 0 }]))

    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        const a = pos.get(ids[i])!
        const b = pos.get(ids[j])!
        const dx = a.x - b.x
        const dy = a.y - b.y
        const d = Math.max(Math.hypot(dx, dy), 0.01)
        const f = (REPEL_K * REPEL_K) / (d * d)
        const ux = dx / d
        const uy = dy / d
        disp.get(ids[i])!.x += ux * f
        disp.get(ids[i])!.y += uy * f
        disp.get(ids[j])!.x -= ux * f
        disp.get(ids[j])!.y -= uy * f
      }
    }

    for (const [s, t] of edgePairs) {
      const a = pos.get(s)
      const b = pos.get(t)
      if (!a || !b) continue
      const dx = b.x - a.x
      const dy = b.y - a.y
      const d = Math.max(Math.hypot(dx, dy), 0.01)
      const f = SPRING_K * (d - SPRING_LEN)
      const ux = dx / d
      const uy = dy / d
      disp.get(s)!.x += ux * f
      disp.get(s)!.y += uy * f
      disp.get(t)!.x -= ux * f
      disp.get(t)!.y -= uy * f

      const wantX = a.x + SPRING_LEN
      if (b.x < wantX) {
        const pull = (wantX - b.x) * DIR_K
        disp.get(t)!.x += pull
        disp.get(s)!.x -= pull
      }
    }

    let cx = 0
    let cy = 0
    for (const id of ids) {
      const p = pos.get(id)!
      cx += p.x
      cy += p.y
    }
    cx /= n
    cy /= n
    for (const id of ids) {
      const p = pos.get(id)!
      disp.get(id)!.x += (cx - p.x) * GRAVITY_K
      disp.get(id)!.y += (cy - p.y) * GRAVITY_K
    }

    for (const id of ids) {
      const d = disp.get(id)!
      const p = pos.get(id)!
      const mag = Math.max(Math.hypot(d.x, d.y), 0.01)
      const cap = Math.max(1, temp)
      const scale = Math.min(1, cap / mag)
      p.x += d.x * scale
      p.y += d.y * scale
    }
  }

  const wPad = NODE_W + COLLISION_PAD
  const hPad = NODE_H + COLLISION_PAD
  for (let pass = 0; pass < COLLISION_PASSES; pass++) {
    let moved = false
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        const a = pos.get(ids[i])!
        const b = pos.get(ids[j])!
        const dx = b.x - a.x
        const dy = b.y - a.y
        const overlapX = wPad - Math.abs(dx)
        const overlapY = hPad - Math.abs(dy)
        if (overlapX > 0 && overlapY > 0) {
          moved = true
          if (overlapX < overlapY) {
            const push = (overlapX / 2) * (dx >= 0 ? 1 : -1)
            a.x -= push
            b.x += push
          } else {
            const push = (overlapY / 2) * (dy >= 0 ? 1 : -1)
            a.y -= push
            b.y += push
          }
        }
      }
    }
    if (!moved) break
  }

  return tasks.map((t) => ({
    id: t.id,
    type: "task",
    position: pos.get(t.id)!,
    data: { task: t },
    draggable: false,
    connectable: false,
    selectable: false,
  }))
}

function edgesFrom(tasks: Task[]): Edge[] {
  const ids = new Set(tasks.map((t) => t.id))
  return tasks.flatMap((t) =>
    t.needs
      .filter((needId) => ids.has(needId))
      .map((needId) => ({
        id: `${needId}->${t.id}`,
        source: needId,
        target: t.id,
        style: { stroke: "var(--border)", strokeWidth: 1.5 },
        markerEnd: { type: MarkerType.ArrowClosed, color: "var(--border)", width: 16, height: 16 },
      })),
  )
}

function TaskNodeView({ data }: NodeProps<TaskNode>) {
  return (
    <div style={{ width: NODE_W, height: NODE_H }}>
      <Handle type="target" position={Position.Left} isConnectable={false} className="opacity-0" />
      <TaskCard task={data.task} className="h-full" />
      <Handle type="source" position={Position.Right} isConnectable={false} className="opacity-0" />
    </div>
  )
}

const NODE_TYPES = { task: TaskNodeView }

/**
 * The board as a dependency graph: one node per row, edges from `needs`, color by status,
 * blocked todo tasks (a need not yet done) as the primary highlight, the unblocked frontier
 * picked out with a ring. Read-only -- panning/zooming aside, the only interaction is following
 * a node's issue link. By default shows only the live board (plans/board.yaml), which is
 * already the not-done view (issue #113: done rows live in board-archive.yaml); a toggle adds
 * the archive back in for the full history.
 */
export function GraphView() {
  const [showDone, setShowDone] = useState(false)

  const filteredTasks = useMemo(() => (showDone ? ALL_TASKS : BOARD), [showDone])

  const edges = useMemo(() => edgesFrom(filteredTasks), [filteredTasks])
  const nodes = useMemo(() => layout(filteredTasks, edges), [edges, filteredTasks])

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <TaskLegend />
        <Button variant="outline" size="sm" onClick={() => setShowDone((v) => !v)}>
          {showDone ? "Hide done" : "Show done"}
        </Button>
      </div>
      <div className="h-[70vh] rounded-lg border border-border">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={NODE_TYPES}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={false}
          panOnScroll
          fitView
        >
          <Background />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
    </div>
  )
}
