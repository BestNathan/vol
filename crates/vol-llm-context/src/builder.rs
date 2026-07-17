use vol_llm_core::{Message, MessageRole};

use crate::{
    estimate_tokens, AttentionAnchor, ContextBlock, ContextContributor, ContextError, TokenBudget,
};

/// Output from ContextBuilder — ready-to-send LLM messages.
pub struct ContextOutput {
    pub messages: Vec<Message>,
}

/// Metadata about a context contributor for UI display.
#[derive(Debug, Clone)]
pub struct ContributorInfo {
    pub name: String,
    pub anchor_zone: String,
    pub estimated_tokens: usize,
    pub message_count: usize,
}

/// A message from a contributor snapshot, suitable for frontend display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextMessage {
    pub role: String,
    pub content: String,
}

/// Orchestrator that collects contributors and produces messages.
pub struct ContextBuilder {
    contributors: Vec<Box<dyn ContextContributor>>,
    token_budget: TokenBudget,
}

impl ContextBuilder {
    pub fn new(token_budget: TokenBudget) -> Self {
        Self {
            contributors: Vec::new(),
            token_budget,
        }
    }

    /// Add a contributor to the builder.
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>) {
        self.contributors.push(contributor);
    }

    /// Replace a contributor by name. If no contributor with the given name
    /// exists, adds the new contributor as a fallback.
    pub fn replace_contributor(&mut self, name: &str, contributor: Box<dyn ContextContributor>) {
        if let Some(pos) = self.contributors.iter().position(|c| c.name() == name) {
            if let Some(c) = self.contributors.get_mut(pos) {
                *c = contributor;
            }
        } else {
            self.contributors.push(contributor);
        }
    }

    /// Get a reference to the token budget.
    pub fn token_budget(&self) -> &TokenBudget {
        &self.token_budget
    }

    /// Get contributor names as a list.
    pub fn contributor_names(&self) -> Vec<&str> {
        self.contributors.iter().map(|c| c.name()).collect()
    }

    /// Get info for all contributors (calls contribute() for message_count + anchor_zone).
    pub async fn contributor_infos(&self) -> Result<Vec<ContributorInfo>, ContextError> {
        let mut infos = Vec::new();
        for contributor in &self.contributors {
            let blocks = contributor.contribute().await?;
            let anchor_zone = blocks
                .first()
                .map(|b| match b.anchor {
                    AttentionAnchor::Head(_) => "head",
                    AttentionAnchor::Middle(_) => "middle",
                    AttentionAnchor::Tail(_) => "tail",
                })
                .unwrap_or("unknown")
                .to_string();
            let message_count: usize = blocks.iter().map(|b| b.messages.len()).sum();
            infos.push(ContributorInfo {
                name: contributor.name().to_string(),
                anchor_zone,
                estimated_tokens: contributor.estimate_size(),
                message_count,
            });
        }
        Ok(infos)
    }

    /// Get full message snapshot from a named contributor.
    pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
        for contributor in &self.contributors {
            if contributor.name() == name {
                let blocks = contributor.contribute().await?;
                let messages: Vec<ContextMessage> = blocks
                    .into_iter()
                    .flat_map(|b| b.messages)
                    .map(|msg| {
                        let role = match msg.role {
                            MessageRole::System => "system",
                            MessageRole::User => "user",
                            MessageRole::Assistant => "assistant",
                            MessageRole::Tool => "tool",
                        }
                        .to_string();
                        let content = msg
                            .content
                            .as_ref()
                            .map(|c| c.as_str().to_string())
                            .unwrap_or_default();
                        ContextMessage { role, content }
                    })
                    .collect();
                return Ok(messages);
            }
        }
        Err(ContextError::ContributorError(
            name.to_string(),
            "contributor not found".to_string(),
        ))
    }

    /// Build the context: collect blocks, check budget, compress if needed, produce messages.
    pub async fn build(mut self) -> Result<ContextOutput, ContextError> {
        // Step 1: Collect blocks
        let mut all_blocks = Vec::new();
        for contributor in &self.contributors {
            let blocks = contributor.contribute().await?;
            all_blocks.extend(blocks);
        }

        // Step 2: Estimate total tokens
        let total_tokens: usize = all_blocks
            .iter()
            .flat_map(|b| &b.messages)
            .map(estimate_tokens)
            .sum();

        let budget = self.token_budget.clone().with_used(total_tokens);

        // Step 3: If over budget, compress external contributors
        if budget.is_exceeded() {
            for contributor in &mut self.contributors {
                contributor.compress().await;
            }

            // Step 4: Re-collect blocks
            all_blocks.clear();
            for contributor in &self.contributors {
                let blocks = contributor.contribute().await?;
                all_blocks.extend(blocks);
            }
        }

        // Step 5: Separate by zone (single pass to avoid consuming the vec)
        let mut head_blocks: Vec<ContextBlock> = Vec::new();
        let mut middle_blocks: Vec<ContextBlock> = Vec::new();
        let mut tail_blocks: Vec<ContextBlock> = Vec::new();
        for block in all_blocks {
            match block.anchor {
                AttentionAnchor::Head(_) => head_blocks.push(block),
                AttentionAnchor::Middle(_) => middle_blocks.push(block),
                AttentionAnchor::Tail(_) => tail_blocks.push(block),
            }
        }

        // Sort within zones by position (ascending)
        head_blocks.sort_by_key(|a| a.anchor.position());
        middle_blocks.sort_by_key(|a| a.anchor.position());

        // Step 6: Drop lowest-priority middle blocks if over budget
        let middle_budget = self
            .token_budget
            .total
            .saturating_sub(self.token_budget.head_size)
            .saturating_sub(self.token_budget.tail_size);

        while {
            let current_middle: usize = middle_blocks
                .iter()
                .flat_map(|b| &b.messages)
                .map(estimate_tokens)
                .sum();
            current_middle > middle_budget && !middle_blocks.is_empty()
        } {
            middle_blocks.pop();
        }

        // Step 7: Concatenate
        let mut messages = Vec::new();
        for block in head_blocks {
            messages.extend(block.messages);
        }
        for block in middle_blocks {
            messages.extend(block.messages);
        }
        for block in tail_blocks {
            messages.extend(block.messages);
        }

        Ok(ContextOutput { messages })
    }
}

