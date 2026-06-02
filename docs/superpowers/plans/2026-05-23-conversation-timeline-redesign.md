# Conversation Timeline UI Redesign & Event Cleanup — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `LlmCall*` UI events and redesign conversation view as a monochrome timeline (dots + connecting lines).

**Architecture:** Remove `LlmCallStart/Complete/Error` from `UiEvent`, `UiEventKind`, `ConversationEntry`, and all handler/subscription code. Replace card-based `ConversationView` rendering with timeline layout: each entry gets a white dot + vertical line connector, last dot pulses during streaming.

**Tech Stack:** Rust, Dioxus 0.6, Tailwind CSS

---

### Task 1: Remove LlmCall from state/mod.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Remove `LlmCall` from `ConversationEntry` enum**

Delete lines 141-142 (the `LlmCall { iteration: u32, model: String }` variant):
```rust
// DELETE:
    LlmCall { iteration: u32, model: String },
```

- [ ] **Step 2: Remove `LlmCall*` from `UiEvent` enum**

Delete lines 39-42:
```rust
// DELETE:
    // LLM call
    LlmCallStart { iteration: u32 },
    LlmCallComplete { model: String },
    LlmCallError { error: String },
```

- [ ] **Step 3: Remove `LlmCall*` from `UiEventKind` enum**

On line 79, remove `LlmCallStart, LlmCallComplete, LlmCallError,`:
```rust
// BEFORE:
    LlmCallStart, LlmCallComplete, LlmCallError,
    ContentStart, ContentDelta, ContentComplete,

// AFTER:
    ContentStart, ContentDelta, ContentComplete,
```

- [ ] **Step 4: Remove `LlmCall*` arms from `UiEvent::kind()`**

Delete lines 98-100:
```rust
// DELETE:
            UiEvent::LlmCallStart { .. } => UiEventKind::LlmCallStart,
            UiEvent::LlmCallComplete { .. } => UiEventKind::LlmCallComplete,
            UiEvent::LlmCallError { .. } => UiEventKind::LlmCallError,
```

- [ ] **Step 5: Remove `LlmCall*` handlers from `UiState::apply`**

Delete lines 944-954:
```rust
// DELETE:
            UiEvent::LlmCallStart { iteration } => {
                self.conversation.push(ConversationEntry::LlmCall { iteration, model: String::new() });
            }
            UiEvent::LlmCallComplete { model } => {
                if let Some(ConversationEntry::LlmCall { model: m, .. }) = self.conversation.last_mut() {
                    *m = model.clone();
                }
            }
            UiEvent::LlmCallError { error } => {
                self.conversation.push(ConversationEntry::Error { message: format!("LLM error: {error}") });
            }
```

- [ ] **Step 6: Run cargo check to verify**

```bash
cargo check -p vol-llm-ui --no-default-features --features web 2>&1 | tail -10
```
Expected: compilation errors in other files (conversation.rs, app.rs, event_buffer.rs) — will fix in subsequent tasks.

---

### Task 2: Remove LlmCall from conversation.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`

- [ ] **Step 1: Remove `LlmCall*` handlers from `reduce_conversation`**

Delete lines 56-66 (the three LlmCall match arms):
```rust
// DELETE:
        UiEvent::LlmCallStart { iteration } => {
            conv.entries.push(ConversationEntry::LlmCall { iteration: *iteration, model: String::new() });
        }
        UiEvent::LlmCallComplete { model } => {
            if let Some(ConversationEntry::LlmCall { model: m, .. }) = conv.entries.last_mut() {
                *m = model.clone();
            }
        }
        UiEvent::LlmCallError { error } => {
            conv.entries.push(ConversationEntry::Error { message: format!("LLM error: {error}") });
        }
```

- [ ] **Step 2: Remove `LlmCall` rendering from `MessageEntry`**

