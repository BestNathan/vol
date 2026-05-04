//! AgentTool — dispatches sub-agents by type, running a full ReAct loop.
//!
//! # YAGNI Notes
//!
//! - **Tool filtering**: `AgentDef.tools` and `AgentDef.disallowed_tools` are parsed from
//!   frontmatter but not enforced at runtime. Sub-agents inherit all parent tools.
//!   The fields serve as metadata for the LLM and are reserved for future enforcement.
//! - **Model override**: `AgentDef.model` is parsed but not used. Sub-agents inherit the
//!   parent's LLM client.
//! - **Sensitivity**: `AgentTool` returns `Safe` because it is a dispatcher. The actual
//!   tool calls made by the sub-agent go through the parent's `ToolRegistry`, where each
//!   tool's own `sensitivity()` is still evaluated by the HITL system.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_core::LLMClient;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity};
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

use crate::agent_def::{AgentDef, AgentPath};
use crate::agent_loader::AgentLoader;
use crate::react::{AgentConfig, PluginRegistry};

/// Default system prompt for agents with empty body.
const DEFAULT_AGENT_PROMPT: &str =
    "You are a specialized AI agent. Follow the instructions provided.";

/// Parameters for the Agent tool.
#[derive(Debug, Deserialize)]
pub struct AgentToolParams {
    /// Agent type to dispatch (dispatch key)
    pub r#type: String,
    /// Full task instructions for the sub-agent
    pub prompt: String,
    /// Short (3-5 word) description of the task
    pub description: String,
}

/// Tool that dispatches sub-agents by type.
pub struct AgentTool {
    loader: Arc<AgentLoader>,
    llm: Arc<dyn LLMClient>,
    agent_path: AgentPath,
    max_depth: u32,
    parent_tools: Arc<ToolRegistry>,
    working_dir: PathBuf,
}

impl AgentTool {
    /// Create a new AgentTool.
    pub fn new(
        loader: Arc<AgentLoader>,
        llm: Arc<dyn LLMClient>,
        agent_path: AgentPath,
        max_depth: u32,
        parent_tools: Arc<ToolRegistry>,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            loader,
            llm,
            agent_path,
            max_depth,
            parent_tools,
            working_dir,
        }
    }

    /// Build tool registry for a sub-agent.
    /// Currently inherits all parent tools; tool filtering is deferred (YAGNI).
    fn build_tool_registry(&self, _def: &AgentDef) -> Arc<ToolRegistry> {
        self.parent_tools.clone()
    }

    /// Format an error response with available agent types.
    async fn format_type_not_found(&self, r#type: &str) -> String {
        let metadata = self.loader.list_metadata().await;
        let mut output = format!("Agent type '{}' not found.\n\n", r#type);
        if metadata.is_empty() {
            output.push_str(
                "No agents are defined. Create .md files in .agents/agents/ to define custom agents.",
            );
        } else {
            output.push_str("Available agent types:\n");
            for m in &metadata {
                output.push_str(&format!(
                    "- {} ({}): {}\n",
                    m.r#type, m.name, m.description
                ));
            }
        }
        output
    }
}

