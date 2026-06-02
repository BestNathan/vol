# UI JSON-RPC Protocol Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update vol-llm-ui raw JSON wire format to match the new vol-llm-agent-channel protocol — `req_id` → `run_id`, event format flattened, event type mapping updated to `AgentStreamEvent` variant keys.

**Architecture:** Four targeted edits to two files. `client.rs`: update `AgentEvent` struct, event parsing in `handle_message`, and `cancel` params key. `app.rs`: remap `agent_event_to_ui` function to match `AgentStreamEvent` CamelCase variant keys and field names.

**Tech Stack:** Rust, serde, serde_json, WASM (wasm32-unknown-unknown target)

---

### Task 1: Update AgentEvent struct and handle_message event parsing

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs:24-30,835-859`

- [ ] **Step 1: Update AgentEvent struct**

Replace lines 24-30:
```rust
/// Agent event received from the server subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub req_id: String,
    #[serde(rename = "event_type")]
    pub event_type: String,
    pub data: serde_json::Value,
}
```

With:
```rust
/// Agent event received from the server subscription.
/// Format matches AgentPayload::Event { run_id, event }.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub run_id: String,
    pub event: serde_json::Value,
}
```

- [ ] **Step 2: Update handle_message event parsing**

Replace lines 835-846 (the event handling branch):
```rust
    fn handle_message(inner: &Rc<ClientInner>, data: &str) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
            if val.get("method").and_then(|m| m.as_str()) == Some("agent.event") {
                if let Some(params) = val.get("params") {
                    if let Some(result) = params.get("result") {
                        if let Ok(event) = serde_json::from_value::<AgentEvent>(result.clone()) {
                            let _ = inner.event_tx.unbounded_send(event);
                        } else {
                            log::warn!("Failed to parse AgentEvent: {}", result);
                        }
                    }
                }
            } else if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
```

With:
```rust
    fn handle_message(inner: &Rc<ClientInner>, data: &str) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
            if val.get("method").and_then(|m| m.as_str()) == Some("agent.event") {
                if let Some(params) = val.get("params") {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params.clone()) {
                        let _ = inner.event_tx.unbounded_send(event);
                    } else {
                        log::warn!("Failed to parse AgentEvent: {}", params);
                    }
                }
            } else if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
```

Key change: event data is now directly in `params` (flat `{run_id, event}`), not nested under `params.result`.

- [ ] **Step 3: Verify the rest of handle_message is unchanged**

Lines 847-858 (response callback dispatch) remain unchanged.

- [ ] **Step 4: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -20`
Expected: errors in `app.rs` referencing old `AgentEvent` fields (will fix in Task 2)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "refactor: update AgentEvent struct and handle_message for new protocol format"
```

---

### Task 2: Update agent.cancel params

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs:297-310`

- [ ] **Step 1: Change cancel params from req_id to run_id**

Replace line 304:
```rust
            "params": { "req_id": req_id },
```
With:
```rust
            "params": { "run_id": req_id },
```

And rename the parameter on line 298 from `req_id` to `run_id`:
```rust
    pub fn cancel(&self, run_id: &str) -> Result<(), String> {
```

- [ ] **Step 2: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -10`

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "refactor: update agent.cancel params req_id -> run_id"
```

---

### Task 3: Update agent_event_to_ui mapping in app.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs:55-126`

- [ ] **Step 1: Replace agent_event_to_ui function**

Replace the entire function (lines 55-126) with:

```rust
fn agent_event_to_ui(event: &AgentEvent) -> Option<UiEvent> {
    let ev = &event.event;
    // AgentStreamEvent is externally-tagged: {"VariantName": {...fields}}
    // Extract the variant key and data from the single-key object.
    let (variant, data) = ev.as_object()
        .and_then(|obj| obj.iter().next())
        .map(|(k, v)| (k.as_str(), v))?;

    match variant {
        "AgentStart" => Some(UiEvent::AgentStart {
            input: data.get("input").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "AgentComplete" => Some(UiEvent::AgentComplete {
            response: data.get("response")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }),
        "AgentAborted" => Some(UiEvent::AgentAborted {
            reason: data.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "ThinkingStart" => Some(UiEvent::ThinkingStart),
        "ThinkingDelta" => Some(UiEvent::ThinkingDelta {
            delta: data.get("delta").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "ThinkingComplete" => Some(UiEvent::ThinkingComplete),
        "LLMCallStart" => Some(UiEvent::LlmCallStart {
            iteration: data.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        }),
        "LLMCallComplete" => Some(UiEvent::LlmCallComplete {
            model: data.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "LLMCallError" => Some(UiEvent::LlmCallError {
            error: data.get("error").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "ContentStart" => Some(UiEvent::ContentStart),
        "ContentDelta" => Some(UiEvent::ContentDelta {
            delta: data.get("delta").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "ContentComplete" => Some(UiEvent::ContentComplete {
            content: data.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "ToolCallBegin" => Some(UiEvent::ToolCallBegin {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            arguments: data.get("arguments").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "ToolCallArgumentDelta" => Some(UiEvent::ToolCallArgumentDelta {
            delta: data.get("delta").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "ToolCallComplete" => Some(UiEvent::ToolCallComplete {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            result: data.get("result").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            duration_ms: data.get("duration_ms").and_then(|v| v.as_u64()),
        }),
        "ToolCallError" => Some(UiEvent::ToolCallError {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            error: data.get("error").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            duration_ms: data.get("duration_ms").and_then(|v| v.as_u64()),
        }),
        "ToolCallSkipped" => Some(UiEvent::ToolCallSkipped {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            reason: data.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            duration_ms: data.get("duration_ms").and_then(|v| v.as_u64()),
        }),
        "MaxIterationsReached" => Some(UiEvent::MaxIterationsReached {
            current: data.get("current_iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            max: data.get("max_iterations").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        }),
        "IterationContinued" => Some(UiEvent::IterationContinued {
            from_iteration: data.get("from_iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        }),
        "IterationComplete" => Some(UiEvent::IterationComplete {
            iteration: data.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            final_answer: data.get("final_answer").and_then(|v| v.as_str()).map(|s| s.to_string()),
        }),
        _ => None,
    }
}
```

Key changes:
- Extract variant key from externally-tagged JSON (e.g., `{"AgentStart": {...}}`)
- Variant keys are CamelCase, not snake_case
- Field names match `AgentStreamEvent` struct fields:
  - `MaxIterationsReached`: `current` → `current_iteration`, `max` → `max_iterations`
  - `IterationContinued`: `from_iteration` (unchanged)
  - `IterationComplete`: `final_answer` (unchanged)
  - `agent_error` event removed (replaced by `LLMCallError` / `AgentAborted`)
  - `tool_call_argument_delta` → `ToolCallArgumentDelta` (CamelCase)

- [ ] **Step 2: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1`
Expected: clean compilation (only pre-existing warnings)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "refactor: remap agent_event_to_ui to AgentStreamEvent variant format"
```

---

### Task 4: Verify web build

**Files:**
- Verify: `crates/vol-llm-ui/` (full WASM build)

- [ ] **Step 1: Run full WASM check**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1`
Expected: clean

- [ ] **Step 2: Check there are no stale references to req_id or old event_type/data fields**

Run: `grep -rn "req_id\|event_type" crates/vol-llm-ui/src/`
Expected: no matches (excluding comments/strings in unrelated files)

- [ ] **Step 3: Commit any remaining fixes or skip**

If clean, no commit needed.
