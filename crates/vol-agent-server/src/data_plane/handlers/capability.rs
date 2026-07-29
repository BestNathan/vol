use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use vol_llm_runtime::CapabilityOverlay;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, ErrorPayload, Operation, Payload,
    ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;
use vol_llm_core::AgentDef;
use vol_llm_mcp::McpManager;
use vol_llm_skill::SkillLoader;
use vol_llm_tool::ToolRegistry;

/// Handler for agent.get_capabilities and agent.update_capabilities.
pub struct CapabilityHandler {
    overlays: Arc<RwLock<HashMap<(String, String), CapabilityOverlay>>>,
    tool_registry: Arc<ToolRegistry>,
    skill_loader: Arc<SkillLoader>,
    mcp_manager: Arc<McpManager>,
    agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
}

impl CapabilityHandler {
    pub fn new(
        overlays: Arc<RwLock<HashMap<(String, String), CapabilityOverlay>>>,
        tool_registry: Arc<ToolRegistry>,
        skill_loader: Arc<SkillLoader>,
        mcp_manager: Arc<McpManager>,
        agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
    ) -> Self {
        Self {
            overlays,
            tool_registry,
            skill_loader,
            mcp_manager,
            agent_defs,
        }
    }

    /// Resolve effective capabilities — overlay if exists, else AgentDef base.
    fn resolve_effective(
        &self,
        agent_id: &str,
        session_id: &str,
        overlays: &HashMap<(String, String), CapabilityOverlay>,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        if let Some(overlay) = overlays.get(&(agent_id.to_string(), session_id.to_string())) {
            return (
                overlay.effective_tools.clone(),
                overlay.effective_skills.clone(),
                overlay.effective_mcp_servers.clone(),
            );
        }
        let def = self
            .agent_defs
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(agent_id)
            .cloned();
        match def {
            Some(d) => (
                d.tools.unwrap_or_default(),
                vec![], // AgentDef has no skills field
                d.mcps.unwrap_or_default(),
            ),
            None => (vec![], vec![], vec![]),
        }
    }

    /// Gather available pools from registries.
    async fn gather_available(&self) -> AvailableLists {
        let tools: Vec<serde_json::Value> = self
            .tool_registry
            .tool_names()
            .iter()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                })
            })
            .collect();

        let skills: Vec<serde_json::Value> = self
            .skill_loader
            .list_metadata()
            .await
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "name": m.name,
                    "version": m.version,
                    "scope": m.scope.to_string(),
                    "description": m.description,
                })
            })
            .collect();

        let mcp_servers: Vec<serde_json::Value> = self
            .mcp_manager
            .server_status()
            .keys()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                })
            })
            .collect();

        AvailableLists {
            tools,
            skills,
            mcp_servers,
        }
    }
}

struct AvailableLists {
    tools: Vec<serde_json::Value>,
    skills: Vec<serde_json::Value>,
    mcp_servers: Vec<serde_json::Value>,
}

#[async_trait]
impl DomainHandler for CapabilityHandler {
    fn name(&self) -> &str {
        "capability"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Agent(AgentOperation::GetCapabilities),
            Operation::Agent(AgentOperation::UpdateCapabilities),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Agent(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("capability")),
        };

        tracing::info!(operation = ?op, "CapabilityHandler received request");

