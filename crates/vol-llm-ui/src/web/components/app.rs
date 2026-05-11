//! Root App component with state management, event loop, and routing.

use dioxus::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use crate::state::{ActiveTab, ApprovalUiState, ConversationState, EventBus, GlobalState, SubscriptionSet, ToolState, UiEvent, UiEventKind, WorkspaceState};
use crate::web::client::{AgentEvent, JsonRpcClient};

use super::approval_dialog::ApprovalDialog;
use super::conversation::ConversationView;
use super::file_content::FileContentView;
use super::file_tree::FileTree;
use super::input_area::InputArea;
use super::log_viewer::LogViewer;
use super::session_dialog::SessionDialog;
use super::skills::SkillsPanel;
use super::status_bar::StatusBar;
use super::tools_tab::ToolsTabContent;

/// Derive WebSocket URL from the page's host at runtime.
fn derive_ws_url() -> String {
    if let Some(window) = web_sys::window() {
        let location = window.location();
        if let Ok(hostname) = location.hostname() {
            return format!("ws://{}:3001/ws", hostname);
        }
    }
    "ws://localhost:3001".to_string()
}

/// Shared application state — no longer holds Signal<UiState>.
#[derive(Clone)]
pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,
    pub active_tab: Signal<ActiveTab>,
}

