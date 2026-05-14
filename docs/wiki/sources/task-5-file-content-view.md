---
type: source
category: implementation
tags: [file-content, tabs, dioxus, component, web]
created: 2026-05-10
updated: 2026-05-10
---

# Task 5: FileContentView Component

**Date:** 2026-05-10
**Plan:** File tree and tools tab implementation plan (2026-05-10)

## Summary

Created `FileContentView` component in `crates/vol-llm-ui/src/web/components/file_content.rs` — a file tab bar with content preview for the Dioxus WASM web frontend. Shows open files as clickable tabs with file icons, names, and close buttons. Content area displays loaded file content, error messages, or loading state.

## Key Design Decisions

- **Non-component render function** for tabs: `render_tab` is a plain function returning `Element` (not a `#[component]`) to avoid `PartialEq` derive issues on `OpenFileTab` and `Vec<OpenFileTab>` props. This follows the same pattern as `render_node` in `file_tree.rs`.
- **Version bump helper**: Extracted `bump_version(&mut Signal<u64>)` function, matching the pattern in `file_tree.rs` — reads via `peek()`, increments via `set()`.
- **Tab close logic**: On close, if the closed tab was selected, selects the tab that shifted into its position (or the last tab). If a tab after the selected one was closed, the selected index shifts down by 1.
- **File icon reuse**: Calls `crate::web::components::file_tree::file_icon(false, &name)` — required making `file_icon` `pub(crate)`.
- **Three-state content display**: `FileContentDisplay` for loaded content (wrapped in `<pre>`), error text for load failures, "Loading..." placeholder when content is not yet available.

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-ui/src/web/components/file_content.rs` | Created — 132 lines |
| `crates/vol-llm-ui/src/web/components/mod.rs` | Added `pub mod file_content` and `pub use` |
| `crates/vol-llm-ui/src/web/components/file_tree.rs` | Made `file_icon` `pub(crate)` |

## Build Status

- WASM build: Compiles with only pre-existing `ActiveTab::Tools` non-exhaustive error (to be fixed in Task 6)
- No new errors introduced by `file_content.rs`

## Related
- [[task-8-dioxus-web-frontend]]: Base component architecture
- [[dioxus-web-pattern]]: Component structure this builds on
- [[dioxus-signal-pattern]]: State management approach
- [[file-tab-pattern]]: Tab interaction pattern introduced here
- [[vol-llm-ui-crate]]: `OpenFileTab` state model
