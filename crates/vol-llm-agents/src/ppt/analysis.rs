//! 需求分析模块。

use std::sync::Arc;
use vol_llm_core::{LLMClient, ConversationRequest, Message};
use serde_json::Value;
use crate::ppt::{StructuredRequirement, prompts};

/// 分析错误
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    #[error("JSON parsing failed: {0}")]
    JsonParseError(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// 需求分析模块
pub struct AnalysisModule {
    llm: Arc<dyn LLMClient>,
}

impl AnalysisModule {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// 分析用户需求，提取结构化信息
    pub async fn analyze(&self, description: &str, context: Option<&str>) -> Result<StructuredRequirement, AnalysisError> {
        let user_message = Message::user(
            prompts::build_analysis_user_prompt(description, context)
        );

        let request = ConversationRequest::with_history(
            Some(prompts::ANALYSIS_SYSTEM_PROMPT.to_string()),
            vec![user_message]
        );

        let response = self.llm.converse(request).await
            .map_err(|e| AnalysisError::LlmError(e.to_string()))?;

        // Parse JSON response - handle Option<MessageContent>
        let content_str = response.message.content
            .as_ref()
            .map(|c| c.as_str())
            .unwrap_or("");
        let json: Value = serde_json::from_str(content_str)
            .map_err(|e| AnalysisError::JsonParseError(e.to_string()))?;

        // Extract fields from JSON
        let topic = json["topic"].as_str()
            .ok_or_else(|| AnalysisError::MissingField("topic".to_string()))?
            .to_string();

        let audience = json.get("audience")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let style = json.get("style")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let purpose = json.get("purpose")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(StructuredRequirement {
            topic,
            audience,
            style,
            purpose,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_error_display() {
        let err = AnalysisError::MissingField("topic".to_string());
        assert_eq!(format!("{}", err), "Missing required field: topic");
    }
}
