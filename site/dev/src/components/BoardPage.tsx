import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { GraphView } from "@dev/components/board/GraphView"
import { KanbanView } from "@dev/components/board/KanbanView"
import { PlansPanel } from "@dev/components/board/PlansPanel"

/**
 * The Board tab (kantord/toylang#33): two switchable views onto the same plans/board.yaml data --
 * a React Flow dependency graph and a plain status kanban, replacing the single mixed view v1
 * shipped. Both stay read-only, click-through to issues, and public-build safe. Above them sits
 * where each plan document stands (kantord/toylang#110), which the board rows themselves cannot
 * carry: a plan is often proposed before the work it would authorize has a row at all.
 */
export function BoardPage() {
  return (
    <div className="space-y-4">
      <p className="max-w-2xl text-sm text-muted-foreground">
        <code className="rounded bg-muted px-1 py-0.5 text-xs">plans/board.yaml</code> as a
        dependency graph or a kanban: what is done, what is being worked on, and what is next in
        line.
      </p>
      <PlansPanel />
      <Tabs defaultValue="graph">
        <TabsList>
          <TabsTrigger value="graph">Graph</TabsTrigger>
          <TabsTrigger value="kanban">Kanban</TabsTrigger>
        </TabsList>
        <TabsContent value="graph">
          <GraphView />
        </TabsContent>
        <TabsContent value="kanban">
          <KanbanView />
        </TabsContent>
      </Tabs>
    </div>
  )
}
