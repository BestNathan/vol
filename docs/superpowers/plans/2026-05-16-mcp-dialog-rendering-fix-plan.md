# MCP Dialog Rendering Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix MCP dialogs not appearing by moving their rendering from inside the scroll container to the App root level, outside all `overflow-hidden` containers.

**Architecture:** Lift `McpState` signal to App-level context provider. Dialog components get `AppState` via `use_context()` instead of as a prop. Render dialogs at App root level.

**Tech Stack:** Dioxus 0.6 WASM, Signal<McpState>, context providers, Tailwind CSS

---

### Task 1: Update dialog components to use context for AppState

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs`

- [ ] **Step 1: Update mcp_tool_dialog.rs**

Replace the current component signature. Find this code:
```rust
#[component]
pub fn ToolCallDialog(mut signal: Signal<McpState>, app_state: AppState) -> Element {
    let rpc_client = app_state.rpc_client.clone();
```

Replace with:
```rust
#[component]
pub fn ToolCallDialog(mut signal: Signal<McpState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();
```

Also add the import for AppState at the top of the file:
```rust
use crate::web::components::app::AppState;
```
(This should already exist — verify it's there)

- [ ] **Step 2: Update mcp_resource_viewer.rs**

Find:
```rust
#[component]
pub fn ResourceViewer(mut signal: Signal<McpState>, app_state: AppState) -> Element {
    let rpc_client = app_state.rpc_client.clone();
```

Replace with:
```rust
#[component]
pub fn ResourceViewer(mut signal: Signal<McpState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();
```

- [ ] **Step 3: Update mcp_prompt_viewer.rs**

Find:
```rust
#[component]
pub fn PromptViewer(mut signal: Signal<McpState>, app_state: AppState) -> Element {
    let _rpc_client = app_state.rpc_client.clone();
```

Replace with:
```rust
#[component]
pub fn PromptViewer(mut signal: Signal<McpState>) -> Element {
    let app_state: AppState = use_context();
    let _rpc_client = app_state.rpc_client.clone();
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-ui --no-default-features --features web 2>&1 | tail -5`
Expected: `Finished` with no errors (components still compile with their old props being passed, just unused)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs
git commit -m "refactor: dialog components use use_context() for AppState instead of prop"
```

### Task 2: Lift McpState signal to App level

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Add McpState import**

Find the imports section near line 7:
```rust
use crate::state::{ActiveTab, ApprovalUiState, AgentsState, ConversationState, EventBus, GlobalState, SessionsState, SubscriptionSet, ToolState, UiEvent, UiEventKind, WorkspaceState};
```

After this block, add:
```rust
use crate::web::components::mcp_panel::McpState;
```

- [ ] **Step 2: Add McpState signal creation**

Find the other signal declarations in `App()` around line 123:
```rust
    let sessions_signal = use_signal(|| SessionsState::new());
```

Add after it:
```rust
    let mcp_signal = use_signal(|| McpState::new());
```

- [ ] **Step 3: Add context provider**

Find the block of `use_context_provider` calls around line 288:
```rust
    use_context_provider(|| sessions_signal);
```

Add after it:
```rust
    use_context_provider(|| mcp_signal);
```

- [ ] **Step 4: Add dialog imports and renders**

Find the dialog component imports in the `use super::` block — actually these are in mcp_panel.rs. Add new imports to `app.rs` after the existing `use super::*` imports (after line 21):

```rust
use super::mcp_tool_dialog::ToolCallDialog;
use super::mcp_resource_viewer::ResourceViewer;
use super::mcp_prompt_viewer::PromptViewer;
```

- [ ] **Step 5: Render dialogs at App root level**

Find the App's rsx block around line 302:
```rust
            ApprovalDialog {}
        }
    }
```

Add dialog renders after `ApprovalDialog {}`:
```rust
            ApprovalDialog {}
            if mcp_signal.read().tool_call_dialog.is_some() {
                ToolCallDialog { signal: mcp_signal }
            }
            if mcp_signal.read().resource_viewer.is_some() {
                ResourceViewer { signal: mcp_signal }
            }
            if mcp_signal.read().prompt_viewer.is_some() {
                PromptViewer { signal: mcp_signal }
            }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-ui --no-default-features --features web 2>&1 | tail -5`
Expected: `Finished` with no errors. Warnings about unused `app_state` prop in dialog components may appear — those are expected and will be cleaned up in the next task.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: lift McpState to App context and render dialogs at root level"
```

### Task 3: Remove dialog renders from McpPanel

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

- [ ] **Step 1: Remove dialog imports**

Remove these three lines from the top of the file (lines 7-9):
```rust
use super::mcp_tool_dialog::ToolCallDialog;
use super::mcp_resource_viewer::ResourceViewer;
use super::mcp_prompt_viewer::PromptViewer;
```

- [ ] **Step 2: Remove dialog renders**

Find and remove these lines from the McpPanel rsx body (around lines 91-99):
```rust
                    if signal.read().tool_call_dialog.is_some() {
                        ToolCallDialog { signal, app_state: app_state.clone() }
                    }
                    if signal.read().resource_viewer.is_some() {
                        ResourceViewer { signal, app_state: app_state.clone() }
                    }
                    if signal.read().prompt_viewer.is_some() {
                        PromptViewer { signal, app_state: app_state.clone() }
                    }
```

The McpPanel rsx body should now look like:
```rust
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            if loading {
                div { class: "text-[#666] text-center p-4 text-[13px]", "Loading MCP data..." }
            } else {
                div {
                    // Sub-tab buttons
                    div { class: "flex gap-1 mb-2",
                        McpSubtabButton { signal, subtab: McpSubtab::Servers, label: "Servers" }
                        McpSubtabButton { signal, subtab: McpSubtab::Tools, label: "Tools" }
                        McpSubtabButton { signal, subtab: McpSubtab::Resources, label: "Resources" }
                        McpSubtabButton { signal, subtab: McpSubtab::Prompts, label: "Prompts" }
                    }
                    // Sub-tab content
                    match active {
                        McpSubtab::Servers => rsx! { ServerList { signal, app_state: app_state.clone() } },
                        McpSubtab::Tools => rsx! { ToolList { signal } },
                        McpSubtab::Resources => rsx! { ResourceList { signal } },
                        McpSubtab::Prompts => rsx! { PromptList { signal } },
                    }
                }
            }
        }
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-ui --no-default-features --features web 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "fix: lift MCP dialog rendering to App level to fix overflow clipping"
```

### Task 4: Final verification

**Files:** No code changes.

- [ ] **Step 1: Run full workspace check**

Run: `cargo check --workspace --features web 2>&1 | tail -10`
Expected: Clean compile (warnings OK, no errors)

- [ ] **Step 2: Serve and verify in browser**

Run: `dx serve --package vol-llm-ui --bin vol-llm-ui-web --no-default-features --features web --addr 0.0.0.0`
Expected: Server starts, no compile errors

Manual test steps after serve starts:
1. Open browser to http://localhost:8080
2. Click MCP tab
3. Click Tools sub-tab
4. Click a tool's "Call" button — dialog should appear as an overlay centered on screen with dark backdrop
5. Click "x" to close dialog
6. Click Resources sub-tab, click a resource's "Read" button — dialog should appear
7. Click Prompts sub-tab, click a prompt's "Get" button — dialog should appear
