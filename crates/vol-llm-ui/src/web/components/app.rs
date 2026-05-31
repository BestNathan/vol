//! Root App component with state management, event loop, and routing.

use dioxus::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use gloo_timers::future::TimeoutFuture;

use crate::state::{ActiveTab, ApprovalUiState, AgentsState, ConversationState, DebugState, EventBus, GlobalState, SessionsState, SubscriptionSet, ToolState, UiEvent, UiEventKind, WorkspaceState};
use crate::state::McpDialogState;
use crate::state::SkillDialogState;
use crate::web::client::{AgentEvent, JsonRpcClient};

use super::agents_panel::AgentsPanel;
use super::approval_dialog::ApprovalDialog;
use super::conversation::ConversationView;
use super::file_content::FileContentView;
use super::file_tree::FileTree;
use super::log_viewer::LogViewer;
use super::mcp_panel::McpPanel;
use super::sessions_panel::SessionsPanel;
use super::skills::SkillsPanel;
use super::skill_detail_dialog::SkillDetailDialog;
use super::debug_panel::DebugPanel;
use super::status_bar::StatusBar;
use super::tools_tab::ToolsTabContent;
use super::mcp_tool_dialog::ToolCallDialog;
use super::mcp_resource_viewer::ResourceViewer;
use super::mcp_prompt_viewer::PromptViewer;

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
    let ev = &event.event;
    // AgentStreamEvent is externally-tagged: {"VariantName": {...fields}}
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

