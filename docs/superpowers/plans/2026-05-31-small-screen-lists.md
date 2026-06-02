# Small-Screen Responsive Lists Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply SkillsPanel dual-layout pattern (`sm:hidden` cards / `hidden sm:table`) to SessionsPanel, ToolsTabContent, and McpPanel (4 sub-lists) so all lists are usable below 480px.

**Architecture:** Each component gets a `sm:hidden` card wrapper and wraps its existing desktop layout in `hidden sm:...` — no abstractions, each component owns its two layouts inline. Follows the exact pattern from SkillsPanel (`skills.rs:179-187`).

**Tech Stack:** Rust, Dioxus 0.6, Tailwind CSS v4 (custom `--breakpoint-sm: 480px`)

---

## File Structure

```
crates/vol-llm-ui/src/web/components/
├── sessions_panel.rs     # MODIFIED: add mobile SessionCard + dual layout
├── tools_tab.rs          # MODIFIED: add mobile ToolCard + HistoryCard + dual layout
├── mcp_panel.rs          # MODIFIED: add mobile cards for servers/tools/resources/prompts
└── agents_panel.rs       # MODIFIED: minor style consistency (border rounded-lg)
```

No new files. No shared abstractions.

---

### Task 1: SessionsPanel — add mobile card layout

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

**Current structure:** `SessionItem` component renders each session as a single table-like `<div>` row (line 290-367). `SessionsPanel` renders all items in a scrollable div.

**Change:** Split `SessionItem` into two parts wrapped in responsive classes: desktop row + mobile card.

- [ ] **Step 1: Add SessionCard component**

Add this new component before `SessionItem` (after `truncate_id` at line 154):

```rust
/// Mobile card for a single session (sm:hidden).
#[component]
fn SessionCard(
    session_id: String,
    entry_count: usize,
    created_at: i64,
    rpc: crate::web::client::JsonRpcClient,
    conversation_signal: Signal<ConversationState>,
    agents_signal: Signal<AgentsState>,
) -> Element {
    let mut show_detail = use_signal(|| false);
    let entries = use_signal(|| Vec::<ConversationEntry>::new());
    let mut loading = use_signal(|| false);
    let is_resuming = use_signal(|| false);
    let had_parse_failure = use_signal(|| false);

    let rpc_view = rpc.clone();
    let sid_view = session_id.clone();
    let rpc_resume = rpc.clone();
    let sid_resume = session_id.clone();
    let conv_resume = conversation_signal;
    let agents_resume = agents_signal;

    rsx! {
        div {
            class: "cursor-pointer rounded-lg border border-[#333355] bg-[#20203a] p-3 active:bg-[#2a2a44]",
            onclick: move |_: Event<MouseData>| {
                if entries.read().is_empty() && !*loading.read() {
                    loading.set(true);
                    let rpc = rpc_view.clone();
                    let sid = sid_view.clone();
                    let mut ent = entries;
                    let mut ld = loading;
                    let mut parse_fail = had_parse_failure;
                    rpc.session_entries(&sid, move |result| {
                        match result {
                            Ok(e) => {
                                let converted = session_entries_to_conversation(e.clone());
                                if e.len() > 0 && converted.is_empty() {
                                    parse_fail.set(true);
                                }
                                ent.set(converted);
                            }
                            Err(e) => {
                                log::error!("Failed to load session entries: {e}");
                                parse_fail.set(true);
                            }
                        }
                        ld.set(false);
                    });
                }
                show_detail.set(true);
            },
            // Row 1: session_id truncated + entry_count badge
            div { class: "flex items-center justify-between",
                span { class: "font-mono text-[13px] text-[#e0e0e0] font-semibold truncate",
                    "{truncate_id(&session_id)}"
                }
                span { class: "bg-[#2a2a44] text-[#aaa] rounded-full px-2 py-0.5 text-[11px] flex-shrink-0 ml-2",
                    "{entry_count} entries"
                }
            }
            // Row 2: relative time + resume button
            div { class: "flex items-center justify-between mt-1.5",
                span { class: "text-[11px] text-[#666]", "{format_age(created_at)}" }
                button {
                    class: "px-2.5 py-0.5 bg-[#408040] text-[#e0e0e0] border-none rounded-[3px] cursor-pointer text-[12px] hover:bg-[#50a050] disabled:bg-[#333355] disabled:cursor-not-allowed",
                    disabled: *is_resuming.read(),
                    onclick: move |evt: Event<MouseData>| {
                        evt.stop_propagation();
                        let mut resuming = is_resuming;
                        let _ = resuming.set(true);
                        let rpc = rpc_resume.clone();
                        let sid = sid_resume.clone();
                        let mut conv = conv_resume;
                        let mut agents = agents_resume;
                        let agent_id = agents.read().selected.clone();
                        rpc.session_resume(&sid, agent_id.as_deref(), move |result| {
                            match result {
                                Ok(resp) => {
                                    let conv_entries = session_entries_to_conversation(resp.entries);
                                    let active_id = agents.read().selected.clone().unwrap_or_default();
                                    conv.with_mut(|s| {
                                        let ac = s.get_or_create(&active_id);
                                        ac.entries = conv_entries;
                                    });
                                    agents.with_mut(|a| a.sub_tab = AgentSubTab::Conversation);
                                }
                                Err(e) => log::error!("Failed to resume session: {e}"),
                            }
                            let _ = resuming.set(false);
                        });
                        let mut resuming_timeout = is_resuming;
                        wasm_bindgen_futures::spawn_local(async move {
                            TimeoutFuture::new(15_000).await;
                            let _ = resuming_timeout.set(false);
                        });
                    },
                    if *is_resuming.read() { "Resuming..." } else { "Resume" }
                }
            }
        }
        SessionDetailOverlay {
            session_id,
            entries,
            loading,
            show: show_detail,
            had_parse_failure,
        }
    }
}
```

