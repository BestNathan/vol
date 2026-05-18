---
type: concept
category: pattern
tags: [ui, mobile, responsive, dioxus, tailwind]
created: 2026-05-18
updated: 2026-05-18
source_count: 1
---

# Drawer UI Pattern

Slide-out file tree drawer on mobile with backdrop overlay, hamburger toggle, and close button.

## Definition

A mobile-first responsive drawer pattern for the Dioxus web frontend where the file tree sidebar transforms into a slide-out overlay on screens below 640px width.

## Key Points

- **Breakpoint**: `sm:` (640px) from Tailwind default
- **Drawer state**: `file_tree_drawer_open: bool` in `WorkspaceState`, managed via `Signal<bool>`
- **Hamburger button**: `sm:hidden absolute top-1 left-1 z-[60]` — visible only on mobile
- **Backdrop**: `sm:hidden fixed inset-0 z-40 bg-black/50` — dismisses drawer on click
- **Close button**: `sm:hidden` X button in drawer header — dismisses drawer
- **Desktop sidebar**: `hidden sm:block sm:w-[33.33%] md:w-[33.33%] lg:w-[240px] ...` — always visible above breakpoint

## How It Works

1. On mobile load: file tree is `hidden` (drawer closed)
2. User taps hamburger → `file_tree_drawer_open = true` → drawer renders as `fixed` overlay
3. Backdrop appears behind drawer — clicking it sets `file_tree_drawer_open = false`
4. Close button in header also sets `file_tree_drawer_open = false`
5. On desktop (`sm:`): drawer classes are overridden by `sm:` prefixed classes, file tree appears as inline sidebar

## Related Concepts
- [[dioxus-signal-pattern]]
- [[tailwind-css-migration]]
- [[dioxus-web-pattern]]

## Sources
- [[mobile-layout-design]]
