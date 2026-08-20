//! AgentTool — dispatches sub-agents by id, running a full ReAct loop.
//!
//! # Design Notes
//!
//! - 子 agent = `.agents/agents/` 中内置定义，按唯一 `AgentDef.id` 派发
//! - 深度守卫：调用方 `AgentDef.depth >= 调用方 tool_config.agent.max_depth`（默认 1）时拒绝
//! - 子 agent 会话按 `def.name` 经运行时 SessionManager 持久化，可被其他 agent / UI 观测
//! - `parent_tools` 以 `Weak` 持有：运行时注册时 registry 先 Arc 化、AgentTool 后注册，
//!   Weak 避免注册期循环引用（execute 时 upgrade）
//!
//! # YAGNI Notes
//!
//! - `AgentDef.tools` / `disallowed_tools` / `model` 不在本周期（见设计文档 Non-Goals）
//! - Sensitivity 返回 `Safe`：派发器本身无副作用；子 agent 的工具调用仍经各自 sensitivity 评估

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_agent::react::{AgentConfig, PluginRegistry};
use vol_llm_agent::{agent_loader::AgentLoader, ReActAgent};
use vol_llm_core::agent_def::AgentDef;
use vol_llm_core::LLMClient;
use vol_llm_tool::{
    ExecutableTool, ToolContext, ToolError, ToolRegistry, ToolResult, ToolResultType,
    ToolSensitivity,
};
use vol_session::{Session, SessionManager};

/// 默认派发深度上限（调用方 tool_config 未配置时）。
pub const DEFAULT_MAX_DEPTH: u32 = 1;

/// Default system prompt for agents with empty body.
const DEFAULT_AGENT_PROMPT: &str =
    "You are a specialized AI agent. Follow the instructions provided.";

/// Parameters for the Agent tool.
#[derive(Debug, Deserialize)]
pub struct AgentToolParams {
    /// 目标 agent 的唯一 id（"{scope}:{name}"，如 "repo:explore"）
    pub id: String,
    /// Full task instructions for the sub-agent
    pub prompt: String,
    /// Short (3-5 word) description of the task
    pub description: String,
}

/// Tool that dispatches sub-agents by id.
pub struct AgentTool {
    loader: Arc<AgentLoader>,
    llm: Arc<dyn LLMClient>,
    session_manager: Arc<dyn SessionManager>,
    parent_tools: Weak<ToolRegistry>,
}

impl AgentTool {
    /// Create a new AgentTool.
    pub fn new(
        loader: Arc<AgentLoader>,
        llm: Arc<dyn LLMClient>,
        session_manager: Arc<dyn SessionManager>,
        parent_tools: Weak<ToolRegistry>,
    ) -> Self {
        Self {
            loader,
            llm,
            session_manager,
            parent_tools,
        }
    }

