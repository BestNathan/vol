mod workspace;

pub use workspace::scan_workspace;

#[cfg(feature = "tui")]
mod event_buffer;

#[cfg(feature = "tui")]
pub use event_buffer::EventBuffer;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(all(feature = "web", not(feature = "tui")))]
use web_time::Instant;
#[cfg(feature = "tui")]
use std::time::Instant;

// === Unified Event Type ======================================================

/// All agent and UI events flow through this type.
/// Local mode: AgentStreamEvent → UiEvent (via EventBuffer, implemented later).
/// Remote mode: JSON-RPC notification → UiEvent (deserialized from WS).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    // Agent lifecycle
    AgentStart { input: String },
    AgentComplete { response: String },
    AgentAborted { reason: String },
    AgentError { message: String },

    // Thinking
    ThinkingStart,
    ThinkingDelta { delta: String },
    ThinkingComplete,

    // LLM call
    LlmCallStart { iteration: u32 },
    LlmCallComplete { model: String },
    LlmCallError { error: String },

    // Content
    ContentStart,
    ContentDelta { delta: String },
    ContentComplete { content: String },

    // Tools
    ToolCallBegin { tool_name: String, arguments: String },
    ToolCallArgumentDelta { delta: String },
    ToolCallComplete { tool_name: String, result: String, duration_ms: Option<u64> },
    ToolCallError { tool_name: String, error: String, duration_ms: Option<u64> },
    ToolCallSkipped { tool_name: String, reason: String, duration_ms: Option<u64> },

    // Iteration
    MaxIterationsReached { current: u32, max: u32 },
    IterationContinued { from_iteration: u32 },
    IterationComplete { iteration: u32, final_answer: Option<String> },

    // HITL
    ApprovalRequest { tool_name: String, reason: String, arguments: String },
    ApprovalResolved { approved: bool },

    // WebSocket connection state
    WsConnected,
    WsConnecting,
    WsDisconnected { reason: Option<String> },
}

/// Coarse-grained event type for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiEventKind {
    AgentStart, AgentComplete, AgentAborted, AgentError,
    ThinkingStart, ThinkingDelta, ThinkingComplete,
    LlmCallStart, LlmCallComplete, LlmCallError,
    ContentStart, ContentDelta, ContentComplete,
    ToolCallBegin, ToolCallArgumentDelta, ToolCallComplete, ToolCallError, ToolCallSkipped,
    ApprovalRequest, ApprovalResolved,
    IterationComplete, IterationContinued, MaxIterationsReached,
    WsConnected, WsConnecting, WsDisconnected,
}

impl UiEvent {
    pub fn kind(&self) -> UiEventKind {
        match self {
            UiEvent::AgentStart { .. } => UiEventKind::AgentStart,
            UiEvent::AgentComplete { .. } => UiEventKind::AgentComplete,
            UiEvent::AgentAborted { .. } => UiEventKind::AgentAborted,
            UiEvent::AgentError { .. } => UiEventKind::AgentError,
            UiEvent::ThinkingStart => UiEventKind::ThinkingStart,
            UiEvent::ThinkingDelta { .. } => UiEventKind::ThinkingDelta,
            UiEvent::ThinkingComplete => UiEventKind::ThinkingComplete,
            UiEvent::LlmCallStart { .. } => UiEventKind::LlmCallStart,
            UiEvent::LlmCallComplete { .. } => UiEventKind::LlmCallComplete,
            UiEvent::LlmCallError { .. } => UiEventKind::LlmCallError,
            UiEvent::ContentStart => UiEventKind::ContentStart,
            UiEvent::ContentDelta { .. } => UiEventKind::ContentDelta,
            UiEvent::ContentComplete { .. } => UiEventKind::ContentComplete,
            UiEvent::ToolCallBegin { .. } => UiEventKind::ToolCallBegin,
            UiEvent::ToolCallArgumentDelta { .. } => UiEventKind::ToolCallArgumentDelta,
            UiEvent::ToolCallComplete { .. } => UiEventKind::ToolCallComplete,
            UiEvent::ToolCallError { .. } => UiEventKind::ToolCallError,
            UiEvent::ToolCallSkipped { .. } => UiEventKind::ToolCallSkipped,
            UiEvent::ApprovalRequest { .. } => UiEventKind::ApprovalRequest,
            UiEvent::ApprovalResolved { .. } => UiEventKind::ApprovalResolved,
            UiEvent::IterationComplete { .. } => UiEventKind::IterationComplete,
            UiEvent::IterationContinued { .. } => UiEventKind::IterationContinued,
            UiEvent::MaxIterationsReached { .. } => UiEventKind::MaxIterationsReached,
            UiEvent::WsConnected => UiEventKind::WsConnected,
            UiEvent::WsConnecting => UiEventKind::WsConnecting,
            UiEvent::WsDisconnected { .. } => UiEventKind::WsDisconnected,
        }
    }
}

