use async_trait::async_trait;
use vol_llm_core::Message;

use crate::{estimate_tokens, AttentionAnchor, ContextBlock, ContextContributor, ContextError};

/// A simple contributor for ad-hoc context blocks.
pub struct SimpleContributor {
    messages: Vec<Message>,
    anchor: AttentionAnchor,
    name: String,
}

impl SimpleContributor {
    pub fn new(name: impl Into<String>, messages: Vec<Message>, anchor: AttentionAnchor) -> Self {
        Self {
            messages,
            anchor,
            name: name.into(),
        }
    }

    /// Create a system prompt contributor (Head zone, position 0).
    pub fn system(content: String) -> Self {
        Self::new(
            "system",
            vec![Message::system(content)],
            AttentionAnchor::Head(0),
        )
    }
}

#[async_trait]
impl ContextContributor for SimpleContributor {
    fn name(&self) -> &str {
        &self.name
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        Ok(vec![ContextBlock::new(
            self.messages.clone(),
            self.anchor.clone(),
        )])
    }

    async fn compress(&mut self) {
        // No-op
    }

    fn estimate_size(&self) -> usize {
        self.messages.iter().map(estimate_tokens).sum()
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(SimpleContributor {
            messages: self.messages.clone(),
            anchor: self.anchor.clone(),
            name: self.name.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SimpleContributor {
        SimpleContributor::new(
            "sample",
            vec![Message::user("hello")],
            AttentionAnchor::Middle(2),
        )
    }

    #[tokio::test]
    async fn test_simple_contributor_new() {
        let c = sample();
        assert_eq!(c.name(), "sample");
        assert_eq!(c.estimate_size(), estimate_tokens(&Message::user("hello")));

        let blocks = c.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].anchor, AttentionAnchor::Middle(2));
        assert_eq!(blocks[0].messages.len(), 1);
        assert_eq!(
            blocks[0].messages[0].content.as_ref().unwrap().as_str(),
            "hello"
        );
    }

    #[tokio::test]
    async fn test_simple_contributor_system() {
        let c = SimpleContributor::system("You are a helpful assistant".to_string());
        assert_eq!(c.name(), "system");

        let blocks = c.contribute().await.unwrap();
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Head(0)));
        assert_eq!(
            blocks[0].messages[0].role,
            vol_llm_core::message::MessageRole::System
        );
        assert_eq!(
            blocks[0].messages[0].content.as_ref().unwrap().as_str(),
            "You are a helpful assistant"
        );
    }

    #[tokio::test]
    async fn test_simple_contributor_compress_noop() {
        let mut c = sample();
        c.compress().await;
        let blocks = c.contribute().await.unwrap();
        assert_eq!(
            blocks[0].messages[0].content.as_ref().unwrap().as_str(),
            "hello"
        );
    }

    #[tokio::test]
    async fn test_simple_contributor_estimate_size_sums_messages() {
        let c = SimpleContributor::new(
            "multi",
            vec![Message::user("first"), Message::system("second")],
            AttentionAnchor::Tail(1),
        );
        let expected =
            estimate_tokens(&Message::user("first")) + estimate_tokens(&Message::system("second"));
        assert_eq!(c.estimate_size(), expected);
    }

    #[tokio::test]
    async fn test_simple_contributor_clone_box() {
        let c = sample();
        let cloned = c.clone_box();
        assert_eq!(cloned.name(), "sample");

        let blocks = cloned.contribute().await.unwrap();
        assert_eq!(
            blocks[0].messages[0].content.as_ref().unwrap().as_str(),
            "hello"
        );
    }
}
