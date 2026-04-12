//! 内容生成工具。

use crate::ppt::{prompts, Outline};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use vol_llm_core::{ConversationRequest, LLMClient, Message};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult};

/// 内容生成工具
pub struct ContentGeneratorTool {
    llm: Arc<dyn LLMClient>,
}

impl ContentGeneratorTool {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// 扩展大纲内容为详细 bullet points
    pub async fn expand(&self, outline: &Outline) -> Result<Outline, ContentError> {
        let _system_message = Message::system(prompts::CONTENT_SYSTEM_PROMPT.to_string());
        let user_message = Message::user(prompts::build_content_user_prompt(
            &serde_json::to_string_pretty(outline).unwrap_or_default(),
        ));

        let mut request = ConversationRequest::with_history(
            Some(prompts::CONTENT_SYSTEM_PROMPT.to_string()),
            vec![user_message],
        );
        request.model_config.temperature = Some(0.7);
        request.model_config.max_tokens = Some(4096);

        let response = self
            .llm
            .converse(request)
            .await
            .map_err(|e| ContentError::LlmError(e.to_string()))?;

        // Parse expanded content - handle Option<MessageContent>
        let content_str = response
            .message
            .content
            .as_ref()
            .map(|c| c.as_str())
            .unwrap_or("");
        let json: Value = serde_json::from_str(content_str)
            .map_err(|e| ContentError::JsonParseError(e.to_string()))?;

        // Merge expanded bullets back into outline
        let mut slides = Vec::new();
        for (i, original_slide) in outline.slides.iter().enumerate() {
            let mut slide = original_slide.clone();

            // Try to get expanded bullets for this slide
            // The JSON response should be an object with slide indices as keys
            let expanded_bullets = if let Some(slides_obj) = json.as_object() {
                // Try to get by slide index as key
                slides_obj
                    .get(&i.to_string())
                    .or_else(|| slides_obj.get(&format!("slide_{}", i)))
                    .and_then(|v| v.as_array())
            } else if let Some(slides_arr) = json.as_array() {
                // Or the response might be an array
                slides_arr.get(i).and_then(|v| v.as_array())
            } else {
                None
            };

            if let Some(bullets) = expanded_bullets {
                slide.bullets = bullets
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }

            slides.push(slide);
        }

        Ok(Outline {
            title: outline.title.clone(),
            slides,
        })
    }
}

#[async_trait]
impl ExecutableTool for ContentGeneratorTool {
    fn name(&self) -> &'static str {
        "generate_content"
    }

    fn description(&self) -> &'static str {
        "Expand outline bullets into detailed, presentation-ready content."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "outline": {"type": "string", "description": "JSON outline to expand"}
            },
            "required": ["outline"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let outline_str = args["outline"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'outline' argument".to_string())
        })?;

        let outline: Outline = serde_json::from_str(outline_str)
            .map_err(|e| ToolError::InvalidArguments(format!("Invalid outline JSON: {}", e)))?;

        match self.expand(&outline).await {
            Ok(expanded) => {
                let json = serde_json::to_value(&expanded).unwrap_or(Value::Null);
                Ok(ToolResult {
                    call_id: String::new(),
                    success: true,
                    content: serde_json::to_string_pretty(&json).unwrap_or_default(),
                    error: None,
                    data: Some(json),
                })
            }
            Err(e) => Ok(ToolResult {
                call_id: String::new(),
                success: false,
                content: format!("Error: {}", e),
                error: Some(e.to_string()),
                data: None,
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    #[error("JSON parsing failed: {0}")]
    JsonParseError(String),
}
