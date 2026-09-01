// frontend/src/components/dialogs/TaskDepGraph.tsx
// Dependency graph view for the Tasks tab.
//
// `build_graph_layout` is a pure function (no React) so it can be unit-tested.
// It walks the transitive closure of a center task: upstream via `dependencies`
// (negative layers, drawn above) and downstream via `blocks` (positive layers,
// drawn below). Cycles are handled defensively via a visited set. Port of
// crates/vol-llm-ui/src/web/components/task_dep_graph.rs.
import { useMemo, useState } from 'react'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { statusColor } from '@/components/panels/TasksPanel'
import type { TaskEntry } from '@/types'

/** A node placed in the layered layout. `known = false` means the id was
 *  referenced (as a dependency/block) but is not present in the loaded task
 *  list — e.g. a cross-agent task filtered out by the agent sub-tab. */
export interface GraphNode {
  id: string
  layer: number
  order: number
  known: boolean
}

/** A directed edge in dependency direction: `from` is the dependency, `to` is
 *  the dependent task it unblocks. */
export interface GraphEdge {
  from: string
  to: string
}

export interface GraphLayout {
  nodes: GraphNode[]
  edges: GraphEdge[]
}

/** Build the layered layout for the dependency graph centered on `center`. */
export function build_graph_layout(tasks: TaskEntry[], center: string): GraphLayout {
  const index = new Map<string, TaskEntry>(tasks.map((t) => [t.id, t]))

  // Phase 1: discover the node set and classify each node's direction
  // relative to the center: -1 = upstream (reached via `dependencies`),
  // +1 = downstream (reached via `blocks`), 0 = center. A shared `visited`
  // set makes discovery cycle-safe.
  const dir_of = new Map<string, number>()
  const known_of = new Map<string, boolean>()
  const visited = new Set<string>()
  const discovery: string[] = []

  visited.add(center)
  dir_of.set(center, 0)
  known_of.set(center, index.has(center))
  discovery.push(center)

  // Upstream discovery via `dependencies`.
  const up: string[] = [center]
  while (up.length > 0) {
    const cur = up.shift()!
    const task = index.get(cur)
    if (!task) continue
    for (const dep of task.dependencies) {
      if (visited.has(dep)) continue
      visited.add(dep)
      dir_of.set(dep, -1)
      const known = index.has(dep)
      known_of.set(dep, known)
      discovery.push(dep)
      if (known) up.push(dep)
    }
  }

  // Downstream discovery via `blocks`.
  const down: string[] = [center]
  while (down.length > 0) {
    const cur = down.shift()!
    const task = index.get(cur)
    if (!task) continue
    for (const blk of task.blocks) {
      if (visited.has(blk)) continue
      visited.add(blk)
      dir_of.set(blk, 1)
      const known = index.has(blk)
      known_of.set(blk, known)
      discovery.push(blk)
      if (known) down.push(blk)
    }
  }

  // Phase 2: longest-path layering so a node sits below ALL of its upstream
  // dependencies and above ALL of its downstream dependents — every edge then
  // points one or more layers downward. Relaxation is order-independent at the
  // fixpoint; it is capped at the node count so it terminates even if the
  // (normally acyclic) graph happens to contain a cycle.
  const layer_of = new Map<string, number>()
  for (const id of discovery) {
    layer_of.set(id, dir_of.get(id)!)
  }
  layer_of.set(center, 0)

  for (let i = 0; i < discovery.length; i++) {
    let changed = false
    for (const id of discovery) {
      const task = index.get(id)
      if (!task) continue
      const here = layer_of.get(id)!
      // Push each downstream block to at least one layer below `id`.
      for (const blk of task.blocks) {
        if (dir_of.get(blk) === 1 && here + 1 > layer_of.get(blk)!) {
          layer_of.set(blk, here + 1)
          changed = true
        }
      }
      // Push each upstream dependency to at least one layer above `id`.
      for (const dep of task.dependencies) {
        if (dir_of.get(dep) === -1 && here - 1 < layer_of.get(dep)!) {
          layer_of.set(dep, here - 1)
          changed = true
        }
      }
    }
    if (!changed) break
  }

  // Assign per-layer order in discovery order.
  const layer_count = new Map<number, number>()
  const nodes: GraphNode[] = []
  for (const id of discovery) {
    const layer = layer_of.get(id)!
    const order = layer_count.get(layer) ?? 0
    layer_count.set(layer, order + 1)
    nodes.push({ id, layer, order, known: known_of.get(id)! })
  }

  // Edges (deduped). `dependencies` and `blocks` are inverse relations, and
  // iterating `discovery` (which includes unknown referenced ids) means edges
  // to unknown nodes are emitted too.
  const seen = new Set<string>()
  const edges: GraphEdge[] = []
  for (const id of discovery) {
    const task = index.get(id)
    if (!task) continue
    for (const dep of task.dependencies) {
      if (dep !== id && layer_of.has(dep) && !seen.has(`${dep}->${id}`)) {
        seen.add(`${dep}->${id}`)
        edges.push({ from: dep, to: id })
      }
    }
    for (const blk of task.blocks) {
      if (blk !== id && layer_of.has(blk) && !seen.has(`${id}->${blk}`)) {
        seen.add(`${id}->${blk}`)
        edges.push({ from: id, to: blk })
      }
    }
  }

  return { nodes, edges }
}

/** Truncate a label to `max` characters, appending an ellipsis if cut. */
function truncate(s: string, max: number): string {
  return s.length > max ? `${s.slice(0, max)}…` : s
}

