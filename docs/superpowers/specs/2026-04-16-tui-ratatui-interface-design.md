# TUI Ratatui Full Interface Design

**Goal:** Replace the current simple crossterm print-based TUI with a full ratatui terminal UI featuring a status bar, tool call panel, tabbed conversation/workspace views, multi-line input, and persistent layout.

**Architecture:** Frame-driven rendering via ratatui. Events from the coding agent update a shared `AppState` (protected by `tokio::sync::Mutex`). The main thread runs a ratatui render loop that reads `AppState` and redraws every frame. `EventBuffer` converts `AgentStreamEvent` into state mutations on `AppState`.

**Tech Stack:** ratatui, ratatui-textarea, crossterm, tokio

---

## Layout

```
┌─ vol-llm-tui ─────────────────────────────────────────────────────────────────┐
│ Session: tui_20260416_143022.123     │ Iter: 3 │ Tools: 7 │ Time: 00:45 │ ◉ │  ← Status bar (fixed, 3 rows)
├──────────────────────────┬────────────────────────────────────────────────────┤
│ [Tools Called (7)]       │  [Conversation ▼]  [Workspace]  ← Tab bar          │
│                          │                                                    │
│ 1. [BashTool]      12ms  │  (Tab content fills remaining space)               │
│    ls -la src/           │                                                    │
│                          │                                                    │
│ 2. [ReadTool]       5ms  │                                                    │
│    path: src/main.rs     │                                                    │
│                          │                                                    │
│ 3. [WriteTool]     15ms  │                                                    │
│    file_path: out.txt    │                                                    │
│                          │                                                    │
│ (scrollable)             │  (scrollable)                                      │
│                          │                                                    │
├──────────────────────────┴────────────────────────────────────────────────────┤
│ Ctrl+Enter: Send  │ Esc: Clear                                                 │  ← Input hint (1 row)
├────────────────────────────────────────────────────────────────────────────────┤
│ > 帮我分析一下 vol-deribit 的                                                  │  ← Multi-line input
│   WebSocket 连接逻辑                                                          │    (ratatui-textarea, 5 rows)
│                                                                               │
├────────────────────────────────────────────────────────────────────────────────┤
│ Tab:切换  PgUp/PgDn:滚动  Ctrl+Enter:发送  Esc:清空  /quit:退出               │  ← Help bar (1 row)
└────────────────────────────────────────────────────────────────────────────────┘
```

### Layout proportions (relative to terminal size after status bar):

| Region | Width | Height |
|--------|-------|--------|
| Left panel (tools) | 30% (min 25, max 40) | All available minus input area |
| Right panel (content) | 70% | All available minus input area |
| Input area | 100% | 5 rows |
| Input hint | 100% | 1 row |
| Help bar | 100% | 1 row |

---

## State Bar

Fixed 3 rows at the top. Shows:

| Field | Source | Update frequency |
|-------|--------|------------------|
| Session ID | `AppState.session_id` | Once at startup |
| Iteration count | `EventBuffer.iteration` | Per `IterationComplete` |
| Tool call count | `EventBuffer.tool_call_count` | Per `ToolCallBegin` |
| Elapsed time | `EventBuffer.run_start` | Every 1s (render tick) |
| Run count | `AppState.run_count` | Per `AgentStart` |

---

## Left Panel: Tools Called

Shows every tool invocation from the **current run** in chronological order. Each entry displays:

- Sequence number
- Tool name in brackets
- Duration (or "ERR" on failure)
- Key parameter preview (one line): `command`, `path`, `file_path`, or `url`

```
┌─ Tools Called (7) ───────────────┐
│ 1. [BashTool]              12ms  │
│    ls -la src/                   │
│                                  │
│ 2. [ReadTool]               5ms  │
│    path: src/main.rs             │
│                                  │
│ 3. [BashTool]              ERR   │  ← red
│    invalid_command               │
└──────────────────────────────────┘
```

**Color coding:**
- Green border/title = success
- Red = error
- Grey = skipped

**Scrolling:** Panel is internally scrollable if entries exceed height. `PgUp`/`PgDn` scrolls when this panel is not focused — actually, since only the input area receives focus, scrolling uses global `PgUp`/`PgDn` on the currently visible right panel. The left panel scrolls to bottom automatically on new entries.

**Reset:** Cleared at the start of each new `agent.run()`.

---

## Right Panel: Tab Views

### Tab 1: Conversation

Scrollable message view showing the full conversation history:

```
>>> 帮我查一下这个文件

🤔 Thinking...
  我需要先看看项目结构...

[BashTool] ls -la src/
  OK 12ms
    total 48
    ...

>>> 找到了，文件在这里

Done · 3 iterations · 3 tool calls · 1.5s
```

**Behavior:**
- Auto-scrolls to bottom on new content
- Manual scroll with `PgUp`/`PgDn` (overrides auto-scroll when user scrolls up)
- Rendering driven by `EventBuffer` — same events as current `render.rs`, but writes to a `Vec<ConversationEntry>` instead of printing

### Tab 2: Workspace

File tree of `working_dir` with modification markers:

```
┌─ Workspace ──────────────────────┐
│ crates/                          │
│ ├── vol-llm-tui/                 │
│ │   ├── src/main.rs        ✎ M   │
│ │   └── src/render.rs            │
│ ├── vol-llm-agents/              │
│ │   └── src/coding/agent.rs ✎ M  │
│ └── Cargo.toml                   │
│ .vol-sessions/                   │
│ └── tui_20260416.jsonl           │
└──────────────────────────────────┘
```

**Markers:**
- `✎ M` = file was modified by `WriteTool` or `EditTool` during current run
- No marker = unchanged

**Data source:**
- File tree scanned once at TUI startup
- Modified file list updated from `EventBuffer` (tracks `ToolCallComplete` events for write/edit tools)
- Re-scanned on demand (future: manual refresh command)

**Directory filtering:** Hide `.git/`, `target/`, `node_modules/` by default.

---

## Bottom Input Area

Multi-line input powered by `ratatui-textarea`:

```
┌──────────────────────────────────────────────────────────────────────┐
│ > 帮我分析一下 vol-deribit 的                                         │
│   WebSocket 连接逻辑                                                 │
│   特别是重连机制                                                     │
│                                                                      │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

**Key bindings:**

| Key | Action |
|-----|--------|
| `Ctrl+Enter` | Send input to agent |
| `Escape` | Clear input buffer |
| Regular keys | Edit text (handled by ratatui-textarea) |

**Behavior during agent run:**
- Input area remains editable (user can type)
- `Ctrl+Enter` is **disabled** while agent is running (prevents queueing)
- After agent completes, any text in the input buffer can be sent

---

## Architecture

### Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│                        main()                                │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  tokio::spawn {                                         │ │
│  │    CodingAgent::new(config)                             │ │
│  │      .with_observer(RatatuiObserver)                    │ │
│  │      .run(input)                                        │ │
│  │                                                         │ │
│  │    RatatuiObserver ──events──▶ EventBuffer ──state──▶  │ │
│  │                                 (update AppState)       │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Main Thread (synchronous)                              │ │
│  │                                                         │ │
│  │  crossterm::event::poll() ──▶ input handling            │ │
│  │     ↓                                                   │ │
│  │  if Ctrl+Enter: send input, spawn agent task            │ │
│  │  if PgUp/PgDn: adjust scroll offset                     │ │
│  │  if Tab: switch tab                                     │ │
│  │  if Escape: clear input                                 │ │
│  │                                                         │ │
│  │  render loop (throttled to ~30fps):                     │ │
│  │    let state = app_state.lock().await;                  │ │
│  │    terminal.draw(|f| render_ui(f, &state))?;            │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Module Structure

| File | Responsibility |
|------|----------------|
| `src/main.rs` | Entry point, session creation, tokio runtime, crossterm setup |
| `src/app.rs` | `AppState` struct, initialization, accessors |
| `src/ui/mod.rs` | Layout orchestration, `render_ui()`, tab switching |
| `src/ui/status_bar.rs` | Status bar widget |
| `src/ui/tools_panel.rs` | Tool call history panel |
| `src/ui/conversation.rs` | Conversation tab widget |
| `src/ui/workspace.rs` | Workspace file tree widget |
| `src/ui/input.rs` | Multi-line input wrapper around ratatui-textarea |
| `src/render.rs` | `EventBuffer` — converts `AgentStreamEvent` to `AppState` mutations (existing file, repurposed) |

---

## Error Handling

### Terminal Recovery

```rust
// On startup: enable panic hook to restore terminal
let original_hook = std::panic::take_hook();
std::panic::set_hook(Box::new(move |panic| {
    // Restore terminal before printing panic
    disable_raw_mode().unwrap();
    execute!(stdout(), LeaveAlternateScreen).unwrap();
    original_hook(panic);
}));
```

### Graceful Degradation

- If `FileMessageStore` fails to create: fall back to in-memory session (same as current)
- If workspace scan fails: show empty workspace panel with error message
- If agent creation fails: show error in conversation panel

---

## Dependencies

Add to `crates/vol-llm-tui/Cargo.toml`:

```toml
ratatui = { version = "0.30", default-features = false, features = ["crossterm"] }
ratatui-textarea = "0.8"
```

Keep (needed for raw mode, alternate screen, event reading):

```toml
crossterm = "0.28"  # already a project dependency
```

**Version compatibility note:** Ratatui 0.30 uses feature flags to support crossterm 0.28+ via `crossterm_0_28`. `ratatui-textarea 0.8` targets ratatui 0.30+. No version conflicts expected.

---

## Key Behaviors

### Agent Run Lifecycle

```
1. User presses Ctrl+Enter with non-empty input
2. Set AppState.is_running = true
3. Clear EventBuffer for new run (tools, iteration counter)
4. Spawn tokio task: CodingAgent::new().with_observer().run(input)
5. Main render loop continues (30fps), showing streaming events
6. Agent completes → AppState.is_running = false
7. Input area re-enabled for next query
```

### Tab Switching

- `Tab` key: cycle between Conversation → Workspace → Conversation
- Active tab highlighted in tab bar
- Scroll position per-tab is preserved on switch

### Scroll Behavior

- Conversation tab: auto-scroll to bottom on new content; manual scroll disables auto-scroll until user scrolls back to bottom
- Tools panel: auto-scroll to bottom on new entry
- Workspace panel: static scroll (file tree doesn't change frequently)
