---
type: source
source_type: incident
date: 2026-05-18
ingested: 2026-05-18
tags: [ui, file-tree, chevron, dioxus]
---

# FileTree Chevron Glyph Refinement

**Authors/Creators:** Claude Code  
**Date:** 2026-05-18  
**Link:** `crates/vol-llm-ui/src/web/components/file_tree.rs`

## TL;DR

Refined the FileTree directory expand/collapse control to use a small CSS-drawn chevron instead of a text glyph. The chevron is drawn from top/right borders so its angle is wider and more controllable; collapsed state points right and expanded state rotates 90 degrees around the icon center, with the old button-like box/background styling removed.

## Key Takeaways

- Directory expand/collapse now uses a CSS-drawn chevron for clearer visual direction.
- Collapsed directories render the chevron pointing right; expanded directories rotate it downward.
- The chevron is styled as a plain shape rather than a rounded icon button.
- Regression tests verify the directory chevron is drawn with CSS borders and no longer uses unicode chevron or triangle glyphs in the directory chevron render path.

## Detailed Summary

The FileTree had already been updated so unloaded empty directories render collapsed by default and first click loads them. This refinement changes only the directory chevron presentation: `TreeNode` now renders an empty wrapper with an inner span drawn from `border-r-2 border-t-2`, uses a smaller `h-1.5 w-1.5` chevron shape, and applies `origin-center rotate-90` in the expanded state so the drawn chevron rotates around its center.

Verification:

- `cargo test -p vol-llm-ui --no-default-features --features web` — 32 tests passed
- `make web-check` — passed
- `git diff --check` — passed

## Entities Mentioned

- [[vol-llm-ui-crate]]: owns the Dioxus FileTree component and directory chevron rendering.

## Concepts Covered

- [[workspace-tree-pattern]]: FileTree visual collapse semantics now use a clearer greater-than-style chevron affordance.

## Notes

This is a visual refinement only; the lazy-loading and collapsed-state semantics from [[file-tree-collapsed-state-follow-up]] are unchanged.
