---
type: source
category: implementation
tags: [file-tree, mobile, drawer, dioxus, tailwind, web-ui]
created: 2026-05-18
updated: 2026-05-18
---

# Mobile File Tree Rail

**Summary:** The mobile file tree no longer uses a floating hamburger button. When closed, `FileTree` stays in the layout as a narrow left rail; when opened, it renders a left drawer scoped to the main content area with backdrop and close button.

**Related:** [[drawer-ui-pattern]], [[workspace-tree-pattern]], [[dioxus-web-pattern]], [[tailwind-css-migration]], [[vol-llm-ui-crate]], [[mobile-layout-design]], [[file-tree-sidebar-scroll-fix]]

## Key Takeaways

- The mobile closed state is an inline `w-10` rail, not `hidden`.
- The drawer open entry point lives inside `FileTree`, so `App` no longer renders a floating mobile hamburger button.
- The main content wrapper uses `min-w-0 flex-1`, so the tab/content region naturally occupies the space remaining after the rail.
- The full tree content is hidden only inside the rail on mobile (`hidden ... sm:flex`), while the outer `FileTree` container remains visible and reserves width.
- The open state was later refined by [[mobile-ui-refinements]] from viewport-level `fixed` positioning to main-content `absolute` positioning so it does not cover `StatusBar`.
- Tailwind CSS must be regenerated after adding rail utilities such as `w-10`, `px-0`, and `hover:bg-[#20203a]`.

## Implementation

- `crates/vol-llm-ui/src/web/components/app.rs`
  - Removed the app-level `sm:hidden absolute top-1 left-1` floating open button.
  - Changed the right-side content wrapper to `min-w-0 flex-1 flex flex-col overflow-hidden`.
  - Added a regression test that prevents `App` from owning `file_tree_drawer_open = true`.
- `crates/vol-llm-ui/src/web/components/file_tree.rs`
  - Changed `file_tree_outer_class(false)` from mobile `hidden` to an inline rail: `flex h-full min-h-0 w-10 flex-col flex-shrink-0 ...`.
  - Added `file_tree_panel_content_class(false)` so the full tree is hidden on mobile rail state but remains visible at `sm:`.
  - Added a rail button with vertical `Files` label and folder icon that opens the drawer.
  - Kept backdrop/close behavior for the open drawer state.
  - Added regression tests for the closed mobile rail and desktop content visibility contract.
- `crates/vol-llm-ui/assets/tailwind.css`
  - Regenerated with `make web-css`.

## Verification

- `cargo test -p vol-llm-ui --no-default-features --features web`
- `make web-check`

## Contradiction Check

This supersedes the initial [[mobile-layout-design]] closed-state description, which used a floating hamburger button and hid the file tree on mobile. The current pattern is encoded in [[drawer-ui-pattern]] and further refined by [[mobile-ui-refinements]]: closed mobile state is a left rail; open mobile state is a main-content-scoped drawer.