// === Display Types ===========================================================

#[derive(Debug, Clone)]
pub enum ToolCallStatus { Running, Success, Error, Skipped }

#[derive(Debug, Clone)]
pub struct ToolCallEntry {
    pub sequence: u32,
    pub tool_name: String,
    pub arg_preview: String,
    pub status: ToolCallStatus,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversationEntry {
    UserInput { text: String },
    Thinking { content: String },
    LlmCall { iteration: u32, model: String },
    ContentStreaming { content: String },
    ToolCall { tool_name: String, arg_preview: String },
    ToolResult { tool_name: String, preview: String, success: bool },
    AgentAnswer { text: String },
    RunSummary { iterations: u32, tool_calls: u32, elapsed_ms: u128 },
    EntryCheckpoint { reason: String, note: Option<String>, created_at: i64 },
    Error { message: String },
}

/// A node in the workspace directory tree.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub loaded: bool,
    pub load_error: bool,
    pub children: Vec<WorkspaceTreeNode>,
}

impl WorkspaceTreeNode {
    pub fn root(name: String, path: String) -> Self {
        Self {
            name,
            path,
            is_dir: true,
            loaded: false,
            load_error: false,
            children: Vec::new(),
        }
    }

    pub fn find_child_mut(&mut self, path: &str) -> Option<&mut Self> {
        if self.path == path { return Some(self); }
        for child in &mut self.children {
            if let Some(found) = child.find_child_mut(path) { return Some(found); }
        }
        None
    }

    pub fn replace_dir_children(&mut self, dir_path: &str, entries: Vec<(String, bool)>) {
        if let Some(node) = self.find_child_mut(dir_path) {
            node.children.clear();
            for (name, is_dir) in entries {
                let child_path = if dir_path == "." || dir_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", dir_path, name)
                };
                node.children.push(WorkspaceTreeNode {
                    name,
                    path: child_path,
                    is_dir,
                    loaded: true,
                    load_error: false,
                    children: Vec::new(),
                });
            }
            node.loaded = true;
            node.load_error = false;
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillDisplayEntry {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
}

/// Full skill detail returned by skill.get RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDetail {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub content: String,
    pub file_listing: Vec<String>,
    pub directory: String,
}

#[derive(Debug, Clone)]
pub struct OpenFileTab {
    pub path: String,
    pub content: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab { Conversation, Sessions, Tools, Workspace, Skills, Mcp, Logs, Agents }

impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Sessions,
            ActiveTab::Sessions => ActiveTab::Tools,
            ActiveTab::Tools => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Mcp,
            ActiveTab::Mcp => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Agents,
            ActiveTab::Agents => ActiveTab::Conversation,
        }
    }
}

/// Sub-tabs within the MCP panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum McpSubtab { Servers, Tools, Resources, Prompts }

#[derive(Debug, Clone)]
pub struct SessionDialogEntry {
    pub session_id: String,
    pub entry_count: usize,
    pub age_label: String,
}

// === ApprovalState ===========================================================

pub struct ApprovalState {
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub arguments: Option<String>,
    pub response: Option<(bool, Option<String>)>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self {
            tool_name: None,
            reason: None,
            arguments: None,
            response: None,
        }
    }

    pub fn has_pending(&self) -> bool {
        self.tool_name.is_some()
    }

    pub fn clear(&mut self) {
        self.tool_name = None;
        self.reason = None;
        self.arguments = None;
        self.response = None;
    }
}

// === Log Types ===============================================================

