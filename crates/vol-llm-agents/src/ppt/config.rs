//! PPT Agent 配置。

use std::path::PathBuf;

/// PPT Agent 配置
#[derive(Clone, Debug, Default)]
pub struct PptAgentConfig {
    /// LLM Provider ID
    pub llm_provider_id: String,
    /// 模板目录路径
    pub template_dir: Option<PathBuf>,
    /// 默认输出目录
    pub default_output_dir: Option<PathBuf>,
}

impl PptAgentConfig {
    pub fn with_llm_provider(mut self, provider_id: impl Into<String>) -> Self {
        self.llm_provider_id = provider_id.into();
        self
    }

    pub fn with_template_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.template_dir = Some(path.into());
        self
    }

    pub fn with_default_output_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.default_output_dir = Some(path.into());
        self
    }
}
