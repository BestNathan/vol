---
type: concept
category: pattern
tags: [event-loop, crossterm, tokio, select, input, rendering]
created: 2026-05-08
updated: 2026-05-08
source_count: 1
---

# UI Event Loop Pattern

**Category:** Event loop pattern for async TUI applications

**Related:** [[vol-llm-ui-crate]], [[tui-frontend-ratatui]], [[ratatui-tui-pattern]]

## Definition

The pattern for multiplexing keyboard input and periodic rendering in an async TUI application using crossterm's `EventStream` and tokio's `select!` macro.

## Key Points
- `EventStream::new()` from crossterm provides an async stream of terminal events
- `tokio::time::interval(33ms)` drives rendering at ~30fps
- `tokio::select!` with `biased` mode prioritizes input events over render ticks
- Shared `UiState` is protected by `Arc<Mutex<UiState>>` — locked briefly during key handling and render
- Agent runs are spawned as separate tokio tasks; state mutations flow through `LocalEventObserver`

## How It Works

```rust
loop {
    tokio::select! {
        biased;

        // 1. Keyboard input (highest priority)
        maybe_event = events.next() => {
            match maybe_event {
                Some(Ok(Event::Key(key))) => {
                    let mut state = ui_state.lock().await;
                    let action = handle_key(key, &mut state, &input_buf);
                    // action: Exit, Send(text), ResumeSession(id), None
                }
                _ => {}
            }
        }

        // 2. Periodic render (lower priority)
        _ = render_interval.tick() => {
            let state = ui_state.lock().await;
            terminal.draw(|f| render_ui(f, &state))?;
        }
    }
}
```

Terminal lifecycle:
1. `enable_raw_mode()` — capture raw key events
2. `execute!(stdout, EnterAlternateScreen)` — switch to alternate screen buffer
3. Set panic hook to restore terminal on crash
4. On exit: `execute!(stdout, LeaveAlternateScreen)` then `disable_raw_mode()`

## Input Handling

`handle_key()` returns `InputAction` enum:
- `Exit` — quit the application (Ctrl+Q)
- `Send(String)` — submit user input to agent (Enter)
- `ResumeSession(String)` — resume a saved session (session dialog)
- `None` — key consumed for navigation, no external action

Priority order in key handling:
1. Approval keys (A/R/S) — highest, bypass all other logic
2. Session dialog keys (Esc/Enter/Up/Down/n/d)
3. Global navigation (Tab, Ctrl+1-4, PageUp/Down, arrows)
4. Mode toggles (Ctrl+U for unsafe mode, Ctrl+S for sessions)
5. Quit (Ctrl+Q)

## Related Concepts
- [[ratatui-tui-pattern]]: The rendering patterns driven by the event loop
- [[human-in-the-loop]]: Approval key handling interrupts normal input flow
- [[agent-event-stream]]: How agent events flow into the UI state
