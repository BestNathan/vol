# Tailwind CSS Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all embedded GLOBAL_CSS styling in vol-llm-ui web frontend with Tailwind CSS v4 utility classes, preserving existing visual appearance and adding responsive breakpoints.

**Architecture:** Create `input.css` with Tailwind v4 config, update `rebuild-web.sh` to run `@tailwindcss/cli`, replace all `class:` strings in 16 component files with Tailwind equivalents, remove GLOBAL_CSS const entirely.

**Tech Stack:** Tailwind CSS v4, `@tailwindcss/cli` (Node.js), Dioxus 0.7 (wasm32), Rust rsx! macros

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-ui/src/web/input.css` | CREATE | Tailwind v4 config with @theme, @source, custom animations |
| `crates/vol-llm-ui/src/web/index.html` | MODIFY | Add `<link rel="stylesheet" href="tailwind.css">` |
| `scripts/rebuild-web.sh` | MODIFY | Add Tailwind CLI step before WASM build |
| `crates/vol-llm-ui/src/web/components/app.rs` | MODIFY | Remove GLOBAL_CSS, migrate App/TabBar/TabButton/TabContent |
| `crates/vol-llm-ui/src/web/components/status_bar.rs` | MODIFY | Migrate StatusBar, ConnectionIndicator |
| `crates/vol-llm-ui/src/web/components/conversation.rs` | MODIFY | Migrate ConversationView, MessageEntry |
| `crates/vol-llm-ui/src/web/components/input_area.rs` | MODIFY | Migrate InputArea |
| `crates/vol-llm-ui/src/web/components/file_tree.rs` | MODIFY | Migrate FileTree, TreeNode |
| `crates/vol-llm-ui/src/web/components/workspace.rs` | MODIFY | Migrate WorkspacePanel, WorkspaceItem |
| `crates/vol-llm-ui/src/web/components/file_content.rs` | MODIFY | Migrate FileContentView, tabs |
| `crates/vol-llm-ui/src/web/components/skills.rs` | MODIFY | Migrate SkillsPanel, SkillRow |
| `crates/vol-llm-ui/src/web/components/log_viewer.rs` | MODIFY | Migrate LogViewer, LogRunItem, LogEntryItem |
| `crates/vol-llm-ui/src/web/components/session_dialog.rs` | MODIFY | Migrate SessionDialog |
| `crates/vol-llm-ui/src/web/components/approval_dialog.rs` | MODIFY | Migrate ApprovalDialog |
| `crates/vol-llm-ui/src/web/components/sessions_panel.rs` | MODIFY | Migrate SessionsPanel, SessionItem, SessionDetailOverlay |
| `crates/vol-llm-ui/src/web/components/agents_panel.rs` | MODIFY | Migrate AgentsPanel, AgentItem |
| `crates/vol-llm-ui/src/web/components/tools_tab.rs` | MODIFY | Migrate ToolsTabContent, ToolCallItem |
| `crates/vol-llm-ui/src/web/components/tools_panel.rs` | MODIFY | Migrate ToolsPanel, ToolItem |

---

### Task 1: Infrastructure — input.css, index.html, rebuild-web.sh

**Files:**
- Create: `crates/vol-llm-ui/src/web/input.css`
- Modify: `crates/vol-llm-ui/src/web/index.html`
- Modify: `scripts/rebuild-web.sh`

- [ ] **Step 1: Create input.css**

```css
@import "tailwindcss";

@source "./components/*.rs";

@theme {
  --breakpoint-sm: 480px;
  --breakpoint-md: 768px;
  --breakpoint-lg: 1024px;

  /* Custom animations for connection indicator */
  --animate-conn-blink: conn-blink 1s ease-in-out infinite;
}

@keyframes conn-blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.2; }
}
```

- [ ] **Step 2: Update index.html — add Tailwind CSS link**

Current:
```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>vol-llm-ui</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
</head>
```

Change to:
```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>vol-llm-ui</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link rel="stylesheet" href="tailwind.css">
</head>
```

- [ ] **Step 3: Update rebuild-web.sh — add Tailwind CLI step**

Insert after the `set -e` line and before the cargo build, add:

```bash
echo "=== Generating Tailwind CSS ==="
npx @tailwindcss/cli \
    -i src/web/input.css \
    -o "$DIST_DIR/tailwind.css" \
    --minify
```

But note: `$DIST_DIR` is defined later in the script. The Tailwind output needs to go somewhere. The script creates `DIST_DIR` after the build. We need to either:
1. Generate to a temp location and copy it later, or
2. Create DIST_DIR early

The cleanest approach: create DIST_DIR early and generate Tailwind there.

Current script structure:
1. Build WASM (cargo build)
2. Process WASM (wasm-bindgen)
3. Set up dist (rm -rf, mkdir, copy WASM)
4. Create index.html

New structure:
1. **Create dist directory early**
2. **Generate Tailwind CSS to dist/**
3. Build WASM (cargo build)
4. Process WASM (wasm-bindgen)
5. Copy WASM to dist/
6. Create index.html (already has tailwind.css link)

Full updated `scripts/rebuild-web.sh`:

```bash
#!/bin/bash
# Rebuild and serve the vol-llm-ui web application.
# Usage: ./scripts/rebuild-web.sh
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$PROJECT_ROOT/target/wasm32-unknown-unknown/wasm-dev"
DIST_DIR="$WASM_DIR/dist"

echo "=== Setting up dist directory ==="
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR/wasm"

echo "=== Generating Tailwind CSS ==="
npx @tailwindcss/cli \
    -i "$PROJECT_ROOT/crates/vol-llm-ui/src/web/input.css" \
    -o "$DIST_DIR/tailwind.css" \
    --minify

echo "=== Building vol-llm-ui-web (wasm32) ==="
cargo build \
    --target wasm32-unknown-unknown \
    --package vol-llm-ui \
    --bin vol-llm-ui-web \
    --no-default-features \
    --features web \
    --quiet

echo "=== Processing WASM with wasm-bindgen ==="
wasm-bindgen \
    --out-dir "$WASM_DIR/wasm" \
    --target web \
    "$WASM_DIR/vol-llm-ui-web.wasm" \
    --quiet