impl Clone for ContextBuilder {
    fn clone(&self) -> Self {
        Self {
            contributors: self.contributors.iter().map(|c| c.clone_box()).collect(),
            token_budget: self.token_budget.clone(),
        }
    }
}

/// Builder pattern for ContextBuilder.
pub struct ContextBuilderBuilder {
    token_limit: usize,
    head_size: usize,
    tail_size: usize,
    contributors: Vec<Box<dyn ContextContributor>>,
}

impl ContextBuilderBuilder {
    pub fn new(token_limit: usize) -> Self {
        Self {
            token_limit,
            head_size: token_limit / 4,
            tail_size: token_limit / 4,
            contributors: Vec::new(),
        }
    }

    pub fn head_size(mut self, size: usize) -> Self {
        self.head_size = size;
        self
    }

    pub fn tail_size(mut self, size: usize) -> Self {
        self.tail_size = size;
        self
    }

    pub fn add_contributor(mut self, contributor: Box<dyn ContextContributor>) -> Self {
        self.contributors.push(contributor);
        self
    }

    /// Copy contributors from an existing ContextBuilder.
    pub fn add_contributors_from(mut self, builder: &ContextBuilder) -> Self {
        for c in &builder.contributors {
            self.contributors.push(c.clone_box());
        }
        self
    }

    pub fn build(self) -> ContextBuilder {
        let budget = TokenBudget::new(self.token_limit, self.head_size, self.tail_size);
        let mut builder = ContextBuilder::new(budget);
        for contributor in self.contributors {
            builder.add_contributor(contributor);
        }
        builder
    }
}

impl Clone for ContextBuilderBuilder {
    fn clone(&self) -> Self {
        Self {
            token_limit: self.token_limit,
            head_size: self.head_size,
            tail_size: self.tail_size,
            contributors: self.contributors.iter().map(|c| c.clone_box()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    struct SimpleContributor {
        messages: Vec<Message>,
        anchor: AttentionAnchor,
        name: String,
    }

    #[async_trait::async_trait]
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

    #[tokio::test]
    async fn test_builder_basic() {
        let builder = ContextBuilderBuilder::new(10000)
            .head_size(2000)
            .tail_size(1000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("You are helpful")],
                anchor: AttentionAnchor::Head(0),
                name: "role".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("Do the task")],
                anchor: AttentionAnchor::Tail(0),
                name: "task".to_string(),
            }))
            .build();

        let output = builder.build().await.unwrap();
        assert_eq!(output.messages.len(), 2);
        assert_eq!(
            output.messages.get(0).unwrap().role,
            vol_llm_core::message::MessageRole::System
        );
        assert_eq!(
            output.messages.get(1).unwrap().role,
            vol_llm_core::message::MessageRole::User
        );
    }

    #[tokio::test]
    async fn test_builder_zone_ordering() {
        let builder = ContextBuilderBuilder::new(10000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("Tail message")],
                anchor: AttentionAnchor::Tail(0),
                name: "tail".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("Head first")],
                anchor: AttentionAnchor::Head(0),
                name: "head1".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("Middle data")],
                anchor: AttentionAnchor::Middle(5),
                name: "middle".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("Head second")],
                anchor: AttentionAnchor::Head(10),
                name: "head2".to_string(),
            }))
            .build();

        let output = builder.build().await.unwrap();
        assert_eq!(
            output
                .messages
                .get(0)
                .unwrap()
                .content
                .as_ref()
                .unwrap()
                .as_str(),
            "Head first"
        );
        assert_eq!(
            output
                .messages
                .get(1)
                .unwrap()
                .content
                .as_ref()
                .unwrap()
                .as_str(),
            "Head second"
        );
        assert_eq!(
            output
                .messages
                .get(2)
                .unwrap()
                .content
                .as_ref()
                .unwrap()
                .as_str(),
            "Middle data"
        );
        assert_eq!(
            output
                .messages
                .get(3)
                .unwrap()
                .content
                .as_ref()
                .unwrap()
                .as_str(),
            "Tail message"
        );
    }
}
