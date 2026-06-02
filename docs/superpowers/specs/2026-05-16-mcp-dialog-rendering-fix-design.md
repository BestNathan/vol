# MCP Dialog Rendering Fix

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix MCP ToolCallDialog, ResourceViewer, and PromptViewer dialogs not appearing — they render inside nested overflow containers which clip `position: fixed` elements.

**Architecture:** Lift dialog rendering to the App component level (outside all overflow containers). Have dialog components use `use_context::<AppState>()` instead of receiving it as a prop.

**Tech Stack:** Dioxus 0.6 WASM, Signal<McpState>, context providers

---

## Problem

The `McpPanel` component is nested inside multiple DOM containers with `overflow-hidden` and `overflow-y-auto`. When the dialog components render with `class: "fixed inset-0 ..."`, modern browsers clip the fixed-position element to its nearest scroll ancestor rather than the viewport. The dialog exists in the DOM but is invisible.

```
div.overflow-hidden                    <-- outer
  div.overflow-hidden
    div.overflow-hidden
      McpPanel
        div.overflow-y-auto            <-- scroll container clips fixed elements
          ToolCallDialog (fixed)       <-- CLIPPED / INVISIBLE
```

## Solution

Move dialog rendering outside of all overflow containers to the App root level. Dialog components get `AppState` via `use_context()` instead of as a prop.

---

### Task 1: Add McpState signal at App level

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Add import and signal**

Add import at the top of `app.rs`:
```rust
use crate::web::components::mcp_panel::McpState;
```

Add signal creation alongside the other signals in `App()`:
```rust
let mcp_signal = use_signal(|| McpState::new());
```

Add context provider after the other `use_context_provider` calls:
```rust
use_context_provider(|| mcp_signal);
```

### Task 2: Update dialog components to use context for AppState

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs`

- [ ] **Step 1: Update mcp_tool_dialog.rs**

Replace the component signature and AppState acquisition:
```rust
#[component]
pub fn ToolCallDialog(mut signal: Signal<McpState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();
```

Remove the `app_state: AppState` parameter from the component.

- [ ] **Step 2: Update mcp_resource_viewer.rs**

Same change — replace `app_state: AppState` parameter with `let app_state: AppState = use_context();`

- [ ] **Step 3: Update mcp_prompt_viewer.rs**

Same change — replace `app_state: AppState` parameter with `let app_state: AppState = use_context();`

### Task 3: Render dialogs at App level, remove from McpPanel

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

- [ ] **Step 1: Add dialog imports to app.rs**

```rust
use super::mcp_tool_dialog::ToolCallDialog;
use super::mcp_resource_viewer::ResourceViewer;
use super::mcp_prompt_viewer::PromptViewer;
```

- [ ] **Step 2: Add dialog renders in App's rsx**

After `ApprovalDialog {}` inside the outermost div:
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

- [ ] **Step 3: Remove dialog renders from McpPanel**

Remove the conditional dialog renders from `McpPanel`'s rsx body.

- [ ] **Step 4: Remove unused dialog imports from mcp_panel.rs**

Remove the `use super::mcp_tool_dialog::ToolCallDialog;` and similar imports.

- [ ] **Step 5: Remove unused `app_state` variable from McpPanel**

Since McpPanel no longer passes `app_state` to ServerList (it will get it from context too), check if app_state is still needed.

### Task 4: Verify and commit

**Files:** No new files.

- [ ] **Step 1: Verify compilation**

Run: `cargo check -p vol-llm-ui --no-default-features --features web 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs crates/vol-llm-ui/src/web/components/mcp_panel.rs crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs
git commit -m "fix: lift MCP dialog rendering to App level to fix overflow clipping"
```
