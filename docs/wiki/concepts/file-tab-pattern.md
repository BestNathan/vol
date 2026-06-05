---
type: concept
category: pattern
tags: [tabs, dioxus, file-viewer, component, rendering]
created: 2026-05-10
updated: 2026-05-10
source_count: 1
---

# File Tab Pattern

**Category:** Dioxus component pattern
**Related:** [[dioxus-web-pattern]], [[dioxus-signal-pattern]], [[vol-llm-ui-crate]], [[workspace-tree-pattern]]

## Definition

Tabbed file viewer pattern for Dioxus WASM: a tab bar showing open files with click-to-select, close-with-redirect, and content display below. Uses a render function (not `#[component]`) for tab elements to avoid `PartialEq` requirements on complex props.

## Key Points

- `FileContentView` is the top-level `#[component]`, consuming `AppState` via `use_context()`
- Individual tabs are rendered by a plain function `render_tab(i, tab, state) -> Element`, not a component — this avoids `PartialEq` derive issues on `OpenFileTab` and `Vec<OpenFileTab>` props
- Tab bar uses `{tab_elements.into_iter()}` inside `rsx!` for rendering a collected `Vec<Element>`
- Each tab has a `key: "{path}"` for Dioxus diffing stability
- Close button uses `evt.stop_propagation()` to prevent tab selection when closing

## State Management

- `open_files: Vec<OpenFileTab>` and `selected_file_tab: Option<usize>` stored in `UiState`
- `Signal<u64>` version counter bumped on every tab interaction to trigger re-render
- Version bump uses `bump_version(&mut Signal<u64>)` helper: `let v = *ver.peek(); ver.set(v.wrapping_add(1))`

## Tab Close Logic

When a tab is closed:
1. If no tabs remain → `selected_file_tab = None`
2. If closed tab was selected → select `pos.min(new_len - 1)` (the tab that shifted in)
3. If closed tab was before selected → shift selected index down by 1

## Content Display States

| content | error | Display |
|---------|-------|---------|
| `Some(c)` | any | `FileContentDisplay { content: c }` — `<pre>` block |
| `None` | `Some(e)` | Error text: `"Error: {e}"` |
| `None` | `None` | Loading placeholder: `"Loading..."` |

## CSS Classes

- `.file-content-view` — flex container
- `.file-tab-bar` — horizontal tab strip
- `.file-tab` / `.file-tab.active` — tab styling with bottom border indicator
- `.file-tab-icon` / `.file-tab-name` / `.file-tab-close` — tab sub-elements
- `.file-content` — `<pre>` block with monospace font
- `.file-content-empty` / `.file-content-error` / `.file-content-loading` — state placeholders

## Related Concepts
- [[dioxus-web-pattern]]: Parent component architecture
- [[dioxus-signal-pattern]]: State management and version bumping
- [[vol-llm-ui-crate]]: `OpenFileTab` struct definition
- [[workspace-tree-pattern]]: File tree that opens files into tabs
- [[dependency-graph-visualization]]: Sibling read-only Tasks-tab view in the same crate
- [[lazy-load-dir-tree]]: Source documenting the tree implementation
