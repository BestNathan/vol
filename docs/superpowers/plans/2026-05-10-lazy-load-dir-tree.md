# Lazy-Loading Directory Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace flat `workspace.entries` with nested `WorkspaceTreeNode`, and fetch directory contents on-demand when expanded.

**Architecture:** `UiState.workspace` becomes a `WorkspaceTreeNode` tree. Expanding a directory triggers `file.list` RPC to fetch children. Every expand fetches fresh data; refresh clears children and re-fetches.

**Tech Stack:** Dioxus 0.6 `Signal<UiState>`, JSON-RPC `file.list` method, `serde::{Serialize, Deserialize}`.

---

### Task 1: Replace `WorkspaceTree` / `WorkspaceEntry` with `WorkspaceTreeNode`

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`
- Modify: `crates/vol-llm-ui/src/state/workspace.rs`

- [ ] **Step 1: Define `WorkspaceTreeNode` with helpers**

Replace lines 86-98 in `state/mod.rs` (the `WorkspaceTree` and `WorkspaceEntry` structs) with:

```rust
/// A node in the workspace directory tree.
#[derive(Debug, Clone)]
pub struct WorkspaceTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// For directories: whether children have been loaded at least once.
    pub loaded: bool,
    /// For directories: whether the last load attempt failed.
    pub load_error: bool,
    pub children: Vec<WorkspaceTreeNode>,
}

impl WorkspaceTreeNode {
    pub fn root(name: String, path: String) -> Self {
        Self {
            name,
            path,
            is_dir: true,
            loaded: false,
            load_error: false,
            children: Vec::new(),
        }
    }

    /// Find a mutable reference to a descendant node by path.
    pub fn find_child_mut(&mut self, path: &str) -> Option<&mut Self> {
        if self.path == path { return Some(self); }
        for child in &mut self.children {
            if let Some(found) = child.find_child_mut(path) { return Some(found); }
        }
        None
    }

    /// Replace a directory node's children with fresh entries.
    /// `entries` is a list of (name, is_dir) tuples.
    pub fn replace_dir_children(&mut self, dir_path: &str, entries: Vec<(String, bool)>) {
        if let Some(node) = self.find_child_mut(dir_path) {
            node.children.clear();
            for (name, is_dir) in entries {
                let child_path = if dir_path == "." || dir_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", dir_path, name)
                };
                node.children.push(WorkspaceTreeNode {
                    name,
                    path: child_path,
                    is_dir,
                    loaded: true,
                    load_error: false,
                    children: Vec::new(),
                });
            }
            node.loaded = true;
            node.load_error = false;
        }
    }
}
```

- [ ] **Step 2: Update `UiState.workspace` field**

In `state/mod.rs` line 198, change:
```rust
pub workspace: WorkspaceTree,
```
to:
```rust
pub workspace: WorkspaceTreeNode,
```

In `state/mod.rs` lines 238-244, change:
```rust
            workspace: WorkspaceTree {
                root: working_dir.to_string(),
                entries: Vec::new(),
            },
```
to:
```rust
            workspace: WorkspaceTreeNode::root(working_dir.to_string(), ".".into()),
