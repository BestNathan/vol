# Task Dependency Graph View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a per-row button to the Tasks tab that opens a modal showing an SVG dependency graph (full transitive closure) centered on the chosen task.

**Architecture:** Frontend-only, read-only. A new component `task_dep_graph.rs` exposes a pure `build_graph_layout()` function (unit-tested, no Dioxus) plus a `TaskDepGraph` Dioxus component that renders the layout as an SVG node-link diagram. `TasksPanel` owns a panel-local `graph_target: Signal<Option<u64>>`, sets it from a per-row button, and renders the modal. Both `dependencies` and `blocks` are already on every `TaskEntry`, so no backend or protocol changes are needed.

**Tech Stack:** Rust, Dioxus 0.6 (WASM web frontend, `vol-llm-ui` crate), native SVG in `rsx!`, Tailwind CSS.

**Spec:** `docs/superpowers/specs/2026-06-03-task-dep-graph-design.md`

---

## File Structure

| File | Responsibility |
|------|----------------|
| `crates/vol-llm-ui/src/state/mod.rs` | (Task 0) Fix pre-existing test-compile breakage that blocks running ANY unit test in the crate. |
| `crates/vol-llm-ui/src/web/components/task_dep_graph.rs` | **New.** Pure layout logic (`build_graph_layout` + `GraphNode`/`GraphEdge`/`GraphLayout`) and unit tests (Task 1); `TaskDepGraph` SVG component + `truncate` helper (Task 2). |
| `crates/vol-llm-ui/src/web/components/mod.rs` | Register the new module (Task 1) and re-export the component (Task 2). |
| `crates/vol-llm-ui/src/web/components/tasks_panel.rs` | Make `status_color` shareable (Task 2); add per-row graph button + `graph_target` signal + render the modal (Task 3). |

## Conventions for every task

- Test command (host, web feature): `cargo test -p vol-llm-ui --no-default-features --features web`
- Compile check: `make web-check` (= `cargo check -p vol-llm-ui --no-default-features --features web`)
- Lint: `make web-clippy`
- The crate's existing tests use `--no-default-features --features web`; the TUI feature is mutually exclusive with web for these modules.

---

## Task 0: Unblock the crate test suite

The `vol-llm-ui` test binary does not currently compile under the web feature: three test sites construct `UiEvent::AgentStart` without the required `run_id: String` field. Until this is fixed, `cargo test` cannot run, so no TDD is possible. This is a targeted fix to the test code only.

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs:1266`, `:1306`, `:1370`

- [ ] **Step 1: Confirm the breakage**

Run: `cargo test -p vol-llm-ui --no-default-features --features web --no-run 2>&1 | grep "missing field"`
Expected: three lines, `missing field \`run_id\`` at `state/mod.rs:1266`, `:1306`, `:1370`.

- [ ] **Step 2: Fix site 1266**

In `crates/vol-llm-ui/src/state/mod.rs`, replace:

```rust
        let event = UiEvent::AgentStart { input: "hello".into() };
```

with:

```rust
        let event = UiEvent::AgentStart { run_id: "test-run".into(), input: "hello".into() };
```

- [ ] **Step 3: Fix site 1306**

Replace:

```rust
        state.apply(UiEvent::AgentStart { input: "fix the bug".into() });
```

with:

```rust
        state.apply(UiEvent::AgentStart { run_id: "test-run".into(), input: "fix the bug".into() });
```

- [ ] **Step 4: Fix site 1370**

Replace:

```rust
        assert_eq!(UiEvent::AgentStart { input: "hi".into() }.kind(), UiEventKind::AgentStart);
```

with:

```rust
        assert_eq!(UiEvent::AgentStart { run_id: "test-run".into(), input: "hi".into() }.kind(), UiEventKind::AgentStart);
```

- [ ] **Step 5: Verify the test suite now compiles and runs**

