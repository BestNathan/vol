---
type: source
source_type: incident
date: 2026-05-18
ingested: 2026-05-18
tags: [ui, file-tree, lazy-loading, dioxus, bugfix]
---

# FileTree Collapsed State Follow-up Fix

**Authors/Creators:** Claude Code  
**Date:** 2026-05-18  
**Link:** `crates/vol-llm-ui/src/web/components/file_tree.rs`

## TL;DR

Fixed the remaining FileTree expansion issue after the loaded-state invariant change: newly discovered directories now render visually collapsed by default when their own children have not been loaded, and first click loads them without inserting the path into `collapsed_dirs`. The directory chevron was enlarged from `w-5 h-5 text-[11px]` to `w-6 h-6 text-[16px]` for a clearer expand affordance.

## Key Takeaways

- `WorkspaceTreeNode::loaded = false` alone was not enough; `TreeNode` also needed collapsed-state semantics for unloaded empty directories.
- `directory_is_collapsed()` treats unloaded empty directories as collapsed even when they are not explicitly present in `collapsed_dirs`.
- `toggle_directory_for_click()` makes the first click on an unloaded directory request `file.list` without adding the directory to `collapsed_dirs`.
- Successful loads remove the path from `collapsed_dirs`, so freshly loaded children render expanded after the fetch completes.
- Regression tests cover unloaded visual collapse, loaded explicit collapse behavior, first-click load behavior, and chevron size.

## Detailed Summary

The earlier state fix inserted discovered child directories with `loaded: false`, but `TreeNode` still computed its UI state from `collapsed_dirs.contains(&node.path)` only. A discovered directory was not in `collapsed_dirs`, so the UI still rendered it as open even though it had no loaded children.

The follow-up fix extracts two pure helpers in `file_tree.rs`: `directory_is_collapsed()` and `toggle_directory_for_click()`. `directory_is_collapsed()` combines explicit user collapse state with the lazy-loading invariant: directories with `loaded == false` and no children are visually collapsed. `toggle_directory_for_click()` returns whether a `file.list` request should be made, ensuring unloaded directories load on first click rather than toggling into a collapsed state.

The chevron styling was also changed to a larger `w-6 h-6 text-[16px] font-bold` affordance while preserving rotation for collapsed state.

Verification:

- `cargo test -p vol-llm-ui --no-default-features --features web` — 31 tests passed
- `make web-check` — passed
- `git diff --check` — passed

## Entities Mentioned

- [[vol-llm-ui-crate]]: owns the Dioxus FileTree component and workspace tree state.

## Concepts Covered

- [[workspace-tree-pattern]]: lazy-loaded directory state requires both data-level and UI-level collapsed semantics.
- [[dioxus-web-pattern]]: FileTree uses pure helper functions around Dioxus component state to keep UI transitions testable.

## Notes

This follow-up completes the user-visible fix: discovered folders no longer initially appear expanded, and their first click loads and expands them directly.