- [ ] **Step 2: Wrap current SessionItem content in `hidden sm:flex`**

The existing `SessionItem` component's outer div (line 291) already has desktop styling. Wrap it with responsive visibility — the card handles `<480px`, the row handles `≥480px`:

In `SessionsPanel` (line 436-447), add a mobile card list alongside the existing items:

```rust
// In SessionsPanel, replace the items block (lines 436-453):

    // Mobile: card layout (sm:hidden)
    let mobile_items: Vec<Element> = sessions.iter().map(|session| {
        rsx! {
            SessionCard {
                session_id: session.id.clone(),
                entry_count: session.entry_count,
                created_at: session.created_at,
                rpc: rpc_for_items.clone(),
                conversation_signal,
                agents_signal,
            }
        }
    }).collect();

    // Desktop: row layout (hidden sm:block)
    let desktop_items: Vec<Element> = sessions.iter().map(|session| {
        rsx! {
            SessionItem {
                session_id: session.id.clone(),
                entry_count: session.entry_count,
                created_at: session.created_at,
                rpc: rpc_for_items.clone(),
                conversation_signal,
                agents_signal,
            }
        }
    }).collect();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Sessions" }
            div { class: "sm:hidden flex flex-col gap-2",
                {mobile_items.into_iter()}
            }
            div { class: "hidden sm:block",
                {desktop_items.into_iter()}
            }
        }
    }
```

- [ ] **Step 3: Verify compiles**

```bash
cargo check -p vol-llm-ui 2>&1 | grep -E "^error" | head -10
```

Expected: no errors related to sessions_panel. (vol-llm-ui may have pre-existing unrelated errors.)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/sessions_panel.rs
git commit -m "feat(ui): add mobile card layout for SessionsPanel (< 480px)"
```

---

### Task 2: ToolsTabContent — add mobile card layouts

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_tab.rs`

