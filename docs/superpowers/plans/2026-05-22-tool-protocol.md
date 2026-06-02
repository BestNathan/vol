# Tool Protocol Operations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `ToolOperation`/`ToolPayload` to the agent server protocol with `tool.list`/`tool.call` methods, a `ToolHandler` domain handler backed by `ToolRegistry`, and update the frontend tools panel to list and invoke tools.

**Architecture:** Backend: add protocol types → add opcodec methods → create ToolHandler → register in server_core. Frontend: add `tool_list()`/`tool_call()` JSON-RPC methods in client.rs → update tools_panel to fetch tools and invoke them.

**Tech Stack:** Rust, serde, serde_json, WASM (for frontend)

---

### Task 1: Add ToolOperation and ToolPayload to protocol

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`

- [ ] **Step 1: Add ToolOperation enum**

Add after `SkillOperation` (around line 122):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolOperation {
    List,
    Call,
}
```

- [ ] **Step 2: Add ToolPayload enum**

Add before `ErrorPayload` (around line 635):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolPayload {
    List,
    ListResult {
        tools: Vec<serde_json::Value>,
    },
    Call {
        tool_name: String,
        arguments: serde_json::Value,
    },
    CallResult {
        tool_name: String,
        result: serde_json::Value,
    },
}
```

- [ ] **Step 3: Add Tool variants to Operation and Payload enums**

Add to `Operation` enum (after `Skill(SkillOperation)`):
```rust
    Tool(ToolOperation),
```

Add to `Payload` enum (after `Skill(SkillPayload)`):
```rust
    Tool(ToolPayload),
```

- [ ] **Step 4: Add method_name mappings**

In `Operation::method_name()`, add:
```rust
            Operation::Tool(ToolOperation::List) => "tool.list",
            Operation::Tool(ToolOperation::Call) => "tool.call",
```

- [ ] **Step 5: Add decode paths in Payload::from_operation()**

Add after the Skill decode block:
```rust
            Operation::Tool(ToolOperation::List) => {
                Ok(Payload::Tool(ToolPayload::List))
            }
            Operation::Tool(ToolOperation::Call) => {
                #[derive(Deserialize)]
                struct P {
                    tool_name: String,
                    #[serde(default)]
                    arguments: serde_json::Value,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("tool.call"))?;
                Ok(Payload::Tool(ToolPayload::Call {
                    tool_name: p.tool_name,
                    arguments: p.arguments,
                }))
            }
```

- [ ] **Step 6: Check compilation**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | tail -20`
Expected: errors in operation_codec.rs (no tool mappings yet), domain registry (no tool handler yet)

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent-channel/src/agent_server_protocol.rs
git commit -m "feat: add ToolOperation and ToolPayload to agent server protocol"
```

---

### Task 2: Add tool method mappings to operation_codec

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/operation_codec.rs`

- [ ] **Step 1: Add method_to_operation mappings**

In `method_to_operation()`, add after the skill mappings:
```rust
        "tool.list" => Operation::Tool(ToolOperation::List),
        "tool.call" => Operation::Tool(ToolOperation::Call),
```

Also add `ToolOperation` import at top:
```rust
use crate::agent_server_protocol::{..., ToolOperation, ToolPayload, ...};
```

- [ ] **Step 2: Add decode_payload mappings**

In `decode_payload()`, add:
```rust
        Operation::Tool(ToolOperation::List) => Payload::from_operation(&operation, value),
        Operation::Tool(ToolOperation::Call) => Payload::from_operation(&operation, value),
```

- [ ] **Step 3: Check compilation**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | tail -10`
Expected: clean (only pre-existing warnings)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/operation_codec.rs
git commit -m "feat: add tool.list/tool.call to operation codec"
```

---

### Task 3: Create ToolHandler and register in server_core