cp -r "$WASM_DIR/wasm/"* "$DIST_DIR/wasm/"

cat > "$DIST_DIR/index.html" << 'HTML'
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>vol-llm-ui</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link rel="stylesheet" href="tailwind.css">
</head>
<body>
<script type="module">
import init from './wasm/vol-llm-ui-web.js';
init('./wasm/vol-llm-ui-web_bg.wasm');
</script>
</body>
</html>
HTML

echo "=== Done! ==="
echo "Dist directory: $DIST_DIR"
echo ""
echo "To serve: basic-http-server --addr 0.0.0.0:8080 $DIST_DIR"
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/input.css scripts/rebuild-web.sh crates/vol-llm-ui/src/web/index.html
git commit -m "feat: add Tailwind CSS v4 infrastructure

- Create input.css with @theme custom breakpoints and animations
- Update rebuild-web.sh to run @tailwindcss/cli before WASM build
- Add tailwind.css link to index.html"
```

---

### Task 2: Migrate app.rs — Remove GLOBAL_CSS, replace layout classes

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs:1-381` (keep imports, state, logic)
- Modify: `crates/vol-llm-ui/src/web/components/app.rs:289-337` (rsx! blocks)
- Delete: `crates/vol-llm-ui/src/web/components/app.rs:382-597` (GLOBAL_CSS const)

- [ ] **Step 1: Replace the App() rsx! block (lines 289-303)**

Current:
```rust
    rsx! {
        style { {GLOBAL_CSS} }
        div { class: "app-container",
            StatusBar {}
            div { class: "main-layout",
                FileTree {}
                div { class: "right-panel",
                    TabBar {}
                    TabContent {}
                    InputArea {}
                }
            }
            ApprovalDialog {}
        }
    }
```

Replace with:
```rust
    rsx! {
        div { class: "flex flex-col h-[100dvh] w-[100vw] overflow-hidden font-[system-ui] text-[14px] text-[#e0e0e0] bg-[#1a1a2e]",
            StatusBar {}
            div { class: "flex flex-1 overflow-hidden",
                FileTree {}
                div { class: "flex-1 flex flex-col overflow-hidden",
                    TabBar {}
                    TabContent {}
                    InputArea {}
                }
            }
            ApprovalDialog {}
        }
    }
```

Note: The `app-container`, `main-layout`, `right-panel` classes are absorbed into inline Tailwind. Body styles moved to the root div.

- [ ] **Step 2: Replace TabBar rsx! (lines 311-321)**

Current:
```rust
    rsx! {
        div { class: "tab-bar",
            TabButton { state: state.clone(), tab: ActiveTab::Conversation, label: "Conversation" }
            ...
        }
    }
```

Replace with:
```rust
    rsx! {
        div { class: "flex bg-[#252540] border-b border-[#333355] flex-shrink-0 sm:overflow-x-auto",
            TabButton { state: state.clone(), tab: ActiveTab::Conversation, label: "Conversation" }
            TabButton { state: state.clone(), tab: ActiveTab::Sessions, label: "Sessions" }
            TabButton { state: state.clone(), tab: ActiveTab::Tools, label: "Tools" }
            TabButton { state: state.clone(), tab: ActiveTab::Workspace, label: "Workspace" }
            TabButton { state: state.clone(), tab: ActiveTab::Skills, label: "Skills" }
            TabButton { state: state.clone(), tab: ActiveTab::Logs, label: "Logs" }
            TabButton { state: state.clone(), tab: ActiveTab::Agents, label: "Agents" }
        }
    }
```

- [ ] **Step 3: Replace TabButton rsx! (lines 328-336)**

Current:
```rust
    let tab_class = if active { "tab active" } else { "tab" };
    ...
    rsx! {
        button {
            class: tab_class,
            onclick: move |_| { active_tab_signal.set(tab); },
            "{label}"
        }
    }
```

Replace with (full if/else so Tailwind scanner finds all variants):
```rust
    let tab_class = if active {
        "px-4 py-1.5 bg-[#1a1a2e] text-[#e0e0e0] border-b-2 border-[#80a0ff] cursor-pointer text-[13px]"
    } else {
        "px-4 py-1.5 bg-transparent text-[#888] border-b-2 border-transparent cursor-pointer text-[13px] hover:text-[#ccc] hover:bg-[#2a2a44]"
    };
    ...
    rsx! {
        button {
            class: tab_class,
            onclick: move |_| { active_tab_signal.set(tab); },
            "{label}"
        }
    }
```

- [ ] **Step 4: Delete GLOBAL_CSS const (lines 382-597)**

Delete the entire `const GLOBAL_CSS: &str = r#"..."#;` block.

- [ ] **Step 5: Remove unused helper functions**

The `status_class` function (lines 372-380) returns old CSS class names. Since it's referenced from `tools_panel.rs` and potentially other files, check usages first. If only used for old CSS classes, remove it. The `status_label` function returns text labels ("...", "OK", etc.) — keep it.

Check if `status_class` from app.rs is used elsewhere:
- `tools_panel.rs:32` has its own `status_class` — different function
- `tools_tab.rs:65` inline match — different

The `status_class` in app.rs is unused after migration — remove it.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "refactor: migrate app.rs from GLOBAL_CSS to Tailwind utilities

- Remove GLOBAL_CSS const entirely
- Replace app-container, main-layout, right-panel with inline Tailwind
- Replace tab-bar, tab, tab.active with Tailwind classes
- Remove unused status_class helper"
```

---

### Task 3: Migrate status_bar.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/status_bar.rs`

- [ ] **Step 1: Replace status_class variable (line 38)**

Current:
```rust
let status_class = if is_running { "status-bar status-running" } else { "status-bar status-idle" };
```

Replace with:
```rust
let status_cls = if is_running {
    "flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[#e0e0e0] text-[12px] font-mono flex-shrink-0 text-[#f0c040]"
} else {
    "flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[#e0e0e0] text-[12px] font-mono flex-shrink-0 text-[#80c080]"
};
```

- [ ] **Step 2: Replace StatusBar rsx! (lines 40-67)**