```

- [ ] **Step 3: Write tests for `WorkspaceTreeNode`**

Add to the `tests` module in `state/mod.rs` (before the closing `}` at line 625):

```rust
    #[test]
    fn test_workspace_tree_node_structure() {
        let mut root = WorkspaceTreeNode::root("root".into(), ".".into());
        root.children.push(WorkspaceTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            loaded: true,
            load_error: false,
            children: vec![
                WorkspaceTreeNode {
                    name: "main.rs".into(),
                    path: "src/main.rs".into(),
                    is_dir: false,
                    loaded: false,
                    load_error: false,
                    children: vec![],
                },
            ],
        });
        assert_eq!(root.children.len(), 1);
        assert!(root.children[0].is_dir);
        assert_eq!(root.children[0].children[0].name, "main.rs");
    }

    #[test]
    fn test_find_child_mut() {
        let mut root = WorkspaceTreeNode::root("root".into(), ".".into());
        root.children.push(WorkspaceTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            loaded: false,
            load_error: false,
            children: vec![],
        });
        let found = root.find_child_mut("src");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "src");
        let not_found = root.find_child_mut("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_replace_dir_children() {
        let mut root = WorkspaceTreeNode::root("root".into(), ".".into());
        root.children.push(WorkspaceTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            loaded: false,
            load_error: false,
            children: vec![],
        });
        root.replace_dir_children("src", vec![
            ("main.rs".into(), false),
            ("lib.rs".into(), false),
            ("utils".into(), true),
        ]);
        let src = root.find_child_mut("src").unwrap();
        assert_eq!(src.children.len(), 3);
        assert!(src.loaded);
        assert_eq!(src.children[0].name, "main.rs");
        assert_eq!(src.children[0].path, "src/main.rs");
        assert_eq!(src.children[2].path, "src/utils");
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-ui -- tree 2>&1`
Expected: 3 PASS

- [ ] **Step 5: Rewrite `scan_workspace` to return `WorkspaceTreeNode`**

Replace `crates/vol-llm-ui/src/state/workspace.rs` entirely:

```rust
use crate::state::WorkspaceTreeNode;
use std::fs;
use std::path::Path;

/// Scan a directory and build a WorkspaceTreeNode tree.
///
/// Ignores hidden files/directories (starting with '.') and common
/// non-source directories (.git, node_modules, target, .cargo).
pub fn scan_workspace(root: &str) -> WorkspaceTreeNode {
    let path = Path::new(root);
    let name = path.file_name()
        .unwrap_or(std::ffi::OsStr::new(root))
        .to_string_lossy()
        .to_string();

    if !path.is_dir() {
        return WorkspaceTreeNode::root(name, root.to_string());
    }

    scan_dir(path, path, &name)
}