export const NODE_W = 150
export const NODE_H = 44
export const COL = 180
export const ROW = 100
export const PAD = 30

interface TaskDepGraphProps {
  tasks: TaskEntry[]
  centerId: string
  onClose: () => void
}

/** Modal showing the dependency graph centered on `centerId`. */
export function TaskDepGraph({ tasks, centerId, onClose }: TaskDepGraphProps) {
  const [selected, setSelected] = useState<string | null>(null)

  const index = useMemo(() => new Map<string, TaskEntry>(tasks.map((t) => [t.id, t])), [tasks])
  const layout = useMemo(() => build_graph_layout(tasks, centerId), [tasks, centerId])

  const minLayer = Math.min(...layout.nodes.map((n) => n.layer), 0)
  const maxLayer = Math.max(...layout.nodes.map((n) => n.layer), 0)
  const maxPerLayer = layout.nodes.reduce(
    (acc, n) => {
      acc[n.layer] = (acc[n.layer] ?? 0) + 1
      return acc
    },
    {} as Record<number, number>,
  )
  const maxPerLayerCount = Math.max(...Object.values(maxPerLayer), 1)

  const pos = (layer: number, order: number): [number, number] => [
    PAD + order * COL,
    PAD + (layer - minLayer) * ROW,
  ]
  const centerXy = new Map<string, [number, number]>(
    layout.nodes.map((n) => {
      const [x, y] = pos(n.layer, n.order)
      return [n.id, [x + NODE_W / 2, y + NODE_H / 2]]
    }),
  )

  const width = PAD * 2 + (maxPerLayerCount - 1) * COL + NODE_W
  const height = PAD * 2 + (maxLayer - minLayer) * ROW + NODE_H

  const selectedTask = selected !== null ? index.get(selected) : undefined

  return (
    <Dialog
      open
      onOpenChange={(next) => {
        if (!next) onClose()
      }}
    >
      <DialogContent className="sm:max-w-[900px]">
        <DialogHeader>
          <DialogTitle>
            <span className="text-[15px] font-semibold text-foreground">
              Dependency Graph — {centerId}
            </span>
          </DialogTitle>
        </DialogHeader>
        <div className="flex flex-col min-h-0 max-h-[70vh]">
          {/* SVG scroll area */}
          <div className="flex-1 overflow-auto">
            <svg width={width} height={height}>
              <defs>
                <marker
                  id="depArrow"
                  markerWidth="8"
                  markerHeight="8"
                  refX="8"
                  refY="4"
                  orient="auto"
                >
                  <path d="M 0 0 L 8 4 L 0 8 z" fill="#7080b0" />
                </marker>
              </defs>
              {/* Edges */}
              {layout.edges.map((e) => {
                const [fx, fy] = centerXy.get(e.from)!
                const [tx, ty] = centerXy.get(e.to)!
                return (
                  <line
                    key={`${e.from}-${e.to}`}
                    x1={fx}
                    y1={fy}
                    x2={tx}
                    y2={ty}
                    stroke="#7080b0"
                    strokeWidth="1.5"
                    markerEnd="url(#depArrow)"
                  />
                )
              })}
              {/* Nodes */}
              {layout.nodes.map((n) => {
                const [x, y] = pos(n.layer, n.order)
                const task = index.get(n.id)
                const subject = task ? truncate(task.subject, 18) : '(not loaded)'
                const status = task ? task.status : 'unknown'
                const fill = n.known ? statusColor(status) : '#3a3a44'
                const isCenter = n.id === centerId
                const stroke = isCenter ? '#ffd040' : '#555577'
                const strokeWidth = isCenter ? '3' : '1'
                const dash = n.known ? '0' : '4'
                const label = isCenter ? `★ ${n.id}` : `${n.id}`
                return (
                  <g key={n.id} style={{ cursor: 'pointer' }} onClick={() => setSelected(n.id)}>
                    <rect
                      x={x}
                      y={y}
                      width={NODE_W}
                      height={NODE_H}
                      rx="6"
                      fill={fill}
                      fillOpacity="0.85"
                      stroke={stroke}
                      strokeWidth={strokeWidth}
                      strokeDasharray={dash}
                    />
                    <text x={x + 8} y={y + 17} fontSize="12" fontWeight="bold" fill="#10101a">
                      {label}
                    </text>
                    <text x={x + 8} y={y + 34} fontSize="11" fill="#10101a">
                      {subject}
                    </text>
                  </g>
                )
              })}
            </svg>
          </div>
          {/* Detail panel for the clicked node */}
          {selected !== null && (
            <div className="mt-2 pt-2 border-t border-border text-[12px] text-foreground/80">
              {selectedTask ? (
                <>
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-primary">{selectedTask.id}</span>
                    <span
                      className="px-1 rounded text-[10px] font-bold"
                      style={{ background: statusColor(selectedTask.status), color: '#10101a' }}
                    >
                      {selectedTask.status}
                    </span>
                    <span className="text-foreground">{selectedTask.subject}</span>
                    {selectedTask.assignee && (
                      <span className="text-muted-foreground/70 ml-auto">
                        {selectedTask.assignee}
                      </span>
                    )}
                  </div>
                  {selectedTask.description !== '' && (
                    <div className="mt-1 text-foreground/70">{selectedTask.description}</div>
                  )}
                </>
              ) : (
                <div className="text-muted-foreground">
                  {selected} — task not loaded (outside current filter)
                </div>
              )}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