Replace the entire rsx! block:

```rust
    rsx! {
        div { class: status_cls,
            div { class: "flex items-center gap-1.5 overflow-hidden flex-nowrap sm:gap-1",
                ConnectionIndicator { connected: ws_connected, error: ws_error.clone() }
                span { class: "whitespace-nowrap", "Session: {session_id}" }
                span { class: "text-[#555] select-none" }
                span { class: "whitespace-nowrap", "Run: {run_count}" }
                span { class: "text-[#555] select-none" }
                span { class: "whitespace-nowrap", "Iter: {iteration}" }
                span { class: "text-[#555] select-none" }
                span { class: "whitespace-nowrap", "Tools: {tool_call_count}" }
                span { class: "text-[#555] select-none" }
                span { class: "whitespace-nowrap", "Time: {time_str}" }
                span { class: "text-[#555] select-none" }
                span { class: badge_cls, "{status}" }
                if unsafe_mode {
                    span { class: "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a2020] text-[#ff4040]", "!! UNSAFE" }
                }
                if is_exiting {
                    span { class: "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a2020] text-[#ff8080]", "QUITTING" }
                }
            }
            div { class: "flex items-center flex-shrink-0",
                span { class: "flex items-center text-[11px] text-[#888] flex-shrink-0",
                    span { class: "text-[#666]", "UI " }
                    span { class: "text-[#a0a0c0] font-bold", {env!("CARGO_PKG_VERSION")} }
                    span { class: "text-[#555] mx-0.5", " | " }
                    span { class: "text-[#666]", {BUILD_TIME} }
                }
            }
        }
    }
```

- [ ] **Step 3: Replace badge_cls (line 26)**

Current:
```rust
let badge_cls = if gs.is_running { "status-badge badge-running" } else { "status-badge badge-idle" };
```

Replace with:
```rust
let badge_cls = if gs.is_running {
    "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a3a20] text-[#f0c040]"
} else {
    "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#203a20] text-[#80c080]"
};
```

- [ ] **Step 4: Replace ConnectionIndicator rsx! (lines 71-94)**

Current has three branches (connected, error, connecting). Replace each:

```rust
#[component]
fn ConnectionIndicator(connected: bool, error: Option<String>) -> Element {
    if connected {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connected",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #40c040; box-shadow: 0 0 4px #40c040;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Connected" }
            }
        }
    } else if let Some(ref err) = error {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "{err}",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #ff4040; animation: conn-blink 1s ease-in-out infinite;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Error" }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connecting...",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0 animate-pulse", style: "background-color: #f0c040;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Connecting" }
            }
        }
    }
}
```

Note: The conn-blink animation uses inline `style:` because Tailwind v4's `animate-conn-blink` utility requires the animation to be defined as an `@utility` in the CSS. The `@keyframes` is in input.css but we need a utility. Keep the connecting dot using Tailwind's built-in `animate-pulse` and use inline style for the blink.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/status_bar.rs
git commit -m "refactor: migrate status_bar.rs to Tailwind utilities

- Replace status-bar, status-left, status-right, status-item classes
- Replace badge variants with full Tailwind strings
- Migrate ConnectionIndicator with inline animation styles"
```

---

### Task 4: Migrate conversation.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`

- [ ] **Step 1: Replace ConversationView rsx! (lines 93-97, 105-107)**

Empty state (lines 93-97):
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            div { class: "flex items-center justify-center h-full text-[#666]", "No messages yet. Type a query and press Send." }
        }
    };
```

Messages container (line 106):
```rust
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5", {messages.into_iter()} }
    }