fn scan_dir(base: &Path, dir: &Path, dir_name: &str) -> WorkspaceTreeNode {
    let ignored = [".git", "node_modules", "target", ".cargo", "__pycache__", ".venv"];
    let rel = dir.strip_prefix(base).unwrap_or(dir);
    let path_str = rel.to_string_lossy().to_string();

    let mut children = Vec::new();

    let Ok(read_dir) = fs::read_dir(dir) else {
        return WorkspaceTreeNode {
            name: dir_name.to_string(),
            path: path_str,
            is_dir: true,
            loaded: true,
            load_error: false,
            children,
        };
    };

    let mut entries: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            !name_str.starts_with('.')
        })
        .collect();

    entries.sort_by_key(|e| {
        let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let name = e.file_name();
        (!is_dir, name)
    });

    for entry in entries {
        let Ok(file_type) = entry.file_type() else { continue };
        let name = entry.file_name().to_string_lossy().to_string();

        if file_type.is_dir() {
            if ignored.contains(&name.as_str()) {
                continue;
            }
            let child = scan_dir(base, &entry.path(), &name);
            children.push(child);
        } else {
            let child_rel = entry.path().strip_prefix(base).unwrap_or(&entry.path());
            children.push(WorkspaceTreeNode {
                name,
                path: child_rel.to_string_lossy().to_string(),
                is_dir: false,
                loaded: false,
                load_error: false,
                children: vec![],
            });
        }
    }

    WorkspaceTreeNode {
        name: dir_name.to_string(),
        path: path_str,
        is_dir: true,
        loaded: true,
        load_error: false,
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_nonexistent_directory() {
        let tree = scan_workspace("/tmp/vol-llm-ui-test-nonexistent-12345");
        assert!(tree.is_dir);
        assert!(tree.children.is_empty());
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = std::env::temp_dir().join("vol-llm-ui-scan-test");
        let _ = fs::create_dir_all(&dir);
        let tree = scan_workspace(dir.to_str().unwrap());
        assert!(tree.children.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_with_files_and_dirs() {
        let dir = std::env::temp_dir().join("vol-llm-ui-scan-test-2");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let _ = fs::create_dir(dir.join("src"));
        let _ = fs::write(dir.join("README.md"), "hello");
        let _ = fs::write(dir.join("src").join("main.rs"), "fn main() {}");
        let _ = fs::write(dir.join(".hidden"), "secret");
        let _ = fs::create_dir(dir.join(".git"));
        let _ = fs::create_dir(dir.join("target"));

        let tree = scan_workspace(dir.to_str().unwrap());

        let names: Vec<_> = tree.children.iter().map(|e| e.name.as_str()).collect();
        assert!(names.iter().any(|n| *n == "README.md"));
        assert!(names.iter().any(|n| *n == "src"));

        let src = tree.children.iter().find(|c| c.name == "src").unwrap();
        assert!(src.children.iter().any(|c| c.name == "main.rs"));

        for child in &tree.children {
            assert!(!child.path.contains(".git"));
            assert!(!child.path.contains("target"));
            assert!(!child.path.contains(".hidden"));
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 6: Run all state tests**

Run: `cargo test -p vol-llm-ui state 2>&1`
Expected: All tests pass (7 total: 3 from mod.rs + 3 from workspace.rs + existing UiState tests)

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/src/state/workspace.rs
git commit -m "feat(state): replace WorkspaceTree/WorkspaceEntry with WorkspaceTreeNode tree"
```

---

### Task 2: Update web file tree to use `WorkspaceTreeNode`

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/file_tree.rs` (complete rewrite)

- [ ] **Step 1: Rewrite `file_tree.rs` with `WorkspaceTreeNode` rendering**

Replace the entire file:

```rust
//! Left sidebar file tree with collapsible directories.

use dioxus::prelude::*;

use crate::state::{ActiveTab, OpenFileTab, WorkspaceTreeNode};
use crate::web::components::app::AppState;

/// Get the icon for a file extension or directory.
pub(crate) fn file_icon(is_dir: bool, name: &str) -> &'static str {
    if is_dir {
        return "\u{1f4c2}";
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "\u{1f980}",
        "toml" | "lock" => "\u{2699}\u{fe0f}",
        "md" => "\u{1f4dd}",
        "json" => "\u{1f4ca}",
        "yaml" | "yml" => "\u{1f4dc}",
        "sh" | "bash" => "\u{1f41a}",
        "html" | "htm" => "\u{1f310}",
        "css" => "\u{1f3a8}",
        "js" | "ts" | "jsx" | "tsx" => "\u{1f4dc}",
        "txt" => "\u{1f4c4}",
        _ => "\u{1f4c4}",
    }
}

/// Render tree nodes recursively.
fn render_nodes(nodes: &[WorkspaceTreeNode], state: AppState, depth: usize) -> Vec<Element> {
    nodes
        .iter()
        .map(|node| render_node(node, state.clone(), depth))
        .collect()
}

fn render_node(node: &WorkspaceTreeNode, state: AppState, depth: usize) -> Element {
    if node.is_dir {
        let collapsed = state.signal.read().collapsed_dirs.contains(&node.path);

        let child_elements = if !collapsed {
            render_nodes(&node.children, state.clone(), depth + 1)
        } else {
            Vec::new()
        };

        let indent_px = depth * 16;
        let chevron_cls = if collapsed {
            "file-tree-chevron collapsed"
        } else {
            "file-tree-chevron"
        };

        let mut dir_sig = state.signal;
        let dir_path = node.path.clone();
        let rpc = state.rpc_client.clone();
        let dir_onclick = move |_: Event<MouseData>| {
            let p = dir_path.clone();
            let rpc_clone = rpc.clone();
            let mut sig = dir_sig.clone();

            sig.with_mut(|s| {
                if s.collapsed_dirs.contains(&p) {
                    s.collapsed_dirs.remove(&p);
                } else {
                    s.collapsed_dirs.insert(p.clone());
                    // Every expand fetches fresh data
                    let p2 = p.clone();
                    rpc_clone.file_list(&p2, move |result| {
                        let mut sig2 = sig.clone();
                        match result {
                            Ok(entries) => {
                                let flat_entries: Vec<(String, bool)> = entries
                                    .into_iter()
                                    .map(|e| (e.name, e.is_dir))
                                    .collect();
                                sig2.with_mut(|s2| {
                                    s2.workspace.replace_dir_children(&p2, flat_entries);
                                });
                            }
                            Err(_) => {
                                sig2.with_mut(|s2| {
                                    if let Some(node) = s2.workspace.find_child_mut(&p2) {
                                        node.children.clear();
                                        node.loaded = true;
                                        node.load_error = true;
                                    }
                                });
                            }
                        }
                    });
                }
            });
        };

        rsx! {
            div {
                div {
                    class: "file-tree-node file-tree-dir",
                    style: format!("padding-left: {}px;", indent_px),
                    onclick: dir_onclick,
                    span { class: "{chevron_cls}", "\u{25be}" }
                    span { class: "file-tree-icon", "{file_icon(true, &node.name)}" }
                    span { class: "file-tree-label dir", "{node.name}" }
                }
                if !collapsed {
                    div { class: "file-tree-children",
                        {child_elements.into_iter()}
                    }
                }
            }
        }
    } else {
        let indent_px = depth * 16;

        let mut sig = state.signal.clone();
        let rpc = state.rpc_client.clone();
        let mut tab = state.active_tab;
        let file_path = node.path.clone();
        let file_onclick = move |_: Event<MouseData>| {
            let p = file_path.clone();
            let rpc_clone = rpc.clone();
            let sig_clone = sig.clone();

            sig.with_mut(|s| {
                let existing = s.open_files.iter().position(|f| f.path == p.clone());
                match existing {
                    Some(idx) => {
                        s.selected_file_tab = Some(idx);
                    }
                    None => {
                        let new_idx = s.open_files.len();
                        s.open_files.push(OpenFileTab {
                            path: p.clone(),
                            content: None,
                            error: None,
                        });
                        s.selected_file_tab = Some(new_idx);

                        let mut sig2 = sig_clone.clone();
                        let read_path = p.clone();
                        rpc_clone.file_read(&p, move |result| {
                            sig2.with_mut(|st| {
                                if let Some(idx) = st.open_files.iter().position(|f| f.path == read_path) {
                                    match result {
                                        Ok(c) => { st.open_files[idx].content = Some(c); }
                                        Err(e) => { st.open_files[idx].error = Some(e); }
                                    }
                                }
                            });
                        });
                    }
                }
            });
            tab.set(ActiveTab::Workspace);
        };

        rsx! {
            div {
                class: "file-tree-node file-tree-file",
                style: format!("padding-left: {}px;", indent_px),
                onclick: file_onclick,
                span { class: "file-tree-chevron hidden", "\u{25be}" }
                span { class: "file-tree-icon", "{file_icon(false, &node.name)}" }
                span { class: "file-tree-label file", "{node.name}" }
            }
        }
    }
}

/// File tree component.
#[component]
pub fn FileTree() -> Element {
    let state: AppState = use_context();
    let workspace = state.signal.read().workspace.clone();

    if workspace.children.is_empty() && !workspace.loaded {
        return rsx! {
            div { class: "sidebar",
                div { class: "sidebar-header", "Explorer" }
                div { class: "file-tree",
                    div { class: "file-tree-empty", "No files loaded" }
                }
            }
        };
    }

    let elements = render_nodes(&workspace.children, state, 0);

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header", "Explorer" }
            div { class: "file-tree",
                {elements.into_iter()}
            }
        }
    }
}
```

- [ ] **Step 2: Check compilation**

Run: `cargo check -p vol-llm-ui 2>&1`
Expected: Compiles without errors (may have warnings)

- [ ] **Step 3: Run all tests**

Run: `cargo test -p vol-llm-ui 2>&1`
Expected: All tests pass (some existing tests may need adjustment if they reference `workspace.entries`)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/file_tree.rs
git commit -m "feat(web): rewrite file tree to use WorkspaceTreeNode with lazy-loading"
```

---

### Task 3: Update `app.rs` workspace initialization

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs:136-154`

- [ ] **Step 1: Update the `file_list` callback to build a tree**

In `app.rs`, replace lines 136-154 (the `file_list` callback inside `on_state_change`):

```rust
            let mut sig_cb = sig.clone();
            client_ws.file_list(".", move |result| {
                if let Ok(entries) = result {
                    sig_cb.with_mut(|state| {
                        // Build tree from root-level entries
                        state.workspace.children.clear();
                        for entry in &entries {
                            state.workspace.children.push(crate::state::WorkspaceTreeNode {
                                name: entry.name.clone(),
                                path: entry.name.clone(),
                                is_dir: entry.is_dir,
                                loaded: true,
                                load_error: false,
                                children: Vec::new(),
                            });
                        }
                        state.workspace.loaded = true;
                    });
                }
            });
```

- [ ] **Step 2: Check compilation**

Run: `cargo check -p vol-llm-ui 2>&1`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(web): update workspace init to build tree from file_list response"
```

---

### Task 4: Update `workspace.rs` web component

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/workspace.rs`

- [ ] **Step 1: Rewrite `workspace.rs` to traverse the tree**

Replace the entire file:

```rust
//! Workspace panel showing the file tree (legacy flat view).

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Flatten a WorkspaceTreeNode tree into (name, is_dir, indent) tuples.
fn flatten_tree(node: &crate::state::WorkspaceTreeNode, indent: usize) -> Vec<(String, bool, usize)> {
    let mut result = Vec::new();
    for child in &node.children {
        result.push((child.name.clone(), child.is_dir, indent));
        if child.is_dir {
            result.extend(flatten_tree(child, indent + 1));
        }
    }
    result
}

/// Workspace panel showing the file tree.
#[component]
pub fn WorkspacePanel() -> Element {
    let state: AppState = use_context();
    let (entries, loaded) = {
        let ui = state.signal.read();
        (flatten_tree(&ui.workspace, 0), ui.workspace.loaded)
    };

    if entries.is_empty() && !loaded {
        return rsx! {
            div { class: "workspace-panel",
                div { class: "workspace-empty", "Workspace directory empty or unavailable" }
            }
        };
    }

    let items = entries.iter().enumerate().map(|(index, (name, is_dir, indent))| {
        let n = name.clone();
        let d = *is_dir;
        let i = *indent;
        rsx! {
            WorkspaceItem { name: n, is_dir: d, indent: i, key: "{index}" }
        }
    }).collect::<Vec<_>>();

    rsx! {
        div { class: "workspace-panel",
            {items.into_iter()}
        }
    }
}

#[component]
fn WorkspaceItem(name: String, is_dir: bool, indent: usize) -> Element {
    if is_dir {
        let display = format!("{}[DIR] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "workspace-entry workspace-dir", "{display}" }
        }
    } else {
        let display = format!("{}[FILE] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "workspace-entry workspace-file", "{display}" }
        }
    }
}
```

- [ ] **Step 2: Check compilation**

Run: `cargo check -p vol-llm-ui 2>&1`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/workspace.rs
git commit -m "feat(web): update workspace panel to traverse WorkspaceTreeNode"
```

---

### Task 5: Update TUI workspace rendering

**Files:**
- Modify: `crates/vol-llm-ui/src/tui/render.rs:390-425`

- [ ] **Step 1: Replace `render_workspace` to traverse the tree**

Replace lines 390-425 in `render.rs`:

```rust
// === Workspace Panel ========================================================

fn flatten_tree_for_tui(node: &crate::state::WorkspaceTreeNode, indent: usize) -> Vec<(String, bool, usize)> {
    let mut result = Vec::new();
    for child in &node.children {
        result.push((child.name.clone(), child.is_dir, indent));
        if child.is_dir {
            result.extend(flatten_tree_for_tui(child, indent + 1));
        }
    }
    result
}

fn render_workspace(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default().borders(Borders::ALL).title(" Workspace ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.workspace.children.is_empty() && !state.workspace.loaded {
        let empty = Paragraph::new("Workspace directory empty or unavailable")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let entries = flatten_tree_for_tui(&state.workspace, 0);
    let lines: Vec<Line> = entries.iter().map(|(name, is_dir, indent)| {
        let prefix = if *is_dir {
            format!("{}[DIR] {}", "  ".repeat(*indent), name)
        } else {
            format!("{}[FILE] {}", "  ".repeat(*indent), name)
        };
        let style = if *is_dir {
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        Line::from(vec![Span::styled(prefix, style)])
    }).collect();

    let paragraph = Paragraph::new(Text::from(lines)).scroll((state.workspace_scroll, 0));
    frame.render_widget(paragraph, inner);
}
```

- [ ] **Step 2: Check compilation**

Run: `cargo check -p vol-llm-ui 2>&1`
Expected: Compiles without errors

- [ ] **Step 3: Run all tests**

Run: `cargo test -p vol-llm-ui 2>&1`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/tui/render.rs
git commit -m "feat(tui): update workspace rendering to traverse WorkspaceTreeNode"
```

---

### Task 6: Add refresh button to directory nodes

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/file_tree.rs`
- Modify: `crates/vol-llm-ui/src/web/components/app.rs` (CSS)

- [ ] **Step 1: Add refresh CSS styles**

In `app.rs`, add to `GLOBAL_CSS` after line 320 (after `.file-tree-dir:hover`):

```css
.file-tree-refresh { font-size: 10px; color: #666; margin-left: 4px; opacity: 0; transition: opacity 0.15s; cursor: pointer; }
.file-tree-node:hover .file-tree-refresh { opacity: 1; }
.file-tree-refresh:hover { color: #aaa; }
```

- [ ] **Step 2: Add refresh button to directory RSX**

In `file_tree.rs`, inside the directory `rsx!` block, add a refresh span after the label:

Change the directory RSX from:
```rust
rsx! {
    div {
        div {
            class: "file-tree-node file-tree-dir",
            style: format!("padding-left: {}px;", indent_px),
            onclick: dir_onclick,
            span { class: "{chevron_cls}", "\u{25be}" }
            span { class: "file-tree-icon", "{file_icon(true, &node.name)}" }
            span { class: "file-tree-label dir", "{node.name}" }
        }
        ...
    }
}
```

to:
```rust
let mut refresh_sig = state.signal;
let refresh_path = node.path.clone();
let refresh_rpc = state.rpc_client.clone();
let refresh_onclick = move |e: Event<MouseData>| {
    e.stop_propagation();
    let p = refresh_path.clone();
    let rpc_clone = refresh_rpc.clone();
    let mut sig = refresh_sig.clone();
    sig.with_mut(|s| {
        // Clear children and re-fetch
        if let Some(node) = s.workspace.find_child_mut(&p) {
            node.children.clear();
            node.loaded = false;
        }
        let p2 = p.clone();
        rpc_clone.file_list(&p2, move |result| {
            if let Ok(entries) = result {
                let flat_entries: Vec<(String, bool)> = entries
                    .into_iter()
                    .map(|e| (e.name, e.is_dir))
                    .collect();
                let mut sig2 = sig.clone();
                sig2.with_mut(|s2| {
                    s2.workspace.replace_dir_children(&p2, flat_entries);
                });
            }
        });
    });
};

rsx! {
    div {
        div {
            class: "file-tree-node file-tree-dir",
            style: format!("padding-left: {}px;", indent_px),
            onclick: dir_onclick,
            span { class: "{chevron_cls}", "\u{25be}" }
            span { class: "file-tree-icon", "{file_icon(true, &node.name)}" }
            span { class: "file-tree-label dir", "{node.name}" }
            span { class: "file-tree-refresh", onclick: refresh_onclick, "\u{21bb}" }
        }
        if !collapsed {
            div { class: "file-tree-children",
                {child_elements.into_iter()}
            }
        }
    }
}
```

- [ ] **Step 3: Check compilation**

Run: `cargo check -p vol-llm-ui 2>&1`
Expected: Compiles without errors

- [ ] **Step 4: Run all tests**

Run: `cargo test -p vol-llm-ui 2>&1`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/file_tree.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(web): add refresh button to directory nodes in file tree"
```
