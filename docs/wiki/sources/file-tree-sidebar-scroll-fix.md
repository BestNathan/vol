---
type: source
category: implementation
tags: [file-tree, dioxus, tailwind, web-ui, layout]
created: 2026-05-18
updated: 2026-05-18
---

# File Tree Sidebar Scroll Fix

**Summary:** The Dioxus web file tree sidebar now has a bounded desktop flex-column layout, so the tree body scrolls correctly when many files are shown. Directory controls were also restyled into compact icon-button affordances.

**Related:** [[workspace-tree-pattern]], [[dioxus-web-pattern]], [[tailwind-css-migration]], [[vol-llm-ui-crate]], [[lazy-load-dir-tree]], [[mobile-layout-design]]

## Key Takeaways

- `DESKTOP_SIDEBAR_CLASSES` must use `sm:flex sm:flex-col sm:h-full sm:min-h-0` rather than mixing `sm:block` with flex utilities.
- The scroll container inside `FileTree` must include `min-h-0 flex-1 overflow-y-auto`; without `min-h-0`, a flex child can grow past its parent and prevent local scrolling.
- Directory rows now use a compact chevron hit target (`w-5 h-5`) and show the refresh button on row hover via `group-hover:opacity-100`.
- Tailwind CSS must be regenerated with `make web-css` after adding new utility classes.

## Root Cause

The desktop sidebar class string included both `sm:block` and `sm:flex`. Combined with the absence of explicit `sm:h-full` and `sm:min-h-0`, the child tree region did not have a reliable height constraint. Its `overflow-y-auto` was present, but it had no bounded box to scroll within.

## Implementation

- `crates/vol-llm-ui/src/web/components/file_tree.rs`
  - Replaced `sm:block` with a bounded desktop flex layout.
  - Added `min-h-0` to the tree body scroller in both loading and loaded states.
  - Restyled directory chevron and refresh controls as compact icon buttons.
  - Added a regression test for the desktop sidebar class contract.
- `crates/vol-llm-ui/assets/tailwind.css`
  - Regenerated from `assets/input.css` via `make web-css`.

## Verification

- `cargo test -p vol-llm-ui --no-default-features --features web desktop_sidebar_is_a_bounded_flex_column`
- `make web-check`
- `make web-build`

## Contradiction Check

No contradictions found. This refines the existing [[workspace-tree-pattern]] and [[drawer-ui-pattern]] documentation: mobile drawer behavior remains unchanged, while the desktop sidebar now has the same bounded-scroll flex convention used by modal and panel layouts.