#[derive(Debug, Clone)]
pub struct LogLine {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub struct LogRunSummary {
    pub run_id: String,
    pub event_count: usize,
    pub last_event: String,
    pub last_event_time: String,
}

// === EventBus (web only) =====================================================

#[cfg(all(feature = "web", not(feature = "tui")))]
use std::collections::HashMap;
#[cfg(all(feature = "web", not(feature = "tui")))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(all(feature = "web", not(feature = "tui")))]
use std::sync::{Arc, Mutex};

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

#[cfg(all(feature = "web", not(feature = "tui")))]
type EventHandler = Box<dyn Fn(&UiEvent) + 'static>;

#[cfg(all(feature = "web", not(feature = "tui")))]
struct Subscriber { id: SubscriptionId, handler: EventHandler }

#[cfg(all(feature = "web", not(feature = "tui")))]
struct EventBusInner {
    next_id: AtomicU64,
    subscribers: Mutex<HashMap<UiEventKind, Vec<Subscriber>>>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl EventBusInner {
    fn subscribe<F>(&self, kind: UiEventKind, handler: F) -> SubscriptionId
    where F: Fn(&UiEvent) + 'static {
        let id = SubscriptionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let mut subs = self.subscribers.lock().unwrap();
        subs.entry(kind).or_default().push(Subscriber { id, handler: Box::new(handler) });
        id
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Clone)]
pub struct EventBus { inner: Arc<EventBusInner> }

#[cfg(all(feature = "web", not(feature = "tui")))]
impl EventBus {
    pub fn new() -> Self {
        Self { inner: Arc::new(EventBusInner { next_id: AtomicU64::new(0), subscribers: Mutex::new(HashMap::new()) }) }
    }
    pub fn subscribe<F>(&self, kind: UiEventKind, handler: F) -> SubscriptionId
    where F: Fn(&UiEvent) + 'static {
        self.inner.subscribe(kind, handler)
    }
    pub fn publish(&self, event: &UiEvent) {
        let kind = event.kind();
        let subs = self.inner.subscribers.lock().unwrap();
        if let Some(handlers) = subs.get(&kind) {
            for sub in handlers { (sub.handler)(event); }
        }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
pub struct SubscriptionSet {
    ids: Vec<(UiEventKind, SubscriptionId)>,
    bus: Arc<EventBusInner>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SubscriptionSet {
    pub fn new(bus: EventBus) -> Self {
        Self { ids: Vec::new(), bus: bus.inner.clone() }
    }
    pub fn subscribe<F>(&mut self, _bus: &EventBus, kind: UiEventKind, handler: F)
    where F: Fn(&UiEvent) + 'static {
        let id = self.bus.subscribe(kind, handler);
        self.ids.push((kind, id));
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl Drop for SubscriptionSet {
    fn drop(&mut self) {
        let mut subs = self.bus.subscribers.lock().unwrap();
        for (kind, id) in &self.ids {
            if let Some(list) = subs.get_mut(kind) { list.retain(|s| s.id != *id); }
        }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
pub trait HasReducer<T> {
    fn reduce(state: &mut T, event: &UiEvent) -> bool;
}

// === Per-Component Local State (web only) =====================================

/// Local state for StatusBar — global run/session/connection info.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct GlobalState {
    pub session_id: String,
    pub run_count: u32,
    pub iteration: u32,
    pub tool_call_count: u32,
    pub run_start: Option<Instant>,
    pub run_elapsed: std::time::Duration,
    pub is_running: bool,
    pub exiting: bool,
    pub ws_url: String,
    pub ws_connected: bool,
    pub ws_last_error: Option<String>,
    pub unsafe_mode: bool,
    pub active_tab: ActiveTab,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl GlobalState {
    pub fn new(ws_url: String) -> Self {
        Self {
            session_id: "web-session".into(), run_count: 0, iteration: 0,
            tool_call_count: 0, run_start: None, run_elapsed: std::time::Duration::ZERO,
            is_running: false, exiting: false, ws_url, ws_connected: false,
            ws_last_error: None, unsafe_mode: false, active_tab: ActiveTab::Conversation,
        }
    }
}

/// Local state for ConversationView.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct ConversationState {
    pub entries: Vec<ConversationEntry>,
    pub conversation_scroll: u16,
    pub auto_scroll: bool,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl ConversationState {
    pub fn new() -> Self {
        Self { entries: Vec::new(), conversation_scroll: 0, auto_scroll: true }
    }
}

/// Local state for ToolsPanel and ToolsTabContent.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct ToolState {
    pub calls: Vec<ToolCallEntry>,
    pub expanded: HashSet<usize>,
    pub scroll: u16,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl ToolState {
    pub fn new() -> Self {
        Self { calls: Vec::new(), expanded: HashSet::new(), scroll: 0 }
    }
}

/// Local state for FileTree and FileContentView.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct WorkspaceState {
    pub workspace: WorkspaceTreeNode,
    pub modified_files: HashSet<String>,
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,
    pub collapsed_dirs: HashSet<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl WorkspaceState {
    pub fn new(working_dir: &str) -> Self {
        Self {
            workspace: WorkspaceTreeNode::root(working_dir.to_string(), ".".into()),
            modified_files: HashSet::new(), open_files: Vec::new(),
            selected_file_tab: None, collapsed_dirs: HashSet::new(),
        }
    }
}

/// Local state for SkillsPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct SkillsState {
    pub skills: Vec<SkillDisplayEntry>,
    pub error: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SkillsState {
    pub fn new() -> Self {
        Self { skills: Vec::new(), error: None }
    }
}

/// Dialog state for viewing a skill's full details.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug, Clone)]
pub struct SkillDialogState {
    pub open: bool,
    pub skill: Option<SkillDetail>,
    pub loading: bool,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SkillDialogState {
    pub fn new() -> Self {
        Self { open: false, skill: None, loading: false }
    }
}

/// Local state for LogViewer.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct LogViewerState {
    pub selected_run: Option<String>, pub entries: Vec<LogLine>,
    pub scroll: u16, pub auto_scroll: bool, pub run_logs: Vec<LogRunSummary>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl LogViewerState {
    pub fn new() -> Self {
        Self { selected_run: None, entries: Vec::new(), scroll: 0, auto_scroll: true, run_logs: Vec::new() }
    }
}

/// A single agent entry returned by agent.list RPC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentListEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub description: String,
    pub scope: String,
}

/// MCP server info returned by mcp.list_servers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub status: String,
}

/// MCP tool info returned by mcp.list_tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub server: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

/// MCP resource info returned by mcp.list_resources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpResourceInfo {
    pub server: String,
    pub name: String,
    pub uri: String,
    pub mime_type: Option<String>,
    pub description: Option<String>,
}

/// MCP resource template info returned by mcp.list_resource_templates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpResourceTemplateInfo {
    pub server: String,
    pub name: String,
    pub uri_template: String,
    pub description: Option<String>,
}

/// MCP prompt info returned by mcp.list_prompts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpPromptInfo {
    pub server: String,
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<McpPromptArgInfo>>,
}

/// MCP prompt argument definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpPromptArgInfo {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

/// Local state for AgentsPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct AgentsState {
    pub agents: Vec<crate::web::client::AgentListEntry>,
    pub expanded: HashSet<usize>,
    pub loading: bool,
    pub error: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl AgentsState {
    pub fn new() -> Self {
        Self { agents: Vec::new(), expanded: HashSet::new(), loading: false, error: None }
    }
}

/// Session list entry from session.list RPC.
#[derive(Debug, Clone)]
pub struct SessionListEntry {
    pub id: String,
    pub entry_count: usize,
    pub created_at: i64,
}

/// Local state for SessionsPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct SessionsState {
    pub sessions: Vec<SessionListEntry>,
    pub loading: bool,
    pub error: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SessionsState {
    pub fn new() -> Self {
        Self { sessions: Vec::new(), loading: false, error: None }
    }
}

/// MCP server display row with reconnect state.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct McpServerRowState {
    pub name: String,
    pub status: String,
    pub reconnecting: bool,
}

/// Local state for McpPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct McpState {
    pub servers: Vec<McpServerInfo>,
    pub tools: Vec<McpToolInfo>,
    pub resources: Vec<McpResourceInfo>,
    pub resource_templates: Vec<McpResourceTemplateInfo>,
    pub prompts: Vec<McpPromptInfo>,
    pub loading: bool,
    pub error: Option<String>,
    pub active_subtab: McpSubtab,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl McpState {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(), tools: Vec::new(), resources: Vec::new(),
            resource_templates: Vec::new(), prompts: Vec::new(),
            loading: true, error: None, active_subtab: McpSubtab::Servers,
        }
    }
}