#[component]
pub fn App() -> Element {
    let ws_url = derive_ws_url();
    let event_bus = use_signal(|| EventBus::new());
    let active_tab = use_signal(|| ActiveTab::Agents);
    let global_signal = use_signal(|| GlobalState::new(ws_url.clone()));
    let approval_signal = use_signal(|| ApprovalUiState::new());
    let workspace_signal = use_signal(|| WorkspaceState::new("."));
    let conversation_signal = use_signal(|| ConversationState::new());
    let tool_signal = use_signal(|| ToolState::new());
    let agents_signal = use_signal(|| AgentsState::new());
    let sessions_signal = use_signal(|| SessionsState::new());
    let mcp_dialog_signal = use_signal(|| McpDialogState::default());
    let skill_dialog_signal = use_signal(|| SkillDialogState::new());
    let debug_signal = use_signal(|| DebugState::new());

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
                    let mut g = global_conn.write_unchecked();
                    g.ws_connected = true;
                    g.ws_last_error = None;
                    // Keep is_running — agent.status will determine actual state.
                    // Clear reconnect state
                    g.reconnecting = false;
                    g.reconnect_attempts = 0;
                    g.reconnect_maxed = false;
                }
                crate::web::client::ConnectionState::Connecting => {
                    global_conn.write_unchecked().ws_connected = false;
                }
                crate::web::client::ConnectionState::Disconnected => {
                    let mut g = global_conn.write_unchecked();
                    g.ws_connected = false;
                    g.ws_last_error = Some("Disconnected".to_string());
                    // Reset running state so input is re-enabled after disconnect.
                    g.is_running = false;
                    // Start reconnect loop if not already reconnecting and not maxed
                    if !g.reconnecting && !g.reconnect_maxed {
                        g.reconnecting = true;
                        g.reconnect_attempts = 0;
                    }
                }
            }
        });

        c.set_debug_state(debug_signal);
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
            UiEventKind::ThinkingComplete,
            UiEventKind::ContentStart, UiEventKind::ContentDelta,
            UiEventKind::ContentComplete, UiEventKind::MaxIterationsReached,
            UiEventKind::IterationContinued, UiEventKind::IterationComplete,
            UiEventKind::ToolCallBegin, UiEventKind::ToolCallComplete,
            UiEventKind::ToolCallError, UiEventKind::ToolCallSkipped,
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

    // Reconnect loop: drives client.reconnect() with exponential backoff
    let reconn_client = client.clone();
    let reconn_global = global_signal.clone();
    let reconn_bus = event_bus.with(|eb| eb.clone());
    wasm_bindgen_futures::spawn_local(async move {
        const MAX_ATTEMPTS: u32 = 10;
        const MIN_DELAY: u64 = 3;
        const MAX_DELAY: u64 = 30;

        loop {
            // Wait until reconnecting flag is set
            loop {
                let (should_reconnect, should_reset_reconnect_state) = {
                    let g = reconn_global.read();
                    (
                        g.reconnecting && !g.reconnect_maxed,
                        g.ws_connected && (g.reconnect_attempts != 0 || g.reconnect_maxed),
                    )
                };
                if should_reconnect {
                    break;
                }
                // If connected (e.g., initial connect), reset after the read guard is dropped.
                if should_reset_reconnect_state {
                    let mut gw = reconn_global.write_unchecked();
                    gw.reconnect_attempts = 0;
                    gw.reconnect_maxed = false;
                }
                TimeoutFuture::new(200).await;
            }

            for attempt in 1..=MAX_ATTEMPTS {
                let delay = (MIN_DELAY * 2u64.pow(attempt - 1)).min(MAX_DELAY);

                // Update state with countdown
                {
                    let mut g = reconn_global.write_unchecked();
                    g.reconnect_attempts = attempt;
                    g.reconnect_delay_secs = delay as u32;
                }
                reconn_bus.publish(&UiEvent::WsReconnecting {
                    attempt,
                    delay_secs: delay as u32,
                });

                // Countdown timer — update delay_secs each second
                for remaining in (1..=delay).rev() {
                    {
                        let mut g = reconn_global.write_unchecked();
                        g.reconnect_delay_secs = remaining as u32;
                    }
                    TimeoutFuture::new(1000).await;

                    // Check if connection was restored externally
                    if reconn_global.read().ws_connected {
                        return;
                    }
                    // Check if reconnect was cancelled
                    if !reconn_global.read().reconnecting {
                        return;
                    }
                }

                // Attempt reconnection
                match reconn_client.reconnect() {
                    Ok(()) => {
                        log::info!("Reconnect attempt {attempt} initiated");
                    }
                    Err(e) => {
                        log::warn!("Reconnect attempt {attempt} failed: {e}");
                    }
                }

                // Wait up to 5 seconds for the connection to establish
                for _ in 0..50 {
                    TimeoutFuture::new(100).await;
                    if reconn_global.read().ws_connected {
                        return;
                    }
                }
            }

            // All attempts exhausted
            {
                let mut g = reconn_global.write_unchecked();
                g.reconnecting = false;
                g.reconnect_maxed = true;
                g.ws_last_error = Some("Connection lost. Please refresh.".to_string());
            }
            reconn_bus.publish(&UiEvent::WsReconnectFailed);

            // Exit the loop — no more reconnect attempts
            break;
        }
    });

    // Session restoration on reconnect success
    let restore_client = client.clone();
    let restore_global = global_signal.clone();
    let restore_conv = conversation_signal.clone();
    let restore_agents = agents_signal.clone();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            // Wait for reconnection to succeed
            loop {
                {
                    let g = restore_global.read();
                    // Was reconnecting (attempts > 0), now connected
                    if g.ws_connected && g.reconnect_attempts > 0 {
                        break;
                    }
                }
                TimeoutFuture::new(200).await;
            }

            log::info!("Reconnected — restoring most recent session");

            // Fetch session list
            let (tx, rx) = futures_channel::oneshot::channel();
            restore_client.session_list(None, move |result| {
                let _ = tx.send(result);
            });
            let sessions = match rx.await {
                Ok(Ok(s)) => s,
                _ => {
                    log::warn!("Failed to fetch session list after reconnect");
                    restore_global.write_unchecked().reconnect_attempts = 0;
                    continue;
                }
            };

            if sessions.is_empty() {
                log::info!("No persisted sessions — nothing to restore");
                restore_global.write_unchecked().reconnect_attempts = 0;
                continue;
            }

            // Pick the most recent session (already sorted by time from backend)
            let latest_id = sessions[0].id.clone();
            log::info!("Restoring session: {latest_id}");

            // Resume the session
            let (tx2, rx2) = futures_channel::oneshot::channel();
            restore_client.session_resume(&latest_id, None, move |result| {
                let _ = tx2.send(result);
            });
            match rx2.await {
                Ok(Ok(_)) => {
                    log::info!("Session resumed");
                }
                _ => {
                    log::warn!("Failed to resume session");
                    restore_global.write_unchecked().reconnect_attempts = 0;
                    continue;
                }
            }

            // Fetch entries and rebuild conversation
            let (tx3, rx3) = futures_channel::oneshot::channel();
            restore_client.session_entries(&latest_id, move |result| {
                let _ = tx3.send(result);
            });
            match rx3.await {
                Ok(Ok(entries)) => {
                    let conv_entries = crate::web::components::sessions_panel::session_entries_to_conversation(entries);
                    let agent_id = restore_agents.read().selected.clone().unwrap_or_default();
                    {
                        let mut conv = restore_conv.write_unchecked();
                        let ac = conv.get_or_create(&agent_id);
                        ac.entries = conv_entries;
                    }
                    log::info!("Conversation restored from session");
                }
                _ => {
                    log::warn!("Failed to fetch session entries");
                }
            }

            // Reset reconnect state
            restore_global.write_unchecked().reconnect_attempts = 0;

            // Wait for next disconnect
            loop {
                if !restore_global.read().ws_connected {
                    break;
                }
                TimeoutFuture::new(200).await;
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
    use_context_provider(|| mcp_dialog_signal);
    use_context_provider(|| skill_dialog_signal);
    use_context_provider(|| debug_signal);

    rsx! {
        // The Stylesheet component inserts a style link into the head of the document
        document::Stylesheet {
            // Urls are relative to your Cargo.toml file
            href: asset!("/assets/tailwind.css")
        }
        div { class: "relative h-[100dvh] w-[100vw] font-[system-ui] text-[14px] text-[#e0e0e0] bg-[#1a1a2e]",
            div { class: "flex flex-col h-full w-full overflow-hidden",
                StatusBar {}
                div { class: "flex flex-1 overflow-hidden relative",
                    FileTree {}
                    div { class: "min-w-0 flex-1 flex flex-col overflow-hidden",
                        TabBar {}
                        TabContent { skill_dialog_signal }
                    }
                }
            }
            ApprovalDialog {}
            ToolCallDialog { signal: mcp_dialog_signal }
            ResourceViewer { signal: mcp_dialog_signal }
            PromptViewer { signal: mcp_dialog_signal }
            SkillDetailDialog { signal: skill_dialog_signal }
            DebugPanel {}
        }
    }
}

