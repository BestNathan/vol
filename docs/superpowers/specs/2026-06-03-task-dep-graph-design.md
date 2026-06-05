# Task Dependency Graph View — Design

- **Date**: 2026-06-03
- **Status**: Approved (pending spec review)
- **Crate**: `vol-llm-ui` (Dioxus 0.6 WASM web frontend)
- **Scope**: Read-only, frontend-only. No backend changes.

## Problem

The Tasks tab (`TasksPanel`) lets a user expand a task row to see its
`Dependencies:` and `Blocks:` as flat lists of task ids. There is no way to
visualize how a task relates to the rest of the dependency chain. We want a
per-row button that opens a **dependency graph** centered on that task, showing
the full transitive set of upstream dependencies and downstream blocked tasks.

## Goals

- A button on **every task row** opens a dependency graph for that task.
- The graph shows the **full transitive closure**: all upstream tasks reachable
  via `dependencies`, and all downstream tasks reachable via `blocks`.
- Rendered as an **SVG node-link diagram**: nodes are rounded rectangles laid
  out by layer, edges are arrows pointing in dependency direction.
- Nodes are **colored by task status**, reusing the existing palette.
- Clicking a node shows a **detail tooltip/panel** (subject / status / assignee /
  description). Clicking a node does **not** re-center the graph.

## Non-Goals

- No backend/protocol changes. No new JSON-RPC methods.
- No task mutation from the graph (creating/editing dependencies). The UI is
  read-only by design — task mutation is LLM-tool-only.
- No graph layout library dependency. Layout is hand-written.
- Clicking a node does not navigate or re-center (explicitly decided against).

## Existing Code Context

| Item | Location |
|------|----------|
| Tasks tab component | `crates/vol-llm-ui/src/web/components/tasks_panel.rs` |
| `status_color(&str) -> &str` palette | `tasks_panel.rs:5-14` |
| Wire-format task struct `TaskEntry` (has `dependencies: Vec<u64>`, `blocks: Vec<u64>`) | `crates/vol-llm-ui/src/web/client.rs:51-66` |
| Panel-local task state `TaskState { tasks, ... }` | `crates/vol-llm-ui/src/state/mod.rs:767-789` |
| Minimal modal pattern to copy | `crates/vol-llm-ui/src/web/components/approval_dialog.rs:21-34` |

The data needed for the graph (`dependencies` and `blocks` on each
`TaskEntry`) is already on the wire and already loaded into the panel-local
`task_state.tasks`. No fetch is required to draw the graph.

## Architecture

Frontend-only, read-only. One new component file:
`crates/vol-llm-ui/src/web/components/task_dep_graph.rs` exposing
`#[component] pub fn TaskDepGraph(...)`.

**State ownership — panel-local (chosen over global dialog state).**
`TasksPanel` already owns the full task list (`task_state`, a panel-local
`use_signal`). The graph needs that same list to walk the closure, so a
panel-local signal is the natural home:

```rust
let mut graph_target = use_signal(|| None::<u64>); // which task's graph is open
```

