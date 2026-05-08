---
type: source
source_type: report
date: 2026-05-08
ingested: 2026-05-08
tags: [task, implementation, tui, ratatui, frontend]
---

# TUI Frontend (ratatui)

**Authors/Creators:** Claude Code (vol-llm-ui team)
**Date:** 2026-05-08
**Link:** Plan at `docs/superpowers/plans/2026-05-08-dioxus-cross-platform-ui.md` Task 7

## TL;DR
Created a terminal UI binary using ratatui 0.30 + crossterm 0.29, migrating 9 render functions from the legacy `vol-llm-tui` crate into `vol-llm-ui` with `UiState` instead of `AppState`.

## Key Takeaways
- All 9 render functions (status bar, tab bar, conversation, tools panel, input area, workspace, log viewer, skills, session dialog) migrated to `UiState`
- `LocalConnection::clone_for_run()` made public to support spawning agent runs from the TUI event loop
- `futures` and `uuid` added as optional TUI feature dependencies
- TUI modules exported via `pub mod tui` behind `#[cfg(feature = "tui")]` in `lib.rs`
- Binary uses `tokio::select!` to multiplex keyboard input (33ms render interval + crossterm `EventStream`)
- `AgentConnection::submit()` returns `mpsc::Receiver<UiEvent>` for terminal events; intermediate events applied by `LocalEventObserver` directly to shared `Arc<Mutex<UiState>>`

## Detailed Summary

Task 7 created the TUI binary and supporting library code in `crates/vol-llm-ui/src/tui/`. The implementation mirrors the legacy `vol-llm-tui/src/ui/` structure but adapts all render functions from `AppState` to the unified `UiState` model.

The file structure:
- `render.rs` (~370 lines): All 9 render functions using ratatui widgets (Paragraph, List, Block, Clear)
- `input.rs` (~140 lines): Keyboard handler with approval keys, tab navigation, scroll controls, session dialog
- `mod.rs`: Re-exports `render_ui`, `handle_key`, `InputAction`
- `bin/tui.rs` (~130 lines): Binary entry point with crossterm raw mode setup, `tokio::select!` event loop, panic-safe terminal cleanup

Key architectural decisions:
- The binary lives in `src/tui/bin/tui.rs` with `mod render` and `mod input` importing from the library's `vol_llm_ui::tui` module
- `LocalConnection` takes `CodingAgentConfig` + `Arc<Mutex<UiState>>` (not the simpler signature from the plan)
- The `LocalEventObserver` applies `AgentStreamEvent` to `UiState` via `EventBuffer::apply_stream()`, so the receiver from `submit()` only gets terminal events
- Emoji characters in the plan's render code (checkmarks, warning symbols) replaced with ASCII equivalents for terminal compatibility

## Entities Mentioned
- [[vol-llm-ui-crate]]: The crate receiving the TUI frontend implementation

## Concepts Covered
- [[ratatui-tui-pattern]]: ratatui widget composition and layout patterns used in the render functions
- [[ui-event-loop-pattern]]: crossterm EventStream + tokio::select! multiplexing pattern
- [[human-in-the-loop]]: Approval panel rendering and key handling

## Notes
- The plan's original `tui.rs` had `mod render` / `mod input` as local binary modules, but the compiler requires them either in the same directory as the binary or exported from the library. The library export approach was chosen.
- `ActiveTab` does not implement `Deref`, so the closure in `render_tab_bar` was adjusted to compare by value instead.
