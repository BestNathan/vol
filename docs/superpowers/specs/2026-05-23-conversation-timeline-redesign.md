# Conversation Timeline UI Redesign & Event Cleanup

## Context

The conversation view currently renders each entry as a colored card with background + left border. It also has `LlmCall` entries showing model/iteration metadata that don't correspond to any persisted session content. This spec:

1. Removes `LlmCall*` UI events — they don't map to session content
2. Redesigns the conversation view as a monochrome timeline (dots + lines)

## Design

### Part 1: Remove LlmCall events

**Rationale:** `LlmCallStart/Complete/Error` produce `ConversationEntry::LlmCall` entries that show transient model name/iteration info. This data is not persisted in sessions — nothing in the session corresponds to `LlmCall`. Remove it.

**Delete:**
- `UiEvent::LlmCallStart`, `LlmCallComplete`, `LlmCallError` variants from `state/mod.rs`
- `UiEventKind::LlmCallStart`, `LlmCallComplete`, `LlmCallError` variants
- `ConversationEntry::LlmCall` variant
- All `LlmCall*` match arms in `reduce_conversation` (conversation.rs)
- `LlmCall*` subscription from `app.rs`
- `LlmCall` rendering from `ConversationView` (conversation.rs) and `SessionDetailOverlay` (sessions_panel.rs)
- `LlmCall*` → `UiEvent` mapping from `event_buffer.rs`
- `AgentStreamEvent::llm_call_start/completed/error` emission from `agent.rs`

**Keep:**
- `ContentStreaming` — streaming typewriter effect preserved
- All other events/entries unchanged

### Part 2: Conversation timeline UI

**Replace** card-based layout (bg + border-l + rounded) with monochrome timeline:

```
○  >>> user message              ← white dot (w-2 h-2 rounded-full bg-white)
│                                ← connecting line (w-px bg-[#333])
○  thinking content...           ← white dot, text #888 italic
│
○  tool_name                     ← white dot
│  └ tool result text            ← indented ml-4, smaller gray text
│
○  agent answer                  ← white dot, text #e0e0e0
│
◉  streaming text...             ← white dot + animate-pulse (processing)
```

**Layout structure per entry:**
```html
<div class="flex gap-3">
  <div class="flex flex-col items-center w-3 shrink-0">
    <div class="w-2 h-2 rounded-full bg-white {animate-pulse if is_running && is_last}" />
    <div class="w-px flex-1 bg-[#333]" />  <!-- hidden on last entry -->
  </div>
  <div class="flex-1 pb-3 min-w-0">
    <!-- entry content -->
  </div>
</div>
```

**Styling rules:**
- Dots: all `bg-white`, no color coding per type
- Dot pulse: last entry dot gets `animate-pulse` when `GlobalState.is_running` is true
- Connecting line: `bg-[#333]` between dots; hidden after last entry
- No card backgrounds, no left borders, no rounded corners anywhere
- Content text colors: white for normal, `#888` for thinking/secondary, `#e0e0e0` for answers
- Error text: red (`#ff6060`)
- RunSummary: green (`#80c080`), centered
- Checkpoint: small italic gray

**Entry rendering by type:**
| Entry | Style |
|-------|-------|
| UserInput | Bold `>>>` prefix, white text |
| Thinking | Italic, `#888`, small |
| AgentAnswer | Normal white, `whitespace-pre-wrap` |
| ContentStreaming | White text, dot pulses |
| ToolCall | White text, tool name |
| ToolResult | Indented `ml-4`, smaller gray text, `[OK]` / `[ERR]` prefix |
| RunSummary | Green centered text |
| EntryCheckpoint | Small italic `#888` |
| Error | Red text |

### Files to modify (6)

| File | Changes |
|------|---------|
| `crates/vol-llm-ui/src/state/mod.rs` | Remove `LlmCall` from `ConversationEntry`. Remove `LlmCall*` from `UiEvent` + `UiEventKind`. Remove `LlmCall*` match arms in `UiState::apply`. |
| `crates/vol-llm-ui/src/web/components/conversation.rs` | Remove `LlmCall*` from `reduce_conversation`. Rewrite `ConversationView` rendering — timeline layout. |
| `crates/vol-llm-ui/src/web/components/app.rs` | Remove `LlmCall*` from subscription arrays. |
| `crates/vol-llm-ui/src/web/components/sessions_panel.rs` | Remove `LlmCall` rendering from `SessionDetailOverlay`. |
| `crates/vol-llm-ui/src/state/event_buffer.rs` | Remove `LlmCall*` mapping from `apply_stream`. |
| `crates/vol-llm-agent/src/react/agent.rs` | Remove `llm_call_start/completed/error` emission. |

## Verification

1. `cargo build -p vol-llm-agent -p vol-llm-agent-channel` — backend compiles
2. `make web-check` — WASM frontend compiles
3. Send a message via WebSocket, verify:
   - No `LlmCallStart/Complete` events in the stream
   - All other events (Thinking, Content, Tool, AgentComplete) still fire
4. Open browser, verify:
   - Conversation shows timeline with white dots + gray lines
   - Last dot pulses during streaming
   - No colored backgrounds or card borders
   - Tool results indented under tool calls