**Files:**
- Create: `crates/vol-llm-agent-channel/src/domain/tool.rs`
- Modify: `crates/vol-llm-agent-channel/src/domain/mod.rs`
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`

- [ ] **Step 1: Create domain/tool.rs**

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_core::ToolCall;
use vol_llm_tool::{ToolContext, ToolRegistry};

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, ToolOperation, ToolPayload,
};
use crate::domain::handler::DomainHandler;

/// Handler for tool-domain operations.
pub struct ToolHandler {
    tool_registry: Arc<ToolRegistry>,
}

impl ToolHandler {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }
}

#[async_trait]
impl DomainHandler for ToolHandler {
    fn name(&self) -> &str {
        "tool"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Tool(ToolOperation::List),
            Operation::Tool(ToolOperation::Call),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Tool(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("tool")),
        };
        match (op, message.payload) {
            (ToolOperation::List, Payload::Tool(ToolPayload::List)) => {
                let tools: Vec<serde_json::Value> = self
                    .tool_registry
                    .definitions()
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "name": d.name,
                            "description": d.description,
                            "parameters": d.parameters,
                        })
                    })
                    .collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Tool(ToolOperation::List),
                    Payload::Tool(ToolPayload::ListResult { tools }),
                )])
            }
            (ToolOperation::Call, Payload::Tool(ToolPayload::Call { tool_name, arguments })) => {
                let call = ToolCall {
                    id: uuid::Uuid::new_v4().simple().to_string(),
                    name: tool_name.clone(),
                    arguments: arguments.to_string(),
                    r#type: "function".to_string(),
                };
                let context = ToolContext::default();
                match self.tool_registry.execute(&call, &context).await {
                    Ok(result) => {
                        let value = serde_json::json!({
                            "success": result.success,
                            "content": result.content,
                            "error": result.error,
                            "data": result.data,
                        });
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Tool(ToolOperation::Call),
                            Payload::Tool(ToolPayload::CallResult {
                                tool_name,
                                result: value,
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Tool(ToolOperation::Call),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "tool_call_failed".to_string(),
                            message: e,
                            detail: None,
                            terminal: false,
                        },
                    )]),
                }
            }
            (ToolOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("tool.list")),
            (ToolOperation::Call, _) => Err(ProtocolError::PayloadDecodeFailed("tool.call")),
        }
    }
}
```

- [ ] **Step 2: Add tool module to domain/mod.rs**

Add `pub mod tool;` to `crates/vol-llm-agent-channel/src/domain/mod.rs`.

- [ ] **Step 3: Register ToolHandler in server_core.rs**

In `AgentServerCoreBuilder::build()`, add after other handler registrations (around where `SkillHandler` is registered):
```rust
        let tool_handler = crate::domain::tool::ToolHandler::new(tool_registry.clone());
        handler_registry.register(Arc::new(tool_handler));
```

- [ ] **Step 4: Check compilation and run tests**

