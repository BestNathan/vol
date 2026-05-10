---
type: source
category: implementation
tags: [file-tree, lazy-loading, workspace, dioxus, json-rpc]
created: 2026-05-10
updated: 2026-05-10
---

# Lazy-Loading Directory Tree

**Summary:** Implementation of on-demand directory loading in the file tree. Clicking a directory expands it and fetches sub-files via JSON-RPC `file.list`; every expand fetches fresh data; a refresh button on each directory re-fetches without collapsing/expanding.

**Related:** [[workspace-tree-pattern]], [[dioxus-signal-pattern]], [[dioxus-web-pattern]], [[json-rpc-websocket]], [[vol-llm-ui-crate]]

## Overview

Replaced the flat `Vec<WorkspaceEntry>` workspace representation with a nested `WorkspaceTreeNode` tree structure. Directories load children lazily on first expand, and re-fetch on every subsequent expand to guarantee fresh data. Each directory node has a refresh button (⟳) for re-fetching without toggling collapse state.

## Key Changes

### Data Structure (`state/mod.rs`)

```rust
pub struct WorkspaceTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub loaded: bool,       // whether children have been fetched
    pub load_error: bool,   // whether last fetch failed
    pub children: Vec<WorkspaceTreeNode>,
}
```

Methods: `root()`, `find_child_mut(path)`, `replace_dir_children(dir_path, entries)`.

### Workspace Scanner (`state/workspace.rs`)

`scan_workspace(root)` recursively builds the tree from the filesystem, sorting directories before files and filtering ignored directories (`.git`, `node_modules`, `target`, etc.) and dotfiles.

### File Tree Component (`web/components/file_tree.rs`)

- `#[component] TreeNode(node, depth)` — reactive Dioxus component for each tree node
- Directory click: toggles `collapsed_dirs` in state; on expand transition, calls `rpc.file_list(dir_path)` and populates children via `replace_dir_children`
- Refresh button: clears children, resets `loaded = false`, re-fetches via `file_list`
- Error handling: sets `load_error = true` on fetch failure, shows error indicator
- File click: opens file tab (existing behavior)

### State Mutation Pattern

All mutations go through `Signal::with_mut()`. The `file_list` callback is called OUTSIDE `with_mut` to avoid Rust borrow checker issues with closure captures — `with_mut` returns a boolean (`was_collapsed`) that gates the network request.

### TUI Rendering (`tui/render.rs`)

`flatten_tree_for_tui()` helper traverses the tree into a flat list for the TUI workspace panel, preserving sort order.

## Architecture

```
FileTree (component)
  └── TreeNode (component) — recursive
        ├── Directory node
        │     ├── chevron + icon + label + refresh button
        │     ├── onclick → toggle collapsed_dirs + file_list on expand
        │     └── children rendered when not collapsed
        └── File node
              ├── icon + label
              └── onclick → open file tab
```

## Verification

- 42 tests passing across vol-llm-ui crate
- WASM compilation verified (pre-existing `mio` incompatibility with wasm32 target, not introduced by these changes)
- `dx serve` tested with live reload