Delete lines 133-136 (the `ConversationEntry::LlmCall` match arm):
```rust
// DELETE:
        ConversationEntry::LlmCall { iteration, model } => {
            let model_label = if model.is_empty() { format!("Calling LLM (iteration {iteration})...") } else { format!("Calling LLM: {model} (iteration {iteration})") };
            rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2030] border-l-[3px] border-[#a060c0]", div { class: "text-[#a060c0] font-bold", {model_label} } } }
        }
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check -p vol-llm-ui --no-default-features --features web 2>&1 | tail -10
```
Expected: errors in app.rs and event_buffer.rs remain.

---

### Task 3: Remove LlmCall from app.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Remove `LlmCall*` from conversation subscription array**

Lines 259-260, remove `LlmCallStart, LlmCallComplete, LlmCallError` entries:
```rust
// BEFORE:
            UiEventKind::ThinkingComplete, UiEventKind::LlmCallStart, UiEventKind::LlmCallComplete,
            UiEventKind::LlmCallError,

// AFTER:
            UiEventKind::ThinkingComplete,
```

- [ ] **Step 2: Remove `LlmCall*` from remote event parser**

Delete lines 77-85 (the LLMCallStart/Complete/Error parsing):
```rust
// DELETE:
        "LLMCallStart" => Some(UiEvent::LlmCallStart {
            iteration: data.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        }),
        "LLMCallComplete" => Some(UiEvent::LlmCallComplete {
            model: data.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "LLMCallError" => Some(UiEvent::LlmCallError {
            error: data.get("error").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check -p vol-llm-ui --no-default-features --features web 2>&1 | tail -10
```
Expected: only errors in event_buffer.rs remain.

---

### Task 4: Remove LlmCall from event_buffer.rs and backend agent.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/state/event_buffer.rs`
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Remove LlmCall comment from event_buffer.rs**

Replace lines 137-141 — the catch-all arm that mentions `LLMCallStart/Complete/Error`:
```rust
// BEFORE:
            // LLM call meta events, plugin events — invisible in UI
            AgentStreamEvent::LLMCallStart { .. }
            | AgentStreamEvent::LLMCallComplete { .. }
            | AgentStreamEvent::LLMCallError { .. }
            | AgentStreamEvent::PluginEvent { .. } => {}

// AFTER:
            AgentStreamEvent::PluginEvent { .. } => {}
```

- [ ] **Step 2: Remove `test_apply_stream_ignores_llm_meta_events` test**

Delete lines 242-265 (the test that verifies LLM meta events are ignored):
```rust
// DELETE entire test:
    #[test]
    fn test_apply_stream_ignores_llm_meta_events() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".", "local");
        buffer.apply_stream(
            &AgentStreamEvent::LLMCallStart { ... },
            &mut state,
        );
        ...
        assert!(state.conversation.is_empty());
    }
```

- [ ] **Step 3: Remove `llm_call_start` emission from agent.rs**

Delete lines 280-286:
```rust
// DELETE:
                // Emit LLMCallStart with full message history
                run_ctx
                    .emit(AgentStreamEvent::llm_call_start(
                        iteration,
                        messages.clone(),
                    ))
                    .await;
```

- [ ] **Step 4: Remove `llm_call_complete` emission from agent.rs**

Delete lines 331-334:
```rust
// DELETE:
                // Emit LLMCallComplete with real model and usage
                run_ctx
                    .emit(AgentStreamEvent::llm_call_complete(model.clone(), usage))
                    .await;
```

- [ ] **Step 5: Remove `llm_call_error` emission from agent.rs (lines 296, 314)**

These two emit `llm_call_error` followed by `agent_aborted`. Remove only the `llm_call_error` calls, keep `agent_aborted`.

Line 294-304:
```rust
// BEFORE:
                    Err(e) => {
                        run_ctx
                            .emit(AgentStreamEvent::llm_call_error(e.to_string()))
                            .await;
                        run_ctx
                            .emit(AgentStreamEvent::agent_aborted(format!(
                                "LLM request failed: {}",
                                e
                            )))
                            .await;
                        return Err(crate::AgentError::Llm(e));
                    }

// AFTER:
                    Err(e) => {
                        run_ctx
                            .emit(AgentStreamEvent::agent_aborted(format!(
                                "LLM request failed: {}",
                                e
                            )))
                            .await;
                        return Err(crate::AgentError::Llm(e));
                    }
```