Run: `cargo test -p vol-llm-ui --no-default-features --features web`
Expected: compiles; existing tests PASS (no `E0063` errors).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "test: add missing run_id to AgentStart in vol-llm-ui state tests"
```

---

## Task 1: Pure graph layout logic + unit tests

Create the new module with only the pure, Dioxus-free layout function and its tests. Register the module (no re-export yet — the component is added in Task 2).

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/task_dep_graph.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`
- Test: same file (`#[cfg(test)] mod tests` in `task_dep_graph.rs`)

- [ ] **Step 1: Write the failing tests**

Create `crates/vol-llm-ui/src/web/components/task_dep_graph.rs` with the data types, a stub function, and the tests:

```rust
//! Dependency graph view for the Tasks tab.
//!
//! `build_graph_layout` is pure (no Dioxus) so it can be unit-tested. It walks
//! the transitive closure of a center task: upstream via `dependencies`
//! (negative layers, drawn above) and downstream via `blocks` (positive layers,
//! drawn below). Cycles are handled defensively via a visited set.

use crate::web::client::TaskEntry;
use std::collections::{HashMap, HashSet, VecDeque};

/// A node placed in the layered layout. `known = false` means the id was
/// referenced (as a dependency/block) but is not present in the loaded task
/// list — e.g. a cross-agent task filtered out by the agent sub-tab.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub id: u64,
    pub layer: i32,
    pub order: usize,
    pub known: bool,
}

/// A directed edge in dependency direction: `from` is the dependency, `to` is
/// the dependent task it unblocks.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdge {
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphLayout {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Build the layered layout for the dependency graph centered on `center`.
pub fn build_graph_layout(tasks: &[TaskEntry], center: u64) -> GraphLayout {
    GraphLayout { nodes: Vec::new(), edges: Vec::new() } // replaced in Step 4
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: u64, deps: Vec<u64>, blocks: Vec<u64>) -> TaskEntry {
        TaskEntry {
            id,
            status: "pending".into(),
            kind: "task".into(),
            publisher: None,
            assignee: None,
            subject: format!("task {id}"),
            description: String::new(),
            active_form: None,
            dependencies: deps,
            blocks,
            created_at: 0,
            started_at: None,
            completed_at: None,
        }
    }

    fn node<'a>(layout: &'a GraphLayout, id: u64) -> &'a GraphNode {
        layout.nodes.iter().find(|n| n.id == id).expect("node present")
    }

    fn has_edge(layout: &GraphLayout, from: u64, to: u64) -> bool {
        layout.edges.iter().any(|e| e.from == from && e.to == to)
    }

    #[test]
    fn linear_chain_layers_above_and_below_center() {
        let tasks = vec![
            t(1, vec![], vec![2]),
            t(2, vec![1], vec![3]),
            t(3, vec![2], vec![]),
        ];
        let layout = build_graph_layout(&tasks, 2);
        assert_eq!(node(&layout, 1).layer, -1);
        assert_eq!(node(&layout, 2).layer, 0);
        assert_eq!(node(&layout, 3).layer, 1);
        assert!(has_edge(&layout, 1, 2));
        assert!(has_edge(&layout, 2, 3));
        assert_eq!(layout.nodes.len(), 3);
    }

    #[test]
    fn diamond_converges_at_deepest_layer() {
        let tasks = vec![
            t(1, vec![], vec![2, 3]),
            t(2, vec![1], vec![4]),
            t(3, vec![1], vec![4]),
            t(4, vec![2, 3], vec![]),
        ];
        let layout = build_graph_layout(&tasks, 1);
        assert_eq!(node(&layout, 1).layer, 0);
        assert_eq!(node(&layout, 2).layer, 1);
        assert_eq!(node(&layout, 3).layer, 1);
        assert_eq!(node(&layout, 4).layer, 2);
        assert!(has_edge(&layout, 1, 2));
        assert!(has_edge(&layout, 1, 3));
        assert!(has_edge(&layout, 2, 4));
        assert!(has_edge(&layout, 3, 4));
        // distinct order within layer 1
        assert_ne!(node(&layout, 2).order, node(&layout, 3).order);
    }

    #[test]
    fn cycle_terminates_and_places_each_node_once() {
        let tasks = vec![
            t(1, vec![2], vec![2]),
            t(2, vec![1], vec![1]),
        ];
        let layout = build_graph_layout(&tasks, 1);
        assert_eq!(layout.nodes.len(), 2);
        assert_eq!(node(&layout, 1).layer, 0);
        assert_eq!(node(&layout, 2).layer, -1);
    }

    #[test]
    fn unknown_referenced_id_is_marked_not_known() {
        let tasks = vec![t(1, vec![99], vec![])];
        let layout = build_graph_layout(&tasks, 1);
        assert!(node(&layout, 1).known);
        assert!(!node(&layout, 99).known);
        assert_eq!(node(&layout, 99).layer, -1);
        assert!(has_edge(&layout, 99, 1));
    }

    #[test]
    fn isolated_task_is_single_node_no_edges() {
        let tasks = vec![t(1, vec![], vec![])];
        let layout = build_graph_layout(&tasks, 1);
        assert_eq!(layout.nodes.len(), 1);
        assert_eq!(node(&layout, 1).layer, 0);
        assert!(layout.edges.is_empty());
    }
}
```

