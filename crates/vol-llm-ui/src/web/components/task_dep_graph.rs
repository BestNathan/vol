//! Dependency graph view for the Tasks tab.
//!
//! `build_graph_layout` is pure (no Dioxus) so it can be unit-tested. It walks
//! the transitive closure of a center task: upstream via `dependencies`
//! (negative layers, drawn above) and downstream via `blocks` (positive layers,
//! drawn below). Cycles are handled defensively via a visited set.

use crate::web::client::TaskEntry;
use crate::web::components::tasks_panel::status_color;
use dioxus::prelude::*;
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
    let index: HashMap<u64, &TaskEntry> = tasks.iter().map(|t| (t.id, t)).collect();

    // Phase 1: discover the node set and classify each node's direction
    // relative to the center: -1 = upstream (reached via `dependencies`),
    // +1 = downstream (reached via `blocks`), 0 = center. A shared `visited`
    // set makes discovery cycle-safe.
    let mut dir_of: HashMap<u64, i32> = HashMap::new();
    let mut known_of: HashMap<u64, bool> = HashMap::new();
    let mut visited: HashSet<u64> = HashSet::new();
    let mut discovery: Vec<u64> = Vec::new();

    visited.insert(center);
    dir_of.insert(center, 0);
    known_of.insert(center, index.contains_key(&center));
    discovery.push(center);

    // Upstream discovery via `dependencies`.
    let mut up: VecDeque<u64> = VecDeque::from([center]);
    while let Some(cur) = up.pop_front() {
        if let Some(task) = index.get(&cur) {
            for &dep in &task.dependencies {
                if visited.insert(dep) {
                    dir_of.insert(dep, -1);
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

    // Downstream discovery via `blocks`.
    let mut down: VecDeque<u64> = VecDeque::from([center]);
    while let Some(cur) = down.pop_front() {
        if let Some(task) = index.get(&cur) {
            for &blk in &task.blocks {
                if visited.insert(blk) {
                    dir_of.insert(blk, 1);
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

    // Phase 2: longest-path layering so a node sits below ALL of its upstream
    // dependencies and above ALL of its downstream dependents — every edge then
    // points one or more layers downward. Relaxation is order-independent at the
    // fixpoint; it is capped at the node count so it terminates even if the
    // (normally acyclic) graph happens to contain a cycle.
    let mut layer_of: HashMap<u64, i32> = HashMap::new();
    for &id in &discovery {
        layer_of.insert(id, dir_of[&id]);
    }
    layer_of.insert(center, 0);

    for _ in 0..discovery.len() {
        let mut changed = false;
        for &id in &discovery {
            if let Some(task) = index.get(&id) {
                let here = layer_of[&id];
                // Push each downstream block to at least one layer below `id`.
                for &blk in &task.blocks {
                    if dir_of.get(&blk).copied() == Some(1) && here + 1 > layer_of[&blk] {
                        layer_of.insert(blk, here + 1);
                        changed = true;
                    }
                }
                // Push each upstream dependency to at least one layer above `id`.
                for &dep in &task.dependencies {
                    if dir_of.get(&dep).copied() == Some(-1) && here - 1 < layer_of[&dep] {
                        layer_of.insert(dep, here - 1);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Assign per-layer order in discovery order.
    let mut layer_count: HashMap<i32, usize> = HashMap::new();
    let mut nodes: Vec<GraphNode> = Vec::with_capacity(discovery.len());
    for id in &discovery {
        let layer = layer_of[id];
        let order = *layer_count.get(&layer).unwrap_or(&0);
        layer_count.insert(layer, order + 1);
        nodes.push(GraphNode {
            id: *id,
            layer,
            order,
            known: known_of[id],
        });
    }

    // Edges (deduped). `dependencies` and `blocks` are inverse relations, and
    // iterating `discovery` (which includes unknown referenced ids) means edges
    // to unknown nodes are emitted too.
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    for id in &discovery {
        if let Some(task) = index.get(id) {
            for &dep in &task.dependencies {
                if dep != *id && layer_of.contains_key(&dep) && seen.insert((dep, *id)) {
                    edges.push(GraphEdge { from: dep, to: *id });
                }
            }
            for &blk in &task.blocks {
                if blk != *id && layer_of.contains_key(&blk) && seen.insert((*id, blk)) {
                    edges.push(GraphEdge { from: *id, to: blk });
                }
            }
        }
    }

    GraphLayout { nodes, edges }
}

/// Truncate a label to `max` characters, appending an ellipsis if cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

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
        layout
            .nodes
            .iter()
            .find(|n| n.id == id)
            .expect("node present")
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
        assert_ne!(node(&layout, 2).order, node(&layout, 3).order);
    }

    #[test]
    fn asymmetric_paths_use_longest_path_layer() {
        // 1 blocks 2 and 4 directly; 2 -> 3 -> 4 is a longer path to 4.
        let tasks = vec![
            t(1, vec![], vec![2, 4]),
            t(2, vec![1], vec![3]),
            t(3, vec![2], vec![4]),
            t(4, vec![1, 3], vec![]),
        ];
        let layout = build_graph_layout(&tasks, 1);
        assert_eq!(node(&layout, 1).layer, 0);
        assert_eq!(node(&layout, 2).layer, 1);
        assert_eq!(node(&layout, 3).layer, 2);
        // 4 is reachable at depth 1 (1->4) and depth 3 (1->2->3->4); longest-path
        // layering must place it at the deeper layer so 3->4 points downward.
        assert_eq!(node(&layout, 4).layer, 3);
        assert!(has_edge(&layout, 1, 4));
        assert!(has_edge(&layout, 3, 4));
    }

    #[test]
    fn cycle_terminates_and_places_each_node_once() {
        let tasks = vec![t(1, vec![2], vec![2]), t(2, vec![1], vec![1])];
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

    #[test]
    fn component_source_has_arrow_marker_and_header() {
        let source = include_str!("task_dep_graph.rs");
        assert!(source.contains("url(#depArrow)"));
        assert!(source.contains("Dependency Graph"));
    }
}
