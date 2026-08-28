//! Test-only constructor for `AgentRuntime`.
//!
//! Gated behind `feature = "test-utils"` (enabled by consumers via dev-deps)
//! or `cfg(test)`. Not part of the production surface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::AgentRuntime;
use vol_llm_agent::AgentLoader;
use vol_llm_agent_tool::AgentTool;
use vol_llm_mcp::McpManager;
use vol_llm_provider::{create_provider, ProviderLoader};
use vol_llm_skill::SkillLoader;
use vol_llm_task::TaskStore;
use vol_llm_tool::ToolRegistry;
use vol_session::{FileSessionManager, SessionManager};

impl AgentRuntime {
    #[doc(hidden)]
    #[expect(clippy::expect_used)]
    pub async fn for_test() -> Self {
        let store_dir = PathBuf::from("/tmp/vol-llm-runtime-test");
        let working_dir = PathBuf::from(".");

        let mut llm_registry = ProviderLoader::load(Some(&working_dir));
        // 测试兜底：无 provider 时插入合成 provider（创建 LLM client 不发起网络请求，测试保持 hermetic）
        if llm_registry.is_empty() {
            llm_registry.insert(
                "test".to_string(),
                vol_llm_provider::ProviderFileConfig {
                    provider: vol_llm_core::LLMProvider::Anthropic,
                    model: "claude-test".to_string(),
                    api_key: vol_llm_provider::Secret::literal("sk-test"),
                    base_url: "https://api.test.com".to_string(),
                    body: None,
                    headers: None,
                },
            );
        }
        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        let task_store: Arc<dyn TaskStore> = Arc::new(vol_llm_task::InMemoryTaskStore::new());
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(store_dir.join("agents")));
        // Register the unified CLI-style `task` tool (agents using `tools: [task]`).
        vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
        // Register the unified CLI-style `fs` tool — single entry point for file ops.
        vol_llm_fs::tools::register_cli(&mut tool_registry);
        // 内置 agent 工具：for_test 用默认 provider 与空 AgentLoader。
        // 与 build() 相同，用 Arc::new_cyclic 让 Weak 与最终 Arc 同一分配（活 Weak）。
        let agent_tool_llm: Arc<dyn vol_llm_core::LLMClient> = {
            let ids = llm_registry.ids();
            let first_id = ids
                .first()
                .expect("for_test always seeds at least one provider");
            let fc = llm_registry
                .get(first_id)
                .expect("provider registered in registry");
            create_provider(&fc.to_llm_config())
                .expect("default provider creates")
                .into()
        };
        let tool_registry = Arc::new_cyclic(|registry_weak| {
            tool_registry.register(AgentTool::new(
                Arc::new(AgentLoader::new_empty()),
                agent_tool_llm,
                session_manager.clone(),
                registry_weak.clone(),
            ));
            tool_registry
        });
        let mcp_manager = Arc::new(McpManager::new(vec![]));
        let sandbox_manager = Arc::new(vol_llm_sandbox::SandboxManager::new());
        let skill_loader = Arc::new(SkillLoader::new_empty());

        AgentRuntime {
            working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            session_manager,
            mcp_manager,
            sandbox_manager,
            skill_loader,
            agent_loader: Arc::new(AgentLoader::new_empty()),
            agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
            agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
            capability_overlays: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}