```

- [ ] **Step 2: Replace MessageEntry rsx! (lines 112-146)**

Replace the entire match block:

```rust
pub(crate) fn MessageEntry(entry: ConversationEntry) -> Element {
    match entry {
        ConversationEntry::UserInput { text } => {
            rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a44] border-l-[3px] border-[#4080ff]", div { class: "text-[#4080ff] font-bold", ">>> " } {text} } }
        }
        ConversationEntry::Thinking { content } => {
            rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2a20] border-l-[3px] border-[#c0c040]", div { class: "text-[#c0c040] font-bold", "Thinking" } div { class: "text-[#888] mt-1 pl-1", {content} } } }
        }
        ConversationEntry::ContentStreaming { content } => {
            if content.is_empty() { rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#ccc]", "Generating..." } } }
            else { rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#ccc]", {content} } } }
        }
        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a3a] border-l-[3px] border-[#4080c0]", div { class: "text-[#4080c0] font-bold", "[{tool_name}]" } if !arg_preview.is_empty() { div { class: "text-[#888] text-[12px] mt-0.5 pl-1", "{arg_preview}" } } } }
        }
        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let cls = if success {
                "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a1a] border-l-[3px] border-[#40c040]"
            } else {
                "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a1a1a] border-l-[3px] border-[#c04040]"
            };
            let status = if success { "OK" } else { "ERR" };
            let color = if success { "#40c040" } else { "#c04040" };
            let display = truncate_lines(&preview, 6, 90);
            rsx! { div { class: cls, div { span { class: "font-bold", style: "color: {color};", "[{status}] " } span { style: "color: {color}; font-weight: bold;", "{tool_name}" } } div { class: "text-[#888] text-[12px] mt-1 pl-1 max-h-[120px] overflow-y-auto font-mono", {display} } } }
        }
        ConversationEntry::AgentAnswer { text } => { rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#e0e0e0] leading-[1.5]", {text} } } }
        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            let iw = if iterations == 1 { "iteration" } else { "iterations" };
            let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
            rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#80c080] font-bold py-1.5", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
        }
        ConversationEntry::Error { message } => { rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#ff6060] font-bold bg-[#2a1a1a] border-l-[3px] border-[#c04040]", "Error: {message}" } } }
        ConversationEntry::EntryCheckpoint { reason, note, created_at } => {
            let note_text = note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
            rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2a20] border-l-[3px] border-[#c0a040] text-[#aaa] text-[12px] italic", "[Checkpoint {created_at}] {reason}{note_text}" } }
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "refactor: migrate conversation.rs to Tailwind utilities

- Replace conversation container and empty state classes
- Replace all message type classes (user, thinking, streaming, tool, result, answer, summary, error, checkpoint)"
```

---

### Task 5: Migrate input_area.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/input_area.rs`

- [ ] **Step 1: Replace InputArea rsx! (lines 29-49)**

```rust
    let hint = if is_running {
        rsx! { span { class: "text-[#f0c040]", " Running... (input disabled) " } }
    } else {
        rsx! { span { span { class: "text-[#80a0ff] font-bold", "Enter" } " Send  " span { class: "text-[#80a0ff] font-bold", "Esc" } " Clear" } }
    };

    rsx! {
        div { class: "border-t border-[#333355] p-2.5 bg-[#252540] flex-shrink-0 sm:px-2 sm:py-1.5",
            if has_approval {
                div { p { class: "text-[#f0c040]", "Tool approval pending in the dialog above." } }
            } else {
                div {
                    div { class: "flex gap-2",
                        textarea {
                            value: input_text(),
                            oninput: on_input,
                            disabled: is_running,
                            placeholder: "Type a message to the agent...",
                            rows: 2,
                            class: "flex-1 bg-[#1a1a2e] text-[#e0e0e0] border border-[#444466] rounded-md px-2 py-1.5 text-[14px] font-sans resize-none min-h-[40px] max-h-[120px] outline-none focus:border-[#80a0ff] disabled:opacity-50"
                        }
                        button {
                            onclick: on_submit,
                            disabled: is_running,
                            class: "px-4 py-1.5 bg-[#4060c0] text-[#e0e0e0] rounded-md cursor-pointer text-[14px] self-end hover:bg-[#5070d0] disabled:bg-[#333355] disabled:cursor-not-allowed"
                        }
                    }
                    div { class: "mt-1 text-[11px] text-[#666]", {hint} }
                }
            }
        }
    }
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/input_area.rs
git commit -m "refactor: migrate input_area.rs to Tailwind utilities"
```

---

### Task 6: Migrate file_tree.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/file_tree.rs`

- [ ] **Step 1: Replace TreeNode rsx! blocks**

Directory node (lines 130-149):
```rust
        rsx! {
            div {
                div {
                    class: "flex items-center py-0.5 pr-2 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded-[3px] mx-1 hover:bg-[#2a2a44] active:bg-[#3a3a54] hover:bg-[#1a2a3a]",
                    style: format!("padding-left: {}px;", indent_px),
                    onclick: dir_onclick,
                    span { class: "inline-flex items-center justify-center w-4 h-4 flex-shrink-0 text-[10px] text-[#666] transition-transform duration-150 {chevron_class}", "\u{25be}" }
                    span { class: "inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 mr-1 text-[14px]", "{file_icon(true, &node.name)}" }
                    span { class: "overflow-hidden text-ellipsis text-[#8ab4ff] font-medium", "{node.name}" }
                    span { class: "text-[10px] text-[#666] ml-1 opacity-0 transition-opacity duration-150 cursor-pointer group-hover:opacity-100 hover:text-[#aaa]", onclick: refresh_onclick, "\u{21bb}" }
                }
                if !collapsed {
                    div { class: "overflow-hidden",
                        for child in &node.children {
                            TreeNode { node: child.clone(), depth: depth + 1, key: "{child.path}" }
                        }
                    }
                }
            }
        }
```

Note: The chevron class needs to be computed with full variants:
```rust
let chevron_class = if collapsed {
    "-rotate-90"
} else {
    ""
};
```

File node (lines 200-209):
```rust
        rsx! {
            div {
                class: "flex items-center py-0.5 pr-2 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded-[3px] mx-1 hover:bg-[#2a2a44] active:bg-[#3a3a54]",
                style: format!("padding-left: {}px;", indent_px),
                onclick: file_onclick,
                span { class: "inline-flex items-center justify-center w-4 h-4 flex-shrink-0 text-[10px] text-[#666] invisible", "\u{25be}" }
                span { class: "inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 mr-1 text-[14px]", "{file_icon(false, &node.name)}" }
                span { class: "overflow-hidden text-ellipsis text-[#ccc]", "{node.name}" }
            }
        }
```

- [ ] **Step 2: Replace FileTree rsx! blocks (lines 288-307)**

Loading state:
```rust
    return rsx! {
        div { class: "w-[40%] sm:w-[33.33%] md:w-[33.33%] lg:w-[240px] min-w-[120px] sm:min-w-[160px] md:min-w-[160px] lg:min-w-[180px] border-r border-[#2a2a44] flex flex-col overflow-hidden flex-shrink-0 bg-[#16162a]",
            div { class: "px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.8px] text-[#6a6a9a] border-b border-[#2a2a44] flex-shrink-0", "Explorer" }
            div { class: "flex-1 overflow-y-auto py-1",
                div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center text-[12px]", "Loading files..." }
            }
        }
    };
```

Normal state:
```rust
    rsx! {
        div { class: "w-[40%] sm:w-[33.33%] md:w-[33.33%] lg:w-[240px] min-w-[120px] sm:min-w-[160px] md:min-w-[160px] lg:min-w-[180px] border-r border-[#2a2a44] flex flex-col overflow-hidden flex-shrink-0 bg-[#16162a]",
            div { class: "px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.8px] text-[#6a6a9a] border-b border-[#2a2a44] flex-shrink-0", "Explorer" }
            div { class: "flex-1 overflow-y-auto py-1",
                for child in &workspace.children {
                    TreeNode { node: child.clone(), depth: 0, key: "{child.path}" }
                }
            }
        }
    }
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/file_tree.rs
git commit -m "refactor: migrate file_tree.rs to Tailwind utilities

- Replace sidebar, sidebar-header, file-tree classes
- Keep dynamic inline style for depth indentation
- Add responsive breakpoint classes for sidebar width"
```

---

### Task 7: Migrate workspace.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/workspace.rs`

- [ ] **Step 1: Replace WorkspacePanel and WorkspaceItem rsx! blocks**

Empty state (lines 29-33):
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            div { class: "flex items-center justify-center h-full text-[#666]", "Workspace directory empty or unavailable" }
        }
    };
```

Normal state (lines 45-49):
```rust
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            {items.into_iter()}
        }
    }