- [ ] **Step 2: Register the module so the tests compile**

In `crates/vol-llm-ui/src/web/components/mod.rs`, add after `pub mod tasks_panel;`:

```rust
pub mod task_dep_graph;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vol-llm-ui --no-default-features --features web task_dep_graph`
Expected: the five `task_dep_graph::tests::*` tests FAIL (empty layout → assertions fail / `expect("node present")` panics).

- [ ] **Step 4: Implement `build_graph_layout`**

Replace the stub function body with:

```rust
pub fn build_graph_layout(tasks: &[TaskEntry], center: u64) -> GraphLayout {
    let index: HashMap<u64, &TaskEntry> = tasks.iter().map(|t| (t.id, t)).collect();

    let mut layer_of: HashMap<u64, i32> = HashMap::new();
    let mut known_of: HashMap<u64, bool> = HashMap::new();
    let mut visited: HashSet<u64> = HashSet::new();
    let mut discovery: Vec<u64> = Vec::new();

    // Center task is layer 0.
    visited.insert(center);
    layer_of.insert(center, 0);
    known_of.insert(center, index.contains_key(&center));
    discovery.push(center);

    // Upstream BFS along `dependencies` (one layer up per hop).
    let mut up: VecDeque<u64> = VecDeque::from([center]);
    while let Some(cur) = up.pop_front() {
        if let Some(task) = index.get(&cur) {
            let cur_layer = layer_of[&cur];
            for &dep in &task.dependencies {
                if visited.insert(dep) {
                    layer_of.insert(dep, cur_layer - 1);
                    let known = index.contains_key(&dep);
                    known_of.insert(dep, known);
                    discovery.push(dep);
                    if known {
                        up.push_back(dep);
                    }
                }
            }
        }
    }

    // Downstream BFS along `blocks` (one layer down per hop).
    let mut down: VecDeque<u64> = VecDeque::from([center]);
    while let Some(cur) = down.pop_front() {
        if let Some(task) = index.get(&cur) {
            let cur_layer = layer_of[&cur];
            for &blk in &task.blocks {
                if visited.insert(blk) {
                    layer_of.insert(blk, cur_layer + 1);
                    let known = index.contains_key(&blk);
                    known_of.insert(blk, known);
                    discovery.push(blk);
                    if known {
                        down.push_back(blk);
                    }
                }
            }
        }
    }

    // Assign per-layer order in discovery order.
    let mut layer_count: HashMap<i32, usize> = HashMap::new();
    let mut nodes: Vec<GraphNode> = Vec::with_capacity(discovery.len());
    for id in &discovery {
        let layer = layer_of[id];
        let order = *layer_count.get(&layer).unwrap_or(&0);
        layer_count.insert(layer, order + 1);
        nodes.push(GraphNode { id: *id, layer, order, known: known_of[id] });
    }

    // Edges (deduped). `dependencies` and `blocks` are inverse relations, so
    // pulling from both and deduping covers edges to unknown nodes too.
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    for id in &discovery {
        if let Some(task) = index.get(id) {
            for &dep in &task.dependencies {
                if layer_of.contains_key(&dep) && seen.insert((dep, *id)) {
                    edges.push(GraphEdge { from: dep, to: *id });
                }
            }
            for &blk in &task.blocks {
                if layer_of.contains_key(&blk) && seen.insert((*id, blk)) {
                    edges.push(GraphEdge { from: *id, to: blk });
                }
            }
        }
    }

    GraphLayout { nodes, edges }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vol-llm-ui --no-default-features --features web task_dep_graph`
