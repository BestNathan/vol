---
type: source
source_type: design
date: 2026-05-18
ingested: 2026-05-18
tags: [mobile, responsive, dioxus, tailwind, frontend, ui]
---

# Mobile Layout Design

**Authors/Creators:** BestNathan
**Date:** 2026-05-18
**Link:** docs/superpowers/specs/2026-05-18-mobile-layout-design.md, docs/superpowers/plans/2026-05-18-mobile-layout.md

## TL;DR
Added responsive mobile layout support to the Dioxus web frontend so phone screens (<640px) get a full interactive experience with a slide-out file tree drawer, compact status bar, and properly sized touch targets. Desktop layout preserved at `sm:` breakpoint (640px) and above.

## Key Takeaways
- File tree becomes a hidden-by-default slide-out drawer on mobile with backdrop overlay and close button
- Status bar hides verbose fields (Run, Iter, Tools, Time) on mobile, keeps connection dot + session ID + badge
- Tab bar uses `flex-nowrap` with horizontal scrolling and smaller text on mobile
- All dialogs use `w-[95vw]` on mobile to fit within viewport
- Conversation messages get tighter padding on mobile
- Implementation uses Tailwind `sm:` responsive utilities only — no new crates or dependencies
- New `file_tree_drawer_open: bool` field added to `WorkspaceState`

## Detailed Summary

### Architecture
- `sm:` breakpoint (640px) used as mobile/desktop boundary across all components
- `WorkspaceState` gains `file_tree_drawer_open: bool` signal field
- File tree drawer: `fixed inset-y-0 left-0 z-50` overlay on mobile, inline sidebar on desktop
- Backdrop: `fixed inset-0 z-40 bg-black/50` — click to dismiss drawer
- Hamburger button: `sm:hidden absolute top-1 left-1 z-[60]` — visible only on mobile

### Component Changes
- **StatusBar**: Run/Iter/Tools/Time fields get `hidden sm:inline`, build info gets `hidden sm:flex`
- **FileTree**: `file_tree_outer_class()` function returns different Tailwind classes based on drawer state; `DESKTOP_SIDEBAR_CLASSES` constant for desktop-only classes
- **TabBar**: `flex-nowrap overflow-x-auto` prevents wrapping, tabs get `text-[11px] sm:text-[13px]`
- **Conversation**: padding reduced from `p-2.5` to `p-1.5` on mobile, message margins similarly adjusted
- **InputArea**: textarea text size `text-[13px] sm:text-[14px]`, hint `text-[10px] sm:text-[11px]`
- **Dialogs**: `w-[95vw]` on mobile, `sm:min-w-[400px] sm:w-[90vw] sm:max-w-[500px]` on desktop

### Code Review Fixes
- Removed duplicate `#[component]` attribute on `MessageEntry`
- Deduplicated redundant `sm:overflow-x-auto` in tab bar
- Extracted `DESKTOP_SIDEBAR_CLASSES` constant to reduce duplication

### Implementation Process
- Brainstorming design doc → writing-plans implementation plan → subagent-driven execution
- 6 feature commits + 1 review fix commit
- `cargo check -p vol-llm-ui --features web` passes clean

## Entities Mentioned
- [[vol-llm-ui-crate]]: the Dioxus web frontend crate that received mobile layout support
- [[dioxus-web-pattern]]: the component pattern used for responsive changes
- [[tailwind-css-migration]]: the Tailwind CSS migration that established the foundation for responsive utility classes

## Concepts Covered
- [[dioxus-signal-pattern]]: `file_tree_drawer_open` signal in `WorkspaceState`
- [[tailwind-css-responsive-pattern]]: Tailwind responsive breakpoint usage
- [[drawer-ui-pattern]]: slide-out drawer pattern with backdrop overlay

## Notes
- No automated test framework exists for WASM UI — manual testing on phone browser required
- Future enhancement: `aria-label` on hamburger and close buttons for accessibility
- Future enhancement: focus-trap inside drawer when open
