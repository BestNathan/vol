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