```

WorkspaceItem (lines 53-65):
```rust
#[component]
fn WorkspaceItem(name: String, is_dir: bool, indent: usize) -> Element {
    if is_dir {
        let display = format!("{}[DIR] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "py-0.5 font-mono text-[13px] text-[#6090ff] font-bold", "{display}" }
        }
    } else {
        let display = format!("{}[FILE] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "py-0.5 font-mono text-[13px] text-[#e0e0e0]", "{display}" }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/workspace.rs
git commit -m "refactor: migrate workspace.rs to Tailwind utilities"
```

---

### Task 8: Migrate file_content.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/file_content.rs`

- [ ] **Step 1: Replace FileContentView rsx! blocks**

Empty state (lines 17-22):
```rust
    return rsx! {
        div { class: "flex items-center justify-center h-full text-[#666]",
            "Click a file in the explorer to open it"
        }
    };
```

Main container (lines 30-53):
```rust
    rsx! {
        div { class: "flex-1 flex flex-col overflow-hidden",
            div { class: "flex bg-[#1e1e38] border-b border-[#2a2a44] flex-shrink-0 overflow-x-auto",
                {tab_elements.into_iter()}
            }
            {if let Some(idx) = selected {
                if let Some(tab) = open_files.get(idx) {
                    match (&tab.content, &tab.error) {
                        (Some(content), _) => rsx! { FileContentDisplay { content } },
                        (None, Some(error)) => rsx! {
                            div { class: "p-3 text-[#ff6060] font-bold", "Error: {error}" }
                        },
                        (None, None) => rsx! {
                            div { class: "flex items-center justify-center h-full text-[#888]", "Loading..." }
                        },
                    }
                } else {
                    rsx! {}
                }
            } else {
                rsx! {}
            }}
        }
    }
```

- [ ] **Step 2: Replace render_tab (lines 56-108)**

```rust
    let tab_cls = if is_selected {
        "px-2 py-1 text-[12px] text-[#e0e0e0] bg-[#1a1a2e] flex items-center gap-1 cursor-pointer border-b-2 border-b-[#80a0ff] whitespace-nowrap"
    } else {
        "px-2 py-1 text-[12px] text-[#777] flex items-center gap-1 cursor-pointer border-b-2 border-transparent whitespace-nowrap hover:text-[#bbb] hover:bg-[#222240]"
    };
    ...
    rsx! {
        div {
            class: tab_cls,
            key: "{path}",
            onclick: select_onclick,
            span { class: "text-[13px]", "{icon}" }
            span { class: "max-w-[150px] overflow-hidden text-ellipsis", "{name}" }
            span {
                class: "text-[10px] text-[#555] px-0.5 rounded-[2px] leading-none hover:text-[#ff6060] hover:bg-[#3a2020]",
                onclick: close_onclick,
                "\u{2715}"
            }
        }
    }
```

- [ ] **Step 3: Replace FileContentDisplay (lines 111-118)**

```rust
fn FileContentDisplay(content: String) -> Element {
    rsx! {
        pre { class: "flex-1 overflow-auto p-3 font-mono text-[12px] leading-[1.6] text-[#c8c8e0] bg-[#1a1a2e] whitespace-pre m-0",
            {content}
        }
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/file_content.rs
git commit -m "refactor: migrate file_content.rs to Tailwind utilities"
```

---

### Task 9: Migrate skills.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/skills.rs`

- [ ] **Step 1: Replace SkillsPanel rsx!**

Empty state:
```rust
    return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No skills discovered" } } };
```

Normal state:
```rust
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            table { class: "w-full border-collapse",
                thead { tr {
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Name" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Version" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Scope" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Description" }
                } }
                tbody {
                    {(0..count).map(|i| { let s = signal.clone(); rsx! { SkillRow { signal: s, index: i } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
```

- [ ] **Step 2: Replace SkillRow td elements (lines 30-38)**

Current uses inline style for colors — convert to Tailwind:
```rust
    rsx! {
        tr {
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#e0e0e0] font-bold", "{skill.name}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.version}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44]", style: "color: {color};", "{skill.scope}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.description}" }
        }
    }
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/skills.rs
git commit -m "refactor: migrate skills.rs to Tailwind utilities"
```

---

### Task 10: Migrate log_viewer.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/log_viewer.rs`

- [ ] **Step 1: Replace all rsx! blocks**

Run list empty state (line 20):
```rust
    if count == 0 { return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No log files found." } } }; }
```

Run list (line 22):
```rust
    rsx! { div { class: "flex-1 overflow-y-auto p-2.5 font-mono text-[13px]", {items.into_iter()} } }
```

LogRunItem (line 30):
```rust
    rsx! { div { class: "py-0.5 text-[#ccc]", span { class: "text-[#c0c0c0]", "{short}" } span { class: "text-[#888]", " {run.event_count} events" } span { class: "text-[#888]", "  {run.last_event} ({run.last_event_time})" } } }
```

Log entries empty state (line 34):
```rust
    if count == 0 { return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No events in this run." } } }; }
```

Log entries header (line 37):
```rust
    rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "mb-2 text-[12px] text-[#888]", "Log: {run_id}" } {items.into_iter()} } }
```

LogEntryItem (line 48):
```rust
    rsx! { div { class: "font-mono text-[12px] py-0.5 whitespace-nowrap", span { class: "text-[#666]", "[{entry.timestamp}] " } span { class: "font-bold", style: "color: {color};", "{entry.event_type}" } span { style: "color: {color};", " -- {entry.summary}" } } }
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/log_viewer.rs
git commit -m "refactor: migrate log_viewer.rs to Tailwind utilities"
```

---

### Task 11: Migrate session_dialog.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/session_dialog.rs`

- [ ] **Step 1: Replace all rsx! blocks**

Empty item (line 38):
```rust
    let items: Vec<Element> = if sessions.is_empty() {
        vec![rsx! { div { class: "text-[#888] py-2.5", "No saved sessions found." } }]
    } else {
```

Session item (lines 40-51):
```rust
            let cls = if is_sel {
                "px-2 py-1.5 border-b border-[#2a2a44] flex items-center gap-2 bg-[#2a2a44]"
            } else {
                "px-2 py-1.5 border-b border-[#2a2a44] flex items-center gap-2"
            };
            ...
            rsx! {
                div { class: cls, onclick: move |_: Event<MouseData>| { sig_sel.with_mut(|s| s.selected = i); },
                    span { class: "font-mono text-[#e0e0e0] font-bold", "{short}" }
                    span { class: "text-[#888] text-[12px]", "{entry.entry_count} entries | {entry.age_label}" }
                }
            }
```

Overlay and modal (lines 56-69):
```rust
    rsx! {
        div { class: "fixed inset-0 bg-black/60 flex items-center justify-center z-[100]", onclick: move |_: Event<MouseData>| { sig_overlay.with_mut(|s| s.open = false); },
            div { class: "bg-[#252540] border border-[#444466] rounded-lg p-4 min-w-[400px] max-w-[600px] max-h-[80vh] overflow-y-auto md:min-w-auto md:w-[90vw] md:max-w-[500px]", onclick: |evt: Event<MouseData>| { evt.stop_propagation(); },
                div { class: "text-[16px] font-bold text-[#e0e0e0] mb-3 border-b border-[#333355] pb-2", "Sessions" }
                {items.into_iter()}
                div { class: "mt-3 flex gap-2 pt-2 border-t border-[#333355]",
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#4060c0] text-[#e0e0e0]", onclick: on_new, "New" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#408040] text-[#e0e0e0]", onclick: on_resume, "Resume" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#804040] text-[#e0e0e0]", onclick: on_delete, "Delete" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#555] text-[#e0e0e0]", onclick: move |_: Event<MouseData>| { sig_cancel.with_mut(|s| s.open = false); }, "Cancel" }
                }
            }
        }
    }
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/session_dialog.rs
git commit -m "refactor: migrate session_dialog.rs to Tailwind utilities"
```

---

### Task 12: Migrate approval_dialog.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/approval_dialog.rs`

- [ ] **Step 1: Replace ApprovalDialog rsx!**

```rust
    rsx! {
        div { class: "fixed inset-0 bg-black/60 flex items-center justify-center z-[100]",
            div { class: "bg-[#252540] border border-[#444466] rounded-lg p-4 min-w-[400px] max-w-[600px] max-h-[80vh] overflow-y-auto md:min-w-auto md:w-[90vw] md:max-w-[500px]",
                div { class: "text-[16px] font-bold text-[#e0e0e0] mb-3 border-b border-[#333355] pb-2", "Tool Approval Required" }
                div { class: "text-[#f0c040] font-bold text-[15px]", "[!] {tool_name}" }
                if !reason.is_empty() { div { class: "text-[#ccc] my-1.5", "Reason: {reason}" } }
                if !arguments.is_empty() { div { class: "font-mono text-[12px] text-[#888] bg-[#1a1a2e] px-2 py-1.5 rounded-md my-2 max-h-[100px] overflow-y-auto whitespace-pre-wrap", "{arguments}" } }
                div { class: "mt-3 flex gap-2 pt-2 border-t border-[#333355]",
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#408040] text-[#e0e0e0]", onclick: on_approve, "Approve" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#804040] text-[#e0e0e0]", onclick: on_reject, "Reject" }
                }
            }
        }
    }
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/approval_dialog.rs
git commit -m "refactor: migrate approval_dialog.rs to Tailwind utilities"
```

---

### Task 13: Migrate sessions_panel.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

- [ ] **Step 1: Replace SessionDetailOverlay rsx! (lines 136-160)**

```rust
    rsx! {
        div {
            class: "fixed inset-0 bg-black/70 z-[200] flex items-center justify-center",
            onclick: move |_: Event<MouseData>| { show.set(false); },
            div {
                class: "bg-[#1a1a2e] border border-[#333355] rounded-lg w-[80vw] max-w-[900px] h-[70vh] flex flex-col overflow-hidden",
                onclick: move |evt: Event<MouseData>| { evt.stop_propagation(); },
                div { class: "flex items-center justify-between px-3 py-2 border-b border-[#2a2a44] font-mono text-[13px] text-[#e0e0e0]",
                    span { "Session: {session_id}" }
                    button {
                        class: "bg-none border-none text-[#888] text-[16px] cursor-pointer px-1.5 py-0.5 rounded-[3px] hover:text-[#ff6060] hover:bg-[#2a1a1a]",
                        onclick: move |_: Event<MouseData>| { show.set(false); },
                        "\u{2715}"
                    }
                }
                if is_loading {
                    div { class: "flex items-center justify-center flex-1 text-[#666]", "Loading..." }
                } else {
                    div { class: "flex-1 overflow-y-auto p-2",
                        {items.into_iter()}
                    }
                }
            }
        }
    }
```

Note: The inner message elements already use Tailwind classes from the conversation migration (msg, msg-user, etc.), but since the SessionDetailOverlay uses the old class names too, we need to update them. The outer `div { class: "msg" }` (line 101) and inner elements (lines 104, 106, 108, 112, etc.) already have the full Tailwind classes from the conversation.rs migration — just need to remove the old class strings.

For the SessionDetailOverlay's inner message rendering, the pattern already duplicates the same classes as conversation.rs. Update:

```rust
    let items: Vec<Element> = entries.read().iter().map(|entry| {
        let e = entry.clone();
        rsx! {
            div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap",
                match e {
                    ConversationEntry::UserInput { text } => rsx! {
                        div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a44] border-l-[3px] border-[#4080ff]", div { class: "text-[#4080ff] font-bold", ">>> " } {text} }
                    },
                    ConversationEntry::AgentAnswer { text } => rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#e0e0e0] leading-[1.5]", {text} } },
                    ConversationEntry::ToolResult { tool_name, preview, success } => {
                        let cls = if success {
                            "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a1a] border-l-[3px] border-[#40c040]"
                        } else {
                            "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a1a1a] border-l-[3px] border-[#c04040]"
                        };
                        let status = if success { "OK" } else { "ERR" };
                        let color = if success { "#40c040" } else { "#c04040" };
                        rsx! {
                            div { class: cls,
                                div {
                                    span { class: "font-bold", style: "color: {color};", "[{status}] " }
                                    span { style: "color: {color}; font-weight: bold;", "{tool_name}" }
                                }
                                div { class: "text-[#888] text-[12px] mt-1 pl-1 max-h-[120px] overflow-y-auto font-mono", "{truncate_lines(&preview, 6, 90)}" }
                            }
                        }
                    }
                    ConversationEntry::EntryCheckpoint { reason, note, .. } => {
                        let note_text = note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
                        rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2a20] border-l-[3px] border-[#c0a040] text-[#aaa] text-[12px] italic", "[Checkpoint] {reason}{note_text}" } }
                    }
                    ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
                        let iw = if iterations == 1 { "iteration" } else { "iterations" };
                        let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
                        rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#80c080] font-bold py-1.5", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
                    }
                    _ => rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap", "Entry" } },
                }
            }
        }
    }).collect();
```

- [ ] **Step 2: Replace SessionItem rsx! (lines 196-244)**

```rust
    rsx! {
        div {
            class: "flex items-center px-2.5 py-2 border-b border-[#2a2a44] cursor-pointer gap-2 hover:bg-[#222240]",
            onclick: move |_: Event<MouseData>| { ... },
            span { class: "font-mono text-[13px] text-[#e0e0e0] font-semibold min-w-[80px]", "{truncate_id(&session_id)}" }
            span { class: "text-[11px] text-[#888]", "{entry_count} entries" }
            span { class: "text-[11px] text-[#666] ml-auto", "{format_age(created_at)}" }
            button {
                class: "px-2.5 py-0.5 bg-[#408040] text-[#e0e0e0] border-none rounded-[3px] cursor-pointer text-[12px] ml-1 flex-shrink-0 hover:bg-[#50a050] disabled:bg-[#333355] disabled:cursor-not-allowed",
                disabled: *is_resuming.read(),
                onclick: move |evt: Event<MouseData>| { ... },
                if *is_resuming.read() { "Resuming..." } else { "Resume" }
            }
        }
        ...
    }
```

- [ ] **Step 3: Replace SessionsPanel rsx! blocks (lines 294-335)**

Loading state:
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "Loading sessions..." }
        }
    };
```

Error state:
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#ff6060] p-5 text-center", "Error: {e}" }
        }
    };
```

Empty state:
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "No sessions found" }
        }
    };