**Current structure:** System tools rendered as `<div>` rows (lines 176-210). Call history rendered as `ToolCallItem` rows (lines 259-284).

**Change:** Add mobile card versions for both sections. Desktop rows get `hidden sm:flex` wrapper, mobile cards get `sm:hidden flex flex-col gap-2`.

- [ ] **Step 1: Add tool card layout in the tools loop**

In `ToolsTabContent`, replace the tools loop (lines 176-210) with dual layouts:

```rust
// After the "System Tools" section header and error/loading displays,
// replace the `for tool in &tools` block:

// Mobile: tool cards
div { class: "sm:hidden flex flex-col gap-2",
    for tool in &tools {
        div { class: "cursor-pointer rounded-lg border border-[#333355] bg-[#20203a] p-3",
            div { class: "flex items-center justify-between",
                div { class: "min-w-0",
                    div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{tool.name}" }
                    if let Some(ref desc) = tool.description {
                        div { class: "mt-0.5 text-[11px] text-[#777] truncate", "{desc}" }
                    }
                }
                button {
                    class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                    onclick: {
                        let client = client.clone();
                        let ts = tool_state;
                        let name = tool.name.clone();
                        move |_| {
                            let args_val = serde_json::json!({});
                            let ts = ts;
                            client.tool_call(&name, &args_val, move |result| {
                                safe_write(ts, |s| {
                                    match result {
                                        Ok(val) => s.call_result = Some(
                                            serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                                        ),
                                        Err(e) => s.call_result = Some(format!("Error: {e}")),
                                    }
                                });
                            });
                        }
                    },
                    "Run"
                }
            }
        }
    }
}
// Desktop: tool rows
div { class: "hidden sm:block",
    for tool in &tools {
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
                        let ts = tool_state;
                        let name = tool.name.clone();
                        move |_| {
                            let args_val = serde_json::json!({});
                            let ts = ts;
                            client.tool_call(&name, &args_val, move |result| {
                                safe_write(ts, |s| {
                                    match result {
                                        Ok(val) => s.call_result = Some(
                                            serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                                        ),
                                        Err(e) => s.call_result = Some(format!("Error: {e}")),
                                    }
                                });
                            });
                        }
                    },
                    "Run"
                }
            }
        }
    }
}
```

- [ ] **Step 2: Add history card layout**

In `ToolsTabContent`, replace the call history section (lines 237-242) to add mobile cards alongside desktop `ToolCallItem`:

```rust
// Replace the call_items rendering block. Add a mobile card version
// that shows summary info on each card:

// Mobile: history cards
div { class: "sm:hidden flex flex-col gap-2",
    {(0..call_count).map(|idx| {
        let s = call_signal.clone();
        rsx! { ToolCallHistoryCard { signal: s, index: idx } }
    }).collect::<Vec<Element>>().into_iter()}
}
// Desktop: history rows (keep existing ToolCallItem)
div { class: "hidden sm:block",
    {call_items.into_iter()}
}
```

- [ ] **Step 3: Add ToolCallHistoryCard component**

After the existing `ToolCallItem` component, add:

```rust
/// Mobile card for tool call history (sm:hidden).
#[component]
fn ToolCallHistoryCard(signal: Signal<ToolState>, index: usize) -> Element {
    let is_expanded = signal.read().expanded.contains(&index);
    let (seq, name, arg, status, dur) = {
        let ui = signal.read();
        match ui.calls.get(index) {
            Some(e) => (e.sequence, e.tool_name.clone(), e.arg_preview.clone(), e.status.clone(), e.duration_ms),
            None => return rsx! {},
        }
    };
    let scls = match status { ToolCallStatus::Running => "text-[#c0c040]", ToolCallStatus::Success => "text-[#40c040]", ToolCallStatus::Error => "text-[#c04040]", ToolCallStatus::Skipped => "text-[#888]" };
    let label = match status { ToolCallStatus::Running => "...", ToolCallStatus::Success => "OK", ToolCallStatus::Error => "ERR", ToolCallStatus::Skipped => "SKIP" };
    let dur_s = dur.map(|ms| format!("{ms}ms")).unwrap_or_default();
    rsx! {
        div {
            class: "cursor-pointer rounded-lg border border-[#333355] bg-[#20203a] p-3 active:bg-[#2a2a44]",
            onclick: move |_: Event<MouseData>| {
                let mut state = signal.write_unchecked();
                if state.expanded.contains(&index) {
                    state.expanded.remove(&index);
                } else {
                    state.expanded.insert(index);
                }
            },
            div { class: "flex items-center gap-2",
                span { class: "text-[#555] text-[11px]", "{seq}." }
                span { class: "font-semibold text-[13px] text-[#e0e0e0] truncate", "[{name}]" }
                span { class: "text-[11px] px-1.5 py-0.5 rounded-[3px] {scls}", "{label}" }
                if !dur_s.is_empty() { span { class: "text-[11px] text-[#666] ml-auto", "{dur_s}" } }
            }
            if is_expanded {
                div { class: "mt-2 pt-2 border-t border-[#2a2a44] text-[12px] font-mono text-[#888] whitespace-pre-wrap break-all",
                    span { class: "text-[#6090ff] font-semibold font-sans", "Input: " }
                    "{arg}"
                }
            }
        }
    }
}
```

- [ ] **Step 4: Verify compiles**

```bash
cargo check -p vol-llm-ui 2>&1 | grep -E "^error" | head -10
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_tab.rs
git commit -m "feat(ui): add mobile card layout for ToolsTabContent (< 480px)"
```

---

### Task 3: McpPanel — add mobile cards for 4 sub-lists

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

**Current structure:** `ServerRow`, `ToolCard` (already a card!), `ResourceRow`, `TemplateRow`, `PromptRow` — each rendered in a list. `ToolCard` already uses `bg-[#252540] rounded p-2 mb-1` — just needs responsive wrapper.

**Change:** For each sub-list, add a `sm:hidden` wrapper with card-style items and `hidden sm:block` wrapper for desktop rows.

- [ ] **Step 1: ServerList — add mobile card layout**

In `ServerList`, add mobile cards alongside existing desktop `ServerRow`:

```rust
// After the empty-state check, replace the content:
rsx! {
    div {
        // Mobile: server cards
        div { class: "sm:hidden flex flex-col gap-2",
            {servers.iter().map(|s| {
                let status_color = match s.status.as_str() {
                    "connected" => "#40c040",
                    "connecting" => "#f0c040",
                    "disconnected" => "#888",
                    _ => "#c04040",
                };
                let sig = signal.clone();
                let app = app_state.clone();
                let name = s.name.clone();
                let status = s.status.clone();
                let show_reconnect = s.status != "connected" && s.status != "connecting";
                rsx! {
                    div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                        div { class: "flex items-center justify-between",
                            div { class: "flex items-center gap-2 min-w-0",
                                span { class: "w-2 h-2 rounded-full flex-shrink-0", style: "background-color: {status_color};" }
                                span { class: "text-[13px] text-[#e0e0e0] truncate", "{name}" }
                            }
                            span { class: "text-[11px] text-[#666] flex-shrink-0 ml-2", "{status}" }
                        }
                        if show_reconnect {
                            button {
                                class: "mt-2 w-full px-2 py-1 bg-[#2a2a44] text-[#aaa] rounded text-[11px] hover:text-[#e0e0e0]",
                                onclick: move |_| {
                                    let srv = s.name.clone();
                                    let client = app_state.rpc_client.clone();
                                    let sig = sig.clone();
                                    client.mcp_reconnect(&srv, move |result| {
                                        if let Ok(true) = result {
                                            let mut sig2 = sig;
                                            client.mcp_list_servers(move |r| {
                                                if let Ok(servers) = r {
                                                    sig2.with_mut(|s| {
                                                        s.servers = servers;
                                                        s.error = None;
                                                    });
                                                }
                                            });
                                        }
                                    });
                                },
                                "Reconnect"
                            }
                        }
                    }
                }
            }).collect::<Vec<Element>>().into_iter()}
        }
        // Desktop: server rows
        div { class: "hidden sm:block font-mono text-[13px]",
            {servers.into_iter().map(|s| {
                let sig = signal.clone();
                let app = app_state.clone();
                rsx! { ServerRow { signal: sig, server: s, app_state: app } }
            }).collect::<Vec<Element>>().into_iter()}
            if let Some(ref e) = error {
                div { class: "text-[#c04040] p-2 text-[12px]", "{e}" }
            }
        }
    }
}
```

