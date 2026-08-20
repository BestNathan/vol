//! AgentInjector — 把可用 agent 定义贡献进上下文，提示可用 `agent` 工具派发 subagent。

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent::agent_loader::AgentLoader;
use vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor, ContextError};
use vol_llm_core::Message;

/// Formats agent metadata for context injection（参考 vol-llm-skill::SkillInjector）。
pub struct AgentInjector {
    loader: Arc<AgentLoader>,
    anchor: AttentionAnchor,
    cached_size: tokio::sync::Mutex<usize>,
}

impl AgentInjector {
    /// Create an AgentInjector; anchor 固定 Head(1)，跟随 skill 惯例。
    pub fn new(loader: Arc<AgentLoader>) -> Self {
        Self {
            loader,
            anchor: AttentionAnchor::Head(1),
            cached_size: tokio::sync::Mutex::new(0),
        }
    }

    /// Format metadata as prompt string. Returns empty string if no agents are defined.
    pub async fn format_metadata(&self) -> String {
        let metadata = self.loader.list_metadata().await;
        if metadata.is_empty() {
            return String::new();
        }
        let mut output = String::from(
            "You can dispatch sub-agents to handle tasks collaboratively using the `agent` tool (args: id, prompt, description). Available agents:\n",
        );
        for m in &metadata {
            output.push_str(&format!("- {} ({}): {}\n", m.id, m.name, m.description));
        }
        output
    }
}

#[async_trait]
impl ContextContributor for AgentInjector {
    fn name(&self) -> &str {
        "agents"
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        let metadata_text = self.format_metadata().await;
        if metadata_text.is_empty() {
            // 无定义时保持固定 Head 槽位（与 SkillInjector 行为一致）
            *self.cached_size.lock().await = 0;
            return Ok(vec![ContextBlock::new(vec![], self.anchor.clone())]);
        }
        let msg = Message::user(metadata_text);
        let size = vol_llm_context::estimate_tokens(&msg);
        *self.cached_size.lock().await = size;
        Ok(vec![ContextBlock::new(vec![msg], self.anchor.clone())])
    }

    async fn compress(&mut self) {
        // 静态提示内容，无需压缩。
    }

    fn estimate_size(&self) -> usize {
        // Try to read cached value without blocking; fall back to 0
        self.cached_size.try_lock().map(|g| *g).unwrap_or(0)
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(AgentInjector {
            loader: self.loader.clone(),
            anchor: self.anchor.clone(),
            cached_size: tokio::sync::Mutex::new(0), // fresh cache for clone
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    async fn loader_with_defs() -> (tempfile::TempDir, Arc<AgentLoader>) {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        let mut f = std::fs::File::create(agents_dir.join("explore.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: explore").unwrap();
        writeln!(f, "type: explore").unwrap();
        writeln!(f, "description: 搜索代码库").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "You explore the codebase.").unwrap();
        let loader = Arc::new(AgentLoader::new(Some(temp_dir.path().to_path_buf())));
        loader.discover_all().await.unwrap();
        (temp_dir, loader)
    }

    #[tokio::test]
    async fn test_injector_contributes_agent_list() {
        let (_temp_dir, loader) = loader_with_defs().await;
        let injector = AgentInjector::new(loader);
        let blocks = injector.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        // ContextBlock.messages 是 pub 字段；用 Debug 输出做稳健断言
        let text = format!("{:?}", blocks[0].messages);
        assert!(text.contains("`agent` tool"), "text: {text}");
        assert!(text.contains("repo:explore"), "text: {text}");
        assert!(text.contains("搜索代码库"), "text: {text}");
    }

    #[tokio::test]
    async fn test_injector_empty_when_no_defs() {
        let injector = AgentInjector::new(Arc::new(AgentLoader::new_empty()));
        let blocks = injector.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].messages.is_empty());
    }
}