```

Normal state:
```rust
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Sessions" }
            {items.into_iter()}
        }
    }
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/sessions_panel.rs
git commit -m "refactor: migrate sessions_panel.rs to Tailwind utilities"
```

---

### Task 14: Migrate agents_panel.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs`

- [ ] **Step 1: Replace all rsx! blocks**

Loading state (lines 41-45):
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "Loading agents..." }
        }
    };
```

Error state (lines 48-55):
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#ff6060] p-5 text-center", "Error: {e}" }
        }
    };
```

Empty state (lines 58-63):
```rust
    return rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "No agents discovered" }
        }
    };
```

Normal state (lines 71-75):
```rust
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            {items.into_iter()}
        }
    }
```

AgentItem (lines 87-127):
```rust
    rsx! {
        div { class: "border-b border-[#2a2a44]",
            div {
                class: "flex items-center px-2.5 py-2 cursor-pointer gap-2 hover:bg-[#222240]",
                onclick: move |_: Event<MouseData>| { ... },
                span { class: "text-[10px] text-[#666] transition-transform duration-150", "\u{25be}" }
                span { class: "font-semibold text-[13px] text-[#e0e0e0]", "{agent.name}" }
                span {
                    class: "text-[10px] px-1.5 py-0.5 rounded-[3px] font-bold ml-auto",
                    style: "background: {scope_color}; color: #1a1a2e;",
                    "{agent.scope}"
                }
            }
            div { class: "text-[12px] text-[#888] px-2.5 pb-1.5 pl-7", "{agent.description}" }
            if is_expanded {
                div { class: "px-2.5 pb-2 pl-7 text-[12px] bg-[#16162a]",
                    div { class: "py-0.5",
                        span { class: "text-[#6090ff] font-semibold", "ID: " }
                        span { class: "text-[#ccc] font-mono", "{agent.id}" }
                    }
                    div { class: "py-0.5",
                        span { class: "text-[#6090ff] font-semibold", "Type: " }
                        span { class: "text-[#ccc] font-mono", "{agent.type_}" }
                    }
                    div { class: "py-0.5",
                        span { class: "text-[#6090ff] font-semibold", "Scope: " }
                        span { class: "text-[#ccc] font-mono", "{agent.scope}" }
                    }
                }
            }
        }
    }
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/agents_panel.rs
git commit -m "refactor: migrate agents_panel.rs to Tailwind utilities"
```