Run: `cargo check -p vol-llm-agent-channel && cargo test -p vol-llm-agent-channel 2>&1 | tail -10`
Expected: compilation clean, all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/tool.rs crates/vol-llm-agent-channel/src/domain/mod.rs crates/vol-llm-agent-channel/src/server_core.rs
git commit -m "feat: add ToolHandler for tool.list and tool.call operations"
```

---

### Task 4: Add tool_list and tool_call to frontend client

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add tool_list method**

Add after `skill_get` (around line 830):

```rust
    /// List all system tools.
    pub fn tool_list(&self, cb: impl FnOnce(Result<Vec<serde_json::Value>, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tool.list",
            "params": {},
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => { cb(Err(e.to_string())); return; }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            match result.get("tools").and_then(|v| v.as_array()) {
                Some(tools) => cb(Ok(tools.clone())),
                None => cb(Err("no tools in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Call a tool directly.
    pub fn tool_call(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        cb: impl FnOnce(Result<serde_json::Value, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tool.call",
            "params": { "tool_name": tool_name, "arguments": arguments },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => { cb(Err(e.to_string())); return; }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            if let Some(error) = result.get("error") {
                let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
                cb(Err(msg.to_string()));
            } else {
                cb(Ok(result.clone()));
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }
```

- [ ] **Step 2: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: clean

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat: add tool_list and tool_call to JSON-RPC client"
```

---

### Task 5: Update tools_panel with tool listing and invocation

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_panel.rs`

- [ ] **Step 1: Add tool list fetching and call UI to ToolsPanel**

Replace `ToolsPanel` component to add fetch button and inline tool calling:

```rust
use crate::web::client::JsonRpcClient;

/// Tool definition from tool.list response.
#[derive(Debug, Clone, serde::Deserialize)]
struct ToolDef {
    name: String,
    description: Option<String>,
    parameters: Option<serde_json::Value>,
}

/// Store for tool list + call results.
struct ToolPanelState {
    tools: Vec<ToolDef>,
    loading: bool,
    error: Option<String>,
    call_result: Option<String>,
    selected_tool: Option<String>,
    args_input: String,
}

impl ToolPanelState {
    fn new() -> Self {
        Self {
            tools: Vec::new(),
            loading: false,
            error: None,
            call_result: None,
            selected_tool: None,
            args_input: "{}".to_string(),
        }
    }
}

#[component]
pub fn ToolsPanel() -> Element {
    let app_state: AppState = use_context();
    let signal = use_signal(|| ToolState::new());
    let tool_state = use_signal(|| ToolPanelState::new());
    let client: JsonRpcClient = app_state.client.clone();

    // Event subscriptions for tool calls during agent runs (existing)
    use_hook(move || {
        let bus = app_state.event_bus.clone();
        let mut set = SubscriptionSet::new(bus.clone());
        for kind in [UiEventKind::ToolCallBegin, UiEventKind::ToolCallComplete, UiEventKind::ToolCallError, UiEventKind::ToolCallSkipped] {
            set.subscribe(&bus, kind, {
                let signal = signal.clone();
                move |event| { reduce_tool_state(&mut *signal.write_unchecked(), event); }
            });
        }
        std::sync::Arc::new(set)
    });

    let count = signal.read().calls.len();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            // Tool list section
            div { class: "mb-3",
                div { class: "flex items-center justify-between mb-2",
                    div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "System Tools" }
                    button {
                        class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                        onclick: {
                            let client = client.clone();
                            let ts = tool_state.clone();
                            move |_| {
                                ts.write_unchecked().loading = true;
                                ts.write_unchecked().error = None;
                                let ts_clone = ts.clone();
                                client.tool_list(move |result| {
                                    let mut s = ts_clone.write_unchecked();
                                    s.loading = false;
                                    match result {
                                        Ok(tools) => {
                                            s.tools = tools.iter()
                                                .filter_map(|t| serde_json::from_value::<ToolDef>(t.clone()).ok())
                                                .collect();
                                        }
                                        Err(e) => s.error = Some(e),
                                    }
                                });
                            }
                        },
                        "Fetch Tools"
                    }
                }
                {tool_state.read().loading.then(|| rsx! { div { class: "text-[12px] text-[#888] px-2", "Loading..." } })}
                {tool_state.read().error.as_ref().map(|e| rsx! { div { class: "text-[12px] text-[#c04040] px-2", "{e}" } })}
                for tool in &tool_state.read().tools {
                    div { class: "border-b border-[#2a2a44] py-1 px-2",
                        div { class: "flex items-center justify-between",
                            div {
                                span { class: "text-[13px] font-semibold text-[#e0e0e0]", "{tool.name}" }
                                if let Some(ref desc) = tool.description {
                                    span { class: "text-[12px] text-[#888] ml-2", " - {desc}" }
                                }
                            }
                            button {
                                class: "px-1.5 py-0.5 text-[11px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#5a5a75]",
                                onclick: {
                                    let client = client.clone();
                                    let ts = tool_state.clone();
                                    let name = tool.name.clone();
                                    move |_| {
                                        let args_val: serde_json::Value = serde_json::from_str(
                                            &ts.read().args_input
                                        ).unwrap_or(serde_json::json!({}));
                                        let ts_clone = ts.clone();
                                        client.tool_call(&name, &args_val, move |result| {
                                            let mut s = ts_clone.write_unchecked();
                                            match result {
                                                Ok(val) => s.call_result = Some(
                                                    serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                                                ),
                                                Err(e) => s.call_result = Some(format!("Error: {e}")),
                                            }
                                        });
                                    }
                                },
                                "Run"
                            }
                        }
                    }
                }
            }

            // Divider
            div { class: "border-t border-[#333] my-2" }

            // Tool call results
            if let Some(ref result) = tool_state.read().call_result {
                div { class: "mb-2",
                    div { class: "text-[12px] font-semibold text-[#888] mb-1", "Call Result" }
                    pre { class: "text-[12px] font-mono text-[#ccc] bg-[#1a1a2e] p-2 rounded overflow-x-auto whitespace-pre-wrap", "{result}" }
                }
            }

            // Tool call history (existing)
            div {
                div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Call History ({count})" }
                div { class: "font-mono text-[13px]",
                    if count == 0 {
                        div { class: "p-2.5 text-[#666] text-center", "No tool calls yet" }
                    } else {
                        {(0..count).map(|idx| { let s = signal.clone(); rsx! { ToolItem { signal: s, index: idx } } }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }
        }
    }
}
```

Keep `ToolItem` and `reduce_tool_state` and `update_status` and `arg_preview` and `status_label` unchanged.

- [ ] **Step 2: Update AppState to expose client**

In `app.rs`, ensure `AppState` has a `client: JsonRpcClient` field. If not already present, add it.

- [ ] **Step 3: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -10`
Expected: clean

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_panel.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: add system tool listing and direct invocation to tools panel"
```

---

### Task 6: Verify end-to-end

**Files:** verify all

- [ ] **Step 1: Full channel tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1 | grep "test result"`
Expected: all pass

- [ ] **Step 2: WASM check**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -3`
Expected: clean

- [ ] **Step 3: Wiki ingest**

Update wiki with new tool protocol operations.
