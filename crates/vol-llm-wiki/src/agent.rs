//! WikiAgent - LLM-powered wiki compression agent.

use std::path::PathBuf;
use std::sync::Arc;

use vol_llm_agent::ReActAgent;
use vol_llm_core::{LLMClient, LLMProvider};
use vol_llm_context::ContextBuilder;
use vol_llm_provider::{LLMProviderConfig, LLMProviderRegistry, LLMConfig, Secret};
use vol_llm_tool::ToolRegistry;

use crate::config::WikiAgentConfig;
use crate::error::WikiAgentError;

/// Result of a wiki compression operation.
#[derive(Debug, Clone)]
pub struct WikiCompressResult {
    pub pages_created: Vec<String>,
    pub pages_updated: Vec<String>,
    pub summary: String,
}

/// Wiki Agent
pub struct WikiAgent {
    config: WikiAgentConfig,
    llm: Arc<dyn LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: ContextBuilder,
}

impl WikiAgent {
    /// Create a new WikiAgent from config.
    ///
    /// If `config.llm` is None, an LLM is created from `ANTHROPIC_AUTH_TOKEN`.
    pub fn new(config: WikiAgentConfig) -> Result<Self, WikiAgentError> {
        let llm = Self::resolve_llm(&config)?;
        let (tool_registry, context_builder) = Self::build_tools_and_context(&config)?;

        Ok(Self {
            config,
            llm,
            tool_registry,
            context_builder,
        })
    }

    /// Resolve LLM from config or create from env.
    fn resolve_llm(config: &WikiAgentConfig) -> Result<Arc<dyn LLMClient>, WikiAgentError> {
        if let Some(llm) = &config.llm {
            return Ok(llm.clone());
        }

        let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
            .map_err(|_| WikiAgentError::Config(
                "ANTHROPIC_AUTH_TOKEN not set and no LLM client provided".to_string(),
            ))?;

        let llm_config = LLMProviderConfig {
            id: config.llm_provider_id.clone(),
            config: LLMConfig {
                provider: LLMProvider::Anthropic,
                model: "qwen3.5-plus".to_string(),
                api_key: Secret::literal(api_key),
                base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
            },
        };
        let registry = LLMProviderRegistry::from_configs(&[llm_config])
            .map_err(|e| WikiAgentError::Config(format!("LLM provider error: {}", e)))?;

        registry.get(&config.llm_provider_id)
            .ok_or_else(|| WikiAgentError::Config(
                format!("LLM provider '{}' not found", config.llm_provider_id),
            ))
            .map(|llm| llm.clone())
    }

    /// Build tool registry and context builder.
    fn build_tools_and_context(config: &WikiAgentConfig) -> Result<(Arc<ToolRegistry>, ContextBuilder), WikiAgentError> {
        let mut tool_registry = ToolRegistry::new();
        Self::register_wiki_tools(&mut tool_registry);

        let wiki_dir = config.working_dir.join(".agent").join("wikis");
        let context_builder = vol_llm_context::ContextBuilderBuilder::new(128_000)
            .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
                Self::system_prompt(&wiki_dir),
            )))
            .build();

        Ok((Arc::new(tool_registry), context_builder))
    }

    /// Register tools for wiki operations.
    fn register_wiki_tools(registry: &mut ToolRegistry) {
        use vol_llm_tools_builtin::read_tool::ReadTool;
        use vol_llm_tools_builtin::write_tool::WriteTool;
        use vol_llm_tools_builtin::edit_tool::EditTool;
        use vol_llm_tools_builtin::glob_tool::GlobTool;
        use vol_llm_tools_builtin::grep_tool::GrepTool;

        registry.register(ReadTool::new());
        registry.register(WriteTool::new());
        registry.register(EditTool::new());
        registry.register(GlobTool::new());
        registry.register(GrepTool::new());
    }

    /// Build the system prompt for WikiAgent.
    fn system_prompt(wiki_dir: &PathBuf) -> String {
        format!(
            r#"你是一个知识管理 agent。你的任务是分析一段对话记录，从中提取有价值的信息，
维护一个位于 {wiki_dir} 的知识 Wiki。

请：
1. 分析对话，提取实体、概念、决策、待办等信息
2. 创建或更新 wiki 页面（使用 write/edit 工具）
3. 更新 INDEX.md 保持目录更新
4. 页面之间保持互相链接（使用相对路径）

规则：
- 页面是纯 Markdown 格式
- 每个页面顶部包含 frontmatter: title, tags, updated_at
- 不要写重复的页面，检查已有页面是否需要更新
- INDEX.md 应该包含所有页面的标题和简要描述"#,
            wiki_dir = wiki_dir.display(),
        )
    }

    /// Run wiki compression on a set of session messages.
    ///
    /// The agent will analyze the messages and create/update wiki pages.
    pub async fn compress(
        &self,
        messages: Vec<vol_session::SessionMessage>,
    ) -> Result<WikiCompressResult, WikiAgentError> {
        // Format messages as text for the agent
        let message_text = messages
            .iter()
            .map(|m| {
                let role = match m.message.role {
                    vol_llm_core::MessageRole::User => "User",
                    vol_llm_core::MessageRole::Assistant => "Assistant",
                    vol_llm_core::MessageRole::System => "System",
                    vol_llm_core::MessageRole::Tool => "Tool",
                };
                let content = m.message.content.as_ref().map(|c| c.as_str()).unwrap_or("(empty)");
                format!("[{}] {}", role, content)
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let prompt = format!(
            "以下是需要压缩的对话记录。请分析并更新 wiki 页面。\n\n=== 对话开始 ===\n{}\n=== 对话结束 ===",
            message_text,
        );

        // Create a session for this run
        use vol_session::InMemoryEntryStore;
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Arc::new(vol_session::Session::new(entry_store));

        let agent_config = vol_llm_agent::react::AgentConfig {
            max_iterations: self.config.max_iterations,
            max_history_messages: 20,
            context_builder: self.context_builder.clone(),
            plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
            agent_id: self.config.agent_id.clone(),
            working_dir: self.config.working_dir.clone(),
        };

        let react_agent = ReActAgent::new(
            self.llm.clone(),
            self.tool_registry.clone(),
            agent_config,
            session,
        );

        let response = react_agent
            .run(&prompt)
            .await
            .map_err(|e| WikiAgentError::Agent(e))?;

        // Extract created/updated pages from the wiki directory
        let wiki_dir = self.config.working_dir.join(".agent").join("wikis");
        let (created, updated) = Self::scan_wiki_changes(&wiki_dir);

        Ok(WikiCompressResult {
            pages_created: created,
            pages_updated: updated,
            summary: response.content,
        })
    }

    /// Scan the wiki directory for changes (naive: returns all files).
    fn scan_wiki_changes(wiki_dir: &PathBuf) -> (Vec<String>, Vec<String>) {
        let mut all = Vec::new();
        if wiki_dir.exists() {
            Self::walk_files(wiki_dir, &mut all);
        }
        (all.clone(), all)
    }

    fn walk_files(dir: &std::path::Path, files: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::walk_files(&path, files);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    files.push(name.to_string());
                }
            }
        }
    }
}