/// Tab bar component.
#[component]
fn TabBar() -> Element {
    let state: AppState = use_context();

    rsx! {
        div { class: "flex flex-nowrap bg-[#252540] border-b border-[#333355] flex-shrink-0 overflow-x-auto",
            TabButton { state: state.clone(), tab: ActiveTab::Agents, label: "Agents" }
            TabButton { state: state.clone(), tab: ActiveTab::Tools, label: "Tools" }
            TabButton { state: state.clone(), tab: ActiveTab::Workspace, label: "Workspace" }
            TabButton { state: state.clone(), tab: ActiveTab::Skills, label: "Skills" }
            TabButton { state: state.clone(), tab: ActiveTab::Mcp, label: "MCP" }
            TabButton { state: state.clone(), tab: ActiveTab::Logs, label: "Logs" }
        }
    }
}

#[component]
fn TabButton(state: AppState, tab: ActiveTab, label: String) -> Element {
    let current_tab = state.active_tab.read();
    let active = *current_tab == tab;
    let tab_class = if active {
        "px-2 sm:px-4 py-1 sm:py-1.5 bg-[#1a1a2e] text-[#e0e0e0] border-b-2 border-[#80a0ff] cursor-pointer text-[11px] sm:text-[13px] whitespace-nowrap flex-shrink-0"
    } else {
        "px-2 sm:px-4 py-1 sm:py-1.5 bg-transparent text-[#888] border-b-2 border-transparent cursor-pointer text-[11px] sm:text-[13px] hover:text-[#ccc] hover:bg-[#2a2a44] whitespace-nowrap flex-shrink-0"
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
fn TabContent(skill_dialog_signal: Signal<SkillDialogState>) -> Element {
    let state: AppState = use_context();
    let active = *state.active_tab.read();

    match active {
        ActiveTab::Conversation => rsx! { ConversationView {} },
        ActiveTab::Sessions => rsx! { SessionsPanel {} },
        ActiveTab::Agents => rsx! { AgentsPanel {} },
        ActiveTab::Tools => rsx! { ToolsTabContent {} },
        ActiveTab::Workspace => rsx! { FileContentView {} },
        ActiveTab::Skills => rsx! { SkillsPanel { dialog_signal: skill_dialog_signal } },
        ActiveTab::Logs => rsx! { LogViewer {} },
        ActiveTab::Mcp => rsx! { McpPanel {} },
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

#[cfg(test)]
mod tests {
    #[test]
    fn app_layout_does_not_use_a_floating_mobile_file_tree_button() {
        let source = include_str!("app.rs");
        let app_level_open_assignment = ["file_tree_drawer_open", "=", "true"].join(" ");

        assert!(!source.contains(&app_level_open_assignment));
    }

    #[test]
    fn reconnect_loop_does_not_write_global_state_while_a_read_guard_is_alive() {
        let source = include_str!("app.rs");
        let read_pos = source
            .find("let g = reconn_global.read();")
            .expect("reconnect loop should read global state");
        let search_end = (read_pos + 500).min(source.len());
        let read_scope = &source[read_pos..search_end];
        let overlapping_write = [
            "if g.ws_connected {",
            "let mut gw = reconn_global.write_unchecked();",
        ];

        assert!(
            !(read_scope.contains(overlapping_write[0])
                && read_scope.contains(overlapping_write[1])),
            "{read_scope}"
        );
    }
}
