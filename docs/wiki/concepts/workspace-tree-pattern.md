---
type: concept
category: pattern
tags: [tree-structure, lazy-loading, file-tree, workspace, dioxus]
created: 2026-05-10
updated: 2026-05-18 (file-tree-chevron-glyph-refinement)
source_count: 6
---

# Workspace Tree Pattern

**Category:** UI data structure and rendering pattern

**Related:** [[vol-llm-ui-crate]], [[dioxus-signal-pattern]], [[dioxus-web-pattern]], [[drawer-ui-pattern]], [[lazy-load-dir-tree]], [[file-tree-sidebar-scroll-fix]], [[mobile-file-tree-rail]], [[file-tree-single-click-expand-fix]], [[file-tree-collapsed-state-follow-up]], [[file-tree-chevron-glyph-refinement]]

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
- `replace_dir_children(dir_path, entries)` — replaces a directory's children with new entries from a fetch; child directories are inserted with `loaded: false` so their first click fetches their own children [[file-tree-single-click-expand-fix]]

## Lazy Loading Strategy

Discovered directory children must remain `loaded: false` until that specific directory's own `file.list` request succeeds. The web `TreeNode` also treats unloaded empty directories as visually collapsed even when they are not in `collapsed_dirs`; otherwise the UI can show a child directory as already expanded before it has fetched children [[file-tree-single-click-expand-fix]], [[file-tree-collapsed-state-follow-up]].

| State | Behavior |
|-------|----------|
| `loaded: false` + not in `collapsed_dirs` | Renders collapsed by default; first click fetches children |
| `loaded: false` + expand triggered | Fetches children via `file.list`, populates on callback, removes explicit collapsed state |
| `loaded: true` + expand triggered | Re-fetches children (fresh data guarantee) |
| Refresh button clicked | Clears children, resets `loaded: false`, re-fetches |
| Fetch fails | Sets `load_error: true`, shows error indicator |

## Web Collapse Semantics

The web FileTree separates data loading from visual collapse state. `WorkspaceTreeNode.loaded` records whether a directory's children have been fetched, while `WorkspaceState.collapsed_dirs` records explicit user collapse choices. `directory_is_collapsed()` combines both signals: an unloaded empty directory is visually collapsed by default, and `toggle_directory_for_click()` makes first click on an unloaded directory trigger `file.list` without adding it to `collapsed_dirs` [[file-tree-collapsed-state-follow-up]]. Directory rows render a CSS-drawn chevron that points right when collapsed and rotates downward when expanded [[file-tree-chevron-glyph-refinement]].

## Dioxus Reactivity

Each tree node is a `#[component] TreeNode` — not a plain function. This is critical because Dioxus reactive subscriptions only work inside components. When `file_list` callback calls `replace_dir_children` via `Signal::with_mut()`, the signal change propagates to all `TreeNode` components that read from it, triggering re-renders.

**Wrong:** Plain `fn render_node()` — signal reads create static `VNode` elements that don't re-render.
**Right:** `#[component] TreeNode(node, depth)` — each node is a reactive component instance.

## Desktop Scroll Containment

The web file tree sidebar must be a bounded flex column on desktop: `sm:flex sm:flex-col sm:h-full sm:min-h-0`. The scrollable tree body uses `min-h-0 flex-1 overflow-y-auto`; this lets long directory trees scroll inside the sidebar instead of expanding the page layout. A regression test covers the `DESKTOP_SIDEBAR_CLASSES` contract after [[file-tree-sidebar-scroll-fix]].

## Mobile Rail Containment

On mobile, the closed file tree is an inline rail rather than a hidden drawer launcher. `file_tree_outer_class(false)` keeps a visible `w-10 flex-shrink-0` container in the main flex layout, while `file_tree_panel_content_class(false)` hides the full tree body until the drawer is opened or the viewport reaches `sm:`. This keeps the tab/content area from spanning behind a floating button and makes the file tree affordance part of the left-side layout. See [[mobile-file-tree-rail]] and [[drawer-ui-pattern]].

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
- [[file-tree-sidebar-scroll-fix]]: Desktop scroll containment and compact tree controls
- [[mobile-file-tree-rail]]: Mobile closed-state rail and drawer trigger ownership
- [[file-tree-single-click-expand-fix]]: Directory child loading invariant that fixes double-click expansion
- [[file-tree-collapsed-state-follow-up]]: Web FileTree visual collapse and first-click transition semantics for unloaded directories
- [[file-tree-chevron-glyph-refinement]]: CSS-drawn directory chevron affordance for collapsed/expanded state