/// Dialog state for MCP panel — managed at App level so dialogs render outside overflow containers.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Clone, Debug, Default)]
pub struct McpDialogState {
    pub tool_call_dialog: Option<McpToolCallState>,
    pub resource_viewer: Option<McpResourceViewerState>,
    pub prompt_viewer: Option<McpPromptViewerState>,
}

/// State for the tool call dialog.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Clone, Debug)]
pub struct McpToolCallState {
    pub server: String,
    pub tool_name: String,
    pub arguments_json: String,
    pub input_schema: Option<serde_json::Value>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

/// State for the resource viewer.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Clone, Debug)]
pub struct McpResourceViewerState {
    pub uri: String,
    pub content: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

/// State for the prompt viewer.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Clone, Debug)]
pub struct McpPromptViewerState {
    pub server: String,
    pub prompt_name: String,
    pub args_json: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

/// Local state for SessionDialog.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct SessionDialogState {
    pub open: bool, pub sessions: Vec<SessionDialogEntry>, pub selected: usize,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SessionDialogState {
    pub fn new() -> Self {
        Self { open: false, sessions: Vec::new(), selected: 0 }
    }
}

/// Shared state for ApprovalDialog (created by App, read via context).
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct ApprovalUiState {
    pub tool_name: Option<String>, pub reason: Option<String>, pub arguments: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl ApprovalUiState {
    pub fn new() -> Self {
        Self { tool_name: None, reason: None, arguments: None }
    }
    pub fn has_pending(&self) -> bool { self.tool_name.is_some() }
    pub fn clear(&mut self) {
        self.tool_name = None; self.reason = None; self.arguments = None;
    }
}

// === UiState =================================================================

pub struct UiState {
    pub session_id: String,
    pub run_count: u32,
    pub iteration: u32,
    pub tool_call_count: u32,
    pub run_start: Option<Instant>,
    pub run_elapsed: std::time::Duration,
    pub is_running: bool,
    pub exiting: bool,
    pub conversation: Vec<ConversationEntry>,
    pub tool_calls: Vec<ToolCallEntry>,
    pub workspace: WorkspaceTreeNode,
    pub modified_files: HashSet<String>,
    pub active_tab: ActiveTab,
    pub conversation_scroll: u16,
    pub workspace_scroll: u16,
    pub tools_scroll: u16,
    pub conversation_auto_scroll: bool,
    pub approval_state: ApprovalState,
    pub session_dialog_open: bool,
    pub session_dialog_sessions: Vec<SessionDialogEntry>,
    pub session_dialog_selected: usize,
    pub log_viewer_selected_run: Option<String>,
    pub log_viewer_entries: Vec<LogLine>,
    pub log_viewer_scroll: u16,
    pub log_viewer_auto_scroll: bool,
    pub log_viewer_run_logs: Vec<LogRunSummary>,
    pub skills: Vec<SkillDisplayEntry>,
    pub unsafe_mode: bool,
    pub last_error: Option<String>,
    pub ws_url: String,
    pub ws_connected: bool,
    pub ws_last_error: Option<String>,
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,
    pub collapsed_dirs: HashSet<String>,
    pub expanded_tool_calls: HashSet<usize>,
}

impl UiState {
    pub fn new(session_id: String, working_dir: &str, url: &str) -> Self {
        Self {
            session_id,
            run_count: 0,
            iteration: 0,
            tool_call_count: 0,
            run_start: None,
            run_elapsed: std::time::Duration::ZERO,
            is_running: false,
            exiting: false,
            conversation: Vec::new(),
            tool_calls: Vec::new(),
            workspace: WorkspaceTreeNode::root(working_dir.to_string(), ".".into()),
            modified_files: HashSet::new(),
            active_tab: ActiveTab::Conversation,
            conversation_scroll: 0,
            workspace_scroll: 0,
            tools_scroll: 0,
            conversation_auto_scroll: true,
            approval_state: ApprovalState::new(),
            session_dialog_open: false,
            session_dialog_sessions: Vec::new(),
            session_dialog_selected: 0,
            log_viewer_selected_run: None,
            log_viewer_entries: Vec::new(),
            log_viewer_scroll: 0,
            log_viewer_auto_scroll: true,
            log_viewer_run_logs: Vec::new(),
            skills: Vec::new(),
            unsafe_mode: false,
            last_error: None,
            ws_url: url.to_string(),
            ws_connected: false,
            ws_last_error: None,
            open_files: Vec::new(),
            selected_file_tab: None,
            collapsed_dirs: HashSet::new(),
            expanded_tool_calls: HashSet::new(),
        }
    }

    pub fn reset_for_run(&mut self) {
        self.iteration = 0;
        self.tool_call_count = 0;
        self.run_start = Some(Instant::now());
        self.run_elapsed = std::time::Duration::ZERO;
        self.tool_calls.clear();
        self.modified_files.clear();
        self.tools_scroll = 0;
        self.run_count += 1;
    }

    /// Apply a UiEvent to mutate state.
    pub fn apply(&mut self, event: UiEvent) {
        match event {
            UiEvent::AgentStart { input } => {
                self.reset_for_run();
                self.is_running = true;
                self.conversation.push(ConversationEntry::UserInput { text: input });
            }
            UiEvent::AgentComplete { response } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::RunSummary {
                    iterations: self.iteration,
                    tool_calls: self.tool_call_count,
                    elapsed_ms: elapsed.as_millis(),
                });
                if !response.is_empty() {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: response });
                }
                self.is_running = false;
            }
            UiEvent::AgentAborted { reason } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::Error { message: reason });
                self.is_running = false;
            }
            UiEvent::AgentError { message } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::Error { message });
                self.is_running = false;
            }
            UiEvent::ThinkingStart => {
                self.conversation.push(ConversationEntry::Thinking { content: String::new() });
            }
            UiEvent::ThinkingDelta { delta } => {
                if let Some(ConversationEntry::Thinking { content }) = self.conversation.last_mut() {
                    content.push_str(&delta);
                }
            }
            UiEvent::ThinkingComplete => {
                // No-op — thinking content already streamed via deltas
            }
            UiEvent::LlmCallStart { iteration } => {
                self.conversation.push(ConversationEntry::LlmCall { iteration, model: String::new() });
            }
            UiEvent::LlmCallComplete { model } => {
                if let Some(ConversationEntry::LlmCall { model: m, .. }) = self.conversation.last_mut() {
                    *m = model.clone();
                }
            }
            UiEvent::LlmCallError { error } => {
                self.conversation.push(ConversationEntry::Error { message: format!("LLM error: {error}") });
            }
            UiEvent::ContentStart => {
                self.conversation.push(ConversationEntry::ContentStreaming { content: String::new() });
            }
            UiEvent::ContentDelta { delta } => {
                if let Some(ConversationEntry::ContentStreaming { content }) = self.conversation.last_mut() {
                    content.push_str(&delta);
                }
            }
            UiEvent::ContentComplete { content } => {
                if let Some(ConversationEntry::ContentStreaming { .. }) = self.conversation.last() {
                    let entry = self.conversation.last_mut().unwrap();
                    *entry = ConversationEntry::AgentAnswer { text: content };
                } else if !content.is_empty() {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: content });
                }
            }
            UiEvent::ToolCallBegin { tool_name, arguments } => {
                let seq = self.tool_call_count + 1;
                self.tool_call_count = seq;
                let preview = extract_arg_preview(&arguments);
                self.tool_calls.push(ToolCallEntry {
                    sequence: seq,
                    tool_name: tool_name.clone(),
                    arg_preview: preview.clone(),
                    status: ToolCallStatus::Running,
                    duration_ms: None,
                });
                self.conversation.push(ConversationEntry::ToolCall {
                    tool_name,
                    arg_preview: preview,
                });
            }
            UiEvent::ToolCallArgumentDelta { delta: _ } => {
                // Invisible in UI
            }
            UiEvent::ToolCallComplete { tool_name, result, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Success, duration_ms);
                let preview = truncate_preview(&result, 200);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview,
                    success: true,
                });
            }
            UiEvent::ToolCallError { tool_name, error, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Error, duration_ms);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview: error,
                    success: false,
                });
            }
            UiEvent::ToolCallSkipped { tool_name, reason, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Skipped, duration_ms);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview: reason,
                    success: false,
                });
            }
            UiEvent::MaxIterationsReached { current, max } => {
                self.conversation.push(ConversationEntry::Error {
                    message: format!("Max iterations reached ({}/{}) — waiting for user decision...", current, max),
                });
            }
            UiEvent::IterationContinued { from_iteration } => {
                self.iteration = from_iteration;
                self.conversation.push(ConversationEntry::AgentAnswer {
                    text: format!("Continuing from iteration {} (counter reset to 0)", from_iteration),
                });
            }
            UiEvent::IterationComplete { iteration, final_answer } => {
                self.iteration = iteration;
                if let Some(answer) = final_answer {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: answer });
                }
            }
            UiEvent::ApprovalRequest { tool_name, reason, arguments } => {
                self.approval_state.tool_name = Some(tool_name);
                self.approval_state.reason = Some(reason);
                self.approval_state.arguments = Some(arguments);
            }
            UiEvent::ApprovalResolved { approved: _ } => {
                self.approval_state.clear();
            }
            UiEvent::WsConnected | UiEvent::WsConnecting | UiEvent::WsDisconnected { .. } => {
                // Connection state handled separately via shared GlobalState signal
            }
        }

        // Auto-scroll
        if self.conversation_auto_scroll {
            self.conversation_scroll = 0;
        }
        self.tools_scroll = self.tool_calls.len() as u16;
    }

    fn flush_pending_content(&mut self) {
        if let Some(ConversationEntry::ContentStreaming { content }) = self.conversation.last() {
            let text = content.clone();
            if !text.is_empty() {
                let entry = self.conversation.last_mut().unwrap();
                *entry = ConversationEntry::AgentAnswer { text };
            }
        }
    }

    fn update_tool_call_status(&mut self, tool_name: &str, status: ToolCallStatus, duration_ms: Option<u64>) {
        // Match the most recent running entry for this tool by sequence (last-written wins).
        for entry in self.tool_calls.iter_mut().rev() {
            if entry.tool_name == tool_name && matches!(entry.status, ToolCallStatus::Running) {
                entry.status = status;
                entry.duration_ms = duration_ms;
                break;
            }
        }
    }

    /// Compute current elapsed time (works mid-run and after completion).
    pub fn elapsed(&self) -> std::time::Duration {
        if self.is_running {
            self.run_start.map(|s| s.elapsed()).unwrap_or(self.run_elapsed)
        } else {
            self.run_elapsed
        }
    }
}