Expected: all five `task_dep_graph::tests::*` tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/task_dep_graph.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat: add pure dependency-graph layout logic for task tab"
```

---

## Task 2: `TaskDepGraph` SVG rendering component

Add the Dioxus component that renders the layout as an SVG node-link diagram, plus a small `truncate` helper. Make `status_color` shareable so the graph reuses the list's palette. Re-export the component.

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/task_dep_graph.rs`
- Modify: `crates/vol-llm-ui/src/web/client.rs:50-51` (derive `PartialEq` on `TaskEntry`)
- Modify: `crates/vol-llm-ui/src/web/components/tasks_panel.rs:5` (visibility of `status_color`)
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs` (re-export)

- [ ] **Step 1: Derive `PartialEq` on `TaskEntry`**

`TaskDepGraph` takes `tasks: Vec<TaskEntry>` as a prop. Dioxus's `#[component]` macro generates a `Props` struct deriving `PartialEq`, which requires `TaskEntry: PartialEq`. In `crates/vol-llm-ui/src/web/client.rs`, change the derive on `TaskEntry` (line 50) from:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEntry {
```

to:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEntry {
```

(All `TaskEntry` fields — `u64`, `String`, `Option<String>`, `Vec<u64>` — are `PartialEq`, so this is safe.)

- [ ] **Step 2: Make `status_color` shareable**

In `crates/vol-llm-ui/src/web/components/tasks_panel.rs`, change line 5 from:

```rust
fn status_color(status: &str) -> &'static str {
```

to:

```rust
pub(crate) fn status_color(status: &str) -> &'static str {
```

- [ ] **Step 3: Add imports and the `truncate` helper to the component file**

At the top of `crates/vol-llm-ui/src/web/components/task_dep_graph.rs`, change the imports block to add Dioxus and the shared color fn:

```rust
use crate::web::client::TaskEntry;
use crate::web::components::tasks_panel::status_color;
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
```

Then, just below the `build_graph_layout` function (before `#[cfg(test)]`), add the helper:

```rust
/// Truncate a label to `max` characters, appending an ellipsis if cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}
```

- [ ] **Step 4: Add the component**

Append the `TaskDepGraph` component to `crates/vol-llm-ui/src/web/components/task_dep_graph.rs` (before `#[cfg(test)]`):