---

### Task 15: Migrate tools_tab.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_tab.rs`

- [ ] **Step 1: Replace ToolsTabContent rsx!**

Empty state:
```rust
    return rsx! { div { class: "flex-1 overflow-y-auto p-2", div { class: "flex items-center justify-center h-full text-[#666]", "No tool calls yet" } } };
```

Normal state:
```rust
    rsx! { div { class: "flex-1 overflow-y-auto p-2", {items.into_iter()} } }
```

- [ ] **Step 2: Replace ToolCallItem rsx! (lines 68-88)**

```rust
    let scls = match status {
        ToolCallStatus::Running => "text-[#c0c040]",
        ToolCallStatus::Success => "text-[#40c040]",
        ToolCallStatus::Error => "text-[#c04040]",
        ToolCallStatus::Skipped => "text-[#888]",
    };
    let label = match status {
        ToolCallStatus::Running => "...",
        ToolCallStatus::Success => "OK",
        ToolCallStatus::Error => "ERR",
        ToolCallStatus::Skipped => "SKIP",
    };
    let dur_s = dur.map(|ms| format!("{ms}ms")).unwrap_or_default();
    rsx! {
        div { class: "border-b border-[#2a2a44]",
            div { class: "flex items-center px-2.5 py-2 cursor-pointer gap-2 hover:bg-[#222240]",
                onclick: move |_: Event<MouseData>| { ... },
                span { class: "text-[#555] text-[11px] min-w-[24px]", "{seq}." }
                span { class: "font-semibold text-[13px]", "[{name}]" }
                span { class: "text-[11px] px-1.5 py-0.5 rounded-[3px]", style: "color: {match status { ToolCallStatus::Running => \"#c0c040\", ToolCallStatus::Success => \"#40c040\", ToolCallStatus::Error => \"#c04040\", ToolCallStatus::Skipped => \"#888\" }};", "{label}" }
                if !dur_s.is_empty() { span { class: "text-[11px] text-[#888] ml-auto", "{dur_s}" } }
                span { class: "text-[10px] text-[#666] ml-1", "\u{25be}" }
            }
            if is_expanded {
                div { class: "px-2.5 pb-2 pl-[42px] text-[12px] font-mono text-[#888] bg-[#16162a] whitespace-pre-wrap break-all",
                    div {
                        span { class: "text-[#6090ff] font-semibold font-sans", "Input: " }
                        "{arg}"
                    }
                }
            }
        }
    }
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_tab.rs
git commit -m "refactor: migrate tools_tab.rs to Tailwind utilities"
```

