//! PPT Agent 核心实现。

use crate::ppt::analysis::{AnalysisError, AnalysisModule};
use crate::ppt::renderer::{PptxRenderer, RendererError};
use crate::ppt::template::TemplateRegistry;
use crate::ppt::tools::content::{ContentError, ContentGeneratorTool};
use crate::ppt::tools::outline::{OutlineError, OutlineGeneratorTool};
use crate::ppt::{PptAgentConfig, PptInput, PptOutput, PptTemplate, StructuredRequirement};
use chrono::Local;
use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_core::LLMClient;
use vol_llm_provider::{LLMProviderConfig, LLMProviderRegistry};

/// PPT Agent
pub struct PptAgent {
    config: PptAgentConfig,
    llm: Arc<dyn LLMClient>,
    template_registry: TemplateRegistry,
}

impl PptAgent {
    /// 创建新的 PPT Agent
    pub async fn new(config: PptAgentConfig) -> Result<Self, PptAgentError> {
        eprintln!(
            "DEBUG: config.llm_provider_id = '{}'",
            config.llm_provider_id
        );

        // Initialize LLM from config
        let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
            .map_err(|_| PptAgentError::ConfigError("ANTHROPIC_AUTH_TOKEN not set".to_string()))?;

        let llm_config = LLMProviderConfig {
            id: config.llm_provider_id.clone(),
            config: vol_llm_provider::LLMConfig {
                provider: vol_llm_core::LLMProvider::Anthropic,
                model: "qwen3.5-plus".to_string(),
                api_key: vol_llm_provider::Secret::literal(api_key),
                base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
                body: None,
                headers: None,
            },
        };

        let registry = LLMProviderRegistry::from_configs(&[llm_config])
            .map_err(|e| PptAgentError::ConfigError(format!("Failed to initialize LLM: {e}")))?;

        let llm = registry.get(&config.llm_provider_id).ok_or_else(|| {
            PptAgentError::ConfigError(format!(
                "LLM provider '{}' not found",
                config.llm_provider_id
            ))
        })?;

        // Initialize template registry
        let mut template_registry = TemplateRegistry::new();
        if let Some(template_dir) = &config.template_dir {
            template_registry.load_from_dir(template_dir).map_err(|e| {
                PptAgentError::ConfigError(format!("Failed to load templates: {e}"))
            })?;
        }

        Ok(Self {
            config,
            llm,
            template_registry,
        })
    }

    /// 生成 PPT 主流程
    pub async fn generate(&self, input: PptInput) -> Result<PptOutput, PptAgentError> {
        // 1. Extract topic and context
        let (description, context) = match &input {
            PptInput::Text {
                description,
                context,
            } => (description.as_str(), context.as_deref()),
        };

        // 2. Analyze requirements
        let analysis = AnalysisModule::new(self.llm.clone());
        let requirements = analysis.analyze(description, context).await?;

        // 3. Generate outline
        let outline_tool = OutlineGeneratorTool::new(self.llm.clone());
        let mut outline = outline_tool
            .generate(
                &requirements.topic,
                requirements.audience.as_deref(),
                requirements.style.as_deref(),
                requirements.purpose.as_deref(),
            )
            .await?;

        // 4. Expand content
        let content_tool = ContentGeneratorTool::new(self.llm.clone());
        outline = content_tool.expand(&outline).await?;

        // 5. Match template
        let template = self.match_template(&requirements);

        // 6. Render PPTX
        let mut renderer = PptxRenderer::new(template.clone());
        renderer.render_outline(&outline)?;

        let output_path = self.generate_output_path(&requirements.topic);
        renderer.save(&output_path)?;

        Ok(PptOutput {
            output_path,
            slide_count: outline.slides.len() + 2, // +2 for title and TOC
            template_id: template.id.clone(),
            slides: outline
                .slides
                .iter()
                .map(|s| s.to_slide(crate::ppt::SlideLayout::TitleAndContent))
                .collect(),
        })
    }

    /// 匹配最佳模板
    #[allow(clippy::expect_used)]
    fn match_template(&self, requirements: &StructuredRequirement) -> Arc<PptTemplate> {
        let mut keywords = vec![requirements.topic.clone()];
        if let Some(style) = &requirements.style {
            keywords.push(style.clone());
        }
        if let Some(audience) = &requirements.audience {
            keywords.push(audience.clone());
        }

        self.template_registry
            .match_template(&keywords)
            .cloned()
            .unwrap_or_else(|| {
                self.template_registry
                    .list_templates()
                    .first()
                    .expect("No templates available")
                    .clone()
            })
    }

    /// 生成输出路径
    fn generate_output_path(&self, topic: &str) -> PathBuf {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let topic_slug = topic
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join("_");
        let filename = format!("{timestamp}_{topic_slug}.pptx");

        self.config
            .default_output_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(filename)
    }
}

/// PPT Agent 错误
#[derive(Debug, thiserror::Error)]
pub enum PptAgentError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Analysis failed: {0}")]
    AnalysisError(#[from] AnalysisError),

    #[error("Outline generation failed: {0}")]
    OutlineError(#[from] OutlineError),

    #[error("Content generation failed: {0}")]
    ContentError(#[from] ContentError),

    #[error("Rendering failed: {0}")]
    RenderError(#[from] RendererError),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