```rust
const NODE_W: i32 = 150;
const NODE_H: i32 = 44;
const COL: i32 = 180;
const ROW: i32 = 100;
const PAD: i32 = 30;

/// Modal showing the dependency graph centered on `center`.
#[component]
pub fn TaskDepGraph(tasks: Vec<TaskEntry>, center: u64, on_close: EventHandler<()>) -> Element {
    let selected = use_signal(|| None::<u64>);

    let index: HashMap<u64, &TaskEntry> = tasks.iter().map(|t| (t.id, t)).collect();
    let layout = build_graph_layout(&tasks, center);

    let min_layer = layout.nodes.iter().map(|n| n.layer).min().unwrap_or(0);
    let max_layer = layout.nodes.iter().map(|n| n.layer).max().unwrap_or(0);
    let max_per_layer = {
        let mut m: HashMap<i32, usize> = HashMap::new();
        for n in &layout.nodes {
            *m.entry(n.layer).or_insert(0) += 1;
        }
        m.values().copied().max().unwrap_or(1)
    };

    let pos = |layer: i32, order: usize| -> (i32, i32) {
        (PAD + order as i32 * COL, PAD + (layer - min_layer) * ROW)
    };
    let center_xy: HashMap<u64, (i32, i32)> = layout
        .nodes
        .iter()
        .map(|n| {
            let (x, y) = pos(n.layer, n.order);
            (n.id, (x + NODE_W / 2, y + NODE_H / 2))
        })
        .collect();

    let width = PAD * 2 + (max_per_layer as i32 - 1).max(0) * COL + NODE_W;
    let height = PAD * 2 + (max_layer - min_layer) * ROW + NODE_H;

    rsx! {
        div {
            class: "fixed inset-0 bg-black/60 flex items-center justify-center z-[100]",
            onclick: move |_| on_close.call(()),
            div {
                class: "bg-[#252540] border border-[#444466] rounded-lg p-3 sm:p-4 w-[95vw] max-w-[900px] max-h-[85vh] flex flex-col overflow-hidden",
                onclick: move |evt| evt.stop_propagation(),
                // Header
                div { class: "flex items-center justify-between border-b border-[#333355] pb-2 mb-2",
                    div { class: "text-[15px] font-bold text-[#e0e0e0]", "Dependency Graph — t{center}" }
                    button {
                        class: "text-[#888] hover:text-[#fff] text-[18px] leading-none px-2",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                // SVG scroll area
                div { class: "flex-1 overflow-auto",
                    svg { width: "{width}", height: "{height}",
                        defs {
                            marker {
                                id: "depArrow",
                                marker_width: "8",
                                marker_height: "8",
                                ref_x: "8",
                                ref_y: "4",
                                orient: "auto",
                                path { d: "M 0 0 L 8 4 L 0 8 z", fill: "#7080b0" }
                            }
                        }
                        // Edges
                        for e in layout.edges.iter() {
                            {
                                let (fx, fy) = center_xy[&e.from];
                                let (tx, ty) = center_xy[&e.to];
                                rsx! {
                                    line {
                                        x1: "{fx}", y1: "{fy}", x2: "{tx}", y2: "{ty}",
                                        stroke: "#7080b0", stroke_width: "1.5",
                                        marker_end: "url(#depArrow)"
                                    }
                                }
                            }
                        }
                        // Nodes
                        for n in layout.nodes.iter() {
                            {
                                let (x, y) = pos(n.layer, n.order);
                                let lx = x + 8;
                                let ty1 = y + 17;
                                let ty2 = y + 34;
                                let nw = NODE_W;
                                let nh = NODE_H;
                                let task = index.get(&n.id);
                                let subject = task.map(|t| truncate(&t.subject, 18)).unwrap_or_else(|| "(not loaded)".into());
                                let status = task.map(|t| t.status.clone()).unwrap_or_else(|| "unknown".into());
                                let fill = if n.known { status_color(&status) } else { "#3a3a44" };
                                let is_center = n.id == center;
                                let stroke = if is_center { "#ffd040" } else { "#555577" };
                                let stroke_w = if is_center { "3" } else { "1" };
                                let dash = if n.known { "0" } else { "4" };
                                let label = if is_center { format!("★ t{}", n.id) } else { format!("t{}", n.id) };
                                let mut sel = selected;
                                let nid = n.id;
                                rsx! {
                                    g {
                                        style: "cursor: pointer;",
                                        onclick: move |_| sel.set(Some(nid)),
                                        rect {
                                            x: "{x}", y: "{y}", width: "{nw}", height: "{nh}", rx: "6",
                                            fill: "{fill}", fill_opacity: "0.85",
                                            stroke: "{stroke}", stroke_width: "{stroke_w}", stroke_dasharray: "{dash}"
                                        }
                                        text { x: "{lx}", y: "{ty1}", font_size: "12", font_weight: "bold", fill: "#10101a", "{label}" }
                                        text { x: "{lx}", y: "{ty2}", font_size: "11", fill: "#10101a", "{subject}" }
                                    }
                                }
                            }
                        }
                    }
                }
                // Detail panel for the clicked node
                if let Some(sid) = selected() {
                    {
                        let task = index.get(&sid);
                        rsx! {
                            div { class: "mt-2 pt-2 border-t border-[#333355] text-[12px] text-[#ccc]",
                                if let Some(t) = task {
                                    div { class: "flex gap-2 items-center",
                                        span { class: "font-mono text-[#80a0ff]", "t{t.id}" }
                                        span {
                                            class: "px-1 rounded text-[10px] font-bold",
                                            style: "background: {status_color(&t.status)}; color: #10101a;",
                                            "{t.status}"
                                        }
                                        span { class: "text-[#e0e0e0]", "{t.subject}" }
                                        if let Some(a) = t.assignee.as_ref() {
                                            span { class: "text-[#666] ml-auto", "{a}" }
                                        }
                                    }
                                    if !t.description.is_empty() {
                                        div { class: "mt-1 text-[#aaa]", "{t.description}" }
                                    }
                                } else {
                                    div { class: "text-[#888]", "t{sid} — task not loaded (outside current filter)" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 5: Re-export the component**

In `crates/vol-llm-ui/src/web/components/mod.rs`, add alongside the other `pub use` lines (e.g. after `pub use tasks_panel::TasksPanel;` if present, otherwise near the `pub use` block):

```rust
pub use task_dep_graph::TaskDepGraph;
```

- [ ] **Step 6: Add a source-string regression test**

This mirrors the crate's existing component-test convention (`skills.rs` checks the rendered source for required tokens). Inside the `#[cfg(test)] mod tests` block in `task_dep_graph.rs`, add:

```rust
    #[test]
    fn component_source_has_arrow_marker_and_header() {
        let source = include_str!("task_dep_graph.rs");
        assert!(source.contains("url(#depArrow)"));
        assert!(source.contains("Dependency Graph"));
    }
```

- [ ] **Step 7: Verify compile, lint, and tests**

Run: `make web-check`
Expected: finishes with no errors.

Run: `make web-clippy`
Expected: no clippy errors.

Run: `cargo test -p vol-llm-ui --no-default-features --features web task_dep_graph`
Expected: all `task_dep_graph::tests::*` tests PASS (now six).

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs crates/vol-llm-ui/src/web/components/task_dep_graph.rs crates/vol-llm-ui/src/web/components/tasks_panel.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat: add TaskDepGraph SVG dependency-graph component"
```

---

## Task 3: Wire the graph button + modal into TasksPanel

Add a per-row button that opens the graph modal for that task, backed by a panel-local `graph_target` signal. The graph is built from the full, unfiltered `tasks` list (already bound at `tasks_panel.rs:65`) so status filtering never hides nodes.

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tasks_panel.rs`

- [ ] **Step 1: Add the import for the component**

Near the top of `crates/vol-llm-ui/src/web/components/tasks_panel.rs`, change:

```rust
use crate::state::TaskState;
```

to:

```rust
use crate::state::TaskState;
use super::task_dep_graph::TaskDepGraph;
```

- [ ] **Step 2: Add the `graph_target` signal**

In `crates/vol-llm-ui/src/web/components/tasks_panel.rs`, immediately after line 19 (`let task_state = use_signal(|| TaskState::new());`), add:

```rust
    let mut graph_target = use_signal(|| None::<u64>);
```

- [ ] **Step 3: Add per-row bindings for the button**

The button needs its own copy of the task id and a mutable copy of the signal (matching the codebase pattern at `tasks_panel.rs:135`, where a signal is copied into a `let mut` binding before being moved into a closure — calling `.set()` directly on a moved signal does not compile). Just after the existing `let task_id2 = task.id;` line (around `tasks_panel.rs:163`), add:

```rust
                        let task_id3 = task.id;
                        let mut graph_open = graph_target;
```

- [ ] **Step 4: Replace the row's trailing assignee span with an assignee+button group**

In the row header (`div { class: "flex items-center gap-2", ... }`), replace this block:

```rust
                                    if let Some(ref assignee) = task.assignee {
                                        span { class: "text-[11px] text-[#666] ml-auto whitespace-nowrap", "{assignee}" }
                                    }
```

with:

```rust
                                    div { class: "flex items-center gap-2 ml-auto",
                                        if let Some(ref assignee) = task.assignee {
                                            span { class: "text-[11px] text-[#666] whitespace-nowrap", "{assignee}" }
                                        }
                                        button {
                                            class: "text-[11px] text-[#80a0ff] hover:text-[#a0c0ff] px-1 rounded whitespace-nowrap",
                                            title: "View dependency graph",
                                            onclick: move |evt| {
                                                evt.stop_propagation();
                                                graph_open.set(Some(task_id3));
                                            },
                                            "⇄ deps"
                                        }
                                    }
```

- [ ] **Step 5: Render the modal**

The list area ends with the task-list `div`. Immediately after that closing `div` (still inside the outer `div { class: "flex flex-col flex-1 min-h-0 overflow-hidden", ... }`, i.e. right before its closing brace), add:

```rust
            if let Some(center) = graph_target() {
                {
                    let mut graph_close = graph_target;
                    rsx! {
                        TaskDepGraph {
                            tasks: tasks.clone(),
                            center,
                            on_close: move |_| graph_close.set(None),
                        }
                    }
                }
            }
```

- [ ] **Step 6: Verify compile, lint, and the full test suite**

Run: `make web-check`
Expected: finishes with no errors.

Run: `make web-clippy`
Expected: no clippy errors.

Run: `cargo test -p vol-llm-ui --no-default-features --features web`
Expected: all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tasks_panel.rs
git commit -m "feat: add dependency-graph button and modal to task tab"
```

---

## Task 4: Manual verification

Confirm the feature works end-to-end in the running app.

**Files:** none (verification only)

- [ ] **Step 1: Build the WASM binary**

Run: `make web-build`
Expected: WASM build succeeds.

- [ ] **Step 2: Start the three dev services (separate terminals)**

```bash
make web-css       # terminal 1 — Tailwind watch
make web-dev       # terminal 2 — Dioxus dev server on :8080
make web-backend   # terminal 3 — JSON-RPC backend on :3001
```

- [ ] **Step 3: Verify in the browser**

Open `http://localhost:8080`, go to the Tasks tab. For a task that has dependencies and/or blocks:
- The row shows a `⇄ deps` button on the right.
- Clicking the button opens the modal WITHOUT toggling the row's inline expand (the `stop_propagation` works).
- The graph shows the center task highlighted (★, gold border) with upstream dependencies above and blocked tasks below.
- Node fill colors match task status (running blue, completed green, failed red, etc.).
- Clicking a node shows its detail (subject/status/assignee/description) in the panel below the graph.
- Clicking the backdrop or the `×` closes the modal.
- Toggling the status filter (e.g. to `completed`) does NOT remove nodes from a graph opened afterward.

- [ ] **Step 4: Spot-check an edge case**

Open the graph for a task whose dependency is not in the current list (e.g. from an agent sub-tab filtered by assignee). Confirm the missing node renders grayed with a dashed border and label `t{id}`, and that clicking it shows "task not loaded".

---

## Post-implementation

Per project convention (CLAUDE.md), after the feature is complete run the `wiki-ingest` skill to update `docs/wiki` with the new dependency-graph view. The spec doc should also be uploaded to Feishu with `lark-cli` (wiki node `Og7twpiPoi0Vbjk2EzvcqX92nsb`) — note `lark-cli` was not available in the planning environment, so confirm it is installed first.
