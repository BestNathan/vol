# TUI HITL Approval + Unsafe Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make HITL approval prompts visible in the ratatui TUI and add an unsafe mode that auto-approves all dangerous tool calls.

**Architecture:** Add a pluggable `ApprovalHandler` trait to `AgentConfig`. The TUI provides its own handler that stores the approval request in `AppState` and uses a `Notify` to wait for keyboard input. Unsafe mode sets `hitl_enabled: false` and `unsafe_mode: true` in the config.

**Tech Stack:** ratatui 0.30, tokio `Notify`, `Arc<Mutex>`, `oneshot`, existing `ApprovalRequest`/`ApprovalResponse` types

---

## Problem

```
Agent loop → approval_tx/rx → run_cli_approval_loop → println! + stdin.read_line()
```

In ratatui raw mode: `println!` is invisible, `stdin` never gets input. Approval blocks forever.

**Solution:** Replace the CLI handler with a TUI handler via a pluggable callback in `AgentConfig`.

---

### Task 1: Add ApprovalHandler trait and wire into AgentConfig

**Files:**
- Modify: `crates/vol-llm-agent/src/react/hitl.rs` — add `ApprovalHandler` trait + `BoxedApprovalHandler`
- Modify: `crates/vol-llm-agent/src/react/agent.rs` — add `approval_handler` to `AgentConfig`, use it in `run()`

- [ ] **Step 1: Add ApprovalHandler trait to hitl.rs**

Add at the end of `crates/vol-llm-agent/src/react/hitl.rs` (after `run_cli_approval_loop`):

```rust
/// Type-erased approval handler for custom approval UIs (e.g., TUI).
/// Implement this to provide a custom approval prompt mechanism.
#[async_trait::async_trait]
pub trait ApprovalHandler: Send + Sync {
    /// Request approval and wait for response.
    /// Called by the agent loop when a tool requires human approval.
    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> Result<Option<ApprovalResponse>, ApprovalError>;
}

/// Cloneable wrapper for boxed ApprovalHandler trait objects.
/// Uses Arc internally to enable Clone semantics.
#[derive(Clone)]
pub struct BoxedApprovalHandler {
    inner: Arc<dyn ApprovalHandler + Send + Sync>,
}

impl BoxedApprovalHandler {
    pub fn new<H: ApprovalHandler + Send + Sync + 'static>(handler: H) -> Self {
        Self {
            inner: Arc::new(handler),
        }
    }
}

#[async_trait::async_trait]
impl ApprovalHandler for BoxedApprovalHandler {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        self.inner.request_approval(request).await
    }
}
```

- [ ] **Step 2: Export BoxedApprovalHandler and ApprovalHandler**

Add to `crates/vol-llm-agent/src/react/mod.rs`, add to the exports:

```rust
pub use hitl::{ApprovalHandler, BoxedApprovalHandler};
```

Also ensure `ApprovalRequest` and `ApprovalResponse` are still exported:
```rust
pub use run_context::{ApprovalRequest, ApprovalResponse};
```

- [ ] **Step 3: Add approval_handler field to AgentConfig**

In `crates/vol-llm-agent/src/react/agent.rs`, modify `AgentConfig` (line 20-30):

```rust
#[derive(Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub prompt_context: PromptContext,
    pub verbose: bool,
    pub plugin_registry: PluginRegistry,

    // Observability fields
    pub agent_id: String,
    pub log_base_path: PathBuf,

    /// When true, skip all HITL approval checks and auto-approve dangerous tools.
    pub unsafe_mode: bool,

    /// Custom approval handler. If set, this replaces the default CLI handler.
    /// Use this for TUI/HTTP-based approval flows.
    pub approval_handler: Option<BoxedApprovalHandler>,
}
```

Add to `Default` impl (line 42-58):

```rust
            unsafe_mode: false,
            approval_handler: None,
```

- [ ] **Step 4: Use approval_handler in run() instead of run_cli_approval_loop**

Replace lines 150-151 in `agent.rs`:

```rust
        // === Phase 1.6: Spawn approval handler for HITL ===
        if self.config.unsafe_mode {
            // Skip approval entirely — approval requests will fail open
            drop(approval_rx);
        } else if let Some(handler) = &self.config.approval_handler {
            // Use custom approval handler (e.g., TUI)
            spawn_custom_approval_handler(approval_rx, handler.clone());
        } else {
            // Default: CLI approval handler
            super::run_cli_approval_loop(approval_rx);
        }
```

- [ ] **Step 5: Add spawn_custom_approval_handler function**

Add to `crates/vol-llm-agent/src/react/hitl.rs` (before the tests section):

```rust
/// Spawn a background task that processes approval requests using the custom handler.
pub fn spawn_custom_approval_handler(
    mut rx: tokio::sync::mpsc::Receiver<(
        super::run_context::ApprovalRequest,
        tokio::sync::oneshot::Sender<super::run_context::ApprovalResponse>,
    )>,
    handler: BoxedApprovalHandler,
) {
    tokio::spawn(async move {
        while let Some((request, response_tx)) = rx.recv().await {
            match handler.request_approval(request).await {
                Ok(Some(response)) => {
                    let _ = response_tx.send(response);
                }
                Ok(None) => {
                    // Timeout/no response — fail open
                    let _ = response_tx.send(super::run_context::ApprovalResponse::approved());
                }
                Err(e) => {
                    tracing::warn!("Custom approval handler error: {}", e);
                    let _ = response_tx.send(super::run_context::ApprovalResponse::approved());
                }
            }
        }
    });
}
```

- [ ] **Step 6: Export spawn_custom_approval_handler**

Add to `crates/vol-llm-agent/src/react/mod.rs`:

```rust
pub use hitl::spawn_custom_approval_handler;
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p vol-llm-agent`
Expected: Compiles successfully

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent/src/react/hitl.rs crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/mod.rs
git commit -m "feat: add pluggable ApprovalHandler to AgentConfig for custom approval UIs"
```

---

### Task 2: Wire ApprovalHandler through CodingAgentConfig

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs`
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`

- [ ] **Step 1: Add fields to CodingAgentConfig**

In `crates/vol-llm-agents/src/coding/config.rs`, add to `CodingAgentConfig` struct (after `session` field):

```rust
    /// Custom approval handler for TUI/HTTP-based approval flows.
    pub approval_handler: Option<vol_llm_agent::react::BoxedApprovalHandler>,
```

Add to `Debug` impl (line 63-70), after the session field:

```rust
            .field("approval_handler", &"<ApprovalHandler>")
```

Add to `Default` impl (line 74-92):

```rust
            approval_handler: None,
```

- [ ] **Step 2: Wire through in CodingAgent::run()**

In `crates/vol-llm-agents/src/coding/agent.rs`, modify the `AgentConfig` construction (around line 202):

```rust
        let agent_config = AgentConfig {
            plugin_registry: self.config.plugin_registry.clone(),
            unsafe_mode: self.config.unsafe_mode,
            approval_handler: self.config.approval_handler.clone(),
            ..state.agent_config.clone()
        };
```

- [ ] **Step 3: Add builder method**

In `crates/vol-llm-agents/src/coding/agent.rs`, add to `CodingAgentBuilder` after `hitl_enabled`:

```rust
    pub fn unsafe_mode(mut self, enabled: bool) -> Self {
        self.config.unsafe_mode = enabled;
        self
    }

    pub fn approval_handler(mut self, handler: vol_llm_agent::react::BoxedApprovalHandler) -> Self {
        self.config.approval_handler = Some(handler);
        self
    }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/config.rs crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: wire ApprovalHandler and unsafe_mode through CodingAgentConfig"
```

---

### Task 3: Add TuiApprovalHandler and AppState fields

**Files:**
- Modify: `crates/vol-llm-tui/src/app.rs`
- Create: `crates/vol-llm-tui/src/approval.rs`

- [ ] **Step 1: Create approval.rs with TuiApprovalHandler**

Create `crates/vol-llm-tui/src/approval.rs`:

```rust
//! TUI approval handler — shows approval requests in the ratatui UI.

use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use vol_llm_agent::react::{ApprovalHandler, ApprovalRequest, ApprovalResponse, ApprovalError, BoxedApprovalHandler};

