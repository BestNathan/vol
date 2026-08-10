use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, MessageKind, Operation, Payload,
};
use vol_llm_agent_protocol::DomainHandler;
use vol_llm_core::agent_def::AgentDef;
use vol_llm_mcp::McpManager;
use vol_llm_skill::SkillLoader;
use vol_llm_tool::ToolRegistry;
use vol_llm_tool::ToolResult;
use vol_llm_tool::ToolResultType;

use super::CapabilityHandler;

/// A simple dummy tool for testing.
struct TestTool(&'static str);

#[async_trait]
impl vol_llm_tool::ExecutableTool for TestTool {
    fn name(&self) -> &'static str {
        self.0
    }
    fn description(&self) -> &'static str {
        "test tool"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn sensitivity(&self, _args: &serde_json::Value) -> vol_llm_tool::ToolSensitivity {
        vol_llm_tool::ToolSensitivity::Safe
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _context: &vol_llm_tool::ToolContext,
    ) -> ToolResultType<ToolResult> {
        Ok(ToolResult {
            call_id: "test".into(),
            success: true,
            content: "ok".into(),
            error: None,
            data: None,
        })
    }
}

fn test_agent_def() -> AgentDef {
    let mut def = AgentDef::default();
    def.tools = Some(vec!["bash".into(), "read".into()]);
    def.disallowed_tools = Some(vec!["dangerous".into()]);
    def.mcps = None; // None = allow all MCP servers
    def
}

fn test_handler() -> CapabilityHandler {
    test_handler_with_def(test_agent_def())
}

fn test_handler_with_def(def: AgentDef) -> CapabilityHandler {
    let overlays = Arc::new(RwLock::new(HashMap::new()));
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(TestTool("bash"));
    tool_registry.register(TestTool("read"));
    let tool_registry = Arc::new(tool_registry);
    let skill_loader = Arc::new(SkillLoader::new_empty());
    let mcp_manager = Arc::new(McpManager::new(vec![]));
    let agent_defs = {
        let mut map: HashMap<String, AgentDef> = HashMap::new();
        map.insert("test-agent".into(), def);
        Arc::new(std::sync::RwLock::new(map))
    };
    CapabilityHandler::new(
        overlays,
        tool_registry,
        skill_loader,
        mcp_manager,
        agent_defs,
    )
}

/// Handler whose SkillLoader has a registered skill (so allowlisted skills
/// pass the registry existence check).
async fn test_handler_with_def_and_skill(def: AgentDef, skill_name: &str) -> CapabilityHandler {
    let overlays = Arc::new(RwLock::new(HashMap::new()));
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(TestTool("bash"));
    tool_registry.register(TestTool("read"));
    let tool_registry = Arc::new(tool_registry);
    let skill_loader = Arc::new(SkillLoader::new_empty());
    skill_loader
        .register(vol_llm_skill::SkillDef::new(skill_name, "test skill"))
        .await;
    let mcp_manager = Arc::new(McpManager::new(vec![]));
    let agent_defs = {
        let mut map: HashMap<String, AgentDef> = HashMap::new();
        map.insert("test-agent".into(), def);
        Arc::new(std::sync::RwLock::new(map))
    };
    CapabilityHandler::new(
        overlays,
        tool_registry,
        skill_loader,
        mcp_manager,
        agent_defs,
    )
}

fn msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
    AgentServerMessage {
        protocol: "agent-server/1".into(),
        message_id: id.into(),
        sender: "client".into(),
        receiver: "data-plane".into(),
        kind: MessageKind::Command,
        operation: op,
        payload,
        meta: Default::default(),
    }
}

#[tokio::test]
async fn get_capabilities_returns_defaults_when_no_overlay() {
    let handler = test_handler();
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::GetCapabilities),
            Payload::Agent(AgentPayload::GetCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
            }),
        ))
        .await
        .unwrap();

    let json = replies[0].payload.data_json();
    let tools = json["effective_tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    // base_tools should match effective_tools when no overlay
    let base = json["base_tools"].as_array().unwrap();
    assert_eq!(base.len(), 2);
}

#[tokio::test]
async fn update_capabilities_rejects_disallowed_tool() {
    let handler = test_handler();
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
                effective_tools: vec!["dangerous".into()],
                effective_skills: vec![],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap();

    let json = replies[0].payload.data_json();
    assert_eq!(json["code"], "tool_disallowed");
}

