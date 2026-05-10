# File Tree + Tools Tab Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current left-panel ToolsPanel with an interactive file tree (foldable directories, icon-per-file-type), move tools to a new Tools tab, and add file-content preview tabs when clicking files.

**Architecture:** Split the left sidebar into a single `FileTree` component with its own collapse state. Add `ToolsTabContent` to the tab panel. Add `FileContentView` for file reading + display. All state lives in `UiState` with a `Signal<u64>` version counter for re-render triggers.

**Tech Stack:** Dioxus 0.6 (WASM), `web_time`, JSON-RPC WebSocket client.

---

## 1. State Model Changes

### 1.1 New types in `UiState`

```rust
/// A file opened in a tab.
pub struct OpenFileTab {
    pub path: String,
    pub content: Option<String>,  // None = loading
    pub error: Option<String>,
}

pub struct UiState {
    // ... existing fields unchanged ...
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,  // index into open_files
    pub collapsed_dirs: HashSet<String>,    // paths of folded directories
    pub expanded_tool_calls: HashSet<usize>, // indices of expanded tool call items
}
```

### 1.2 ActiveTab enum

```rust
pub enum ActiveTab { Conversation, Tools, Workspace, Skills, Logs }
```

`Tools` is new. `Workspace` remains in the enum for backward compat but is **no longer rendered as a tab button** — the workspace content moves to the left sidebar file tree.

## 2. Component Inventory

### 2.1 New files

| File | Components | Responsibility |
|------|-----------|----------------|
| `src/web/components/file_tree.rs` | `FileTree`, `FileTreeNode` | Left sidebar file tree with fold/unfold |
| `src/web/components/tools_tab.rs` | `ToolsTabContent`, `ToolCallItem` | Tools tab with expandable call details |
| `src/web/components/file_content.rs` | `FileContentView` | `<pre>` file content reader |

### 2.2 Modified files

| File | Changes |
|------|---------|
| `src/web/components/app.rs` | Replace `ToolsPanel` with `FileTree` in layout. Add Tools tab button + routing in `TabContent`. Update `TabBar` to remove Workspace button, add Tools button. |
| `src/web/components/workspace.rs` | Delete — content moved to `file_tree.rs`. |
| `src/web/client.rs` | Add `file_read(path, cb)` method for reading file content via JSON-RPC. |
| `src/state/mod.rs` | Add `OpenFileTab`, `open_files`, `selected_file_tab`, `collapsed_dirs` fields. |
| `src/state/workspace.rs` | Unchanged — the file tree builds a tree from flat `workspace.entries` at render time, no structural change needed. |

## 3. File Tree Design

### 3.1 Data model

The server returns flat paths via `file.list`. We build a tree at render time:

```rust
/// A node in the workspace tree.
pub enum FileTreeNode {
    Dir {
        name: String,
        path: String,
        children: Vec<FileTreeNode>,
    },
    File {
        name: String,
        path: String,
    },
}
```

The tree is computed from the flat `workspace.entries` list by grouping paths by their parent directory segments.

### 3.2 Icon mapping

Based on file extension (matched case-insensitively on the filename):

| Extension(s) | Emoji | Label |
|---|---|---|
| directory | 📂 | Folder |
| `.rs` | 🦀 | Rust |
| `.toml`, `.lock` | ⚙️ | Config |
| `.md` | 📝 | Markdown |
| `.json` | 📊 | JSON |
| `.yaml`, `.yml` | 📜 | YAML |
| `.sh`, `.bash` | 🐚 | Shell |
| `.html`, `.htm` | 🌐 | HTML |
| `.css` | 🎨 | CSS |
| `.js`, `.ts`, `.jsx`, `.tsx` | 📜 | JS/TS |
| `.txt` | 📄 | Text |
| default | 📄 | Generic |

### 3.3 Collapse behavior

`collapsed_dirs: HashSet<String>` stores paths of folded directories.

When a directory node is clicked:
- If its path is in `collapsed_dirs` → remove it (expand)
- Otherwise → add it (collapse)

The `FileTreeNode` children are still computed for the full tree, but during rendering, collapsed directories skip rendering their children `div`s entirely.

### 3.4 Click behavior

- **Directory**: toggle collapse
- **File**: open in tab (see §5)

## 4. Tools Tab

Displays the same `tool_calls` list that currently lives in the old `ToolsPanel`, but with expandable detail:

```
┌─────────────────────────────────────────────────┐
│ 1. [Read]          OK    12ms        ▼          │
│    Input:  path: "Cargo.toml"                    │
│    Output: [package] ... (truncated preview)     │
├─────────────────────────────────────────────────┤
│ 2. [Bash]          ERR   45ms        ▼          │
└─────────────────────────────────────────────────┘
```

State tracking is in `UiState.expanded_tool_calls` (§1.1).

## 5. File Open Flow

1. User clicks a file in the left file tree
2. Check if the file is already in `open_files` → if yes, select that tab, switch `active_tab` to the file
3. If not, add a new `OpenFileTab { path, content: None, error: None }` to `open_files`
4. Call `client.file_read(path, cb)` via JSON-RPC
5. On response: set `content` or `error` on the tab, bump version
6. Switch to the new file tab

File tabs show a close button (✕) that removes the entry from `open_files`.

## 6. Tab Bar

New tab order:
```
[💬 Conversation] [🔧 Tools] [🎯 Skills] [📋 Logs] [🦀 app.rs ✕] [📝 README.md ✕]
```

File tabs are appended after the fixed tabs. Only file tabs have a close button.

## 7. Error Handling

- **file.read fails**: show error text in the file tab content area in red
- **workspace.entries empty**: show "No files found" in the file tree
- **File too large**: server-side limit at 500KB, client shows truncated warning

## 8. CSS Changes

New classes added to `GLOBAL_CSS` in `app.rs`:

- `.sidebar` / `.sidebar-header` — left panel container
- `.file-tree` / `.file-tree-node` — tree rows
- `.file-tree-dir` / `.file-tree-file` — icon + label styling
- `.file-tree-chevron` — collapse arrow (rotate on collapse)
- `.file-tree-children` — collapsible child container
- `.tool-call-item` / `.tool-call-detail` — expandable tool calls
- `.file-content` / `.file-tab-header` — file viewer

Existing `.tools-panel` class removed (replaced by `.sidebar`).

## 9. Backward Compatibility

- `ActiveTab::Workspace` enum variant kept but not rendered as a tab
- TUI code unchanged (only web components modified)
- `workspace.entries` field kept for existing code, but the file tree builds a tree on top of it