/// Shared state for pending approval requests in the TUI.
#[derive(Clone)]
pub struct ApprovalState {
    /// Current pending approval request.
    pub pending: Arc<Mutex<Option<PendingApproval>>>,
    /// Notifier signaled when response is ready.
    pub notify: Arc<Notify>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Create a boxed approval handler that uses this state.
    pub fn into_handler(self) -> BoxedApprovalHandler {
        BoxedApprovalHandler::new(TuiApprovalHandler { state: self })
    }
}

/// Pending approval request awaiting user input.
pub struct PendingApproval {
    pub tool_name: String,
    pub reason: String,
    pub arguments: String,
    /// Set this to send the response back to the agent.
    pub response_tx: tokio::sync::oneshot::Sender<ApprovalResponse>,
}

/// Approval handler that stores the request in shared state and waits for UI response.
pub struct TuiApprovalHandler {
    pub state: ApprovalState,
}

#[async_trait::async_trait]
impl ApprovalHandler for TuiApprovalHandler {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        // Store the pending request
        {
            let mut pending = self.state.pending.lock().await;
            *pending = Some(PendingApproval {
                tool_name: request.tool_name.clone(),
                reason: request.reason.clone(),
                arguments: serde_json::to_string_pretty(&request.metadata)
                    .unwrap_or_default(),
                response_tx: tokio::sync::oneshot::channel().1, // We'll use a different approach
            });
        }

        // Wait for keyboard handler to respond
        // Actually, we need a different approach — the oneshot sender comes FROM the agent,
        // not from us. Let me reconsider.

        // The approval channel gives us: (ApprovalRequest, oneshot::Sender<ApprovalResponse>)
        // The ApprovalHandler::request_approval only gets the ApprovalRequest.
        // We need to somehow send back the ApprovalResponse.
        // Since the handler doesn't receive the oneshot sender, we need to store it separately.

        // REVISED: We'll use a shared Arc<Mutex<Option<ApprovalResponse>>> to communicate.
        // The keyboard handler writes the response, we poll until it appears.

        // Wait for response
        self.state.notify.notified().await;

        // Read response
        let pending = self.state.pending.lock().await;
        // ... need to get response from somewhere
        Ok(None)
    }
}
```

Wait — the `ApprovalHandler::request_approval` only receives the `ApprovalRequest`, not the `oneshot::Sender`. The sender is part of the internal channel. We can't use this trait as designed.

Let me reconsider the entire approach. The `spawn_custom_approval_handler` receives `(ApprovalRequest, oneshot::Sender)` pairs. It calls `handler.request_approval(request)` and sends the result through the oneshot. But the handler itself doesn't have access to the oneshot sender.

For the TUI, we need the handler to:
1. Store the request in AppState
2. Wait for keyboard input to fill in a response
3. Return the response from `request_approval()`

The simplest way: use a shared `Arc<Mutex<Option<(bool, Option<String>)>>>` for the response. The handler stores the request, then polls the shared state until the keyboard handler writes the response.

Let me rewrite:

```rust
//! TUI approval handler — shows approval requests in the ratatui UI.

use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use vol_llm_agent::react::{ApprovalHandler, ApprovalRequest, ApprovalResponse, ApprovalError, BoxedApprovalHandler};