- Each task row gets a small icon button; its `onclick` calls
  `evt.stop_propagation()` (so it does not toggle the row's expand state) and
  sets `graph_target.set(Some(task.id))`.
- When `graph_target` is `Some(id)`, `TasksPanel` renders the `TaskDepGraph`
  modal, passing the full (unfiltered) task list and the center id.
- This avoids touching `state/mod.rs` and `app.rs`. (Rejected alternative: a
  global `Signal<GraphDialogState>` hoisted into `App` via
  `use_context_provider`, like `skill_detail_dialog` — overkill for a
  panel-local, read-only feature.)

### Data flow

```
task_state.tasks (Vec<TaskEntry>, full list, unfiltered by status)
        │
        ▼  build index + walk closure (pure fn)
GraphLayout { nodes, edges, unknown_ids }
        │
        ▼  TaskDepGraph renders
SVG node-link diagram + click-to-detail panel
```

The graph is always built from the **full** `task_state.tasks`, never the
status-filtered subset, so status filtering in the list does not hide nodes in
the graph.

## Closure Traversal + Layered Layout

The traversal/layout is a **pure function**, independent of Dioxus, so it can be
unit-tested:

```
fn build_graph_layout(tasks: &[TaskEntry], center: u64) -> GraphLayout
```

with:

```rust
struct GraphNode { id: u64, layer: i32, order: usize, known: bool }
struct GraphEdge { from: u64, to: u64 }      // from = dependency, to = dependent
struct GraphLayout { nodes: Vec<GraphNode>, edges: Vec<GraphEdge> }
```

Algorithm:

1. Build `HashMap<u64, &TaskEntry>` index over all tasks.
2. Center task is **layer 0**.
3. **Upstream BFS** along `dependencies`: a dependency of a layer-`n` node sits
   at layer `n - 1` (drawn above).
4. **Downstream BFS** along `blocks`: a blocked task of a layer-`n` node sits at
   layer `n + 1` (drawn below).
5. A `visited: HashSet<u64>` guards against cycles. The task graph is expected
   to be a DAG, but cycles are handled defensively — a node is assigned a layer
   the first time it is reached and not revisited.
6. Within each layer, nodes are ordered by discovery order; `order` is the index
   within the layer. Rendering maps `layer → y` and `order → x` with even
   horizontal spacing per layer.
7. **Edges**: for every known node `X`, for each `dep` in `X.dependencies` that
   is part of the layout, emit edge `(from = dep, to = X)`. (Edges via `blocks`
   are the inverse relation and are covered by the dependent's `dependencies`,
   so iterate dependencies only to avoid duplicate edges.)

### Edge cases

- **Unknown node**: a referenced id (in `dependencies`/`blocks`) that is not in
  the loaded task list — typical when the agent sub-tab filters tasks by
  assignee and a cross-agent dependency is absent. It is still placed in the
  layout (so the edge can be drawn) and marked `known = false`; rendered as a
  grayed-out node with a dashed border showing only `t{id}`.
- **Cycle**: handled by `visited`; no infinite loop, each node placed once.
- **Isolated task** (no deps, no blocks): graph shows the single center node.
- **Diamond** (two paths converging on one task): the converging node is placed
  once at the deepest layer it is reached at; both incoming edges are drawn.

## Interaction + Rendering

- **Entry button**: a small icon button (e.g. `⇄`) on the right of each task
  row in `tasks_panel.rs`. `onclick`: `evt.stop_propagation()` then
  `graph_target.set(Some(task.id))`.
- **Modal shell**: reuse the `approval_dialog.rs` pattern — outer
  `div.fixed.inset-0.bg-black/60 … z-[100]` with `onclick` closing
  (`graph_target.set(None)`); inner container `stop_propagation`. Sized large
  enough for the graph, e.g. `w-[95vw] max-w-[900px] max-h-[80vh]`, with body
  `overflow-auto` for graphs wider/taller than the viewport. Header shows the
  center task (`t{id} — {subject}`) and a top-right `×` close button.
- **Nodes**: SVG rounded `<rect>` + text (`t{id}` and truncated subject). Fill
  derived from `status_color(&task.status)` (the existing function in
  `tasks_panel.rs` is made shareable — moved to a small shared location or
  re-exported — so both the panel and the graph use it). The center node gets a
  highlighted/bold stroke and a `★` marker.
- **Edges**: SVG `<line>` with an arrowhead `<marker>` (`<defs>`), pointing in
  dependency direction (from dependency down to dependent).
- **Node detail**: clicking a node sets a `selected_node: Option<u64>` signal;
  the modal renders a detail panel at the bottom showing the task's subject /
  status / assignee / description. Each node also carries an SVG `<title>` child
  for native hover tooltips.
- **Styling**: existing Tailwind palette (`#1a1a2e`, `#3a3a55`, etc.), matching
  the other dialogs.

## Testing

- **Unit tests** on the pure `build_graph_layout` function (no Dioxus needed):
  - linear chain (A → B → C)
  - diamond (A → {B, C} → D)
  - cycle (A → B → A) terminates and places each node once
  - multi-layer upstream and downstream from the center
  - unknown referenced id produces an `known = false` node
  - isolated task yields a single-node layout
- **Compile/lint**: `make web-check` and `make web-clippy`.
- **Manual**: run `make web-dev` + `make web-backend`, open a task with
  dependencies, click the graph button, verify layout, colors, arrows, the
  detail panel on node click, and close behavior.

## Files Touched

| File | Change |
|------|--------|
| `crates/vol-llm-ui/src/web/components/task_dep_graph.rs` | **New** — `TaskDepGraph` component + `build_graph_layout` pure fn + unit tests |
| `crates/vol-llm-ui/src/web/components/tasks_panel.rs` | Add per-row graph button + `graph_target` signal + render `TaskDepGraph` modal; share `status_color` |
| `crates/vol-llm-ui/src/web/components/mod.rs` | Register the new component module |

## Out of Scope / Future

- Re-centering the graph by clicking a node (decided against for now).
- Zoom/pan controls for very large graphs (rely on scroll for now).
- Rendering all tasks at once as one global graph (center-task view only).
