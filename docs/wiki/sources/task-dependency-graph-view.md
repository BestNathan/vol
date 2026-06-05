---
type: source
source_type: code
date: 2026-06-04
ingested: 2026-06-04
tags: [ui, web, dioxus, tasks, svg, visualization]
---

# Task Dependency Graph View

**Authors/Creators:** Nathan
**Date:** 2026-06-04
**Link:** `crates/vol-llm-ui/src/web/components/task_dep_graph.rs`; spec `docs/superpowers/specs/2026-06-03-task-dep-graph-design.md`; plan `docs/superpowers/plans/2026-06-03-task-dep-graph.md`

## TL;DR
A per-row "⇄ deps" button was added to the Tasks tab of the Dioxus web frontend. Clicking it opens a modal that renders an SVG node-link graph of the selected task's full transitive dependency closure — upstream `dependencies` above, downstream `blocks` below — with nodes colored by task status, the center task highlighted, and a click-to-detail panel. The feature is read-only and frontend-only; no backend or protocol changes were needed because every `TaskEntry` already carries `dependencies: Vec<u64>` and `blocks: Vec<u64>` on the wire.

## Key Takeaways
- New component file `crates/vol-llm-ui/src/web/components/task_dep_graph.rs` holds a **pure** `build_graph_layout(tasks, center) -> GraphLayout` function plus the `TaskDepGraph` Dioxus SVG component.
- Layout uses **longest-path (Sugiyama-style) layering**: a node sits one layer below every one of its upstream dependencies, so every edge points downward; an initial shortest-path BFS approach was rejected because asymmetric DAGs produced upward/backward arrows.
- Discovery is cycle-safe (shared `visited` set); the relaxation pass is capped at the node count so it terminates even on a (non-DAG) cycle.
- Tasks referenced but not in the loaded list (e.g. a cross-agent dependency filtered out by the agent sub-tab) render as gray, dashed-border "(not loaded)" nodes — edges to them are still drawn.
- The graph is always built from the **full unfiltered** `task_state.tasks`, never the status-filtered subset, so list filtering never hides graph nodes.
- State is **panel-local**: `TasksPanel` owns a `Signal<Option<u64>>` (`graph_target`); no global dialog state was hoisted into `App`. The modal reuses the `approval_dialog.rs` overlay shell.
- `TaskEntry` gained a `PartialEq` derive (required because Dioxus `#[component]` props must be `PartialEq`); `status_color` in `tasks_panel.rs` was promoted to `pub(crate)` and shared.
- Verified: host `cargo check`, `wasm32-unknown-unknown` build, full 39-test suite (incl. 7 graph-layout tests), and a real-browser boot smoke test (Chromium via Playwright) with no errors originating from the change.

## Detailed Summary
The pure layout function indexes tasks by id, runs two BFS passes from the center (upstream via `dependencies` for negative layers, downstream via `blocks` for positive layers), classifies each discovered node's direction, then assigns final layers by relaxation so converging nodes land at their deepest reachable layer. Per-layer horizontal order is assigned in discovery order. Edges are deduped via a `HashSet<(from, to)>`, emitted in dependency direction (`from` = dependency, `to` = dependent), and self-loops (`dep == id`) are skipped.

The `TaskDepGraph` component computes SVG geometry from the layout (`NODE_W/NODE_H/COL/ROW/PAD` constants), renders edges as `<line>` elements with a shared `<marker id="depArrow">` arrowhead, and nodes as `<rect>` + two `<text>` labels filled via the shared `status_color`. The center node gets a gold stroke and ★ marker; unknown nodes get a gray fill and dashed border. Clicking a node sets a `selected` signal that drives a detail panel (id / status / subject / assignee / description) below the SVG. The modal closes on backdrop click or the × button via an `on_close: EventHandler<()>`. All SVG attributes compiled with Dioxus 0.6's snake_case form (`marker_end`, `stroke_width`, `ref_x`, etc.) — no raw-attribute fallback needed.

The work was executed via subagent-driven development across six commits (test unblock → pure logic → longest-path fix → SVG component → panel wiring → self-loop guard).

## Entities Mentioned
- [[vol-llm-ui-crate]]: hosts the new `TaskDepGraph` component, the `graph_target` signal in `TasksPanel`, and the `TaskEntry` `PartialEq` derive.

## Concepts Covered
- [[dependency-graph-visualization]]: the pure-layout + Dioxus-SVG pattern introduced here.
- [[dioxus-signal-pattern]]: panel-local `Signal<Option<u64>>` and the copy-before-move idiom for closures.
- [[dioxus-web-pattern]]: component/modal conventions reused from `approval_dialog.rs`.
- [[file-tab-pattern]]: sibling read-only Tasks/file tab rendering approach.

## Notes
- The actual rendered graph could only be exercised with backend-seeded, dependency-linked tasks; the in-browser visual check (graph layout/colors/arrows with real data) is left to a human running the full three-service stack.
- Follow-up: the Tasks tab itself (`TasksPanel`, `task.list`/`task.get` JSON-RPC) predates this and is not yet captured as its own wiki concept.
