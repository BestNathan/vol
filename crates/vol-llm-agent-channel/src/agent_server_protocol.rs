use serde::{Deserialize, Serialize};

/// Lightweight protocol error type for operation lookup and payload decoding.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error("payload decode failed for {0}")]
    PayloadDecodeFailed(&'static str),
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
    Log(LogOperation),
    System(SystemOperation),
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
            Operation::Log(LogOperation::List) => "log.list",
            Operation::Log(LogOperation::Read) => "log.read",
            Operation::System(SystemOperation::Connected) => "system.connected",
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Payload {
    Agent(AgentPayload),
    File(FilePayload),
    Session(SessionPayload),
    Mcp(McpPayload),
    Skill(SkillPayload),
    Log(LogPayload),
    System(SystemPayload),
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
                    input: String,
                    #[serde(default)]
                    target: Option<String>,
                    #[serde(default)]
                    metadata: Option<serde_json::Map<String, serde_json::Value>>,
                    #[serde(default)]
                    run_id: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.submit"))?;
                Ok(Payload::Agent(AgentPayload::Submit {
                    input: p.input,
                    target: p.target,
                    metadata: p.metadata,
                    run_id: p.run_id,
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
                Ok(Payload::Session(SessionPayload::List))
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
        input: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Map<String, serde_json::Value>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
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
    List,
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