// === Helpers =================================================================

fn extract_arg_preview(arguments: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
            if cmd.chars().count() > 80 {
                return format!("Command: {}...", cmd.chars().take(77).collect::<String>());
            }
            return format!("Command: {}", cmd);
        }
        if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
            return format!("Path: {}", path);
        }
        if let Some(file_path) = parsed.get("file_path").and_then(|v| v.as_str()) {
            return format!("File: {}", file_path);
        }
        if arguments.chars().count() > 80 {
            return format!("Args: {}...", arguments.chars().take(77).collect::<String>());
        }
        return format!("Args: {}", arguments);
    }
    String::new()
}

fn truncate_preview(s: &str, max_chars: usize) -> String {
    let total_chars = s.chars().count();
    if total_chars <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{}...", truncated)
}

// === Tests ===================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_event_agent_start_serializes() {
        let event = UiEvent::AgentStart { input: "hello".into() };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"agent_start""#));
        assert!(json.contains(r#""input":"hello""#));
    }

    #[test]
    fn test_ui_event_tool_call_begin_serializes() {
        let event = UiEvent::ToolCallBegin {
            tool_name: "bash".into(),
            arguments: r#"{"cmd":"ls"}"#.into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"tool_call_begin""#));
        assert!(json.contains(r#""tool_name":"bash""#));
    }

    #[test]
    fn test_ui_event_deserializes_from_remote() {
        let json = r#"{"type":"content_complete","content":"The answer is 42."}"#;
        let event: UiEvent = serde_json::from_str(json).unwrap();
        match event {
            UiEvent::ContentComplete { content } => assert_eq!(content, "The answer is 42."),
            _ => panic!("Expected ContentComplete"),
        }
    }

    #[test]
    fn test_ui_state_new_initializes() {
        let state = UiState::new("test-session".into(), "/tmp/test", "ws://localhost:3001/ws");
        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.run_count, 0);
        assert!(state.conversation.is_empty());
        assert!(state.tool_calls.is_empty());
        assert!(!state.is_running);
    }

    #[test]
    fn test_ui_state_apply_agent_start() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::AgentStart { input: "fix the bug".into() });
        assert!(state.is_running);
        assert_eq!(state.run_count, 1);
        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            ConversationEntry::UserInput { text } => assert_eq!(text, "fix the bug"),
            _ => panic!("Expected UserInput"),
        }
    }

    #[test]
    fn test_ui_state_apply_thinking_deltas() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::ThinkingStart);
        state.apply(UiEvent::ThinkingDelta { delta: "Let me ".into() });
        state.apply(UiEvent::ThinkingDelta { delta: "think...".into() });
        state.apply(UiEvent::ThinkingComplete);
        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            ConversationEntry::Thinking { content } => assert_eq!(content, "Let me think..."),
            _ => panic!("Expected Thinking"),
        }
    }

    #[test]
    fn test_ui_state_apply_tool_call_lifecycle() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::ToolCallBegin {
            tool_name: "bash".into(),
            arguments: r#"{"command":"ls"}"#.into(),
        });
        assert_eq!(state.tool_calls.len(), 1);
        assert_eq!(state.tool_call_count, 1);
        assert_eq!(state.conversation.len(), 1);

        state.apply(UiEvent::ToolCallComplete {
            tool_name: "bash".into(),
            result: "file.txt".into(),
            duration_ms: Some(42),
        });
        match &state.tool_calls[0].status {
            ToolCallStatus::Success => (),
            _ => panic!("Expected Success"),
        }
        assert_eq!(state.tool_calls[0].duration_ms, Some(42));
    }

    #[test]
    fn test_ui_state_approval_flow() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::ApprovalRequest {
            tool_name: "write".into(),
            reason: "modifying file".into(),
            arguments: r#"{"path":"test.rs"}"#.into(),
        });
        assert!(state.approval_state.has_pending());
        assert_eq!(state.approval_state.tool_name, Some("write".into()));

        state.apply(UiEvent::ApprovalResolved { approved: true });
        assert!(!state.approval_state.has_pending());
    }

    #[test]
    fn test_ui_event_kind_mapping() {
        assert_eq!(UiEvent::AgentStart { input: "hi".into() }.kind(), UiEventKind::AgentStart);
        assert_eq!(UiEvent::WsConnected.kind(), UiEventKind::WsConnected);
        assert_eq!(UiEvent::WsDisconnected { reason: None }.kind(), UiEventKind::WsDisconnected);
        assert_eq!(UiEvent::ToolCallBegin { tool_name: "x".into(), arguments: "{}".into() }.kind(), UiEventKind::ToolCallBegin);
    }

    #[test]
    fn test_active_tab_toggle() {
        use ActiveTab::*;
        assert_eq!(Conversation.toggle(), Sessions);
        assert_eq!(Sessions.toggle(), Tools);
        assert_eq!(Tools.toggle(), Workspace);
        assert_eq!(Workspace.toggle(), Skills);
        assert_eq!(Skills.toggle(), Mcp);
        assert_eq!(Mcp.toggle(), Logs);
        assert_eq!(Logs.toggle(), Agents);
        assert_eq!(Agents.toggle(), Conversation);
    }

    #[test]
    fn test_extract_arg_preview() {
        // JSON with command
        let preview = extract_arg_preview(r#"{"command":"ls -la"}"#);
        assert_eq!(preview, "Command: ls -la");

        // JSON with path
        let preview = extract_arg_preview(r#"{"path":"/tmp/test.txt"}"#);
        assert_eq!(preview, "Path: /tmp/test.txt");

        // JSON with file_path
        let preview = extract_arg_preview(r#"{"file_path":"src/main.rs"}"#);
        assert_eq!(preview, "File: src/main.rs");

        // Non-JSON or parse failure
        let preview = extract_arg_preview("not json");
        assert_eq!(preview, "");
    }

    #[test]
    fn test_workspace_tree_node_structure() {
        let mut root = WorkspaceTreeNode::root("root".into(), ".".into());
        root.children.push(WorkspaceTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            loaded: true,
            load_error: false,
            children: vec![
                WorkspaceTreeNode {
                    name: "main.rs".into(),
                    path: "src/main.rs".into(),
                    is_dir: false,
                    loaded: false,
                    load_error: false,
                    children: vec![],
                },
            ],
        });
        assert_eq!(root.children.len(), 1);
        assert!(root.children[0].is_dir);
        assert_eq!(root.children[0].children[0].name, "main.rs");
    }

    #[test]
    fn test_find_child_mut() {
        let mut root = WorkspaceTreeNode::root("root".into(), ".".into());
        root.children.push(WorkspaceTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            loaded: false,
            load_error: false,
            children: vec![],
        });
        let found = root.find_child_mut("src");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "src");
        let not_found = root.find_child_mut("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_replace_dir_children() {
        let mut root = WorkspaceTreeNode::root("root".into(), ".".into());
        root.children.push(WorkspaceTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            loaded: false,
            load_error: false,
            children: vec![],
        });
        root.replace_dir_children("src", vec![
            ("main.rs".into(), false),
            ("lib.rs".into(), false),
            ("utils".into(), true),
        ]);
        let src = root.find_child_mut("src").unwrap();
        assert_eq!(src.children.len(), 3);
        assert!(src.loaded);
        assert_eq!(src.children[0].name, "main.rs");
        assert_eq!(src.children[0].path, "src/main.rs");
        assert_eq!(src.children[2].path, "src/utils");
    }
}