        match (op, message.payload) {
            (
                AgentOperation::GetCapabilities,
                Payload::Agent(AgentPayload::GetCapabilities {
                    agent_id,
                    session_id,
                }),
            ) => {
                let overlays = self.overlays.read().await;
                let (effective_tools, effective_skills, effective_mcp_servers) =
                    self.resolve_effective(&agent_id, &session_id, &overlays);

                let def = self
                    .agent_defs
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&agent_id)
                    .cloned();
                let (base_tools, base_skills, base_mcp_servers) = match def {
                    Some(d) => (
                        d.tools.unwrap_or_default(),
                        vec![], // AgentDef has no skills field
                        d.mcps.unwrap_or_default(),
                    ),
                    None => (vec![], vec![], vec![]),
                };

                let available = self.gather_available().await;
                drop(overlays);

                tracing::info!(
                    agent = %agent_id,
                    tools = effective_tools.len(),
                    skills = effective_skills.len(),
                    mcps = effective_mcp_servers.len(),
                    "CapabilityHandler: get_capabilities ok"
                );

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::GetCapabilities),
                    Payload::Agent(AgentPayload::GetCapabilitiesResult {
                        effective_tools,
                        effective_skills,
                        effective_mcp_servers,
                        available_tools: available.tools,
                        available_skills: available.skills,
                        available_mcp_servers: available.mcp_servers,
                        base_tools,
                        base_skills,
                        base_mcp_servers,
                    }),
                )])
            }

            (
                AgentOperation::UpdateCapabilities,
                Payload::Agent(AgentPayload::UpdateCapabilities {
                    agent_id,
                    session_id,
                    effective_tools,
                    effective_skills,
                    effective_mcp_servers,
                }),
            ) => {
                // --- Validation ---

                // 1. Check AgentDef disallowed_tools
                let def = self
                    .agent_defs
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&agent_id)
                    .cloned();
                if let Some(ref def) = def {
                    let disallowed: std::collections::HashSet<&str> = def
                        .disallowed_tools
                        .as_ref()
                        .map(|v| v.iter().map(String::as_str).collect())
                        .unwrap_or_default();
                    for tool in &effective_tools {
                        if disallowed.contains(tool.as_str()) {
                            return Ok(vec![AgentServerMessage::new_error(
                                message.message_id,
                                Operation::Agent(AgentOperation::UpdateCapabilities),
                                ErrorPayload {
                                    code: "tool_disallowed".to_string(),
                                    message: format!(
                                        "Tool '{tool}' is disallowed by agent definition"
                                    ),
                                    detail: None,
                                    terminal: false,
                                },
                            )]);
                        }
                    }

                    // 2. Check mcps allowlist constraint
                    if let Some(ref allowed_mcps) = def.mcps {
                        for server in &effective_mcp_servers {
                            if !allowed_mcps.contains(server) {
                                return Ok(vec![AgentServerMessage::new_error(
                                    message.message_id,
                                    Operation::Agent(AgentOperation::UpdateCapabilities),
                                    ErrorPayload {
                                        code: "mcp_not_allowed".to_string(),
                                        message: format!("MCP server '{server}' is not in agent's allowed mcps list"),
                                        detail: None,
                                        terminal: false,
                                    },
                                )]);
                            }
                        }
                    }
                }

                // 3. Validate tool names exist in master registry
                let master_tool_names: std::collections::HashSet<&str> =
                    self.tool_registry.tool_names().iter().copied().collect();
                for tool in &effective_tools {
                    if !master_tool_names.contains(tool.as_str()) {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::UpdateCapabilities),
                            ErrorPayload {
                                code: "unknown_tool".to_string(),
                                message: format!("Tool '{tool}' not found in registry"),
                                detail: None,
                                terminal: false,
                            },
                        )]);
                    }
                }

                // 4. Validate skill names exist
                let skill_metadata = self.skill_loader.list_metadata().await;
                let skill_names: std::collections::HashSet<&str> =
                    skill_metadata.iter().map(|m| m.name.as_str()).collect();
                for skill in &effective_skills {
                    if !skill_names.contains(skill.as_str()) {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::UpdateCapabilities),
                            ErrorPayload {
                                code: "unknown_skill".to_string(),
                                message: format!("Skill '{skill}' not found"),
                                detail: None,
                                terminal: false,
                            },
                        )]);
                    }
                }

                // 5. Validate MCP server names exist
                let server_status = self.mcp_manager.server_status();
                for server in &effective_mcp_servers {
                    if !server_status.contains_key(server.as_str()) {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::UpdateCapabilities),
                            ErrorPayload {
                                code: "unknown_mcp_server".to_string(),
                                message: format!("MCP server '{server}' not found"),
                                detail: None,
                                terminal: false,
                            },
                        )]);
                    }
                }

                // --- Apply overlay ---
                let key = (agent_id.clone(), session_id.clone());
                let mut overlays = self.overlays.write().await;

                if effective_tools.is_empty()
                    && effective_skills.is_empty()
                    && effective_mcp_servers.is_empty()
                {
                    // Empty lists = reset to default → remove overlay
                    overlays.remove(&key);
                } else if let Some(existing) = overlays.get_mut(&key) {
                    existing.update(
                        effective_tools.clone(),
                        effective_skills.clone(),
                        effective_mcp_servers.clone(),
                    );
                } else {
                    overlays.insert(
                        key,
                        CapabilityOverlay::new(
                            effective_tools.clone(),
                            effective_skills.clone(),
                            effective_mcp_servers.clone(),
                        ),
                    );
                }
                drop(overlays);

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::UpdateCapabilities),
                    Payload::Agent(AgentPayload::UpdateCapabilitiesResult {
                        effective_tools,
                        effective_skills,
                        effective_mcp_servers,
                    }),
                )])
            }

            (AgentOperation::GetCapabilities, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.get_capabilities"))
            }
            (AgentOperation::UpdateCapabilities, _) => Err(ProtocolError::PayloadDecodeFailed(
                "agent.update_capabilities",
            )),
            _ => Err(ProtocolError::PayloadDecodeFailed("capability")),
        }
    }
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