- [ ] **Step 2: ToolList — wrap existing ToolCard in responsive**

`ToolCard` already uses card styling. Just add `sm:hidden` / `hidden sm:block` wrappers:

```rust
// In ToolList, replace the tools loop with:
// Mobile: compact tool cards
div { class: "sm:hidden flex flex-col gap-2",
    for t in &tools {
        let dsig = dialog_signal.clone();
        let tool = t.clone();
        rsx! {
            div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                div { class: "flex items-center justify-between",
                    div { class: "min-w-0",
                        div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{tool.name}" }
                        if let Some(ref desc) = tool.description {
                            div { class: "mt-0.5 text-[11px] text-[#777] truncate", "{desc}" }
                        }
                    }
                    button {
                        class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                        onclick: move |_| {
                            let t = tool.clone();
                            dsig.write_unchecked().tool_call_dialog = Some(crate::state::McpToolCallState {
                                server: t.server.clone(),
                                tool_name: t.name.clone(),
                                arguments_json: t.input_schema.as_ref().map(|v| serde_json::to_string_pretty(v).unwrap_or_default()).unwrap_or_else(|| "{}".to_string()),
                                input_schema: t.input_schema.clone(),
                                result: None,
                                error: None,
                                loading: false,
                            });
                        },
                        "Call"
                    }
                }
            }
        }
    }
}
// Desktop: existing grouped layout
div { class: "hidden sm:block font-mono text-[13px]",
    {groups.into_iter().map(|(server, tools)| {
        rsx! {
            div { class: "mb-2",
                div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({tools.len()} tools)" }
                {tools.into_iter().map(|t| {
                    let dsig = dialog_signal.clone();
                    rsx! { ToolCard { signal: dsig, tool: t } }
                }).collect::<Vec<Element>>().into_iter()}
            }
        }
    }).collect::<Vec<Element>>().into_iter()}
}
```

- [ ] **Step 3: ResourceList — add mobile cards**

Same pattern — mobile cards + desktop rows:

```rust
// Mobile: resource cards
div { class: "sm:hidden flex flex-col gap-2",
    for r in &resources {
        let dsig = dialog_signal.clone();
        let res = r.clone();
        rsx! {
            div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                div { class: "flex items-center justify-between",
                    div { class: "min-w-0 flex-1",
                        div { class: "text-[13px] text-[#e0e0e0] truncate", "{res.name}" }
                        div { class: "text-[11px] text-[#666] font-mono truncate mt-0.5", "{res.uri}" }
                    }
                    button {
                        class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                        onclick: move |_| {
                            let r = res.clone();
                            dsig.write_unchecked().resource_viewer = Some(crate::state::McpResourceViewerState {
                                uri: r.uri.clone(),
                                content: None,
                                error: None,
                                loading: false,
                            });
                        },
                        "Read"
                    }
                }
            }
        }
    }
}
// Desktop: existing grouped layout
div { class: "hidden sm:block font-mono text-[13px]",
    {all_servers.into_iter().map(|server| {
        // ... existing desktop code unchanged ...
    }).collect::<Vec<Element>>().into_iter()}
}
```

