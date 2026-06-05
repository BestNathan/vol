---
type: concept
category: pattern
tags: [ui, web, dioxus, svg, visualization, tasks, graph]
created: 2026-06-04
updated: 2026-06-04
source_count: 1
---

# Dependency Graph Visualization

**Category:** UI pattern — rendering a node-link dependency graph in the Dioxus web frontend

A pattern for visualizing a task's dependency relationships as a layered SVG node-link diagram, split into a **pure layout function** (testable, no Dioxus) and a **thin SVG rendering component**. Introduced for the Tasks tab dependency-graph view. [[task-dependency-graph-view]]

## Key Points
- **Pure core, thin shell.** `build_graph_layout(tasks: &[TaskEntry], center: u64) -> GraphLayout` contains all graph logic and is unit-tested with zero Dioxus/WASM dependencies. The `TaskDepGraph` component only turns the resulting `GraphLayout` into SVG.
- **Layered by direction.** The center task is layer 0; upstream `dependencies` get negative layers (drawn above), downstream `blocks` get positive layers (drawn below). Each `GraphNode { id, layer, order, known }`; each `GraphEdge { from, to }` points in dependency direction (dependency → dependent).
- **Longest-path layering.** Layers are assigned by relaxation so a node sits below *all* of its upstream dependencies — every edge then points downward. A naive shortest-path BFS is insufficient: in an asymmetric DAG (e.g. `1→4` plus `1→2→3→4`) it places the converging node too shallow and draws backward/upward arrows.
- **Cycle-safe and terminating.** Discovery uses a shared `visited` set; the relaxation loop is bounded by the node count, so it terminates even if the (normally acyclic) data contains a cycle.
- **Unknown nodes.** Ids referenced via `dependencies`/`blocks` but absent from the loaded list are placed with `known = false` and rendered gray with a dashed border; edges to them are still drawn. This surfaces cross-context dependencies instead of silently dropping them.
- **Full dataset, not the filtered view.** The graph is built from the complete unfiltered task list so list-level status filtering never hides graph nodes.

## How It Works
1. Index tasks by id into a `HashMap<u64, &TaskEntry>`.
2. BFS upstream along `dependencies` and downstream along `blocks` from the center to discover the node set and classify each node's direction.
3. Assign final layers by relaxation (downstream pushes a block at least one layer below; upstream pushes a dependency at least one layer above), capped at the node count.
4. Assign per-layer horizontal `order` in discovery order.
5. Emit deduped edges (skipping self-loops) in dependency direction.
6. The component maps `(layer, order)` to SVG coordinates via fixed `NODE_W/NODE_H/COL/ROW/PAD` constants, draws `<line>` edges with a shared `<marker>` arrowhead and `<rect>`+`<text>` nodes colored by [[dioxus-web-pattern]]'s shared `status_color`, highlights the center (gold stroke + ★), and shows a click-to-detail panel driven by a `selected` signal.

## Examples
- `crates/vol-llm-ui/src/web/components/task_dep_graph.rs` — the full implementation and its seven unit tests (linear chain, diamond, asymmetric longest-path, cycle, unknown id, isolated, source regression).

## Related Concepts
- [[dioxus-web-pattern]] — overall Dioxus component/modal conventions; reuses the `approval_dialog.rs` overlay shell and shared `status_color`.
- [[dioxus-signal-pattern]] — panel-local `Signal<Option<u64>>` trigger and the copy-before-move closure idiom.
- [[file-tab-pattern]] — sibling read-only tab rendering in the same crate.
