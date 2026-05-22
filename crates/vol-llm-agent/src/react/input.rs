use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use vol_llm_core::{ContentPart, ImageUrl, MessageContent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInputError;

impl fmt::Display for AgentInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Agent input must contain at least one part")
    }
}

impl std::error::Error for AgentInputError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputPart {
    Text {
        text: String,
    },
    ImageUrl {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub parts: Vec<InputPart>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AgentInputWire {
    Text(String),
    Structured {
        #[serde(default)]
        run_id: Option<String>,
        #[serde(default)]
        parts: Vec<InputPart>,
        #[serde(default)]
        metadata: HashMap<String, serde_json::Value>,
    },
}

impl<'de> Deserialize<'de> for AgentInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match AgentInputWire::deserialize(deserializer)? {
            AgentInputWire::Text(text) => Ok(Self::text(text)),
            AgentInputWire::Structured {
                run_id,
                parts,
                metadata,
            } => Ok(Self {
                run_id,
                parts,
                metadata,
            }),
        }
    }
}

impl AgentInput {
    pub fn new() -> Self {
        Self {
            run_id: None,
            parts: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new().text_part(text)
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub fn with_metadata_value(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn text_part(mut self, text: impl Into<String>) -> Self {
        self.parts.push(InputPart::Text { text: text.into() });
        self
    }

    pub fn image_url(mut self, url: impl Into<String>) -> Self {
        self.parts.push(InputPart::ImageUrl {
            url: url.into(),
            detail: None,
        });
        self
    }

    pub fn image_url_with_detail(
        mut self,
        url: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        self.parts.push(InputPart::ImageUrl {
            url: url.into(),
            detail: Some(detail.into()),
        });
        self
    }

    pub fn text_content(&self) -> String {
        self.display_text()
    }

    pub fn display_text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|part| match part {
                InputPart::Text { text } => Some(text.as_str()),
                InputPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn to_message_content(&self) -> Result<MessageContent, AgentInputError> {
        if self.parts.is_empty() {
            return Err(AgentInputError);
        }

        if let [InputPart::Text { text }] = self.parts.as_slice() {
            return Ok(MessageContent::Text(text.clone()));
        }

        let parts = self
            .parts
            .iter()
            .map(|part| match part {
                InputPart::Text { text } => ContentPart::Text { text: text.clone() },
                InputPart::ImageUrl { url, detail } => ContentPart::Image {
                    image_url: ImageUrl {
                        url: url.clone(),
                        detail: detail.clone(),
                    },
                },
            })
            .collect();

        Ok(MessageContent::MultiPart(parts))
    }
}

impl Default for AgentInput {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for AgentInput {
    fn from(value: String) -> Self {
        Self::text(value)
    }
}

impl From<&str> for AgentInput {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_converts_to_text_message_content() {
        let input = AgentInput::text("hello");

        assert_eq!(input.display_text(), "hello");
        assert_eq!(
            input.to_message_content().unwrap(),
            MessageContent::Text("hello".to_string())
        );
    }

    #[test]
    fn text_and_image_convert_to_multipart_message_content() {
        let input = AgentInput::new()
            .text_part("look")
            .image_url_with_detail("https://example.test/image.png", "high");

        assert_eq!(input.display_text(), "look");
        assert_eq!(
            input.to_message_content().unwrap(),
            MessageContent::MultiPart(vec![
                ContentPart::Text {
                    text: "look".to_string()
                },
                ContentPart::Image {
                    image_url: ImageUrl {
                        url: "https://example.test/image.png".to_string(),
                        detail: Some("high".to_string()),
                    },
                },
            ])
        );
    }

    #[test]
    fn empty_input_returns_error() {
        let err = AgentInput::new().to_message_content().unwrap_err();
        assert_eq!(
            err.to_string(),
            "Agent input must contain at least one part"
        );
    }

    #[test]
    fn string_deserializes_as_text_input() {
        let input: AgentInput = serde_json::from_str(r#""hello""#).unwrap();
        assert_eq!(input, AgentInput::text("hello"));
    }

    #[test]
    fn object_deserializes_as_structured_input() {
        let input: AgentInput = serde_json::from_str(
            r#"
        {
          "run_id": "run-1",
          "parts": [
            { "type": "text", "text": "look" },
            { "type": "image_url", "url": "data:image/png;base64,AAAA", "detail": "low" }
          ],
          "metadata": { "source": "test" }
        }
        "#,
        )
        .unwrap();

        assert_eq!(input.run_id.as_deref(), Some("run-1"));
        assert_eq!(input.parts.len(), 2);
        assert_eq!(
            input.metadata.get("source"),
            Some(&serde_json::json!("test"))
        );
    }
}
