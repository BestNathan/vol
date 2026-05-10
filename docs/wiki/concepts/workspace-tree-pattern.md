---
type: concept
category: pattern
tags: [tree-structure, lazy-loading, file-tree, workspace, dioxus]
created: 2026-05-10
updated: 2026-05-10
source_count: 1
---

# Workspace Tree Pattern

**Category:** UI data structure and rendering pattern

**Related:** [[vol-llm-ui-crate]], [[dioxus-signal-pattern]], [[dioxus-web-pattern]], [[lazy-load-dir-tree]]

## Definition

A recursive `WorkspaceTreeNode` tree structure replacing a flat `Vec<WorkspaceEntry>` for workspace representation. Each node knows its name, path, whether it's a directory, whether its children have been loaded, and its child nodes. Directories load children lazily on-demand.

## Structure

```rust
pub struct WorkspaceTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub loaded: bool,
    pub load_error: bool,
    pub children: Vec<WorkspaceTreeNode>,
}
```

## Key Methods

- `root(name, path)` — creates an empty directory node with `loaded: false`
- `find_child_mut(path)` — recursive tree traversal to find a node by its full path
- `replace_dir_children(dir_path, entries)` — replaces a directory's children with new entries from a fetch

## Lazy Loading Strategy

| State | Behavior |
|-------|----------|
| `loaded: false` + collapsed | No children fetched, shows empty |
| `loaded: false` + expand triggered | Fetches children via `file.list`, populates on callback |
| `loaded: true` + expand triggered | Re-fetches children (fresh data guarantee) |
| Refresh button clicked | Clears children, resets `loaded: false`, re-fetches |
| Fetch fails | Sets `load_error: true`, shows error indicator |

## Dioxus Reactivity

Each tree node is a `#[component] TreeNode` — not a plain function. This is critical because Dioxus reactive subscriptions only work inside components. When `file_list` callback calls `replace_dir_children` via `Signal::with_mut()`, the signal change propagates to all `TreeNode` components that read from it, triggering re-renders.

**Wrong:** Plain `fn render_node()` — signal reads create static `VNode` elements that don't re-render.
**Right:** `#[component] TreeNode(node, depth)` — each node is a reactive component instance.

## Borrow Checker Pattern

When mutating state and then making an async callback:

```rust
// WRONG — sig borrowed by with_mut while moved into callback
sig.with_mut(|s| {
    rpc.file_list(&path, move |result| { sig.clone() ... });
});

// RIGHT — return value from with_mut, then call outside
let was_collapsed = sig.with_mut(|s| { /* toggle */ });
if was_collapsed {
    rpc.file_list(&path_str, move |result| { /* sig.clone() ok */ });
}
```

## TUI Flattening

The TUI workspace panel needs a flat list for rendering. `flatten_tree_for_tui()` recursively traverses the tree, collecting visible nodes into a `Vec<(String, bool, String)>` (name, is_dir, path) while preserving sort order and indentation depth.

## Related Concepts

- [[dioxus-signal-pattern]]: Signal-based state management driving tree mutations
- [[dioxus-web-pattern]]: Component architecture built on top of this tree
- [[lazy-load-dir-tree]]: Source documenting the implementation
- [[vol-llm-ui-crate]]: Crate defining `WorkspaceTreeNode` and `UiState`
