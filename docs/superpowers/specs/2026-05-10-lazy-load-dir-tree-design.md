# Lazy-Loading Directory Tree Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat `workspace.entries` with a nested `WorkspaceTreeNode` tree, and fetch directory contents on-demand when a directory is expanded. Refresh re-fetches the directory.

**Architecture:** `UiState.workspace` becomes a tree structure. Expanding a directory triggers `file.list(dir_path)` RPC call, which replaces that node's `children`. Refreshing clears children and re-fetches.

**Tech Stack:** Dioxus `Signal<UiState>`, JSON-RPC `file.list` method.

---

## Design

### 1. Data Structure Change

Replace `WorkspaceTree` and `WorkspaceEntry` in `state/mod.rs`:

```rust
/// A node in the workspace directory tree.
#[derive(Debug, Clone)]
pub struct WorkspaceTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// For directories: whether children have been loaded at least once.
    /// When false, children is empty. When true, children may be stale
    /// (will be replaced on next expand/refresh).
    pub loaded: bool,
    pub children: Vec<WorkspaceTreeNode>,
}
```

`UiState.workspace` changes from `WorkspaceTree { root, entries: Vec<WorkspaceEntry> }` to a single `WorkspaceTreeNode` representing the workspace root.

### 2. File Tree Rendering

Delete `build_tree_at` in `file_tree.rs` — no longer needed since we already have a tree. The `render_nodes` / `render_node` functions become simpler, reading directly from `WorkspaceTreeNode.children`.

The directory collapse state (`collapsed_dirs: HashSet<String>`) stays the same. The difference is:
- **Current**: `collapsed_dirs` controls visibility of children that already exist in flat entries
- **New**: `collapsed_dirs` controls visibility of `children` on the tree node, and `loaded` controls whether we need to fetch

### 3. Expand Directory (OnClick)

When a user clicks a directory to expand it (collapse → expand transition):

1. If the directory was **collapsed**, clicking makes it **expanded**:
   a. If `loaded` is `false`, call `rpc.file_list(&dir_path, cb)` to fetch children.
   b. If `loaded` is `true`, still call `rpc.file_list(&dir_path, cb)` — **every expand fetches fresh data** per user requirement.
   c. In the callback: replace the node's `children` with the new entries, set `loaded = true`.

2. If the directory was **expanded**, clicking makes it **collapsed**: toggle `collapsed_dirs` only, no network request.

The `loaded` flag exists purely to show a "loading..." placeholder while the first fetch is in-flight. After the first load, `loaded` is always `true` and children are replaced on every expand.

### 4. Refresh

Each directory node gets a refresh button (e.g., small ⟳ icon on hover). Clicking it:
1. Clear the node's `children` to empty.
2. Set `loaded = false`.
3. Call `file.list(&dir_path, cb)` to repopulate.
4. Set `loaded = true` when done.

### 5. State Mutation Pattern

All mutations happen via `Signal::with_mut()`. The callback receives `&mut UiState`, finds the node by path, and modifies its `children` and `loaded` fields.

To find a node by path in the tree, use a recursive helper:

```rust
fn find_node_mut(root: &mut WorkspaceTreeNode, path: &str) -> Option<&mut WorkspaceTreeNode> {
    if root.path == path { return Some(root); }
    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, path) { return Some(found); }
    }
    None
}
```

### 6. File Impact

| File | Change |
|------|--------|
| `src/state/mod.rs` | Replace `WorkspaceTree` + `WorkspaceEntry` with `WorkspaceTreeNode`. Update `UiState.workspace` field. |
| `src/state/workspace.rs` | Rewrite `scan_workspace` to return a `WorkspaceTreeNode` tree instead of flat entries. |
| `src/web/components/file_tree.rs` | Delete `build_tree_at`. Rewrite `render_node` to use `WorkspaceTreeNode` directly. Add refresh button. |
| `src/web/components/app.rs` | Update initial workspace loading to build tree from `file.list` root response. |
| `src/web/client.rs` | No changes (already has `file_list`). |
| `src/web/components/workspace.rs` | Update to use new `WorkspaceTreeNode` type (if it references `WorkspaceEntry`). |
| `src/tui/render.rs` | Update workspace panel rendering to traverse `WorkspaceTreeNode` instead of iterating flat entries. |

### 7. Error Handling

- If `file.list` fails (e.g., permission denied), set `loaded = true` but leave `children` empty. Show a small error indicator on the directory node.
- Network failures should not crash the UI — show error in the directory label or a tooltip.

### 8. Backward Compatibility

- `UiState` struct field names change (`workspace` from struct with `entries` to `WorkspaceTreeNode`).
- TUI code that references `workspace.entries` must be updated to traverse the tree.
- `collapsed_dirs: HashSet<String>` remains unchanged.