    /// 读取调用方 tool_config 中的 `agent.max_depth`（缺省/非法 → DEFAULT_MAX_DEPTH）。
    fn caller_max_depth(caller: Option<&AgentDef>) -> u32 {
        caller
            .and_then(|d| d.tool_config.as_ref())
            .and_then(|cfg| cfg.get("agent"))
            .and_then(|v| v.get("max_depth"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(DEFAULT_MAX_DEPTH)
    }

    /// 由调用方与目标定义生成子 agent 定义：写入 parent_agent 与 depth+1。
    fn prepare_sub_def(caller: Option<&AgentDef>, def: &AgentDef) -> AgentDef {
        let mut sub_def = def.clone();
        sub_def.parent_agent = caller.map(|c| c.id.clone());
        sub_def.depth = caller.map(|c| c.depth + 1).unwrap_or(1);
        sub_def
    }

    /// Format an error response with available agents.
    async fn format_id_not_found(&self, id: &str) -> String {
        let metadata = self.loader.list_metadata().await;
        let mut output = format!("Agent id '{id}' not found.\n\n");
        if metadata.is_empty() {
            output.push_str(
                "No agents are defined. Create .md files in .agents/agents/ to define custom agents.",
            );
        } else {
            output.push_str("Available agents:\n");
            for m in &metadata {
                output.push_str(&format!("- {} ({}): {}\n", m.id, m.name, m.description));
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
        "Dispatch a sub-agent (defined in .agents/agents/) by id to handle a task. \
         The sub-agent runs a full ReAct loop and its final result is returned."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Id of the agent to dispatch (e.g. repo:explore)"
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
            "required": ["id", "prompt", "description"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: AgentToolParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse arguments: {e}")))?;

        let caller = context.agent_def.as_ref();
        let caller_depth = caller.map(|c| c.depth).unwrap_or(0);
        let max_depth = Self::caller_max_depth(caller);

        // 深度守卫：默认 1 = 只允许派发一层（根 agent 可派发，depth≥1 拒绝）
        if caller_depth >= max_depth {
            return Err(ToolError::ExecutionFailed(format!(
                "Cannot dispatch: maximum dispatch depth ({max_depth}) reached (caller depth {caller_depth})"
            )));
        }

        // 按 id 查找内置定义
        let def = match self.loader.get_by_id(&params.id).await {
            Some(def) => def,
            None => {
                let error_msg = self.format_id_not_found(&params.id).await;
                return Err(ToolError::ExecutionFailed(error_msg));
            }
        };

        // 生成子 agent 定义：记录 parent_agent 与 depth
        let sub_def = Self::prepare_sub_def(caller, &def);

        let system_prompt = if sub_def.prompt.trim().is_empty() {
            DEFAULT_AGENT_PROMPT.to_string()
        } else {
            sub_def.prompt.clone()
        };

        // 子 agent 会话按 name 持久化（与 register_agent 一致，session.list 可查）
        let parent_tools = self
            .parent_tools
            .upgrade()
            .ok_or_else(|| ToolError::ExecutionFailed("tool registry unavailable".to_string()))?;
        let session = Arc::new(Session::new(
            self.session_manager.entry_store_for_agent(&sub_def.name),
        ));

        let agent_config = AgentConfig::builder()
            .with_def(sub_def)
            .with_llm(self.llm.clone())
            .with_tools(parent_tools)
            .with_session(session)
            .with_system_prompt(system_prompt)
            .with_plugin_registry(PluginRegistry::new())
            .build()
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to build agent config: {e}"))
            })?;

        let base_tools = agent_config.tools.clone();
        let skill_loader = agent_config.skill_loader.clone();
        let sub_agent = ReActAgent::new(agent_config, base_tools, skill_loader);

        let response = sub_agent
            .run(&params.prompt)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Sub-agent failed: {e}")))?;

        Ok(ToolResult::success(response.content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vol_llm_agent::agent_loader::AgentLoader;
    use vol_llm_core::{
        ConversationRequest, ConversationResponse, LLMProvider, StreamEvent, StreamEventData,
        StreamReceiver, SupportedParam,
    };
    use vol_session::FileSessionManager;

    /// Mock LLM：返回固定文本，统计调用次数。
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

    /// 往临时目录的 `.agents/agents/echo.md` 写入 echo 定义并加载。
    async fn loader_with_echo() -> (tempfile::TempDir, Arc<AgentLoader>) {
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
        let loader = Arc::new(AgentLoader::new(Some(temp_dir.path().to_path_buf())));
        loader.discover_all().await.unwrap();
        (temp_dir, loader)
    }

    fn caller_def(depth: u32) -> AgentDef {
        let mut def = AgentDef::new("root", "root agent");
        def.id = "repo:root".to_string();
        def.depth = depth;
        def
    }

    #[test]
    fn test_prepare_sub_def_writes_parent_and_depth() {
        let caller = caller_def(2);
        let mut target = AgentDef::new("echo", "prompt");
        target.id = "repo:echo".to_string();
        let sub = AgentTool::prepare_sub_def(Some(&caller), &target);
        assert_eq!(sub.parent_agent, Some("repo:root".to_string()));
        assert_eq!(sub.depth, 3);
        assert_eq!(sub.id, "repo:echo");
        assert_eq!(sub.name, "echo");
    }

    #[test]
    fn test_caller_max_depth_defaults_and_parses() {
        assert_eq!(AgentTool::caller_max_depth(None), 1);
        let def = caller_def(0);
        assert_eq!(AgentTool::caller_max_depth(Some(&def)), 1);

        let mut configured = caller_def(0);
        let mut tool_config = std::collections::HashMap::new();
        tool_config.insert("agent".to_string(), serde_json::json!({ "max_depth": 3 }));
        configured.tool_config = Some(tool_config);
        assert_eq!(AgentTool::caller_max_depth(Some(&configured)), 3);

        let mut invalid = caller_def(0);
        let mut bad_config = std::collections::HashMap::new();
        bad_config.insert(
            "agent".to_string(),
            serde_json::json!({ "max_depth": "nope" }),
        );
        invalid.tool_config = Some(bad_config);
        assert_eq!(AgentTool::caller_max_depth(Some(&invalid)), 1);
    }

    #[tokio::test]
    async fn test_agent_tool_depth_guard_default_rejects_nested() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("ECHO".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(loader, llm, session_manager, Arc::downgrade(&registry));

        // 调用方 depth=1、无 tool_config → 默认 max_depth=1 → 拒绝
        let ctx = ToolContext {
            agent_def: Some(caller_def(1)),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:echo",
            "prompt": "help me",
            "description": "get help"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("maximum dispatch depth"));
    }

    #[tokio::test]
    async fn test_agent_tool_tool_config_allows_nested() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("ECHO: nested".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(loader, llm, session_manager, Arc::downgrade(&registry));

        // 调用方 tool_config.agent.max_depth=3、depth=1 → 允许
        let mut caller = caller_def(1);
        let mut tool_config = std::collections::HashMap::new();
        tool_config.insert("agent".to_string(), serde_json::json!({ "max_depth": 3 }));
        caller.tool_config = Some(tool_config);
        let ctx = ToolContext {
            agent_def: Some(caller),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:echo",
            "prompt": "nested task",
            "description": "nested"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "ECHO: nested");
    }

    #[tokio::test]
    async fn test_agent_tool_id_not_found_lists_available() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("unused".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(loader, llm, session_manager, Arc::downgrade(&registry));

        let ctx = ToolContext {
            agent_def: Some(caller_def(0)),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:missing",
            "prompt": "do something",
            "description": "test task"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
        assert!(err.contains("repo:echo"));
    }

    #[tokio::test]
    async fn test_agent_tool_dispatch_persists_session_by_name() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("ECHO: persisted".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(
            loader,
            llm.clone(),
            session_manager.clone(),
            Arc::downgrade(&registry),
        );

        let ctx = ToolContext {
            agent_def: Some(caller_def(0)),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:echo",
            "prompt": "test prompt",
            "description": "test echo"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_ok());
        assert_eq!(llm.call_count(), 1);

        // 会话按 def.name（"echo"）持久化，list_sessions 可查
        let sessions = session_manager.list_sessions(Some("echo")).await.unwrap();
        assert_eq!(sessions.len(), 1);
    }
}