#[async_trait]
impl ExecutableTool for AgentTool {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn description(&self) -> &'static str {
        "Dispatch a specialized sub-agent to handle a task. \
         Sub-agents run independently with their own tools and system prompt."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "description": "Type of agent to dispatch"
                },
                "prompt": {
                    "type": "string",
                    "description": "Full task instructions for the sub-agent"
                },
                "description": {
                    "type": "string",
                    "description": "Short (3-5 word) description of the task"
                }
            },
            "required": ["type", "prompt", "description"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: AgentToolParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        // Depth check
        if self.agent_path.depth() >= self.max_depth as usize {
            return Err(ToolError::ExecutionFailed(format!(
                "Cannot dispatch: maximum dispatch depth ({}) reached at path '{}'",
                self.max_depth,
                self.agent_path
            )));
        }

        // Lookup agent by type
        let agents = self.loader.get_by_type(&params.r#type).await;
        if agents.is_empty() {
            let error_msg = self.format_type_not_found(&params.r#type).await;
            return Err(ToolError::ExecutionFailed(error_msg));
        }

        let def = agents[0].clone();

        // Build system prompt
        let system_prompt = if def.prompt.trim().is_empty() {
            DEFAULT_AGENT_PROMPT.to_string()
        } else {
            def.prompt.clone()
        };

        let tools = self.build_tool_registry(&def);

        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));

        let agent_config = AgentConfig::builder()
            .with_def((*def).clone())
            .with_llm(self.llm.clone())
            .with_tools(tools)
            .with_session(session)
            .with_system_prompt(system_prompt)
            .with_plugin_registry(PluginRegistry::new())
            .build()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to build agent config: {}", e)))?;

        let sub_agent = crate::react::ReActAgent::new(agent_config);

        let response = sub_agent.run(&params.prompt).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Sub-agent failed: {}", e))
        })?;

        Ok(ToolResult::success(response.content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vol_llm_core::{
        ConversationRequest, ConversationResponse, LLMProvider, StreamEvent, StreamEventData,
        StreamReceiver, SupportedParam,
    };

    use crate::agent_def::AgentScope;

    /// Mock LLM for testing AgentTool.
    struct MockLlm {
        response_text: String,
        call_count: Arc<AtomicUsize>,
    }

    impl MockLlm {
        fn new(response_text: String) -> Self {
            Self {
                response_text,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "mock-model"
        }
        fn supported_params(&self) -> &[SupportedParam] {
            &[]
        }

        async fn converse(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            unimplemented!("Use converse_stream")
        }

        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<StreamReceiver> {
            use tokio::sync::mpsc;
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel(10);
            let text = self.response_text.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
                        data: StreamEventData::ContentComplete { content: text },
                    }))
                    .await;
            });
            Ok(StreamReceiver::new(rx))
        }
    }

    #[tokio::test]
    async fn test_agent_tool_depth_limit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let mut f = std::fs::File::create(agents_dir.join("helper.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: helper").unwrap();
        writeln!(f, "type: helper").unwrap();
        writeln!(f, "description: A helper agent").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "You are a helper.").unwrap();

        let mut loader = AgentLoader::new_empty();
        loader.add_root(AgentScope::User, agents_dir);
        loader.discover_all().await.unwrap();

        let mock_llm = Arc::new(MockLlm::new("I am the answer.".to_string()));
        let parent_tools = Arc::new(ToolRegistry::new());

        // depth = 3, max_depth = 3 → should fail (depth >= max_depth)
        let deep_path = AgentPath::root().push("a").push("b");
        let tool = AgentTool::new(
            Arc::new(loader),
            mock_llm,
            deep_path,
            3,
            parent_tools,
            PathBuf::from("."),
        );

        let args = serde_json::json!({
            "type": "helper",
            "prompt": "help me",
            "description": "get help"
        });
        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("maximum dispatch depth"));
    }

    #[tokio::test]
    async fn test_agent_tool_type_not_found() {
        let loader = AgentLoader::new(None);
        let mock_llm = Arc::new(MockLlm::new("answer".to_string()));
        let parent_tools = Arc::new(ToolRegistry::new());

        let tool = AgentTool::new(
            Arc::new(loader),
            mock_llm,
            AgentPath::root(),
            3,
            parent_tools,
            PathBuf::from("."),
        );

        let args = serde_json::json!({
            "type": "nonexistent",
            "prompt": "do something",
            "description": "test task"
        });
        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn test_agent_tool_dispatch_and_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let mut f = std::fs::File::create(agents_dir.join("echo.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: echo").unwrap();
        writeln!(f, "type: echo").unwrap();
        writeln!(f, "description: Echoes back the prompt").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "Echo the user's prompt exactly.").unwrap();

        let mut loader = AgentLoader::new_empty();
        loader.add_root(AgentScope::User, agents_dir);
        loader.discover_all().await.unwrap();

        let mock_llm = Arc::new(MockLlm::new("ECHO: test prompt".to_string()));
        let parent_tools = Arc::new(ToolRegistry::new());

        let tool = AgentTool::new(
            Arc::new(loader),
            mock_llm.clone(),
            AgentPath::root(),
            3,
            parent_tools,
            PathBuf::from("."),
        );

        let args = serde_json::json!({
            "type": "echo",
            "prompt": "test prompt",
            "description": "test echo"
        });
        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_ok());
        let content = result.unwrap().content;
        assert_eq!(content, "ECHO: test prompt");
        assert_eq!(mock_llm.call_count(), 1);
    }
}
