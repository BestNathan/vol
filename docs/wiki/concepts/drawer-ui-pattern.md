---
type: concept
category: pattern
tags: [ui, mobile, responsive, dioxus, tailwind]
created: 2026-05-18
updated: 2026-05-18
source_count: 3
---

# Drawer UI Pattern

Mobile file tree drawer with an inline collapsed rail, backdrop overlay, and close button.

## Definition

A mobile-first responsive drawer pattern for the Dioxus web frontend where the file tree sidebar collapses to a narrow inline rail on phone screens and expands into a slide-out overlay when opened.

## Key Points

- **Breakpoint**: `sm:` (480px in this project Tailwind theme)
- **Drawer state**: `file_tree_drawer_open: bool` in `WorkspaceState`, managed via `Signal<bool>`
- **Closed mobile rail**: `w-10 flex-shrink-0` stays in the flex layout and reserves width for the file tree affordance
- **Open trigger**: Rail button lives inside `FileTree`, not as an app-level floating button
- **Backdrop**: `sm:hidden absolute inset-0 z-40 bg-black/50` — dismisses drawer on click and only covers the main content below the status bar
- **Close button**: `sm:hidden` X button in drawer header — dismisses drawer
- **Desktop sidebar**: `sm:w-[33.33%] md:w-[33.33%] lg:w-[240px] ...` with bounded flex-column scroll containment — always visible above breakpoint

## How It Works

1. On mobile load: file tree is a narrow inline rail, not hidden
2. User taps the rail → `file_tree_drawer_open = true` → drawer renders as an `absolute` overlay inside the main content area
3. Backdrop appears behind drawer — clicking it sets `file_tree_drawer_open = false`
4. Close button in header also sets `file_tree_drawer_open = false`
5. On desktop (`sm:`): full tree content is visible inline; closed mobile rail content is hidden with `sm:` overrides

## Layout Contract

`App` should not own a floating mobile drawer button. `FileTree` owns both the collapsed rail and the open drawer, while the main content region uses `min-w-0 flex-1` so the tab bar and tab content reserve only the rail width on mobile. The open drawer/backdrop should be `absolute`, not `fixed`, because `FileTree` is rendered inside the area below `StatusBar`; viewport-level positioning would cover the status bar.

## Related Concepts
- [[dioxus-signal-pattern]]
- [[tailwind-css-migration]]
- [[dioxus-web-pattern]]
- [[workspace-tree-pattern]]

## Sources
- [[mobile-layout-design]]
- [[mobile-file-tree-rail]]
- [[mobile-ui-refinements]]
