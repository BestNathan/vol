//! Root App component with state management, event loop, and routing.

use dioxus::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use crate::state::{ActiveTab, ApprovalUiState, AgentsState, ConversationState, EventBus, GlobalState, SessionsState, SubscriptionSet, ToolState, UiEvent, UiEventKind, WorkspaceState};
use crate::web::client::{AgentEvent, JsonRpcClient};

use super::agents_panel::AgentsPanel;
use super::approval_dialog::ApprovalDialog;
use super::conversation::ConversationView;
use super::file_content::FileContentView;
use super::file_tree::FileTree;
use super::input_area::InputArea;
use super::log_viewer::LogViewer;
use super::sessions_panel::SessionsPanel;
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
    let agents_signal = use_signal(|| AgentsState::new());
    let sessions_signal = use_signal(|| SessionsState::new());

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
    use_context_provider(|| agents_signal);
    use_context_provider(|| sessions_signal);

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
}

/// Tab bar component.
#[component]
fn TabBar() -> Element {
    let state: AppState = use_context();

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
}

#[component]
fn TabButton(state: AppState, tab: ActiveTab, label: String) -> Element {
    let current_tab = state.active_tab.read();
    let active = *current_tab == tab;
    let tab_class = if active {
        "px-4 py-1.5 bg-[#1a1a2e] text-[#e0e0e0] border-b-2 border-[#80a0ff] cursor-pointer text-[13px]"
    } else {
        "px-4 py-1.5 bg-transparent text-[#888] border-b-2 border-transparent cursor-pointer text-[13px] hover:text-[#ccc] hover:bg-[#2a2a44]"
    };
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
        ActiveTab::Agents => rsx! { AgentsPanel {} },
        ActiveTab::Sessions => rsx! { SessionsPanel {} },
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