impl PartialEq for AppState {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

fn agent_event_to_ui(event: &AgentEvent) -> Option<UiEvent> {
    let data = &event.data;
    match event.event_type.as_str() {
        "agent_start" => Some(UiEvent::AgentStart {
            input: data.get("input").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "agent_complete" => Some(UiEvent::AgentComplete {
            response: data.get("response").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "agent_error" => Some(UiEvent::AgentError {
            message: data.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "agent_aborted" => Some(UiEvent::AgentAborted {
            reason: data.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "thinking_start" => Some(UiEvent::ThinkingStart),
        "thinking_delta" => Some(UiEvent::ThinkingDelta {
            delta: data.get("delta").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "thinking_complete" => Some(UiEvent::ThinkingComplete),
        "content_start" => Some(UiEvent::ContentStart),
        "content_delta" => Some(UiEvent::ContentDelta {
            delta: data.get("delta").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "content_complete" => Some(UiEvent::ContentComplete {
            content: data.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "tool_call_begin" => Some(UiEvent::ToolCallBegin {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            arguments: data.get("arguments").map(|v| v.to_string()).unwrap_or_default(),
        }),
        "tool_call_argument_delta" => Some(UiEvent::ToolCallArgumentDelta {
            delta: data.get("delta").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "tool_call_complete" => Some(UiEvent::ToolCallComplete {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            result: data.get("result").map(|v| v.to_string()).unwrap_or_default(),
            duration_ms: data.get("duration_ms").and_then(|v| v.as_u64()),
        }),
        "tool_call_error" => Some(UiEvent::ToolCallError {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            error: data.get("error").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            duration_ms: data.get("duration_ms").and_then(|v| v.as_u64()),
        }),
        "tool_call_skipped" => Some(UiEvent::ToolCallSkipped {
            tool_name: data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            reason: data.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            duration_ms: data.get("duration_ms").and_then(|v| v.as_u64()),
        }),
        "max_iterations_reached" => Some(UiEvent::MaxIterationsReached {
            current: data.get("current").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            max: data.get("max").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        }),
        "iteration_continued" => Some(UiEvent::IterationContinued {
            from_iteration: data.get("from_iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        }),
        "iteration_complete" => Some(UiEvent::IterationComplete {
            iteration: data.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            final_answer: data.get("final_answer").and_then(|v| v.as_str()).map(|s| s.to_string()),
        }),
        _ => None,
    }
}

#[component]
pub fn App() -> Element {
    let ws_url = derive_ws_url();
    let event_bus = use_signal(|| EventBus::new());
    let active_tab = use_signal(|| ActiveTab::Conversation);
    let global_signal = use_signal(|| GlobalState::new(ws_url.clone()));
    let approval_signal = use_signal(|| ApprovalUiState::new());
    let workspace_signal = use_signal(|| WorkspaceState::new("."));
    let conversation_signal = use_signal(|| ConversationState::new());
    let tool_signal = use_signal(|| ToolState::new());

    let client = use_hook(|| {
        let c = JsonRpcClient::new(&ws_url);
        let bus = event_bus.with(|eb| eb.clone());
        let global = global_signal.clone();
        let bus_conn = bus.clone();
        let global_conn = global.clone();
        c.on_state_change(move |cs| {
            let event = match cs {
                crate::web::client::ConnectionState::Connected => UiEvent::WsConnected,
                crate::web::client::ConnectionState::Connecting => UiEvent::WsConnecting,
                crate::web::client::ConnectionState::Disconnected =>
                    UiEvent::WsDisconnected { reason: Some("Disconnected".to_string()) },
            };
            bus_conn.publish(&event);
            match cs {
                crate::web::client::ConnectionState::Connected => {
                    global_conn.write_unchecked().ws_connected = true;
                    global_conn.write_unchecked().ws_last_error = None;
                }
                crate::web::client::ConnectionState::Connecting => {
                    global_conn.write_unchecked().ws_connected = false;
                }
                crate::web::client::ConnectionState::Disconnected => {
                    global_conn.write_unchecked().ws_connected = false;
                    global_conn.write_unchecked().ws_last_error = Some("Disconnected".to_string());
                }
            }
        });

        c
    });

    // EventBus subscriptions for shared signals — stored in hook, cleaned up on Drop
    use_hook(|| {
        let bus = event_bus.with(|eb| eb.clone());
        let mut set = SubscriptionSet::new(bus.clone());
        let global = global_signal.clone();
        let approval = approval_signal.clone();

        // GlobalState: agent lifecycle events
        set.subscribe(&bus, UiEventKind::AgentStart, {
            let global = global.clone();
            move |_e| {
                let mut s = global.write_unchecked();
                s.run_count += 1; s.iteration = 0; s.tool_call_count = 0;
                s.run_start = Some(web_time::Instant::now());
                s.run_elapsed = Duration::ZERO; s.is_running = true;
            }
        });
        for kind in [UiEventKind::AgentComplete, UiEventKind::AgentAborted, UiEventKind::AgentError] {
            let global = global.clone();
            set.subscribe(&bus, kind, move |_e| {
                let mut s = global.write_unchecked();
                if let Some(start) = s.run_start { s.run_elapsed = start.elapsed(); }
                s.is_running = false;
            });
        }
        set.subscribe(&bus, UiEventKind::IterationComplete, {
            let global = global.clone();
            move |e| {
                if let UiEvent::IterationComplete { iteration, .. } = e {
                    global.write_unchecked().iteration = *iteration;
                }
            }
        });

        // ApprovalUiState
        set.subscribe(&bus, UiEventKind::ApprovalRequest, {
            let approval = approval.clone();
            move |e| {
                if let UiEvent::ApprovalRequest { tool_name, reason, arguments } = e {
                    let mut s = approval.write_unchecked();
                    s.tool_name = Some(tool_name.clone());
                    s.reason = Some(reason.clone());
                    s.arguments = Some(arguments.clone());
                }
            }
        });
        set.subscribe(&bus, UiEventKind::ApprovalResolved, {
            let approval = approval.clone();
            move |_e| {
                approval.write_unchecked().clear();
            }
        });

        // Return Arc to keep alive until component drops
        Arc::new(set)
    });

    // Conversation event subscriptions — kept at App level so events are never lost
    use_hook(|| {
        let bus = event_bus.with(|eb| eb.clone());
        let mut set = SubscriptionSet::new(bus.clone());
        let conv = conversation_signal.clone();

        for kind in [
            UiEventKind::AgentStart, UiEventKind::AgentComplete, UiEventKind::AgentAborted,
            UiEventKind::AgentError, UiEventKind::ThinkingStart, UiEventKind::ThinkingDelta,
            UiEventKind::ThinkingComplete, UiEventKind::ContentStart, UiEventKind::ContentDelta,
            UiEventKind::ContentComplete, UiEventKind::MaxIterationsReached,
            UiEventKind::IterationContinued, UiEventKind::IterationComplete,
        ] {
            set.subscribe(&bus, kind, {
                let conv = conv.clone();
                move |event| {
                    let mut s = conv.write_unchecked();
                    crate::web::components::conversation::reduce_conversation(&mut s, event);
                }
            });
        }
        Arc::new(set)
    });

    // Tool event subscriptions — kept at App level so events are never lost
    use_hook(|| {
        let bus = event_bus.with(|eb| eb.clone());
        let mut set = SubscriptionSet::new(bus.clone());
        let tool = tool_signal.clone();

        for kind in [UiEventKind::ToolCallBegin, UiEventKind::ToolCallComplete, UiEventKind::ToolCallError, UiEventKind::ToolCallSkipped] {
            set.subscribe(&bus, kind, {
                let tool = tool.clone();
                move |event| {
                    let mut s = tool.write_unchecked();
                    crate::web::components::tools_tab::reduce_tool_state(&mut s, event);
                }
            });
        }
        Arc::new(set)
    });

    // WS event loop
    let bus_ev = event_bus.with(|eb| eb.clone());
    let client_ev = client.clone();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            match client_ev.next_event().await {
                Some(event) => {
                    if let Some(ui_event) = agent_event_to_ui(&event) {
                        bus_ev.publish(&ui_event);
                    }
                }
                None => {
                    log::warn!("Event stream closed");
                    bus_ev.publish(&UiEvent::AgentError { message: "Event stream closed".to_string() });
                    bus_ev.publish(&UiEvent::WsDisconnected { reason: Some("Event stream closed".to_string()) });
                    break;
                }
            }
        }
    });

    use_context_provider(|| AppState {
        event_bus: event_bus.with(|eb| eb.clone()),
        rpc_client: client.clone(),
        active_tab,
    });
    use_context_provider(|| global_signal);
    use_context_provider(|| approval_signal);
    use_context_provider(|| workspace_signal);
    use_context_provider(|| conversation_signal);
    use_context_provider(|| tool_signal);

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
            SessionDialog {}
            ApprovalDialog {}
        }
    }
}

/// Tab bar component.
#[component]
fn TabBar() -> Element {
    let state: AppState = use_context();

    rsx! {
        div { class: "tab-bar",
            TabButton { state: state.clone(), tab: ActiveTab::Conversation, label: "Conversation" }
            TabButton { state: state.clone(), tab: ActiveTab::Tools, label: "Tools" }
            TabButton { state: state.clone(), tab: ActiveTab::Workspace, label: "Workspace" }
            TabButton { state: state.clone(), tab: ActiveTab::Skills, label: "Skills" }
            TabButton { state: state.clone(), tab: ActiveTab::Logs, label: "Logs" }
        }
    }
}

#[component]
fn TabButton(state: AppState, tab: ActiveTab, label: String) -> Element {
    let current_tab = state.active_tab.read();
    let active = *current_tab == tab;
    let tab_class = if active { "tab active" } else { "tab" };
    let mut active_tab_signal = state.active_tab;
    rsx! {
        button {
            class: tab_class,
            onclick: move |_| { active_tab_signal.set(tab); },
            "{label}"
        }
    }
}

/// Tab content router.
#[component]
fn TabContent() -> Element {
    let state: AppState = use_context();
    let active = *state.active_tab.read();

    match active {
        ActiveTab::Conversation => rsx! { ConversationView {} },
        ActiveTab::Tools => rsx! { ToolsTabContent {} },
        ActiveTab::Workspace => rsx! { FileContentView {} },
        ActiveTab::Skills => rsx! { SkillsPanel {} },
        ActiveTab::Logs => rsx! { LogViewer {} },
        ActiveTab::Agents => rsx! { div { "Agents panel (coming soon)" } },
    }
}

/// Helper: format a Duration as MM:SS.
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Helper: format a tool status as a short label.
pub fn status_label(status: crate::state::ToolCallStatus) -> &'static str {
    match status {
        crate::state::ToolCallStatus::Running => "...",
        crate::state::ToolCallStatus::Success => "OK",
        crate::state::ToolCallStatus::Error => "ERR",
        crate::state::ToolCallStatus::Skipped => "SKIP",
    }
}

/// Helper: CSS class for a tool status.
pub fn status_class(status: crate::state::ToolCallStatus) -> &'static str {
    match status {
        crate::state::ToolCallStatus::Running => "status-running",
        crate::state::ToolCallStatus::Success => "status-success",
        crate::state::ToolCallStatus::Error => "status-error",
        crate::state::ToolCallStatus::Skipped => "status-skipped",
    }
}

const GLOBAL_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; font-size: 14px; color: #e0e0e0; background: #1a1a2e; }
.app-container { display: flex; flex-direction: column; height: 100dvh; width: 100vw; overflow: hidden; }
.status-bar { display: flex; align-items: center; justify-content: space-between; padding: 4px 12px; background: #2d2d44; color: #e0e0e0; font-size: 12px; font-family: monospace; flex-shrink: 0; }
.status-left { display: flex; align-items: center; gap: 6px; overflow: hidden; flex-wrap: nowrap; }
.status-right { display: flex; align-items: center; flex-shrink: 0; }
.status-item { white-space: nowrap; }
.status-divider { color: #555; user-select: none; }
.status-badge { padding: 1px 6px; border-radius: 3px; font-size: 11px; font-weight: bold; }
.badge-running { background: #3a3a20; color: #f0c040; }
.badge-idle { background: #203a20; color: #80c080; }
.badge-unsafe { background: #3a2020; color: #ff4040; }
.badge-exiting { background: #3a2020; color: #ff8080; }
.conn-indicator { display: flex; align-items: center; gap: 4px; margin-right: 4px; }
.conn-dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; flex-shrink: 0; }
.conn-dot-connected { box-shadow: 0 0 4px #40c040; }
.conn-dot-connecting { animation: conn-pulse 1.5s ease-in-out infinite; }
.conn-dot-error { animation: conn-blink 1s ease-in-out infinite; }
.conn-label { font-size: 10px; color: #888; max-width: 80px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.build-info { display: flex; align-items: center; font-size: 11px; color: #888; flex-shrink: 0; }
.build-label { color: #666; }
.build-version { color: #a0a0c0; font-weight: bold; }
.build-separator { color: #555; margin: 0 2px; }
.build-time { color: #666; }
@keyframes conn-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
@keyframes conn-blink { 0%, 100% { opacity: 1; } 50% { opacity: 0.2; } }
.status-running { color: #f0c040; }
.status-idle { color: #80c080; }
.unsafe-mode { color: #ff4040; font-weight: bold; }
.main-layout { display: flex; flex: 1; overflow: hidden; }
.sidebar { width: 240px; min-width: 180px; border-right: 1px solid #2a2a44; display: flex; flex-direction: column; overflow: hidden; flex-shrink: 0; background: #16162a; }
.sidebar-header { padding: 8px 12px; font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.8px; color: #6a6a9a; border-bottom: 1px solid #2a2a44; flex-shrink: 0; }
.file-tree { flex: 1; overflow-y: auto; padding: 4px 0; }
.file-tree-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; padding: 20px; text-align: center; font-size: 12px; }
.file-tree-node { display: flex; align-items: center; padding: 3px 8px 3px 0; cursor: pointer; font-size: 13px; white-space: nowrap; user-select: none; border-radius: 3px; margin: 0 4px; }
.file-tree-node:hover { background: #2a2a44; }
.file-tree-node:active { background: #3a3a54; }
.file-tree-dir:hover { background: #1a2a3a; }
.file-tree-refresh { font-size: 10px; color: #666; margin-left: 4px; opacity: 0; transition: opacity 0.15s; cursor: pointer; }
.file-tree-node:hover .file-tree-refresh { opacity: 1; }
.file-tree-refresh:hover { color: #aaa; }
.file-tree-dir .file-tree-label { color: #8ab4ff; font-weight: 500; }
.file-tree-file .file-tree-label { color: #ccc; }
.file-tree-chevron { display: inline-flex; align-items: center; justify-content: center; width: 16px; height: 16px; flex-shrink: 0; font-size: 10px; color: #666; transition: transform 0.15s; }
.file-tree-chevron.collapsed { transform: rotate(-90deg); }
.file-tree-chevron.hidden { visibility: hidden; }
.file-tree-icon { display: inline-flex; align-items: center; justify-content: center; width: 18px; height: 18px; flex-shrink: 0; margin-right: 4px; font-size: 14px; }
.file-tree-label { overflow: hidden; text-overflow: ellipsis; }
.file-tree-children { overflow: hidden; }
.right-panel { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.tab-bar { display: flex; background: #252540; border-bottom: 1px solid #333355; flex-shrink: 0; }
.tab { padding: 6px 16px; background: transparent; border: none; color: #888; cursor: pointer; font-size: 13px; border-bottom: 2px solid transparent; }
.tab:hover { color: #ccc; background: #2a2a44; }
.tab.active { color: #e0e0e0; background: #1a1a2e; border-bottom: 2px solid #80a0ff; }
.conversation { flex: 1; overflow-y: auto; padding: 10px; }
.conversation-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.msg { margin-bottom: 10px; padding: 8px 10px; border-radius: 6px; max-width: 100%; word-wrap: break-word; white-space: pre-wrap; }
.msg-user { background: #1a2a44; border-left: 3px solid #4080ff; }
.msg-user-prefix { color: #4080ff; font-weight: bold; }
.msg-thinking { background: #2a2a20; border-left: 3px solid #c0c040; }
.msg-thinking-prefix { color: #c0c040; font-weight: bold; }
.msg-thinking-content { color: #888; margin-top: 4px; padding-left: 4px; }
.msg-streaming { color: #ccc; }
.msg-tool { background: #1a2a3a; border-left: 3px solid #4080c0; }
.msg-tool-name { color: #4080c0; font-weight: bold; }
.msg-tool-arg { color: #888; font-size: 12px; margin-top: 2px; padding-left: 4px; }
.msg-tool-result { background: #1a2a1a; border-left: 3px solid #40c040; }
.msg-tool-result-error { background: #2a1a1a; border-left: 3px solid #c04040; }
.msg-tool-result-prefix { font-weight: bold; }
.msg-tool-result-content { color: #888; font-size: 12px; margin-top: 4px; padding-left: 4px; max-height: 120px; overflow-y: auto; font-family: monospace; }
.msg-answer { color: #e0e0e0; line-height: 1.5; }
.msg-summary { color: #80c080; font-weight: bold; padding: 6px 0; }
.msg-error { color: #ff6060; font-weight: bold; background: #2a1a1a; border-left: 3px solid #c04040; }
.input-area { border-top: 1px solid #333355; padding: 8px 10px; background: #252540; flex-shrink: 0; }
.input-row { display: flex; gap: 8px; }
.input-area textarea { flex: 1; background: #1a1a2e; color: #e0e0e0; border: 1px solid #444466; border-radius: 4px; padding: 6px 8px; font-size: 14px; font-family: inherit; resize: none; min-height: 40px; max-height: 120px; outline: none; }
.input-area textarea:focus { border-color: #80a0ff; }
.input-area textarea:disabled { opacity: 0.5; }
.input-area button { padding: 6px 16px; background: #4060c0; color: #e0e0e0; border: none; border-radius: 4px; cursor: pointer; font-size: 14px; align-self: flex-end; }
.input-area button:hover { background: #5070d0; }
.input-area button:disabled { background: #333355; cursor: not-allowed; }
.input-hint { margin-top: 4px; font-size: 11px; color: #666; }
.input-hint-key { color: #80a0ff; font-weight: bold; }
.input-hint-running { color: #f0c040; }
.workspace-panel { flex: 1; overflow-y: auto; padding: 10px; }
.workspace-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.workspace-entry { padding: 2px 0; font-family: monospace; font-size: 13px; }
.workspace-dir { color: #6090ff; font-weight: bold; }
.workspace-file { color: #e0e0e0; }
.workspace-file-modified { color: #c0c040; }
.skills-panel { flex: 1; overflow-y: auto; padding: 10px; }
.skills-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.skills-table { width: 100%; border-collapse: collapse; }
.skills-table th { text-align: left; padding: 4px 8px; border-bottom: 1px solid #333355; font-size: 12px; color: #888; }
.skills-table td { padding: 4px 8px; font-size: 13px; border-bottom: 1px solid #2a2a44; }
.log-viewer { flex: 1; overflow-y: auto; padding: 10px; }
.log-run-list { font-family: monospace; font-size: 13px; }
.log-run-item { padding: 3px 0; color: #ccc; }
.log-run-item-id { color: #c0c0c0; }
.log-run-item-count { color: #888; }
.log-entry { font-family: monospace; font-size: 12px; padding: 2px 0; white-space: nowrap; }
.log-entry-time { color: #666; }
.log-entry-type { font-weight: bold; }
.log-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.modal-overlay { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0, 0, 0, 0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
.modal-content { background: #252540; border: 1px solid #444466; border-radius: 8px; padding: 16px; min-width: 400px; max-width: 600px; max-height: 80vh; overflow-y: auto; }
.modal-title { font-size: 16px; font-weight: bold; color: #e0e0e0; margin-bottom: 12px; border-bottom: 1px solid #333355; padding-bottom: 8px; }
.modal-empty { color: #888; padding: 10px 0; }
.modal-session-item { padding: 6px 8px; border-bottom: 1px solid #2a2a44; display: flex; align-items: center; gap: 8px; }
.modal-session-item.selected { background: #2a2a44; }
.modal-session-id { font-family: monospace; color: #e0e0e0; font-weight: bold; }
.modal-session-meta { color: #888; font-size: 12px; }
.modal-actions { margin-top: 12px; display: flex; gap: 8px; padding-top: 8px; border-top: 1px solid #333355; }
.modal-actions button { padding: 6px 12px; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; }
.btn-new { background: #4060c0; color: #e0e0e0; }
.btn-resume { background: #408040; color: #e0e0e0; }
.btn-delete { background: #804040; color: #e0e0e0; }
.btn-cancel { background: #555; color: #e0e0e0; }
.btn-approve { background: #408040; color: #e0e0e0; }
.btn-reject { background: #804040; color: #e0e0e0; }
.btn-stop { background: #662020; color: #e0e0e0; }
.approval-tool-name { color: #f0c040; font-weight: bold; font-size: 15px; }
.approval-reason { color: #ccc; margin: 6px 0; }
.approval-args { font-family: monospace; font-size: 12px; color: #888; background: #1a1a2e; padding: 6px 8px; border-radius: 4px; margin: 8px 0; max-height: 100px; overflow-y: auto; white-space: pre-wrap; }

/* Tools tab */
.tools-tab { flex: 1; overflow-y: auto; padding: 8px; }
.tools-tab-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.tool-call-item { border-bottom: 1px solid #2a2a44; }
.tool-call-header { display: flex; align-items: center; padding: 8px 10px; cursor: pointer; gap: 8px; }
.tool-call-header:hover { background: #222240; }
.tool-call-seq { color: #555; font-size: 11px; min-width: 24px; }
.tool-call-name { font-weight: 600; font-size: 13px; }
.tool-call-status { font-size: 11px; padding: 1px 6px; border-radius: 3px; }
.tool-call-duration { font-size: 11px; color: #888; margin-left: auto; }
.tool-call-chevron { font-size: 10px; color: #666; margin-left: 4px; }
.tool-call-detail { padding: 8px 10px 8px 42px; font-size: 12px; font-family: monospace; color: #888; background: #16162a; white-space: pre-wrap; word-break: break-all; }
.tool-detail-label { color: #6090ff; font-weight: 600; font-family: sans-serif; }

/* File content viewer */
.file-content-view { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.file-tab-bar { display: flex; background: #1e1e38; border-bottom: 1px solid #2a2a44; flex-shrink: 0; overflow-x: auto; }
.file-tab { padding: 4px 8px; font-size: 12px; color: #777; display: flex; align-items: center; gap: 4px; cursor: pointer; border-bottom: 2px solid transparent; white-space: nowrap; }
.file-tab:hover { color: #bbb; background: #222240; }
.file-tab.active { color: #e0e0e0; background: #1a1a2e; border-bottom-color: #80a0ff; }
.file-tab-icon { font-size: 13px; }
.file-tab-name { max-width: 150px; overflow: hidden; text-overflow: ellipsis; }
.file-tab-close { font-size: 10px; color: #555; padding: 0 2px; border-radius: 2px; line-height: 1; }
.file-tab-close:hover { color: #ff6060; background: #3a2020; }
.file-content { flex: 1; overflow: auto; padding: 12px; font-family: 'JetBrains Mono', 'Fira Code', monospace; font-size: 12px; line-height: 1.6; color: #c8c8e0; background: #1a1a2e; white-space: pre; margin: 0; }
.file-content-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.file-content-error { padding: 12px; color: #ff6060; font-weight: bold; }
.file-content-loading { display: flex; align-items: center; justify-content: center; height: 100%; color: #888; }

@media (max-width: 1024px) {
    .sidebar { width: 33.33%; min-width: 200px; }
    .tab { padding: 6px 12px; font-size: 12px; }
}
@media (max-width: 768px) {
    .main-layout { flex-direction: row; }
    .sidebar { width: 33.33%; min-width: 160px; border-right: 1px solid #333355; border-bottom: none; }
    .right-panel { flex: 1; min-width: 0; }
    .tab { padding: 6px 10px; }
    .modal-content { min-width: auto; width: 90vw; max-width: 500px; }
}
@media (max-width: 480px) {
    .app-container { height: 100dvh; }
    .status-bar { font-size: 10px; padding: 3px 8px; }
    .tab-bar { overflow-x: auto; }
    .tab { padding: 6px 8px; font-size: 11px; white-space: nowrap; }
    .input-area { padding: 6px 8px; }
    .input-area button { padding: 6px 12px; font-size: 13px; }
    .main-layout { flex-direction: row; }
    .sidebar { width: 40%; min-width: 120px; max-height: none; border-right: 1px solid #333355; border-bottom: none; }
    .right-panel { flex: 1; min-width: 0; }
    .modal-content { padding: 12px; }
}
"#;
