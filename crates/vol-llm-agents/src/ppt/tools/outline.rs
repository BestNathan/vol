//! 大纲生成工具。

use crate::ppt::{prompts, Outline, SlideDef, SlideType};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use vol_llm_core::{ConversationRequest, LLMClient, Message};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult};

/// 大纲生成工具错误类型
#[derive(Debug, thiserror::Error)]
pub enum OutlineError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    #[error("JSON parsing failed: {0}")]
    JsonParseError(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// 大纲生成工具
pub struct OutlineGeneratorTool {
    llm: Arc<dyn LLMClient>,
}

impl OutlineGeneratorTool {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// 生成大纲
    pub async fn generate(
        &self,
        topic: &str,
        audience: Option<&str>,
        style: Option<&str>,
        purpose: Option<&str>,
    ) -> Result<Outline, OutlineError> {
        let user_message = Message::user(format!(
            r#"Create a presentation outline for:
- Topic: {}
- Audience: {}
- Style: {}
- Purpose: {}

Generate title slide, table of contents, 5-10 content slides, and summary. Return ONLY valid JSON."#,
            topic,
            audience.unwrap_or("general"),
            style.unwrap_or("professional"),
            purpose.unwrap_or("inform")
        ));

        let request = ConversationRequest::with_history(
            Some(prompts::OUTLINE_SYSTEM_PROMPT.to_string()),
            vec![user_message],
        );
        let response = self
            .llm
            .converse(request)
            .await
            .map_err(|e| OutlineError::LlmError(e.to_string()))?;

        // Parse JSON response - handle Option<MessageContent>
        let content_str = response
            .message
            .content
            .as_ref()
            .map(|c| c.as_str())
            .unwrap_or("");
        let json: Value = serde_json::from_str(content_str)
            .map_err(|e| OutlineError::JsonParseError(e.to_string()))?;

        // Parse into Outline struct
        let title = json["title"]
            .as_str()
            .ok_or_else(|| OutlineError::MissingField("title".to_string()))?
            .to_string();

        let slides_array = json["slides"]
            .as_array()
            .ok_or_else(|| OutlineError::MissingField("slides".to_string()))?;

        let mut slides = Vec::new();
        for slide_json in slides_array {
            let slide_type_str = slide_json["type"]
                .as_str()
                .ok_or_else(|| OutlineError::MissingField("slide type".to_string()))?;

            let slide_type = match slide_type_str.to_lowercase().as_str() {
                "title" => SlideType::Title,
                "toc" | "table_of_contents" => SlideType::TableOfContents,
                "section_header" => SlideType::SectionHeader,
                _ => SlideType::Content,
            };

            let slide_def = SlideDef {
                slide_type,
                title: slide_json["title"]
                    .as_str()
                    .unwrap_or("Untitled")
                    .to_string(),
                subtitle: slide_json
                    .get("subtitle")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                bullets: slide_json
                    .get("bullets")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                sections: slide_json
                    .get("sections")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
            };

            slides.push(slide_def);
        }

        Ok(Outline { title, slides })
    }
}

#[async_trait]
impl ExecutableTool for OutlineGeneratorTool {
    fn name(&self) -> &'static str {
        "generate_outline"
    }

    fn description(&self) -> &'static str {
        "Generate a structured outline for a PowerPoint presentation based on topic and requirements."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {"type": "string", "description": "Presentation topic"},
                "audience": {"type": "string", "description": "Target audience"},
                "style": {"type": "string", "description": "Preferred style"},
                "purpose": {"type": "string", "description": "Presentation purpose"}
            },
            "required": ["topic"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let topic = args["topic"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'topic' argument".to_string())
        })?;

        let audience = args["audience"].as_str();
        let style = args["style"].as_str();
        let purpose = args["purpose"].as_str();

        match self.generate(topic, audience, style, purpose).await {
            Ok(outline) => {
                let json = serde_json::to_value(&outline).unwrap_or(Value::Null);
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