Line 312-322:
```rust
// BEFORE:
                        Err(e) => {
                            run_ctx
                                .emit(AgentStreamEvent::llm_call_error(e.to_string()))
                                .await;
                            run_ctx
                                .emit(AgentStreamEvent::agent_aborted(format!(
                                    "LLM stream failed: {}",
                                    e
                                )))
                                .await;
                            return Err(e);
                        }

// AFTER:
                        Err(e) => {
                            run_ctx
                                .emit(AgentStreamEvent::agent_aborted(format!(
                                    "LLM stream failed: {}",
                                    e
                                )))
                                .await;
                            return Err(e);
                        }
```

- [ ] **Step 6: Build all affected crates**

```bash
cargo build -p vol-llm-agent -p vol-llm-agent-channel 2>&1 | tail -5
make web-check 2>&1 | tail -5
```
Expected: both compile clean.

---

### Task 5: Rewrite ConversationView as monochrome timeline

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`

- [ ] **Step 1: Add `GlobalState` import and read `is_running`**

Add to imports at top of file:
```rust
use crate::state::{
    ConversationEntry, ConversationState, GlobalState, UiEvent,
};
```

- [ ] **Step 2: Rewrite `ConversationView` component**

Replace lines 100-122 with:
```rust
#[component]
pub fn ConversationView() -> Element {
    let signal: Signal<ConversationState> = use_context();
    let global: Signal<GlobalState> = use_context();

    let guard = signal.read();
    let count = guard.active_entries().len();
    if count == 0 {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-1.5 sm:p-2.5 min-h-0",
                div { class: "flex items-center justify-center h-full text-[#666]", "No messages yet. Type a query and press Send." }
            }
        };
    }

    let entries = guard.active_entries().to_vec();
    let is_running = global.read().is_running;
    let messages: Vec<Element> = (0..count).map(|index| {
        let entry = entries[index].clone();
        let is_last = index == count - 1;
        rsx! { TimelineEntry { entry, is_last, is_running } }
    }).collect();
    rsx! {
        div { class: "flex-1 overflow-y-auto p-1.5 sm:p-2.5 min-h-0", {messages.into_iter()} }
    }
}
```

- [ ] **Step 3: Rewrite `MessageEntry` → `TimelineEntry` with timeline layout**

Replace `MessageEntry` component (lines 124-167) with:

```rust
#[component]
fn TimelineEntry(entry: ConversationEntry, is_last: bool, is_running: bool) -> Element {
    let dot_class = if is_last && is_running {
        "w-2 h-2 rounded-full bg-white animate-pulse shrink-0"
    } else {
        "w-2 h-2 rounded-full bg-white shrink-0"
    };

    let content = match entry {
        ConversationEntry::UserInput { text } => {
            rsx! { div { span { class: "font-bold", ">>> " } {text} } }
        }
        ConversationEntry::Thinking { content } => {
            rsx! { div { class: "text-[#888] italic text-sm", {content} } }
        }
        ConversationEntry::ContentStreaming { content } => {
            if content.is_empty() { rsx! { div { class: "text-[#888]", "Generating..." } } }
            else { rsx! { div { class: "text-[#e0e0e0]", {content} } } }
        }
        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! {
                div {
                    span { class: "font-bold", "[{tool_name}]" }
                    if !arg_preview.is_empty() {
                        div { class: "text-[#888] text-xs mt-0.5", "{arg_preview}" }
                    }
                }
            }
        }
        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let status = if success { "OK" } else { "ERR" };
            let color = if success { "#40c040" } else { "#c04040" };
            let display = truncate_lines(&preview, 6, 90);
            rsx! {
                div { class: "ml-4",
                    div { class: "text-xs",
                        span { class: "font-bold", style: "color: {color};", "[{status}] " }
                        span { style: "color: {color};", "{tool_name}" }
                    }
                    div { class: "text-[#888] text-xs mt-0.5 max-h-[120px] overflow-y-auto font-mono", {display} }
                }
            }
        }
        ConversationEntry::AgentAnswer { text } => {
            rsx! { div { class: "text-[#e0e0e0] whitespace-pre-wrap leading-[1.5]", {text} } }
        }
        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            let iw = if iterations == 1 { "iteration" } else { "iterations" };
            let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
            rsx! { div { class: "text-[#80c080] font-bold text-center text-sm", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
        }
        ConversationEntry::EntryCheckpoint { reason, note, created_at } => {
            let note_text = note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
            rsx! { div { class: "text-[#888] text-xs italic", "[Checkpoint {created_at}] {reason}{note_text}" } }
        }
        ConversationEntry::Error { message } => {
            rsx! { div { class: "text-[#ff6060] font-bold", "Error: {message}" } }
        }
    };

    rsx! {
        div { class: "flex gap-3",
            // Dot + line column
            div { class: "flex flex-col items-center w-3 shrink-0",
                div { class: dot_class }
                if !is_last {
                    div { class: "w-px flex-1 bg-[#333] min-h-[16px]" }
                }
            }
            // Content column
            div { class: "flex-1 pb-3 min-w-0 break-words",
                {content}
            }
        }
    }
}
```

- [ ] **Step 4: Build WASM frontend**

```bash
make web-check 2>&1 | tail -5
```
Expected: compile clean.

---

### Task 6: Final build and verification

**Files:** (none — verification only)

- [ ] **Step 1: Full backend build**

```bash
cargo build -p vol-llm-agent -p vol-llm-agent-channel 2>&1 | tail -5
```
Expected: compile clean.

- [ ] **Step 2: Full WASM build**

```bash
make web-build 2>&1 | tail -5
```
Expected: compile clean.

- [ ] **Step 3: Run backend tests**

```bash
cargo test -p vol-llm-agent -p vol-llm-agent-channel 2>&1 | grep -E "^test result|FAILED"
```
Expected: all pass (ignoring pre-existing code_agent_simulation MCP EPIPE failure).

- [ ] **Step 4: Verify LlmCall events removed from WebSocket stream**

```bash
python3 -c "
import asyncio, json, websockets
async def t():
    async with websockets.connect('ws://localhost:3001/ws') as ws:
        await ws.send(json.dumps({'jsonrpc':'2.0','method':'agent.subscribe','params':{},'id':1}))
        await ws.recv()
        await ws.send(json.dumps({'jsonrpc':'2.0','method':'agent.submit','params':{'input':'hi'},'id':2}))
        count = 0
        while True:
            raw = await asyncio.wait_for(ws.recv(), timeout=120)
            msg = json.loads(raw)
            if 'method' in msg and msg['method'] == 'agent.event':
                et = list(msg['params']['event'].keys())[0]
                count += 1
                print(f'[{count}] {et}')
                if 'LLMCall' in et:
                    print('FAIL: LLMCall event still present!')
                    return
                if et == 'AgentComplete':
                    break
        print(f'OK: {count} events, no LLMCall events')
asyncio.run(t())
"
```
Expected: no `LLMCall*` events in output; `AgentComplete` received.

- [ ] **Step 5: Commit all changes**

```bash
git add crates/vol-llm-ui/src/state/mod.rs \
        crates/vol-llm-ui/src/state/event_buffer.rs \
        crates/vol-llm-ui/src/web/components/conversation.rs \
        crates/vol-llm-ui/src/web/components/app.rs \
        crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: remove LlmCall events, redesign conversation as monochrome timeline

- Remove LlmCallStart/Complete/Error from UiEvent, UiEventKind, ConversationEntry
- Remove LlmCall handling from reduce_conversation, UiState::apply, event_buffer
- Remove llm_call_start/complete/error emission from agent loop
- Redesign ConversationView: white dots + gray lines, no card backgrounds
- ToolResult indented under ToolCall, last dot pulses during streaming"
```
