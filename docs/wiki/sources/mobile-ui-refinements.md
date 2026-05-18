---
type: source
category: implementation
tags: [mobile, responsive, dioxus, tailwind, web-ui, skills]
created: 2026-05-18
updated: 2026-05-18
---

# Mobile UI Refinements

**Summary:** Follow-up mobile fixes for the Dioxus web frontend: the file tree drawer is scoped to the main content area, the message textarea uses a mobile-safe font size to avoid browser zoom on focus, and the Skills tab gets a mobile card layout instead of a table.

**Related:** [[drawer-ui-pattern]], [[dioxus-web-pattern]], [[tailwind-css-migration]], [[skills-panel-json-rpc]], [[vol-llm-ui-crate]], [[mobile-file-tree-rail]]

## Key Takeaways

- File tree open state uses `absolute inset-y-0 left-0` inside the main content container instead of viewport-level `fixed`, so it does not cover `StatusBar`.
- File tree backdrop uses `absolute inset-0` for the same reason: it dims only the main content area below the status bar.
- `InputArea` textarea uses `text-[16px] sm:text-[14px]`; this preserves compact desktop typography while avoiding mobile browser zoom on focus.
- `SkillsPanel` renders mobile cards in `sm:hidden flex flex-col gap-2` and keeps the table as `hidden sm:table w-full`.
- Mobile skill cards show name, version, scope badge, and description in a denser touch-friendly layout.
- Regression tests cover drawer positioning, textarea mobile font size, and Skills mobile/desktop layout split.

## Implementation

- `crates/vol-llm-ui/src/web/components/file_tree.rs`
  - `file_tree_outer_class(true)` changed from `fixed ...` to `absolute ...`.
  - Mobile drawer backdrop changed from `fixed inset-0` to `absolute inset-0`.
  - Added tests preventing viewport-level drawer/backdrop positioning.
- `crates/vol-llm-ui/src/web/components/input_area.rs`
  - Textarea class changed from `text-[13px] sm:text-[14px]` to `text-[16px] sm:text-[14px]`.
  - Added a regression test for the mobile-safe font size.
- `crates/vol-llm-ui/src/web/components/skills.rs`
  - Added `SkillCard` mobile component.
  - Added mobile card list and desktop-only table split.
  - Added a regression test for the mobile card / desktop table contract.
- `crates/vol-llm-ui/assets/tailwind.css`
  - Regenerated with `make web-css`.

## Verification

- `cargo test -p vol-llm-ui --no-default-features --features web`
- `make web-check`
- `git diff --check`

## Contradiction Check

This refines [[mobile-file-tree-rail]] and [[drawer-ui-pattern]]: the open drawer is still a drawer, but it is now positioned relative to the main content area so it cannot cover the status bar. It also extends [[skills-panel-json-rpc]] with a presentation rule: the same skill list data should be rendered as cards on mobile and a table on desktop.
