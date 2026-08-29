import { useMemo } from "react"
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

import { BOARD, rankOf, type Task } from "@dev/lib/board"
import { TaskCard, TaskLegend } from "@dev/components/board/TaskCard"

const NODE_W = 236
const NODE_H = 112
const GAP_X = 72
const GAP_Y = 16

type TaskNode = Node<{ task: Task }, "task">

/** Longest-path layering from board.ts, columns left to right; within a column, board order --
 *  the file's own priority ordering (plans/board.yaml's header: "position is priority"). This is
 *  dependency depth, not status -- the graph never groups by status (that's the Kanban tab). */
function layout(tasks: Task[]): TaskNode[] {
  const rank = rankOf(tasks)
  const byRank = new Map<number, Task[]>()
  for (const t of tasks) {
    const r = rank.get(t.id) ?? 0
    const bucket = byRank.get(r)
    if (bucket) bucket.push(t)
    else byRank.set(r, [t])
  }

  const nodes: TaskNode[] = []
  for (const [r, bucket] of byRank) {
    bucket.forEach((t, i) => {
      nodes.push({
        id: t.id,
        type: "task",
        position: { x: r * (NODE_W + GAP_X), y: i * (NODE_H + GAP_Y) },
        data: { task: t },
        draggable: false,
        connectable: false,
        selectable: false,
      })
    })
  }
  return nodes
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
 * The board as a dependency graph: one node per plans/board.yaml row, edges from `needs`, color
 * by status, blocked todo tasks (a need not yet done) as the primary highlight, the unblocked
 * frontier picked out with a ring. Read-only -- panning/zooming aside, the only interaction is
 * following a node's issue link.
 */
export function GraphView() {
  const nodes = useMemo(() => layout(BOARD), [])
  const edges = useMemo(() => edgesFrom(BOARD), [])

  return (
    <div className="space-y-3">
      <TaskLegend />
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
