---
type: source
source_type: incident
date: 2026-05-18
ingested: 2026-05-18
tags: [ui, file-tree, lazy-loading, dioxus, bugfix]
---

# FileTree Single-Click Expand Fix

**Authors/Creators:** Claude Code  
**Date:** 2026-05-18  
**Link:** `crates/vol-llm-ui/src/state/mod.rs`

## TL;DR

Fixed a FileTree lazy-loading state bug where newly discovered child directories required two clicks to expand. `WorkspaceTreeNode::replace_dir_children()` now inserts discovered directories with `loaded: false` instead of `loaded: true`, so the first click is treated as an expand-and-fetch action rather than a collapse action.

## Key Takeaways

- The bug appeared because discovered child directories had no children yet but were marked `loaded: true`.
- `TreeNode` uses `collapsed_dirs` to determine expanded/collapsed rendering, while `loaded` determines whether children need fetching.
- A discovered directory should remain unloaded until its own `file.list` request succeeds.
- Files remain effectively loaded because they do not lazily fetch children.
- Regression test `test_replace_dir_children_keeps_child_dirs_unloaded` covers the expected state invariant.

## Detailed Summary

The FileTree renders `WorkspaceTreeNode` recursively and fetches directory children through JSON-RPC `file.list`. When one directory was loaded, `replace_dir_children()` created every returned entry with `loaded: true`, including returned child directories.

That created an inconsistent state for child directories: the node had `is_dir: true`, `loaded: true`, and `children: []`. Since `collapsed_dirs` did not contain the newly discovered child path, the UI considered it expanded already. The user's first click inserted the path into `collapsed_dirs`, effectively collapsing the empty directory instead of fetching its children. The second click removed it from `collapsed_dirs`, which finally triggered the fetch path and made the directory expand.

The fix changes child node creation so discovered directories start with `loaded: false`, while file nodes still use `loaded: true`. This preserves the lazy-loading invariant: directory nodes are only marked loaded after their own children have been fetched.

Verification:

- `cargo test -p vol-llm-ui --no-default-features --features web` — 27 tests passed
- `make web-check` — passed

## Entities Mentioned

- [[vol-llm-ui-crate]]: owns `WorkspaceTreeNode`, `WorkspaceState`, and the Dioxus `FileTree` component.

## Concepts Covered

- [[workspace-tree-pattern]]: lazy-loaded recursive workspace tree and directory state invariants.
- [[dioxus-web-pattern]]: Dioxus component rendering and signal-driven UI updates.

## Notes

The user-visible symptom was “folders need two clicks to expand,” but the root cause was in the shared workspace tree state, not in the DOM click handler itself.