#[tokio::test]
async fn update_capabilities_creates_overlay_and_returns_result() {
    let handler = test_handler();
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
                effective_tools: vec!["bash".into()],
                effective_skills: vec![],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap();

    let json = replies[0].payload.data_json();
    assert_eq!(json["effective_tools"].as_array().unwrap().len(), 1);

    // Subsequent get should return overlay values (1 tool, not 2 from base)
    let replies2 = handler
        .handle(msg(
            "2",
            Operation::Agent(AgentOperation::GetCapabilities),
            Payload::Agent(AgentPayload::GetCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
            }),
        ))
        .await
        .unwrap();
    let json2 = replies2[0].payload.data_json();
    assert_eq!(json2["effective_tools"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn update_capabilities_empty_lists_remove_overlay() {
    let handler = test_handler();
    // First create an overlay
    handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
                effective_tools: vec!["bash".into()],
                effective_skills: vec![],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap();

    // Now reset with empty lists
    handler
        .handle(msg(
            "2",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
                effective_tools: vec![],
                effective_skills: vec![],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap();

    // Should be back to defaults (2 tools)
    let replies = handler
        .handle(msg(
            "3",
            Operation::Agent(AgentOperation::GetCapabilities),
            Payload::Agent(AgentPayload::GetCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
            }),
        ))
        .await
        .unwrap();
    let json = replies[0].payload.data_json();
    assert_eq!(json["effective_tools"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn update_capabilities_rejects_unknown_tool() {
    let handler = test_handler();
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
                effective_tools: vec!["nonexistent_tool".into()],
                effective_skills: vec![],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap();
    let json = replies[0].payload.data_json();
    assert_eq!(json["code"], "unknown_tool");
}

#[tokio::test]
async fn update_capabilities_rejects_skill_not_in_allowlist() {
    let mut def = test_agent_def();
    def.skills = Some(vec!["skill-a".into()]);
    let handler = test_handler_with_def(def);
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
                effective_tools: vec![],
                effective_skills: vec!["skill-b".into()],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap();

    let json = replies[0].payload.data_json();
    assert_eq!(json["code"], "skill_not_allowed");
}

#[tokio::test]
async fn update_capabilities_allows_skill_in_allowlist() {
    let mut def = test_agent_def();
    def.skills = Some(vec!["skill-a".into()]);
    let handler = test_handler_with_def_and_skill(def, "skill-a").await;
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
                effective_tools: vec![],
                effective_skills: vec!["skill-a".into()],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap();

    let json = replies[0].payload.data_json();
    let skills = json["effective_skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0], "skill-a");
}

#[tokio::test]
async fn get_capabilities_returns_base_skills_from_def() {
    let mut def = test_agent_def();
    def.skills = Some(vec!["skill-a".into()]);
    let handler = test_handler_with_def_and_skill(def, "skill-a").await;
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::GetCapabilities),
            Payload::Agent(AgentPayload::GetCapabilities {
                agent_id: "test-agent".into(),
                session_id: "sess-1".into(),
            }),
        ))
        .await
        .unwrap();

    let json = replies[0].payload.data_json();
    let base = json["base_skills"].as_array().unwrap();
    assert_eq!(base.len(), 1);
    assert_eq!(base[0], "skill-a");
    let effective = json["effective_skills"].as_array().unwrap();
    assert_eq!(effective.len(), 1);
    assert_eq!(effective[0], "skill-a");
}

#[tokio::test]
async fn get_capabilities_returns_empty_for_unknown_agent() {
    let handler = test_handler();
    let replies = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::GetCapabilities),
            Payload::Agent(AgentPayload::GetCapabilities {
                agent_id: "no-such-agent".into(),
                session_id: "sess-1".into(),
            }),
        ))
        .await
        .unwrap();
    let json = replies[0].payload.data_json();
    assert_eq!(json["effective_tools"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn update_capabilities_wrong_payload_returns_error() {
    let handler = test_handler();
    let err = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::UpdateCapabilities),
            Payload::Agent(AgentPayload::GetCapabilities {
                agent_id: "x".into(),
                session_id: "y".into(),
            }),
        ))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("agent.update_capabilities"));
}

#[tokio::test]
async fn get_capabilities_wrong_payload_returns_error() {
    let handler = test_handler();
    let err = handler
        .handle(msg(
            "1",
            Operation::Agent(AgentOperation::GetCapabilities),
            Payload::Agent(AgentPayload::UpdateCapabilities {
                agent_id: "x".into(),
                session_id: "y".into(),
                effective_tools: vec![],
                effective_skills: vec![],
                effective_mcp_servers: vec![],
            }),
        ))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("agent.get_capabilities"));
}