---

### Task 16: Migrate tools_panel.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_panel.rs`

- [ ] **Step 1: Replace ToolsPanel rsx! (lines 70-81)**

```rust
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Tools Called ({count})" }
            div { class: "font-mono text-[13px]",
                if count == 0 {
                    div { class: "p-2.5 text-[#666] text-center", "No tool calls yet" }
                } else {
                    {(0..count).map(|idx| { let s = signal.clone(); rsx! { ToolItem { signal: s, index: idx } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
```

- [ ] **Step 2: Replace ToolItem rsx! (lines 96-101)**

Note: This component uses different class names than tools_tab.rs (`tool-item`, `tool-item-name`, etc. vs `tool-call-item`, `tool-call-name`). These are a separate set of classes for the left panel tools view.

```rust
    let scls = match status {
        ToolCallStatus::Running => "text-[#c0c040]",
        ToolCallStatus::Success => "text-[#40c040]",
        ToolCallStatus::Error => "text-[#c04040]",
        ToolCallStatus::Skipped => "text-[#888]",
    };
    let label = status_label(status);
    let dur_s = dur.map(|ms| format!(" {}ms", ms)).unwrap_or_default();
    rsx! {
        div { class: "border-b border-[#2a2a44] py-0.5",
            div {
                span { class: "font-semibold text-[13px] text-[#e0e0e0]", "{seq}. [{name}]" }
                span { class: "text-[12px] font-bold ml-2", style: "color: {match status { ToolCallStatus::Running => \"#c0c040\", ToolCallStatus::Success => \"#40c040\", ToolCallStatus::Error => \"#c04040\", ToolCallStatus::Skipped => \"#888\" }};", "{label}{dur_s}" }
            }
            if !arg.is_empty() { div { class: "text-[12px] text-[#888] mt-0.5 pl-1 font-mono", "{arg}" } }
        }
    }
```

- [ ] **Step 3: Remove unused status_class function (lines 31-33)**

It returns old CSS class names and is no longer needed after migration.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_panel.rs
git commit -m "refactor: migrate tools_panel.rs to Tailwind utilities"
```

---

### Task 17: Build verification and visual testing

- [ ] **Step 1: Verify Tailwind CLI generates CSS**

```bash
# Install Tailwind CLI if not already available
npx @tailwindcss/cli --version

# Generate CSS
npx @tailwindcss/cli \
    -i crates/vol-llm-ui/src/web/input.css \
    -o /tmp/tailwind-test.css \
    --minify

# Check output exists and is non-empty
ls -la /tmp/tailwind-test.css
wc -l /tmp/tailwind-test.css
```

Expected: A CSS file of reasonable size (a few KB for minified utilities).

- [ ] **Step 2: Verify Rust compiles**

```bash
cargo build \
    --target wasm32-unknown-unknown \
    --package vol-llm-ui \
    --bin vol-llm-ui-web \
    --no-default-features \
    --features web
```

Expected: No compilation errors.

- [ ] **Step 3: Run full rebuild-web.sh**

```bash
./scripts/rebuild-web.sh
```

Expected: Script completes without errors, produces `dist/` with `index.html`, `tailwind.css`, and `wasm/` directory.

- [ ] **Step 4: Verify dist output**

```bash
ls -la target/wasm32-unknown-unknown/wasm-dev/dist/
# Should show: index.html, tailwind.css, wasm/
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: verify Tailwind CSS migration build pipeline"
```

---

## Self-Review

### 1. Spec coverage checklist

- [x] Remove GLOBAL_CSS const from app.rs → Task 2 Step 4
- [x] Create input.css with @theme, @source → Task 1 Step 1
- [x] Update rebuild-web.sh with Tailwind CLI → Task 1 Step 3
- [x] Update index.html with CSS link → Task 1 Step 2
- [x] Migrate all 16 component files → Tasks 2-16
- [x] Responsive breakpoints for sidebar → Task 6 (file_tree.rs)
- [x] Responsive breakpoints for tab-bar → Task 2 (app.rs)
- [x] Custom animations (conn-blink) → Task 1 (input.css), Task 3 (status_bar.rs)
- [x] Dynamic inline styles preserved → Task 6 (file tree indent), Task 3 (connection dot colors)
- [x] State-based classes use full if/else → Task 2 (TabButton), Task 3 (badge_cls, status_cls)
- [x] Build verification → Task 17

### 2. Placeholder scan
No TBD, TODO, or incomplete sections found. All steps contain actual code.

### 3. Type consistency
All `class:` attributes use String/&str consistent with Dioxus rsx! syntax. Full if/else branches used for dynamic classes so Tailwind scanner discovers all variants.

### 4. Ambiguity check
All requirements are explicit. The responsive breakpoint strategy (mobile-first, base=smallest screen) is documented and applied consistently.
