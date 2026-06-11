use serde::{Deserialize, Serialize};
use vol_llm_agent::AgentInput;

/// Lightweight protocol error type for operation lookup and payload decoding.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error("payload decode failed for {0}")]
    PayloadDecodeFailed(&'static str),
    #[error("payload decode failed: {0}")]
    PayloadDecodeFailedOwned(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Command,
    Ack,
    Event,
    Result,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Agent(AgentOperation),
    File(FileOperation),
    Session(SessionOperation),
    Mcp(McpOperation),
    Skill(SkillOperation),
    Tool(ToolOperation),
    Log(LogOperation),
    System(SystemOperation),
    Task(TaskOperation),
    Control(ControlOperation),
}

impl Operation {
    pub fn method_name(&self) -> &'static str {
        match self {
            Operation::Agent(AgentOperation::Submit) => "agent.submit",
            Operation::Agent(AgentOperation::Cancel) => "agent.cancel",
            Operation::Agent(AgentOperation::Subscribe) => "agent.subscribe",
            Operation::Agent(AgentOperation::Unsubscribe) => "agent.unsubscribe",
            Operation::Agent(AgentOperation::Approve) => "agent.approve",
            Operation::Agent(AgentOperation::List) => "agent.list",
            Operation::Agent(AgentOperation::Event) => "agent.event",
            Operation::Agent(AgentOperation::Status) => "agent.status",
            Operation::Agent(AgentOperation::ContextConfig) => "agent.context_config",
            Operation::Agent(AgentOperation::ContextSnapshot) => "agent.context_snapshot",
            Operation::Task(TaskOperation::List) => "task.list",
            Operation::Task(TaskOperation::Get) => "task.get",
            Operation::File(FileOperation::List) => "file.list",
            Operation::File(FileOperation::Read) => "file.read",
            Operation::Session(SessionOperation::List) => "session.list",
            Operation::Session(SessionOperation::Resume) => "session.resume",
            Operation::Session(SessionOperation::Entries) => "session.entries",
            Operation::Mcp(McpOperation::ListServers) => "mcp.list_servers",
            Operation::Mcp(McpOperation::ListTools) => "mcp.list_tools",
            Operation::Mcp(McpOperation::CallTool) => "mcp.call_tool",
            Operation::Mcp(McpOperation::ListResources) => "mcp.list_resources",
            Operation::Mcp(McpOperation::ListResourceTemplates) => "mcp.list_resource_templates",
            Operation::Mcp(McpOperation::ReadResource) => "mcp.read_resource",
            Operation::Mcp(McpOperation::ListPrompts) => "mcp.list_prompts",
            Operation::Mcp(McpOperation::GetPrompt) => "mcp.get_prompt",
            Operation::Mcp(McpOperation::Reconnect) => "mcp.reconnect",
            Operation::Mcp(McpOperation::ServerStatus) => "mcp.server_status",
            Operation::Skill(SkillOperation::List) => "skill.list",
            Operation::Skill(SkillOperation::Get) => "skill.get",
            Operation::Skill(SkillOperation::Refresh) => "skill.refresh",
            Operation::Tool(ToolOperation::List) => "tool.list",
            Operation::Tool(ToolOperation::Call) => "tool.call",
            Operation::Log(LogOperation::List) => "log.list",
            Operation::Log(LogOperation::Read) => "log.read",
            Operation::System(SystemOperation::Connected) => "system.connected",
            Operation::Control(ControlOperation::Register) => "control.register",
            Operation::Control(ControlOperation::Heartbeat) => "control.heartbeat",
            Operation::Control(ControlOperation::CapabilitySnapshot) => {
                "control.capability_snapshot"
            }
            Operation::Control(ControlOperation::CapabilityDelta) => "control.capability_delta",
            Operation::Control(ControlOperation::Event) => "control.event",
            Operation::Control(ControlOperation::Command) => "control.command",
            Operation::Control(ControlOperation::CommandAck) => "control.command_ack",
            Operation::Control(ControlOperation::CommandResult) => "control.command_result",
            Operation::Control(ControlOperation::NodeList) => "control.node_list",
            Operation::Control(ControlOperation::NodeGet) => "control.node_get",
            Operation::Control(ControlOperation::CapabilityList) => "control.capability_list",
            Operation::Control(ControlOperation::RunStatus) => "control.run_status",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentOperation {
    Submit,
    Cancel,
    Subscribe,
    Unsubscribe,
    Approve,
    List,
    Event,
    Status,
    ContextConfig,
    ContextSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperation {
    List,
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionOperation {
    List,
    Resume,
    Entries,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpOperation {
    ListServers,
    ListTools,
    CallTool,
    ListResources,
    ListResourceTemplates,
    ReadResource,
    ListPrompts,
    GetPrompt,
    Reconnect,
    ServerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillOperation {
    List,
    Get,
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolOperation {
    List,
    Call,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogOperation {
    List,
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemOperation {
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskOperation {
    List,
    Get,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlOperation {
    Register,
    Heartbeat,
    CapabilitySnapshot,
    CapabilityDelta,
    Event,
    Command,
    CommandAck,
    CommandResult,
    NodeList,
    NodeGet,
    CapabilityList,
    RunStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Payload {
    Agent(AgentPayload),
    File(FilePayload),
    Session(SessionPayload),
    Mcp(McpPayload),
    Skill(SkillPayload),
    Tool(ToolPayload),
    Log(LogPayload),
    System(SystemPayload),
    Task(TaskPayload),
    Control(ControlPayload),
    Error(ErrorPayload),
}

impl Payload {
    /// Decode a flat JSON value into the exact payload type for the given operation.
    pub fn from_operation(
        operation: &Operation,
        value: serde_json::Value,
    ) -> Result<Self, ProtocolError> {
        use serde::Deserialize;
        match operation {
            // ── Agent ──
            Operation::Agent(AgentOperation::Submit) => {
                #[derive(Deserialize)]
                struct P {
                    input: AgentInput,
                    #[serde(default)]
                    target: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.submit"))?;
                Ok(Payload::Agent(AgentPayload::Submit {
                    input: p.input,
                    target: p.target,
                }))
            }
            Operation::Agent(AgentOperation::Cancel) => {
                #[derive(Deserialize)]
                struct P {
                    run_id: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.cancel"))?;
                Ok(Payload::Agent(AgentPayload::Cancel { run_id: p.run_id }))
            }
            Operation::Agent(AgentOperation::Subscribe) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    target: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.subscribe"))?;
                Ok(Payload::Agent(AgentPayload::Subscribe { target: p.target }))
            }
            Operation::Agent(AgentOperation::Unsubscribe) => {
                #[derive(Deserialize)]
                struct P {
                    subscription_id: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.unsubscribe"))?;
                Ok(Payload::Agent(AgentPayload::Unsubscribe {
                    subscription_id: p.subscription_id,
                }))
            }
            Operation::Agent(AgentOperation::Approve) => {
                #[derive(Deserialize)]
                struct P {
                    run_id: String,
                    approved: bool,
                    #[serde(default)]
                    reason: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.approve"))?;
                Ok(Payload::Agent(AgentPayload::Approve {
                    run_id: p.run_id,
                    approved: p.approved,
                    reason: p.reason,
                }))
            }
            Operation::Agent(AgentOperation::List) => {
                Ok(Payload::Agent(AgentPayload::ListResult { agents: vec![] }))
            }
            Operation::Agent(AgentOperation::Status) => {
                #[derive(Deserialize)]
                struct P {
                    agent_id: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.status"))?;
                Ok(Payload::Agent(AgentPayload::Status {
                    agent_id: p.agent_id,
                }))
            }
            Operation::Agent(AgentOperation::ContextConfig) => {
                #[derive(Deserialize)]
                struct P {
                    agent_id: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.context_config"))?;
                Ok(Payload::Agent(AgentPayload::ContextConfig {
                    agent_id: p.agent_id,
                }))
            }
            Operation::Agent(AgentOperation::ContextSnapshot) => {
                #[derive(Deserialize)]
                struct P {
                    agent_id: String,
                    contributor_name: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.context_snapshot"))?;
                Ok(Payload::Agent(AgentPayload::ContextSnapshot {
                    agent_id: p.agent_id,
                    contributor_name: p.contributor_name,
                }))
            }
            Operation::Agent(AgentOperation::Event) => {
                #[derive(Deserialize)]
                struct P {
                    run_id: String,
                    event: serde_json::Value,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.event"))?;
                Ok(Payload::Agent(AgentPayload::Event {
                    run_id: p.run_id,
                    event: p.event,
                }))
            }
            // ── File ──
            Operation::File(FileOperation::List) => {
                #[derive(Deserialize)]
                struct P {
                    path: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("file.list"))?;
                Ok(Payload::File(FilePayload::List { path: p.path }))
            }
            Operation::File(FileOperation::Read) => {
                #[derive(Deserialize)]
                struct P {
                    path: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("file.read"))?;
                Ok(Payload::File(FilePayload::Read { path: p.path }))
            }
            // ── Session ──
            Operation::Session(SessionOperation::List) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    agent_id: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("session.list"))?;
                Ok(Payload::Session(SessionPayload::List {
                    agent_id: p.agent_id,
                }))
            }
            Operation::Session(SessionOperation::Resume) => {
                #[derive(Deserialize)]
                struct P {
                    session_id: String,
                    #[serde(default)]
                    agent_id: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("session.resume"))?;
                Ok(Payload::Session(SessionPayload::Resume {
                    session_id: p.session_id,
                    agent_id: p.agent_id,
                }))
            }
            Operation::Session(SessionOperation::Entries) => {
                #[derive(Deserialize)]
                struct P {
                    session_id: String,
                    #[serde(default)]
                    agent_id: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("session.entries"))?;
                Ok(Payload::Session(SessionPayload::Entries {
                    session_id: p.session_id,
                    agent_id: p.agent_id,
                }))
            }
            // ── MCP ──
            Operation::Mcp(McpOperation::ListServers) => Ok(Payload::Mcp(McpPayload::ListServers)),
            Operation::Mcp(McpOperation::ListTools) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    server: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.list_tools"))?;
                Ok(Payload::Mcp(McpPayload::ListTools { server: p.server }))
            }
            Operation::Mcp(McpOperation::CallTool) => {
                #[derive(Deserialize)]
                struct P {
                    server: String,
                    tool_name: String,
                    #[serde(default)]
                    arguments: serde_json::Value,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.call_tool"))?;
                Ok(Payload::Mcp(McpPayload::CallTool {
                    server: p.server,
                    tool_name: p.tool_name,
                    arguments: p.arguments,
                }))
            }
            Operation::Mcp(McpOperation::ListResources) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    server: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.list_resources"))?;
                Ok(Payload::Mcp(McpPayload::ListResources { server: p.server }))
            }
            Operation::Mcp(McpOperation::ListResourceTemplates) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    server: Option<String>,
                }
                let p: P = serde_json::from_value(value).map_err(|_| {
                    ProtocolError::PayloadDecodeFailed("mcp.list_resource_templates")
                })?;
                Ok(Payload::Mcp(McpPayload::ListResourceTemplates {
                    server: p.server,
                }))
            }
            Operation::Mcp(McpOperation::ReadResource) => {
                #[derive(Deserialize)]
                struct P {
                    uri: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.read_resource"))?;
                Ok(Payload::Mcp(McpPayload::ReadResource { uri: p.uri }))
            }
            Operation::Mcp(McpOperation::ListPrompts) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    server: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.list_prompts"))?;
                Ok(Payload::Mcp(McpPayload::ListPrompts { server: p.server }))
            }
            Operation::Mcp(McpOperation::GetPrompt) => {
                #[derive(Deserialize)]
                struct P {
                    name: String,
                    #[serde(default)]
                    arguments: Option<serde_json::Map<String, serde_json::Value>>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.get_prompt"))?;
                Ok(Payload::Mcp(McpPayload::GetPrompt {
                    name: p.name,
                    arguments: p.arguments,
                }))
            }
            Operation::Mcp(McpOperation::Reconnect) => {
                #[derive(Deserialize)]
                struct P {
                    server: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.reconnect"))?;
                Ok(Payload::Mcp(McpPayload::Reconnect { server: p.server }))
            }
            Operation::Mcp(McpOperation::ServerStatus) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    server: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("mcp.server_status"))?;
                Ok(Payload::Mcp(McpPayload::ServerStatus { server: p.server }))
            }
            // ── Skill ──
            Operation::Skill(SkillOperation::List) => Ok(Payload::Skill(SkillPayload::List)),
            Operation::Skill(SkillOperation::Get) => {
                #[derive(Deserialize)]
                struct P {
                    name: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("skill.get"))?;
                Ok(Payload::Skill(SkillPayload::Get { name: p.name }))
            }
            Operation::Skill(SkillOperation::Refresh) => Ok(Payload::Skill(SkillPayload::Refresh)),
            // ── Tool ──
            Operation::Tool(ToolOperation::List) => Ok(Payload::Tool(ToolPayload::List)),
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
            // ── Log ──
            Operation::Log(LogOperation::List) => Ok(Payload::Log(LogPayload::List)),
            Operation::Log(LogOperation::Read) => {
                #[derive(Deserialize)]
                struct P {
                    run_id: String,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("log.read"))?;
                Ok(Payload::Log(LogPayload::Read { run_id: p.run_id }))
            }
            // ── System ──
            Operation::System(SystemOperation::Connected) => {
                Ok(Payload::System(SystemPayload::Empty))
            }
            // ── Task ──
            Operation::Task(TaskOperation::List) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    status: Option<String>,
                    #[serde(default)]
                    assignee: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("task.list"))?;
                Ok(Payload::Task(TaskPayload::List {
                    status: p.status,
                    assignee: p.assignee,
                }))
            }
            Operation::Task(TaskOperation::Get) => {
                #[derive(Deserialize)]
                struct P {
                    task_id: u64,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("task.get"))?;
                Ok(Payload::Task(TaskPayload::Get { task_id: p.task_id }))
            }
            Operation::Control(ControlOperation::Register) => serde_json::from_value(value)
                .map(ControlPayload::Register)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.register")),
            Operation::Control(ControlOperation::Heartbeat) => serde_json::from_value(value)
                .map(ControlPayload::Heartbeat)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.heartbeat")),
            Operation::Control(ControlOperation::CapabilitySnapshot) => {
                serde_json::from_value(value)
                    .map(ControlPayload::CapabilitySnapshot)
                    .map(Payload::Control)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.capability_snapshot"))
            }
            Operation::Control(ControlOperation::CapabilityDelta) => serde_json::from_value(value)
                .map(ControlPayload::CapabilityDelta)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.capability_delta")),
            Operation::Control(ControlOperation::Event) => serde_json::from_value(value)
                .map(ControlPayload::Event)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.event")),
            Operation::Control(ControlOperation::Command) => serde_json::from_value(value)
                .map(ControlPayload::Command)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.command")),
            Operation::Control(ControlOperation::CommandAck) => serde_json::from_value(value)
                .map(ControlPayload::CommandAck)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.command_ack")),
            Operation::Control(ControlOperation::CommandResult) => serde_json::from_value(value)
                .map(ControlPayload::CommandResult)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.command_result")),
            Operation::Control(ControlOperation::NodeList) => serde_json::from_value(value)
                .map(ControlPayload::NodeList)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.node_list")),
            Operation::Control(ControlOperation::NodeGet) => serde_json::from_value(value)
                .map(ControlPayload::NodeGet)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.node_get")),
            Operation::Control(ControlOperation::CapabilityList) => serde_json::from_value(value)
                .map(ControlPayload::CapabilityList)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.capability_list")),
            Operation::Control(ControlOperation::RunStatus) => serde_json::from_value(value)
                .map(ControlPayload::RunStatus)
                .map(Payload::Control)
                .map_err(|_| ProtocolError::PayloadDecodeFailed("control.run_status")),
        }
    }

    /// Encode the payload as flat JSON (no domain/data or variant wrappers).
    pub fn data_json(&self) -> serde_json::Value {
        let val = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        // With untagged Payload, the value is the variant's data directly, e.g.
        // {"SubmitResult":{"run_id":"x"}}. Strip the variant name wrapper.
        if let Some(obj) = val.as_object() {
            if obj.len() == 1 {
                if let Some((_key, inner)) = obj.iter().next() {
                    return inner.clone();
                }
            }
        }
        val
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentPayload {
    Submit {
        input: AgentInput,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    SubmitAck {
        run_id: String,
        accepted: bool,
    },
    SubmitResult {
        run_id: String,
        response: serde_json::Value,
    },
    Cancel {
        run_id: String,
    },
    CancelResult {
        run_id: String,
        cancelled: bool,
    },
    Subscribe {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    SubscribeResult {
        subscription_id: String,
    },
    Unsubscribe {
        subscription_id: String,
    },
    UnsubscribeResult {
        subscription_id: String,
        removed: bool,
    },
    Approve {
        run_id: String,
        approved: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    ApproveResult {
        run_id: String,
        accepted: bool,
    },
    ListResult {
        agents: Vec<serde_json::Value>,
    },
    Event {
        run_id: String,
        event: serde_json::Value,
    },
    Status {
        agent_id: String,
    },
    StatusResult {
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
    },
    ContextConfig {
        agent_id: String,
    },
    ContextConfigResult {
        contributors: Vec<serde_json::Value>,
    },
    ContextSnapshot {
        agent_id: String,
        contributor_name: String,
    },
    ContextSnapshotResult {
        messages: Vec<serde_json::Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilePayload {
    List {
        path: String,
    },
    ListResult {
        entries: Vec<serde_json::Value>,
    },
    Read {
        path: String,
    },
    ReadResult {
        content: String,
        metadata: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionPayload {
    List {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
    },
    ListResult {
        sessions: Vec<serde_json::Value>,
    },
    Resume {
        session_id: String,
        agent_id: Option<String>,
    },
    ResumeResult {
        session_id: String,
        restored: bool,
        entry_count: usize,
        entries: Vec<serde_json::Value>,
    },
    Entries {
        session_id: String,
        agent_id: Option<String>,
    },
    EntriesResult {
        entries: Vec<serde_json::Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum McpPayload {
    ListServers,
    ListServersResult {
        servers: Vec<serde_json::Value>,
    },
    ListTools {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
    },
    ListToolsResult {
        tools: Vec<serde_json::Value>,
    },
    CallTool {
        server: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    CallToolResult {
        tool_name: String,
        result: serde_json::Value,
    },
    ListResources {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
    },
    ListResourcesResult {
        resources: Vec<serde_json::Value>,
    },
    ListResourceTemplates {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
    },
    ListResourceTemplatesResult {
        templates: Vec<serde_json::Value>,
    },
    ReadResource {
        uri: String,
    },
    ReadResourceResult {
        uri: String,
        content: String,
    },
    ListPrompts {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
    },
    ListPromptsResult {
        prompts: Vec<serde_json::Value>,
    },
    GetPrompt {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    },
    GetPromptResult {
        name: String,
        prompt: serde_json::Value,
    },
    Reconnect {
        server: String,
    },
    ReconnectResult {
        reconnected: bool,
    },
    ServerStatus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
    },
    ServerStatusResult {
        server: String,
        status: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SkillPayload {
    List,
    ListResult {
        skills: Vec<serde_json::Value>,
    },
    Get {
        name: String,
    },
    GetResult {
        skill: serde_json::Value,
        name: String,
    },
    Refresh,
    RefreshResult {
        discovered: usize,
    },
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogPayload {
    List,
    ListResult { runs: Vec<serde_json::Value> },
    Read { run_id: String },
    ReadResult { entries: Vec<serde_json::Value> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SystemPayload {
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskPayload {
    List {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assignee: Option<String>,
    },
    ListResult {
        tasks: Vec<serde_json::Value>,
    },
    Get {
        task_id: u64,
    },
    GetResult {
        task: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ControlPayload {
    Register(NodeRegistration),
    RegisterAck(RegisterAck),
    Heartbeat(NodeHeartbeat),
    CapabilitySnapshot(CapabilitySnapshot),
    CapabilityDelta(CapabilityDelta),
    Event(DataPlaneEvent),
    Command(ControlCommand),
    CommandAck(CommandAck),
    CommandResult(CommandResult),
    NodeList(NodeListRequest),
    NodeListResult(NodeListResult),
    NodeGet(NodeGetRequest),
    NodeGetResult(NodeGetResult),
    CapabilityList(CapabilityListRequest),
    CapabilityListResult(CapabilityListResult),
    RunStatus(RunStatusRequest),
    RunStatusResult(RunStatusResult),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRegistration {
    pub node_id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterAck {
    pub node_id: String,
    pub accepted: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeHeartbeat {
    pub node_id: String,
    pub status: String,
    #[serde(default)]
    pub load: NodeLoad,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeLoad {
    pub running: u64,
    pub queued: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    pub node_id: String,
    pub revision: u64,
    #[serde(default)]
    pub generated_at_ms: Option<u64>,
    #[serde(default)]
    pub agents: Vec<AgentCapability>,
    #[serde(default)]
    pub tools: Vec<ToolCapability>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerCapability>,
    #[serde(default)]
    pub skills: Vec<SkillCapability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityDelta {
    pub node_id: String,
    pub base_revision: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCapability {
    pub agent_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCapability {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sensitivity: Option<String>,
    #[serde(default)]
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerCapability {
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillCapability {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataPlaneEvent {
    pub node_id: String,
    pub event_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlCommand {
    pub command_id: String,
    pub node_id: String,
    pub operation: ControlCommandOperation,
    #[serde(default)]
    pub deadline_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", content = "payload")]
pub enum ControlCommandOperation {
    SubmitAgent {
        target: Option<String>,
        input: AgentInput,
    },
    CancelRun {
        run_id: String,
    },
    CallTool {
        name: String,
        args: serde_json::Value,
    },
    CallMcpTool {
        server: String,
        name: String,
        args: serde_json::Value,
    },
    RefreshCapabilities,
    HealthCheck,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandAck {
    pub command_id: String,
    pub accepted: bool,
    #[serde(default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: String,
    pub status: String,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeListRequest {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeListResult {
    pub nodes: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeGetRequest {
    pub node_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeGetResult {
    pub node: Option<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub status: String,
    #[serde(default)]
    pub last_seen_at_ms: Option<u64>,
    #[serde(default)]
    pub capability_revision: u64,
    #[serde(default)]
    pub load: NodeLoad,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CapabilityListRequest {
    #[serde(default)]
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityListResult {
    pub snapshots: Vec<CapabilitySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatusRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatusResult {
    pub run_id: String,
    pub status: String,
    #[serde(default)]
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
    pub terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentServerMessage {
    pub protocol: String,
    pub message_id: String,
    pub sender: String,
    pub receiver: String,
    pub kind: MessageKind,
    pub operation: Operation,
    pub payload: Payload,
    #[serde(default)]
    pub meta: MessageMeta,
}

impl AgentServerMessage {
    pub fn new_command(
        message_id: impl Into<String>,
        operation: Operation,
        payload: Payload,
    ) -> Self {
        Self::new(message_id, MessageKind::Command, operation, payload)
    }

    pub fn new_ack(message_id: impl Into<String>, operation: Operation, payload: Payload) -> Self {
        Self::new(message_id, MessageKind::Ack, operation, payload)
    }

    pub fn new_result(
        message_id: impl Into<String>,
        operation: Operation,
        payload: Payload,
    ) -> Self {
        Self::new(message_id, MessageKind::Result, operation, payload)
    }

    pub fn new_event(
        message_id: impl Into<String>,
        operation: Operation,
        payload: Payload,
    ) -> Self {
        Self::new(message_id, MessageKind::Event, operation, payload)
    }

    pub fn new_error(
        message_id: impl Into<String>,
        operation: Operation,
        payload: ErrorPayload,
    ) -> Self {
        Self::new(
            message_id,
            MessageKind::Error,
            operation,
            Payload::Error(payload),
        )
    }

    fn new(
        message_id: impl Into<String>,
        kind: MessageKind,
        operation: Operation,
        payload: Payload,
    ) -> Self {
        Self {
            protocol: "agent-server/1".to_string(),
            message_id: message_id.into(),
            sender: "client".to_string(),
            receiver: "server".to_string(),
            kind,
            operation,
            payload,
            meta: MessageMeta::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::AgentInput;

    // ── ProtocolError tests ──

    #[test]
    fn protocol_error_display_unknown_method() {
        let err = ProtocolError::UnknownMethod("foo.bar".into());
        assert_eq!(err.to_string(), "unknown method: foo.bar");
    }

    #[test]
    fn protocol_error_display_payload_decode_failed() {
        let err = ProtocolError::PayloadDecodeFailed("agent.submit");
        assert_eq!(err.to_string(), "payload decode failed for agent.submit");
    }

    #[test]
    fn protocol_error_display_payload_decode_failed_owned() {
        let err = ProtocolError::PayloadDecodeFailedOwned("bad payload".into());
        assert_eq!(err.to_string(), "payload decode failed: bad payload");
    }

    // ── MessageKind serialization ──

    #[test]
    fn message_kind_serialize_snake_case() {
        assert_eq!(
            serde_json::to_value(MessageKind::Command).unwrap(),
            serde_json::json!("command")
        );
        assert_eq!(
            serde_json::to_value(MessageKind::Ack).unwrap(),
            serde_json::json!("ack")
        );
        assert_eq!(
            serde_json::to_value(MessageKind::Event).unwrap(),
            serde_json::json!("event")
        );
        assert_eq!(
            serde_json::to_value(MessageKind::Result).unwrap(),
            serde_json::json!("result")
        );
        assert_eq!(
            serde_json::to_value(MessageKind::Error).unwrap(),
            serde_json::json!("error")
        );
    }

    #[test]
    fn message_kind_deserialize_snake_case() {
        assert_eq!(
            serde_json::from_value::<MessageKind>(serde_json::json!("command")).unwrap(),
            MessageKind::Command
        );
        assert_eq!(
            serde_json::from_value::<MessageKind>(serde_json::json!("ack")).unwrap(),
            MessageKind::Ack
        );
    }

    // ── MessageMeta tests ──

    #[test]
    fn message_meta_default_skips_none_fields() {
        let meta = MessageMeta::default();
        let json = serde_json::to_value(meta).unwrap();
        assert!(json.as_object().unwrap().is_empty());
    }

    #[test]
    fn message_meta_with_all_fields() {
        let meta = MessageMeta {
            trace_id: Some("trace-1".into()),
            timestamp_ms: Some(1234567890),
            stream_seq: Some(42),
            gateway: Some("ws-gateway".into()),
        };
        let json = serde_json::to_value(meta).unwrap();
        assert_eq!(json["trace_id"], "trace-1");
        assert_eq!(json["timestamp_ms"], 1234567890);
        assert_eq!(json["stream_seq"], 42);
        assert_eq!(json["gateway"], "ws-gateway");
    }

    // ── Operation method_name exhaustive tests ──

    #[test]
    fn operation_method_names_all_variants() {
        // Agent
        assert_eq!(Operation::Agent(AgentOperation::Submit).method_name(), "agent.submit");
        assert_eq!(Operation::Agent(AgentOperation::Cancel).method_name(), "agent.cancel");
        assert_eq!(Operation::Agent(AgentOperation::Subscribe).method_name(), "agent.subscribe");
        assert_eq!(Operation::Agent(AgentOperation::Unsubscribe).method_name(), "agent.unsubscribe");
        assert_eq!(Operation::Agent(AgentOperation::Approve).method_name(), "agent.approve");
        assert_eq!(Operation::Agent(AgentOperation::List).method_name(), "agent.list");
        assert_eq!(Operation::Agent(AgentOperation::Event).method_name(), "agent.event");
        assert_eq!(Operation::Agent(AgentOperation::Status).method_name(), "agent.status");
        assert_eq!(Operation::Agent(AgentOperation::ContextConfig).method_name(), "agent.context_config");
        assert_eq!(Operation::Agent(AgentOperation::ContextSnapshot).method_name(), "agent.context_snapshot");
        // Task
        assert_eq!(Operation::Task(TaskOperation::List).method_name(), "task.list");
        assert_eq!(Operation::Task(TaskOperation::Get).method_name(), "task.get");
        // File
        assert_eq!(Operation::File(FileOperation::List).method_name(), "file.list");
        assert_eq!(Operation::File(FileOperation::Read).method_name(), "file.read");
        // Session
        assert_eq!(Operation::Session(SessionOperation::List).method_name(), "session.list");
        assert_eq!(Operation::Session(SessionOperation::Resume).method_name(), "session.resume");
        assert_eq!(Operation::Session(SessionOperation::Entries).method_name(), "session.entries");
    }

    #[test]
    fn operation_method_names_mcp() {
        assert_eq!(Operation::Mcp(McpOperation::ListServers).method_name(), "mcp.list_servers");
        assert_eq!(Operation::Mcp(McpOperation::ListTools).method_name(), "mcp.list_tools");
        assert_eq!(Operation::Mcp(McpOperation::CallTool).method_name(), "mcp.call_tool");
        assert_eq!(Operation::Mcp(McpOperation::ListResources).method_name(), "mcp.list_resources");
        assert_eq!(Operation::Mcp(McpOperation::ListResourceTemplates).method_name(), "mcp.list_resource_templates");
        assert_eq!(Operation::Mcp(McpOperation::ReadResource).method_name(), "mcp.read_resource");
        assert_eq!(Operation::Mcp(McpOperation::ListPrompts).method_name(), "mcp.list_prompts");
        assert_eq!(Operation::Mcp(McpOperation::GetPrompt).method_name(), "mcp.get_prompt");
        assert_eq!(Operation::Mcp(McpOperation::Reconnect).method_name(), "mcp.reconnect");
        assert_eq!(Operation::Mcp(McpOperation::ServerStatus).method_name(), "mcp.server_status");
    }

    #[test]
    fn operation_method_names_skill_tool_log_system_control() {
        assert_eq!(Operation::Skill(SkillOperation::List).method_name(), "skill.list");
        assert_eq!(Operation::Skill(SkillOperation::Get).method_name(), "skill.get");
        assert_eq!(Operation::Skill(SkillOperation::Refresh).method_name(), "skill.refresh");
        assert_eq!(Operation::Tool(ToolOperation::List).method_name(), "tool.list");
        assert_eq!(Operation::Tool(ToolOperation::Call).method_name(), "tool.call");
        assert_eq!(Operation::Log(LogOperation::List).method_name(), "log.list");
        assert_eq!(Operation::Log(LogOperation::Read).method_name(), "log.read");
        assert_eq!(Operation::System(SystemOperation::Connected).method_name(), "system.connected");
        assert_eq!(Operation::Control(ControlOperation::Register).method_name(), "control.register");
        assert_eq!(Operation::Control(ControlOperation::Heartbeat).method_name(), "control.heartbeat");
        assert_eq!(Operation::Control(ControlOperation::CapabilitySnapshot).method_name(), "control.capability_snapshot");
        assert_eq!(Operation::Control(ControlOperation::CapabilityDelta).method_name(), "control.capability_delta");
        assert_eq!(Operation::Control(ControlOperation::Event).method_name(), "control.event");
        assert_eq!(Operation::Control(ControlOperation::Command).method_name(), "control.command");
        assert_eq!(Operation::Control(ControlOperation::CommandAck).method_name(), "control.command_ack");
        assert_eq!(Operation::Control(ControlOperation::CommandResult).method_name(), "control.command_result");
        assert_eq!(Operation::Control(ControlOperation::NodeList).method_name(), "control.node_list");
        assert_eq!(Operation::Control(ControlOperation::NodeGet).method_name(), "control.node_get");
        assert_eq!(Operation::Control(ControlOperation::CapabilityList).method_name(), "control.capability_list");
        assert_eq!(Operation::Control(ControlOperation::RunStatus).method_name(), "control.run_status");
    }

    // ── Payload::from_operation tests for previously uncovered arms ──

    #[test]
    fn payload_from_operation_session_list() {
        let op = Operation::Session(SessionOperation::List);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Session(SessionPayload::List { agent_id: None }));
    }

    #[test]
    fn payload_from_operation_session_resume() {
        let op = Operation::Session(SessionOperation::Resume);
        let p = Payload::from_operation(&op, serde_json::json!({"session_id": "s1"})).unwrap();
        assert_eq!(p, Payload::Session(SessionPayload::Resume { session_id: "s1".into(), agent_id: None }));
    }

    #[test]
    fn payload_from_operation_session_entries() {
        let op = Operation::Session(SessionOperation::Entries);
        let p = Payload::from_operation(&op, serde_json::json!({"session_id": "s1"})).unwrap();
        assert_eq!(p, Payload::Session(SessionPayload::Entries { session_id: "s1".into(), agent_id: None }));
    }

    #[test]
    fn payload_from_operation_task_list() {
        let op = Operation::Task(TaskOperation::List);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Task(TaskPayload::List { status: None, assignee: None }));
    }

    #[test]
    fn payload_from_operation_task_get() {
        let op = Operation::Task(TaskOperation::Get);
        let p = Payload::from_operation(&op, serde_json::json!({"task_id": 42})).unwrap();
        assert_eq!(p, Payload::Task(TaskPayload::Get { task_id: 42 }));
    }

    #[test]
    fn payload_from_operation_log_list() {
        let op = Operation::Log(LogOperation::List);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Log(LogPayload::List));
    }

    #[test]
    fn payload_from_operation_log_read() {
        let op = Operation::Log(LogOperation::Read);
        let p = Payload::from_operation(&op, serde_json::json!({"run_id": "run-1"})).unwrap();
        assert_eq!(p, Payload::Log(LogPayload::Read { run_id: "run-1".into() }));
    }

    #[test]
    fn payload_from_operation_skill_list() {
        let op = Operation::Skill(SkillOperation::List);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Skill(SkillPayload::List));
    }

    #[test]
    fn payload_from_operation_skill_refresh() {
        let op = Operation::Skill(SkillOperation::Refresh);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Skill(SkillPayload::Refresh));
    }

    #[test]
    fn payload_from_operation_tool_list() {
        let op = Operation::Tool(ToolOperation::List);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Tool(ToolPayload::List));
    }

    #[test]
    fn payload_from_operation_system_connected() {
        let op = Operation::System(SystemOperation::Connected);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::System(SystemPayload::Empty));
    }

    #[test]
    fn payload_from_operation_agent_subscribe() {
        let op = Operation::Agent(AgentOperation::Subscribe);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Agent(AgentPayload::Subscribe { target: None }));
    }

    #[test]
    fn payload_from_operation_agent_unsubscribe() {
        let op = Operation::Agent(AgentOperation::Unsubscribe);
        let p = Payload::from_operation(&op, serde_json::json!({"subscription_id": "sub-1"})).unwrap();
        assert_eq!(p, Payload::Agent(AgentPayload::Unsubscribe { subscription_id: "sub-1".into() }));
    }

    #[test]
    fn payload_from_operation_agent_approve() {
        let op = Operation::Agent(AgentOperation::Approve);
        let p = Payload::from_operation(&op, serde_json::json!({"run_id": "run-1", "approved": true, "reason": "looks good"})).unwrap();
        assert_eq!(p, Payload::Agent(AgentPayload::Approve { run_id: "run-1".into(), approved: true, reason: Some("looks good".into()) }));
    }

    #[test]
    fn payload_from_operation_agent_status() {
        let op = Operation::Agent(AgentOperation::Status);
        let p = Payload::from_operation(&op, serde_json::json!({"agent_id": "agent-a"})).unwrap();
        assert_eq!(p, Payload::Agent(AgentPayload::Status { agent_id: "agent-a".into() }));
    }

    #[test]
    fn payload_from_operation_agent_context_config() {
        let op = Operation::Agent(AgentOperation::ContextConfig);
        let p = Payload::from_operation(&op, serde_json::json!({"agent_id": "agent-a"})).unwrap();
        assert_eq!(p, Payload::Agent(AgentPayload::ContextConfig { agent_id: "agent-a".into() }));
    }

    #[test]
    fn payload_from_operation_agent_context_snapshot() {
        let op = Operation::Agent(AgentOperation::ContextSnapshot);
        let p = Payload::from_operation(&op, serde_json::json!({"agent_id": "agent-a", "contributor_name": "skills"})).unwrap();
        assert_eq!(p, Payload::Agent(AgentPayload::ContextSnapshot { agent_id: "agent-a".into(), contributor_name: "skills".into() }));
    }

    #[test]
    fn payload_from_operation_mcp_list_servers() {
        let op = Operation::Mcp(McpOperation::ListServers);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Mcp(McpPayload::ListServers));
    }

    #[test]
    fn payload_from_operation_mcp_call_tool() {
        let op = Operation::Mcp(McpOperation::CallTool);
        let p = Payload::from_operation(&op, serde_json::json!({"server": "mcp-srv", "tool_name": "read", "arguments": {"path": "/tmp"}})).unwrap();
        assert_eq!(p, Payload::Mcp(McpPayload::CallTool { server: "mcp-srv".into(), tool_name: "read".into(), arguments: serde_json::json!({"path": "/tmp"}) }));
    }

    #[test]
    fn payload_from_operation_mcp_reconnect() {
        let op = Operation::Mcp(McpOperation::Reconnect);
        let p = Payload::from_operation(&op, serde_json::json!({"server": "mcp-srv"})).unwrap();
        assert_eq!(p, Payload::Mcp(McpPayload::Reconnect { server: "mcp-srv".into() }));
    }

    #[test]
    fn payload_from_operation_mcp_get_prompt() {
        let op = Operation::Mcp(McpOperation::GetPrompt);
        let p = Payload::from_operation(&op, serde_json::json!({"name": "greet", "arguments": {"lang": "en"}})).unwrap();
        let expected_args: Option<serde_json::Map<String, serde_json::Value>> = Some(
            [("lang".into(), serde_json::json!("en"))].into_iter().collect(),
        );
        assert_eq!(p, Payload::Mcp(McpPayload::GetPrompt { name: "greet".into(), arguments: expected_args }));
    }

    #[test]
    fn payload_from_operation_mcp_server_status() {
        let op = Operation::Mcp(McpOperation::ServerStatus);
        let p = Payload::from_operation(&op, serde_json::json!({})).unwrap();
        assert_eq!(p, Payload::Mcp(McpPayload::ServerStatus { server: None }));
    }

    #[test]
    fn payload_from_operation_file_read() {
        let op = Operation::File(FileOperation::Read);
        let p = Payload::from_operation(&op, serde_json::json!({"path": "/tmp/f.txt"})).unwrap();
        assert_eq!(p, Payload::File(FilePayload::Read { path: "/tmp/f.txt".into() }));
    }

    // ── Error payload tests ──

    #[test]
    fn error_payload_serialize_deserialize() {
        let err = ErrorPayload {
            code: "agent_error".into(),
            message: "something went wrong".into(),
            detail: Some(serde_json::json!({"run_id": "run-1"})),
            terminal: true,
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "agent_error");
        assert_eq!(json["message"], "something went wrong");
        assert_eq!(json["detail"]["run_id"], "run-1");
        assert_eq!(json["terminal"], true);
        let deser: ErrorPayload = serde_json::from_value(json).unwrap();
        assert_eq!(deser, err);
    }

    #[test]
    fn error_payload_no_detail() {
        let err = ErrorPayload {
            code: "timeout".into(),
            message: "timed out".into(),
            detail: None,
            terminal: false,
        };
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.get("detail").is_none());
    }

    // ── Payload::data_json tests ──

    #[test]
    fn payload_data_json_strips_variant_wrapper() {
        let p = Payload::File(FilePayload::Read { path: "/tmp/f.txt".into() });
        let json = p.data_json();
        assert_eq!(json, serde_json::json!({"path": "/tmp/f.txt"}));
    }

    #[test]
    fn payload_data_json_unit_variant() {
        let p = Payload::Skill(SkillPayload::List);
        let json = p.data_json();
        assert_eq!(json, serde_json::json!("List"));
    }

    #[test]
    fn payload_data_json_submit_ack() {
        let p = Payload::Agent(AgentPayload::SubmitAck { run_id: "run-1".into(), accepted: true });
        let json = p.data_json();
        assert_eq!(json, serde_json::json!({"run_id": "run-1", "accepted": true}));
    }

    #[test]
    fn payload_data_json_control_payload() {
        let p = Payload::Control(ControlPayload::Register(NodeRegistration { node_id: "n1".into(), name: "Node 1".into(), version: "1.0".into() }));
        let json = p.data_json();
        assert_eq!(json, serde_json::json!({"node_id": "n1", "name": "Node 1", "version": "1.0"}));
    }

    // ── AgentServerMessage constructor tests ──

    #[test]
    fn agent_server_message_new_command() {
        let msg = AgentServerMessage::new_command("msg-1", Operation::Agent(AgentOperation::Submit), Payload::Agent(AgentPayload::Submit { input: AgentInput::text("hello"), target: None }));
        assert_eq!(msg.protocol, "agent-server/1");
        assert_eq!(msg.message_id, "msg-1");
        assert_eq!(msg.sender, "client");
        assert_eq!(msg.receiver, "server");
        assert_eq!(msg.kind, MessageKind::Command);
    }

    #[test]
    fn agent_server_message_new_ack() {
        let msg = AgentServerMessage::new_ack("msg-1", Operation::Agent(AgentOperation::Submit), Payload::Agent(AgentPayload::SubmitAck { run_id: "run-1".into(), accepted: true }));
        assert_eq!(msg.kind, MessageKind::Ack);
    }

    #[test]
    fn agent_server_message_new_result() {
        let msg = AgentServerMessage::new_result("msg-1", Operation::Agent(AgentOperation::Submit), Payload::Agent(AgentPayload::SubmitResult { run_id: "run-1".into(), response: serde_json::json!({"agents": []}) }));
        assert_eq!(msg.kind, MessageKind::Result);
    }

    #[test]
    fn agent_server_message_new_event() {
        let msg = AgentServerMessage::new_event("msg-1", Operation::Agent(AgentOperation::Event), Payload::Agent(AgentPayload::Event { run_id: "run-1".into(), event: serde_json::json!({"type": "thought"}) }));
        assert_eq!(msg.kind, MessageKind::Event);
    }

    #[test]
    fn agent_server_message_new_error() {
        let msg = AgentServerMessage::new_error("msg-1", Operation::Agent(AgentOperation::Submit), ErrorPayload { code: "timeout".into(), message: "request timed out".into(), detail: None, terminal: true });
        assert_eq!(msg.kind, MessageKind::Error);
        match msg.payload {
            Payload::Error(ref e) => assert_eq!(e.code, "timeout"),
            _ => panic!("expected Error payload"),
        }
    }

    // ── AgentPayload serde round-trip ──

    #[test]
    fn agent_payload_round_trip_all_variants() {
        let variants: Vec<AgentPayload> = vec![
            AgentPayload::Submit { input: AgentInput::text("hi"), target: None },
            AgentPayload::SubmitAck { run_id: "r1".into(), accepted: true },
            AgentPayload::SubmitResult { run_id: "r1".into(), response: serde_json::json!("ok") },
            AgentPayload::Cancel { run_id: "r1".into() },
            AgentPayload::CancelResult { run_id: "r1".into(), cancelled: true },
            AgentPayload::Subscribe { target: None },
            AgentPayload::SubscribeResult { subscription_id: "s1".into() },
            AgentPayload::Unsubscribe { subscription_id: "s1".into() },
            AgentPayload::UnsubscribeResult { subscription_id: "s1".into(), removed: true },
            AgentPayload::Approve { run_id: "r1".into(), approved: true, reason: None },
            AgentPayload::ApproveResult { run_id: "r1".into(), accepted: true },
            AgentPayload::ListResult { agents: vec![] },
            AgentPayload::Event { run_id: "r1".into(), event: serde_json::json!({"type": "start"}) },
            AgentPayload::Status { agent_id: "a1".into() },
            AgentPayload::StatusResult { status: "idle".into(), run_id: None },
            AgentPayload::ContextConfig { agent_id: "a1".into() },
            AgentPayload::ContextConfigResult { contributors: vec![] },
            AgentPayload::ContextSnapshot { agent_id: "a1".into(), contributor_name: "skills".into() },
            AgentPayload::ContextSnapshotResult { messages: vec![] },
        ];
        for v in variants {
            let json = serde_json::to_value(&v).unwrap();
            let back: AgentPayload = serde_json::from_value(json).unwrap();
            assert_eq!(back, v);
        }
    }

    // ── ControlPayload serde round-trip ──

    #[test]
    fn control_payload_round_trip() {
        let variants: Vec<ControlPayload> = vec![
            ControlPayload::Register(NodeRegistration { node_id: "n1".into(), name: "node-1".into(), version: "1.0".into() }),
            ControlPayload::RegisterAck(RegisterAck { node_id: "n1".into(), accepted: true, generation: 1 }),
            ControlPayload::Heartbeat(NodeHeartbeat { node_id: "n1".into(), status: "online".into(), load: NodeLoad::default() }),
            ControlPayload::CapabilitySnapshot(CapabilitySnapshot { node_id: "n1".into(), revision: 1, generated_at_ms: None, agents: vec![], tools: vec![], mcp_servers: vec![], skills: vec![] }),
            ControlPayload::CapabilityDelta(CapabilityDelta { node_id: "n1".into(), base_revision: 0, revision: 1 }),
            ControlPayload::Event(DataPlaneEvent { node_id: "n1".into(), event_type: "status_change".into(), data: serde_json::json!({}) }),
            ControlPayload::Command(ControlCommand { command_id: "cmd-1".into(), node_id: "n1".into(), operation: ControlCommandOperation::HealthCheck, deadline_ms: None }),
            ControlPayload::CommandAck(CommandAck { command_id: "cmd-1".into(), accepted: true, run_id: None }),
            ControlPayload::CommandResult(CommandResult { command_id: "cmd-1".into(), status: "completed".into(), result: serde_json::json!({}), error: None }),
            ControlPayload::NodeList(NodeListRequest {}),
            ControlPayload::NodeListResult(NodeListResult { nodes: vec![] }),
            ControlPayload::NodeGet(NodeGetRequest { node_id: "n1".into() }),
            ControlPayload::NodeGetResult(NodeGetResult { node: None }),
            ControlPayload::CapabilityList(CapabilityListRequest { node_id: None }),
            ControlPayload::CapabilityListResult(CapabilityListResult { snapshots: vec![] }),
            ControlPayload::RunStatus(RunStatusRequest { run_id: "run-1".into() }),
            ControlPayload::RunStatusResult(RunStatusResult { run_id: "run-1".into(), status: "running".into(), node_id: None }),
        ];
        for v in variants {
            let json = serde_json::to_value(&v).unwrap();
            let back: ControlPayload = serde_json::from_value(json).unwrap();
            assert_eq!(back, v);
        }
    }

    // ── ControlCommandOperation serde ──

    #[test]
    fn control_command_operation_serde() {
        let submit = ControlCommandOperation::SubmitAgent { target: Some("agent-a".into()), input: AgentInput::text("hello") };
        let json = serde_json::to_value(&submit).unwrap();
        assert_eq!(json["op"], "SubmitAgent");
        assert_eq!(json["payload"]["target"], "agent-a");
        let back: ControlCommandOperation = serde_json::from_value(json).unwrap();
        assert_eq!(back, submit);

        let health = ControlCommandOperation::HealthCheck;
        let json = serde_json::to_value(&health).unwrap();
        assert_eq!(json["op"], "HealthCheck");
        let back: ControlCommandOperation = serde_json::from_value(json).unwrap();
        assert_eq!(back, health);
    }

    // ── Payload::from_operation decode error paths ──

    #[test]
    fn payload_from_operation_decode_errors() {
        let cases: Vec<(Operation, serde_json::Value, &str)> = vec![
            (Operation::Agent(AgentOperation::Submit), serde_json::json!({}), "agent.submit"),
            (Operation::Agent(AgentOperation::Cancel), serde_json::json!({}), "agent.cancel"),
            (Operation::Agent(AgentOperation::Unsubscribe), serde_json::json!({}), "agent.unsubscribe"),
            (Operation::Agent(AgentOperation::Status), serde_json::json!({}), "agent.status"),
            (Operation::Agent(AgentOperation::ContextSnapshot), serde_json::json!({}), "agent.context_snapshot"),
            (Operation::File(FileOperation::List), serde_json::json!({}), "file.list"),
            (Operation::Session(SessionOperation::Resume), serde_json::json!({}), "session.resume"),
            (Operation::Mcp(McpOperation::CallTool), serde_json::json!({}), "mcp.call_tool"),
            (Operation::Mcp(McpOperation::ReadResource), serde_json::json!({}), "mcp.read_resource"),
            (Operation::Mcp(McpOperation::Reconnect), serde_json::json!({}), "mcp.reconnect"),
            (Operation::Skill(SkillOperation::Get), serde_json::json!({}), "skill.get"),
            (Operation::Tool(ToolOperation::Call), serde_json::json!({}), "tool.call"),
            (Operation::Log(LogOperation::Read), serde_json::json!({}), "log.read"),
            (Operation::Task(TaskOperation::Get), serde_json::json!({}), "task.get"),
        ];
        for (op, value, expected_method) in cases {
            let err = Payload::from_operation(&op, value).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains(expected_method), "expected '{}' in '{:?}'", expected_method, msg);
        }
    }

    // ── Struct serde round-trips ──

    #[test]
    fn capability_structures_serde() {
        let agent_cap = AgentCapability { agent_id: "a1".into(), name: "agent-a".into(), description: Some("test agent".into()), status: Some("idle".into()) };
        let json = serde_json::to_value(&agent_cap).unwrap();
        assert_eq!(json["agent_id"], "a1");
        let back: AgentCapability = serde_json::from_value(json).unwrap();
        assert_eq!(back, agent_cap);

        let tool_cap = ToolCapability { name: "read_file".into(), description: Some("Reads a file".into()), sensitivity: Some("medium".into()), requires_approval: true };
        let json = serde_json::to_value(&tool_cap).unwrap();
        let back: ToolCapability = serde_json::from_value(json).unwrap();
        assert_eq!(back, tool_cap);

        let mcp_cap = McpServerCapability { name: "mcp-srv".into(), status: Some("connected".into()) };
        let json = serde_json::to_value(&mcp_cap).unwrap();
        let back: McpServerCapability = serde_json::from_value(json).unwrap();
        assert_eq!(back, mcp_cap);

        let skill_cap = SkillCapability { name: "test-skill".into(), description: Some("a skill".into()) };
        let json = serde_json::to_value(&skill_cap).unwrap();
        let back: SkillCapability = serde_json::from_value(json).unwrap();
        assert_eq!(back, skill_cap);
    }

    #[test]
    fn node_load_default() {
        let load = NodeLoad::default();
        assert_eq!(load.running, 0);
        assert_eq!(load.queued, 0);
    }

    #[test]
    fn agent_server_message_round_trip() {
        let msg = AgentServerMessage::new_command("msg-1", Operation::Agent(AgentOperation::Submit), Payload::Agent(AgentPayload::Submit { input: AgentInput::text("hello world"), target: Some("coding".into()) }));
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: AgentServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.message_id, msg.message_id);
        assert_eq!(decoded.operation.method_name(), "agent.submit");
        match decoded.payload {
            Payload::Agent(AgentPayload::Submit { ref input, ref target }) => {
                assert_eq!(input.display_text(), "hello world");
                assert_eq!(target.as_deref(), Some("coding"));
            }
            _ => panic!("unexpected payload"),
        }
    }
}
