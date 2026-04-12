//! PPT Agent 需求分析模块。

use crate::ppt::{prompts, StructuredRequirement};
use serde_json::Value;
use std::sync::Arc;
use vol_llm_core::{ConversationRequest, LLMClient, Message, MessageContent};

/// 需求分析模块
pub struct AnalysisModule {
    llm: Arc<dyn LLMClient>,
}

impl AnalysisModule {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// 分析用户需求，提取结构化信息
    pub async fn analyze(
        &self,
        description: &str,
        context: Option<&str>,
    ) -> Result<StructuredRequirement, AnalysisError> {
        // Build messages
        let system_message = Message::system(prompts::ANALYSIS_SYSTEM_PROMPT.to_string());
        let user_message = Message::user(prompts::build_analysis_user_prompt(description, context));

        // Call LLM
        let request = ConversationRequest::with_history(None, vec![system_message, user_message]);
        let response = self
            .llm
            .converse(request)
            .await
            .map_err(|e| AnalysisError::LlmError(e.to_string()))?;

        // Extract content from response message
        let content = response
            .message
            .content
            .ok_or_else(|| AnalysisError::EmptyResponse)?;

        let content_str = match &content {
            MessageContent::Text(s) => s.as_str(),
            MessageContent::MultiPart(_) => "",
        };

        if content_str.is_empty() {
            return Err(AnalysisError::EmptyResponse);
        }

        // Parse JSON response
        let json: Value = serde_json::from_str(content_str)
            .map_err(|e| AnalysisError::JsonParseError(e.to_string()))?;

        // Extract fields
        let topic = json["topic"]
            .as_str()
            .ok_or_else(|| AnalysisError::MissingField("topic".to_string()))?
            .to_string();

        let audience = json["audience"].as_str().map(|s| s.to_string());
        let style = json["style"].as_str().map(|s| s.to_string());
        let purpose = json["purpose"].as_str().map(|s| s.to_string());

        Ok(StructuredRequirement {
            topic,
            audience,
            style,
            purpose,
        })
    }
}

/// 分析错误
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("LLM call failed: {0}")]
    LlmError(String),

    #[error("JSON parsing failed: {0}")]
    JsonParseError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Empty response from LLM")]
    EmptyResponse,
}