/// Shared state for pending approval requests in the TUI.
#[derive(Clone)]
pub struct ApprovalState {
    /// Current pending tool name (for display).
    pub tool_name: Arc<Mutex<Option<String>>>,
    /// Current pending reason (for display).
    pub reason: Arc<Mutex<Option<String>>>,
    /// Current pending arguments preview (for display).
    pub arguments: Arc<Mutex<Option<String>>>,
    /// Response to be set by keyboard handler: (approved, reason).
    pub response: Arc<Mutex<Option<(bool, Option<String>)>>>,
    /// Notifier signaled when response is set by keyboard handler.
    pub notify: Arc<Notify>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self {
            tool_name: Arc::new(Mutex::new(None)),
            reason: Arc::new(Mutex::new(None)),
            arguments: Arc::new(Mutex::new(None)),
            response: Arc::new(Mutex::new(None)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn into_handler(self) -> BoxedApprovalHandler {
        BoxedApprovalHandler::new(TuiApprovalHandler { state: self.clone() })
    }

    /// Check if there's a pending approval request.
    pub async fn is_pending(&self) -> bool {
        self.tool_name.lock().await.is_some()
    }

    /// Clear the pending state after response is sent.
    pub async fn clear(&self) {
        *self.tool_name.lock().await = None;
        *self.reason.lock().await = None;
        *self.arguments.lock().await = None;
        *self.response.lock().await = None;
    }
}

/// Approval handler that stores the request and waits for UI response.
pub struct TuiApprovalHandler {
    state: ApprovalState,
}

#[async_trait::async_trait]
impl ApprovalHandler for TuiApprovalHandler {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        // Store the pending request for UI display
        *self.state.tool_name.lock().await = Some(request.tool_name.clone());
        *self.state.reason.lock().await = Some(request.reason.clone());
        *self.state.arguments.lock().await = Some(
            serde_json::to_string_pretty(&request.metadata)
                .unwrap_or_default(),
        );

        // Wait for keyboard handler to set the response
        self.state.notify.notified().await;

        // Read and return the response
        let resp = self.state.response.lock().await.take();
        match resp {
            Some((true, _)) => Ok(Some(ApprovalResponse::approved())),
            Some((false, reason)) => Ok(Some(ApprovalResponse::rejected(
                reason.unwrap_or_else(|| "User rejected".to_string()),
            ))),
            None => Ok(None),
        }
    }
}

/// Respond to a pending approval request from keyboard input.
pub async fn respond_to_approval(state: &ApprovalState, approved: bool, reason: Option<String>) {
    *state.response.lock().await = Some((approved, reason));
    state.notify.notify_one();
}
```

- [ ] **Step 2: Update main.rs to use ApprovalState**

In `crates/vol-llm-tui/src/main.rs`, add imports at the top:

```rust
mod approval;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully (may have dead code warnings)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tui/src/approval.rs crates/vol-llm-tui/src/main.rs
git commit -m "feat: add TuiApprovalHandler with shared ApprovalState"
```

---

### Task 4: Wire TUI approval handler into spawn_agent and keyboard

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs` — `spawn_agent`, `handle_key`
- Modify: `crates/vol-llm-tui/src/app.rs` — replace `pending_approval` with `ApprovalState`

- [ ] **Step 1: Simplify AppState fields**

In `crates/vol-llm-tui/src/app.rs`, remove the `pending_approval`, `approval_notify` fields from Task 2 (if already added). Instead, the `ApprovalState` from `approval.rs` will be the single source of truth.

Remove from `AppState`:
```rust
// Remove these if added from Task 2:
// pub pending_approval: Option<PendingApproval>,
// pub approval_notify: Arc<Notify>,
```

Add to `AppState`:
```rust
    /// Approval state shared with the TUI approval handler.
    pub approval_state: approval::ApprovalState,
```

Update `AppState::new()`:
```rust
            approval_state: approval::ApprovalState::new(),
```

Update `reset_for_run()`:
```rust
        self.approval_state.clear().await;
```

But wait — `reset_for_run()` is sync and `clear()` is async. Make it sync:

```rust
    pub fn reset_for_run(&mut self) {
        // ... existing ...
        // Note: approval_state.clear() is async, called separately before run
    }
```

Actually, let's keep it simple. The approval state is cleared at the start of each run in `spawn_agent` before creating the handler:

- [ ] **Step 2: Wire approval handler in spawn_agent**

In `crates/vol-llm-tui/src/main.rs`, modify `spawn_agent`:

```rust
fn spawn_agent(
    input: String,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<vol_llm_agents::coding::Session>,
) {
    tokio::spawn(async move {
        // Set running flag and clear approval state
        {
            let mut state = state.lock().await;
            state.is_running = true;
            state.approval_state.clear().await;
        }

        // Configure tools
        let mut tool_config = ToolConfig::new();
        // ... existing tool config ...

        let working_dir = std::env::current_dir().unwrap_or_default();
        let unsafe_mode = {
            let state_guard = state.lock().await;
            state_guard.unsafe_mode
        };

        // Get approval state for handler
        let approval_state = {
            let state_guard = state.lock().await;
            state_guard.approval_state.clone()
        };

        let config = CodingAgentConfig {
            max_iterations: 10,
            working_dir,
            hitl_enabled: !unsafe_mode,
            unsafe_mode,
            approval_handler: if !unsafe_mode {
                Some(approval_state.into_handler())
            } else {
                None
            },
            verbose: false,
            html_report_path: None,
            session: Some(session.clone()),
            tool_config,
            ..Default::default()
        };

        // ... rest of agent creation (same) ...
    });
}
```

- [ ] **Step 3: Add approval keyboard handling in handle_key**

In `crates/vol-llm-tui/src/main.rs`, at the top of `handle_key`:

```rust
fn handle_key(key: KeyEvent, state: &mut AppState) -> KeyAction {
    // Check for pending approval — intercept keys before textarea
    match key.code {
        KeyCode::Char('a') | KeyCode::Char('A')
            if state.pending_approval_key().is_some() =>
        {
            respond_approval(state, true, None);
            return KeyAction::None;
        }
        KeyCode::Char('r') | KeyCode::Char('R')
            if state.pending_approval_key().is_some() =>
        {
            respond_approval(state, false, Some("User rejected".to_string()));
            return KeyAction::None;
        }
        KeyCode::Char('s') | KeyCode::Char('S')
            if state.pending_approval_key().is_some() =>
        {
            respond_approval(state, false, Some("User stopped execution".to_string()));
            return KeyAction::None;
        }
        _ => {}
    }

    // ... existing key handling ...
```

Add helper function to `AppState` in `app.rs`:

```rust
impl AppState {
    // ... existing methods ...

    /// Check if there's a pending approval request (returns tool name).
    pub async fn pending_approval_key(&self) -> Option<String> {
        if self.approval_state.is_pending().await {
            self.approval_state.tool_name.lock().await.clone()
        } else {
            None
        }
    }
}
```

And add `respond_approval` function in `main.rs`:

```rust
fn respond_approval(state: &mut AppState, approved: bool, reason: Option<String>) {
    // Use tokio::spawn since we're in a sync context but need async call
    let approval_state = state.approval_state.clone();
    tokio::spawn(async move {
        approval::respond_to_approval(&approval_state, approved, reason).await;
    });
}
```

Wait — `tokio::spawn` from within the event loop's sync `handle_key` won't work because the spawned task won't run until we yield back to the select loop. We need to set the response synchronously.

Let me use a blocking-friendly approach. `ApprovalState` should have a sync version of `respond_to_approval`:

Actually — the `ApprovalState.response` is an `Arc<Mutex<...>>`. In the sync `handle_key`, we can't `.await`. But we can use `try_lock()`:

```rust
fn respond_approval(state: &mut AppState, approved: bool, reason: Option<String>) {
    if let Ok(mut response) = state.approval_state.response.try_lock() {
        *response = Some((approved, reason));
        state.approval_state.notify.notify_one();
    }
}
```

This works because the tokio `Mutex`'s `try_lock()` doesn't require an async context — it just returns an error if already locked (which it shouldn't be during keyboard handling).

- [ ] **Step 4: Add has_pending_approval() sync method**

The `is_pending()` method is async because it locks. We need a sync version for `handle_key`:

```rust
impl ApprovalState {
    /// Sync check if there's a pending approval request.
    pub fn has_pending_approval(&self) -> bool {
        self.tool_name.try_lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }
}
```

Then in `handle_key`:

```rust
fn handle_key(key: KeyEvent, state: &mut AppState) -> KeyAction {
    // Handle approval response keys
    if state.approval_state.has_pending_approval() {
        match key.code {
            KeyCode::Char('a') | KeyCode::Char('A') => {
                respond_approval(state, true, None);
                return KeyAction::None;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                respond_approval(state, false, Some("User rejected".to_string()));
                return KeyAction::None;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                respond_approval(state, false, Some("User stopped execution".to_string()));
                return KeyAction::None;
            }
            _ => {}
        }
    }

    // ... rest of key handling ...
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/app.rs crates/vol-llm-tui/src/main.rs
git commit -m "feat: wire TUI approval handler into spawn_agent and keyboard"
```

---

### Task 5: Create approval banner UI widget

**Files:**
- Create: `crates/vol-llm-tui/src/ui/approval_banner.rs`
- Modify: `crates/vol-llm-tui/src/ui/mod.rs` — export
- Modify: `crates/vol-llm-tui/src/ui/conversation.rs` — insert banner

- [ ] **Step 1: Create approval_banner.rs**

Create `crates/vol-llm-tui/src/ui/approval_banner.rs`:

```rust
//! Approval banner widget — displayed in conversation when a tool requires HITL approval.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the approval banner in the conversation panel.
/// Returns true if a banner was rendered.
pub fn render_approval_banner(frame: &mut Frame, area: Rect, state: &AppState) -> bool {
    if !state.approval_state.has_pending_approval() {
        return false;
    }

    // Get the tool name and reason for display
    let tool_name = state.approval_state.tool_name
        .try_lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let reason = state.approval_state.reason
        .try_lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "".to_string());

    let banner_height = 5u16;
    if area.height < banner_height {
        return false;
    }

    // Position banner near the bottom of the visible area
    let banner_area = Rect {
        x: area.x,
        y: area.y + area.height - banner_height,
        width: area.width,
        height: banner_height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Approval Required ")
        .style(Style::default().fg(Color::Yellow));

    let inner = block.inner(banner_area);
    frame.render_widget(block, banner_area);

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(Color::Yellow)),
            Span::styled(&tool_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(format!("  {}", reason), Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" [A] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Approve   ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [R] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Reject   ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [S] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Stop", Style::default().fg(Color::DarkGray)),
        ]),
    ]);

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);

    true
}
```

- [ ] **Step 2: Export from mod.rs**

Add to `crates/vol-llm-tui/src/ui/mod.rs`:

```rust
mod approval_banner;
pub use approval_banner::render_approval_banner;
```

- [ ] **Step 3: Insert banner into conversation rendering**

In `crates/vol-llm-tui/src/ui/conversation.rs`, modify `render_conversation`:

```rust
pub fn render_conversation(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Conversation ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.conversation.is_empty() && !state.approval_state.has_pending_approval() {
        let empty = Paragraph::new("No messages yet. Type a query and press Ctrl+Enter.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let lines = build_conversation_lines(state);
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph.scroll((state.conversation_scroll, 0)), inner);

    // Render approval banner overlay
    render_approval_banner(frame, inner, state);
}
```

- [ ] **Step 4: Update input area hints during approval**

In `crates/vol-llm-tui/src/ui/input_area.rs`, modify the hint paragraph section:

```rust
    let hint = if state.approval_state.has_pending_approval() {
        Line::from(vec![
            Span::styled(" [A] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Approve  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [R] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Reject  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [S] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Stop", Style::default().fg(Color::DarkGray)),
        ])
    } else if state.is_running {
        Line::from(vec![
            Span::styled(
                " Running... (input disabled) ",
                Style::default().fg(Color::Yellow),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Ctrl+Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Esc ", Style::default().fg(Color::Blue)),
            Span::styled("Clear  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+Q ", Style::default().fg(Color::Red)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ])
    };
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/ui/approval_banner.rs crates/vol-llm-tui/src/ui/mod.rs crates/vol-llm-tui/src/ui/conversation.rs crates/vol-llm-tui/src/ui/input_area.rs
git commit -m "feat: add approval banner widget to conversation panel"
```

---

### Task 6: Unsafe mode UI — status bar badge and /unsafe command

**Files:**
- Modify: `crates/vol-llm-tui/src/app.rs` — `unsafe_mode` field
- Modify: `crates/vol-llm-tui/src/main.rs` — `/unsafe` command, `unsafe_mode` in spawn_agent
- Modify: `crates/vol-llm-tui/src/ui/status_bar.rs` — UNSAFE badge
- Modify: `crates/vol-llm-tui/src/ui/input_area.rs` — unsafe mode hint

- [ ] **Step 1: Add unsafe_mode to AppState**

In `crates/vol-llm-tui/src/app.rs`, add to `AppState`:

```rust
    /// Whether unsafe mode is active (auto-approve all tool approvals).
    pub unsafe_mode: bool,
```

Initialize in `new()`:
```rust
            unsafe_mode: false,
```

- [ ] **Step 2: Add /unsafe command in handle_key**

In `crates/vol-llm-tui/src/main.rs`, add before the "All other keys" fallback:

```rust
        // Unsafe mode toggle
        (_, KeyCode::Char('u')) if key.modifiers == KeyModifiers::CONTROL => {
            state.unsafe_mode = !state.unsafe_mode;
            state.conversation.push(ConversationEntry::AgentAnswer {
                text: if state.unsafe_mode {
                    "Unsafe mode enabled — all tool approvals auto-approved".to_string()
                } else {
                    "Unsafe mode disabled — HITL approval required for dangerous tools".to_string()
                },
            });
            KeyAction::None
        }
```

- [ ] **Step 3: Use unsafe_mode in spawn_agent**

Already handled in Task 4 Step 2 — `spawn_agent` reads `state.unsafe_mode` and passes it to `CodingAgentConfig`.

- [ ] **Step 4: Add UNSAFE badge to status bar**

In `crates/vol-llm-tui/src/ui/status_bar.rs`, modify the status text:

```rust
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let elapsed = state.run_start
        .map(|s| s.elapsed())
        .unwrap_or_default();
    let elapsed_secs = elapsed.as_secs();
    let time_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);

    let mut status_parts = Vec::new();
    if state.is_running {
        status_parts.push("Running");
    } else {
        status_parts.push("Idle");
    }
    if state.unsafe_mode {
        status_parts.push("UNSAFE");
    }
    let status = status_parts.join(" · ");

    let prefix = if state.exiting { "QUITTING · " } else { "" };
    let unsafe_prefix = if state.unsafe_mode { "⚠ " } else { "" };
    let text = format!(
        " {}Session: {}{} │ Run: {} │ Iter: {} │ Tools: {} │ Time: {} │ {}",
        unsafe_prefix,
        prefix,
        state.session_id,
        state.run_count,
        state.iteration,
        state.tool_call_count,
        time_str,
        status,
    );

    let status_style = if state.unsafe_mode {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };

    let paragraph = Paragraph::new(text)
        .style(status_style)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" vol-llm-tui ")
            .style(Style::default().fg(Color::Cyan)));

    frame.render_widget(paragraph, area);
}
```

- [ ] **Step 5: Add unsafe mode hint in input area**

In `crates/vol-llm-tui/src/ui/input_area.rs`, when not running and no approval pending but unsafe_mode is on:

```rust
    } else if state.unsafe_mode {
        Line::from(vec![
            Span::styled(" Ctrl+Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+U ", Style::default().fg(Color::Red)),
            Span::styled("Unsafe  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+Q ", Style::default().fg(Color::Red)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ])
    } else {
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-tui/src/app.rs crates/vol-llm-tui/src/main.rs crates/vol-llm-tui/src/ui/status_bar.rs crates/vol-llm-tui/src/ui/input_area.rs
git commit -m "feat: add unsafe mode toggle with status bar badge and input hints"
```

---

### Task 7: Full workspace verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: All crates compile

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace --lib`
Expected: All tests pass (except pre-existing vol-llm-provider failure)

- [ ] **Step 3: Commit if needed**

```bash
git add -A
git commit -m "fix: address review feedback from workspace verification"
```

---

## Summary

| Task | Files Changed | Purpose |
|------|---------------|---------|
| 1 | `vol-llm-agent/src/react/hitl.rs`, `agent.rs`, `mod.rs` | Add `ApprovalHandler` trait + `AgentConfig` integration |
| 2 | `vol-llm-agents/src/coding/config.rs`, `agent.rs` | Wire through `CodingAgentConfig` |
| 3 | `vol-llm-tui/src/approval.rs` | Create `TuiApprovalHandler` + `ApprovalState` |
| 4 | `vol-llm-tui/src/app.rs`, `main.rs` | Wire handler into spawn_agent and keyboard |
| 5 | `vol-llm-tui/src/ui/approval_banner.rs`, `mod.rs`, `conversation.rs`, `input_area.rs` | Render approval banner in UI |
| 6 | `vol-llm-tui/src/app.rs`, `main.rs`, `status_bar.rs`, `input_area.rs` | Unsafe mode toggle with visual feedback |
| 7 | Full workspace | Verify all compiles, tests pass |