- [ ] **Step 4: PromptList — add mobile cards**

Same pattern:

```rust
// Mobile: prompt cards
div { class: "sm:hidden flex flex-col gap-2",
    for p in &prompts {
        let dsig = dialog_signal.clone();
        let prompt = p.clone();
        rsx! {
            div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                div { class: "flex items-center justify-between",
                    div { class: "min-w-0",
                        div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{prompt.name}" }
                        if let Some(ref desc) = prompt.description {
                            div { class: "mt-0.5 text-[11px] text-[#777] truncate", "{desc}" }
                        }
                    }
                    button {
                        class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                        onclick: move |_| {
                            let p = prompt.clone();
                            dsig.write_unchecked().prompt_viewer = Some(crate::state::McpPromptViewerState {
                                server: p.server.clone(),
                                prompt_name: p.name.clone(),
                                args_json: "{}".to_string(),
                                result: None,
                                error: None,
                                loading: false,
                            });
                        },
                        "Get"
                    }
                }
            }
        }
    }
}
// Desktop: existing grouped layout
div { class: "hidden sm:block font-mono text-[13px]",
    {groups.into_iter().map(|(server, prompts)| {
        rsx! {
            div { class: "mb-2",
                div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({prompts.len()} prompts)" }
                {prompts.into_iter().map(|p| {
                    let dsig = dialog_signal.clone();
                    rsx! { PromptRow { signal: dsig, prompt: p } }
                }).collect::<Vec<Element>>().into_iter()}
            }
        }
    }).collect::<Vec<Element>>().into_iter()}
}
```

- [ ] **Step 5: Verify compiles**

```bash
cargo check -p vol-llm-ui 2>&1 | grep -E "^error" | head -10
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "feat(ui): add mobile card layout for McpPanel 4 sub-lists (< 480px)"
```

---

### Task 4: AgentsPanel — style consistency tweak

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs`

**Change:** Ensure agent cards use the same `rounded-lg border border-[#333355] bg-[#20203a]` base style as other list cards.

- [ ] **Step 1: Read the current agent card styling**

Check `agents_panel.rs` for the current card class on the agent item div. Update to match the unified card style:

```rust
// Find the agent card div and ensure it uses:
class: "cursor-pointer rounded-lg border border-[#333355] bg-[#20203a] p-3 hover:bg-[#2a2a44] transition-colors"
```

- [ ] **Step 2: Verify compiles and commit**

```bash
cargo check -p vol-llm-ui 2>&1 | grep -E "^error" | head -5
git add crates/vol-llm-ui/src/web/components/agents_panel.rs
git commit -m "style(ui): unify agent card border style with other list cards"
```

---

### Task 5: Visual verification

- [ ] **Step 1: Start web dev servers**

```bash
# Terminal 1: Tailwind CSS
make web-css

# Terminal 2: Dioxus WASM dev server
make web-dev

# Terminal 3: Backend
make web-backend
```

- [ ] **Step 2: Test in browser**

Open `http://localhost:8080` and:
1. Resize viewport to < 480px — verify all tabs show card layouts
2. Resize viewport to > 480px — verify all tabs show table/row layouts
3. Click through each tab (Agents, Tools, Skills, MCP) and each MCP sub-tab
4. Verify card click interactions work (view details, resume, run tools)
5. Verify smooth transition across the 480px breakpoint

- [ ] **Step 3: Commit any fixes**

```bash
git add -A && git commit -m "fix(ui): visual verification fixes for small-screen lists"
```

---

## Testing Strategy

- **Build verification**: `cargo check -p vol-llm-ui` after each task
- **Visual verification**: Browser resize + phone testing (Task 5)
- **Regression**: Existing desktop layouts preserved behind `hidden sm:block` wrappers — no behavior change above 480px
- **No automated tests**: Dioxus components don't have unit tests in this project; visual testing is the standard
