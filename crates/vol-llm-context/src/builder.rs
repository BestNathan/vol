use vol_llm_core::{Message, MessageContent, MessageRole};

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
                        // display_text renders multipart image parts as `[image]`
                        // (as_str returns "" for multipart, losing the content).
                        let content = msg
                            .content
                            .as_ref()
                            .map(MessageContent::display_text)
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
    use vol_llm_core::{ContentPart, ImageUrl, Message, MessageContent};

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

    fn multipart_user_message() -> Message {
        Message {
            role: MessageRole::User,
            content: Some(MessageContent::MultiPart(vec![
                ContentPart::Text {
                    text: "look at this".to_string(),
                },
                ContentPart::Image {
                    image_url: ImageUrl {
                        url: "data:image/png;base64,AAAA".to_string(),
                        detail: None,
                    },
                },
            ])),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            thinking: None,
        }
    }

    #[tokio::test]
    async fn test_snapshot_by_name_multipart_shows_image_marker() {
        let builder = ContextBuilderBuilder::new(10000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![multipart_user_message()],
                anchor: AttentionAnchor::Head(0),
                name: "snap".to_string(),
            }))
            .build();

        let snapshot = builder.snapshot_by_name("snap").await.unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].role, "user");
        // Text part is kept and the image part renders as `[image]`.
        assert_eq!(snapshot[0].content, "look at this\n[image]");
    }

    #[tokio::test]
    async fn test_snapshot_by_name_text_message_unchanged() {
        let builder = ContextBuilderBuilder::new(10000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("plain text")],
                anchor: AttentionAnchor::Head(0),
                name: "snap".to_string(),
            }))
            .build();

        let snapshot = builder.snapshot_by_name("snap").await.unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].role, "system");
        assert_eq!(snapshot[0].content, "plain text");
    }

    #[tokio::test]
    async fn test_replace_contributor_replaces_in_place() {
        let mut builder = ContextBuilderBuilder::new(10000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("old prompt")],
                anchor: AttentionAnchor::Head(0),
                name: "role".to_string(),
            }))
            .build();

        builder.replace_contributor(
            "role",
            Box::new(SimpleContributor {
                messages: vec![Message::system("new prompt")],
                anchor: AttentionAnchor::Head(0),
                name: "role".to_string(),
            }),
        );

        // Replacement keeps the contributor's position/name.
        assert_eq!(builder.contributor_names(), vec!["role"]);
        let snapshot = builder.snapshot_by_name("role").await.unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].content, "new prompt");
    }

    #[tokio::test]
    async fn test_replace_contributor_missing_name_appends() {
        let mut builder = ContextBuilderBuilder::new(10000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("role")],
                anchor: AttentionAnchor::Head(0),
                name: "role".to_string(),
            }))
            .build();

        // Unknown name → the contributor is added as a fallback.
        builder.replace_contributor(
            "missing",
            Box::new(SimpleContributor {
                messages: vec![Message::user("extra context")],
                anchor: AttentionAnchor::Tail(0),
                name: "extra".to_string(),
            }),
        );

        assert_eq!(builder.contributor_names(), vec!["role", "extra"]);
    }

    #[test]
    fn test_token_budget_getter() {
        let builder = ContextBuilderBuilder::new(10_000)
            .head_size(3_000)
            .tail_size(2_000)
            .build();
        let budget = builder.token_budget();
        assert_eq!(budget.total, 10_000);
        assert_eq!(budget.head_size, 3_000);
        assert_eq!(budget.tail_size, 2_000);
        assert_eq!(budget.used, 0);
    }

    #[test]
    fn test_contributor_names_lists_in_order() {
        let builder = ContextBuilderBuilder::new(10_000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("a")],
                anchor: AttentionAnchor::Head(0),
                name: "alpha".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("b")],
                anchor: AttentionAnchor::Tail(0),
                name: "beta".to_string(),
            }))
            .build();
        assert_eq!(builder.contributor_names(), vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn test_contributor_infos_zone_labels_and_counts() {
        let builder = ContextBuilderBuilder::new(10_000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("head content")],
                anchor: AttentionAnchor::Head(0),
                name: "heady".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("m1"), Message::user("m2")],
                anchor: AttentionAnchor::Middle(5),
                name: "middy".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("tail content")],
                anchor: AttentionAnchor::Tail(3),
                name: "taily".to_string(),
            }))
            .add_contributor(Box::new(EmptyContributor))
            .build();

        let infos = builder.contributor_infos().await.unwrap();
        assert_eq!(infos.len(), 4);

        assert_eq!(infos[0].name, "heady");
        assert_eq!(infos[0].anchor_zone, "head");
        assert_eq!(infos[0].message_count, 1);
        assert!(infos[0].estimated_tokens > 0);

        assert_eq!(infos[1].name, "middy");
        assert_eq!(infos[1].anchor_zone, "middle");
        assert_eq!(infos[1].message_count, 2);
        assert!(infos[1].estimated_tokens > 0);

        assert_eq!(infos[2].name, "taily");
        assert_eq!(infos[2].anchor_zone, "tail");
        assert_eq!(infos[2].message_count, 1);

        // A contributor that produces no blocks reports an unknown zone.
        assert_eq!(infos[3].anchor_zone, "unknown");
        assert_eq!(infos[3].message_count, 0);
        assert_eq!(infos[3].estimated_tokens, 0);
    }

    #[tokio::test]
    async fn test_snapshot_by_name_all_roles_and_none_content() {
        let builder = ContextBuilderBuilder::new(10_000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![
                    Message::system("sys"),
                    Message::assistant("assistant says hi"),
                    Message::tool("tool result", "call_1".to_string()),
                    Message {
                        role: vol_llm_core::message::MessageRole::User,
                        content: None,
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                        thinking: None,
                    },
                ],
                anchor: AttentionAnchor::Head(0),
                name: "roles".to_string(),
            }))
            .build();

        let snapshot = builder.snapshot_by_name("roles").await.unwrap();
        assert_eq!(snapshot.len(), 4);
        assert_eq!(snapshot[0].role, "system");
        assert_eq!(snapshot[0].content, "sys");
        assert_eq!(snapshot[1].role, "assistant");
        assert_eq!(snapshot[1].content, "assistant says hi");
        assert_eq!(snapshot[2].role, "tool");
        assert_eq!(snapshot[2].content, "tool result");
        // Messages without content render as an empty string.
        assert_eq!(snapshot[3].role, "user");
        assert_eq!(snapshot[3].content, "");
    }

    #[tokio::test]
    async fn test_snapshot_by_name_not_found() {
        let builder = ContextBuilderBuilder::new(10_000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("a")],
                anchor: AttentionAnchor::Head(0),
                name: "alpha".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("b")],
                anchor: AttentionAnchor::Tail(0),
                name: "beta".to_string(),
            }))
            .build();

        let err = builder.snapshot_by_name("ghost").await.unwrap_err();
        match err {
            ContextError::ContributorError(name, message) => {
                assert_eq!(name, "ghost");
                assert_eq!(message, "contributor not found");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    /// Contributor that actually shrinks its content when compressed.
    struct CompressingContributor {
        content: String,
    }

    #[async_trait::async_trait]
    impl ContextContributor for CompressingContributor {
        fn name(&self) -> &str {
            "compressor"
        }

        async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
            Ok(vec![ContextBlock::new(
                vec![Message::user(self.content.clone())],
                AttentionAnchor::Head(0),
            )])
        }

        async fn compress(&mut self) {
            self.content = self.content.chars().take(5).collect();
        }

        fn estimate_size(&self) -> usize {
            self.content.len() / 4
        }

        fn clone_box(&self) -> Box<dyn ContextContributor> {
            Box::new(CompressingContributor {
                content: self.content.clone(),
            })
        }
    }

    /// Contributor that never produces blocks (e.g. an empty file set).
    struct EmptyContributor;

    #[async_trait::async_trait]
    impl ContextContributor for EmptyContributor {
        fn name(&self) -> &str {
            "empty"
        }

        async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
            Ok(Vec::new())
        }

        async fn compress(&mut self) {}

        fn estimate_size(&self) -> usize {
            0
        }

        fn clone_box(&self) -> Box<dyn ContextContributor> {
            Box::new(EmptyContributor)
        }
    }

    /// Contributor whose contribute() always fails.
    struct FailingContributor;

    #[async_trait::async_trait]
    impl ContextContributor for FailingContributor {
        fn name(&self) -> &str {
            "failing"
        }

        async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
            Err(ContextError::BudgetExceeded(999))
        }

        async fn compress(&mut self) {}

        fn estimate_size(&self) -> usize {
            0
        }

        fn clone_box(&self) -> Box<dyn ContextContributor> {
            Box::new(FailingContributor)
        }
    }

    #[tokio::test]
    async fn test_build_compresses_contributors_when_over_budget() {
        let builder = ContextBuilderBuilder::new(100)
            .add_contributor(Box::new(CompressingContributor {
                content: "x".repeat(10_000),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("y".repeat(10_000))],
                anchor: AttentionAnchor::Tail(0),
                name: "tail".to_string(),
            }))
            .build();

        let output = builder.build().await.unwrap();
        // The head block was compressed to its first 5 chars; the no-op
        // SimpleContributor tail message is unchanged.
        assert_eq!(output.messages.len(), 2);
        assert_eq!(
            output.messages[0].content.as_ref().unwrap().as_str(),
            "xxxxx"
        );
        assert_eq!(
            output.messages[1].content.as_ref().unwrap().as_str(),
            "y".repeat(10_000)
        );
    }

    #[tokio::test]
    async fn test_build_drops_lowest_priority_middle_when_over_budget() {
        let builder = ContextBuilderBuilder::new(1_000)
            .head_size(100)
            .tail_size(100)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("head")],
                anchor: AttentionAnchor::Head(0),
                name: "head".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("A".repeat(900))],
                anchor: AttentionAnchor::Middle(1),
                name: "m1".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("B".repeat(900))],
                anchor: AttentionAnchor::Middle(2),
                name: "m2".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("C".repeat(900))],
                anchor: AttentionAnchor::Middle(3),
                name: "m3".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("D".repeat(900))],
                anchor: AttentionAnchor::Middle(4),
                name: "m4".to_string(),
            }))
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::user("tail")],
                anchor: AttentionAnchor::Tail(0),
                name: "tail".to_string(),
            }))
            .build();

        let output = builder.build().await.unwrap();
        // middle_budget = 1000 - 100 - 100 = 800; each middle block ≈ 230
        // tokens, so 4 blocks ≈ 930 > 800 → the last (highest position,
        // lowest priority) middle block is dropped.
        assert_eq!(output.messages.len(), 5);
        assert_eq!(
            output.messages[0].content.as_ref().unwrap().as_str(),
            "head"
        );
        assert_eq!(
            output.messages[4].content.as_ref().unwrap().as_str(),
            "tail"
        );
        assert_eq!(
            output.messages[1].content.as_ref().unwrap().as_str(),
            "A".repeat(900)
        );
        assert_eq!(
            output.messages[2].content.as_ref().unwrap().as_str(),
            "B".repeat(900)
        );
        assert_eq!(
            output.messages[3].content.as_ref().unwrap().as_str(),
            "C".repeat(900)
        );
    }

    #[tokio::test]
    async fn test_build_propagates_contributor_error() {
        let builder = ContextBuilderBuilder::new(1_000)
            .add_contributor(Box::new(FailingContributor))
            .build();

        let err = match builder.build().await {
            Ok(_) => panic!("build() should have failed"),
            Err(e) => e,
        };
        assert!(matches!(err, ContextError::BudgetExceeded(999)));
    }

    #[tokio::test]
    async fn test_contributor_infos_propagates_error() {
        let builder = ContextBuilderBuilder::new(1_000)
            .add_contributor(Box::new(FailingContributor))
            .build();

        let err = builder.contributor_infos().await.unwrap_err();
        assert!(matches!(err, ContextError::BudgetExceeded(999)));
    }

    #[tokio::test]
    async fn test_snapshot_by_name_propagates_contributor_error() {
        let builder = ContextBuilderBuilder::new(1_000)
            .add_contributor(Box::new(FailingContributor))
            .build();

        let err = builder.snapshot_by_name("failing").await.unwrap_err();
        assert!(matches!(err, ContextError::BudgetExceeded(999)));
    }

    #[tokio::test]
    async fn test_context_builder_clone_builds_independently() {
        let builder = ContextBuilderBuilder::new(10_000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("clone me")],
                anchor: AttentionAnchor::Head(0),
                name: "src".to_string(),
            }))
            .build();

        let cloned = builder.clone();
        let original_output = builder.build().await.unwrap();
        let cloned_output = cloned.build().await.unwrap();
        assert_eq!(original_output.messages.len(), 1);
        assert_eq!(cloned_output.messages.len(), 1);
        assert_eq!(
            cloned_output.messages[0].content.as_ref().unwrap().as_str(),
            "clone me"
        );
    }

    #[tokio::test]
    async fn test_builder_builder_clone_and_copy_contributors() {
        let source = ContextBuilderBuilder::new(10_000)
            .add_contributor(Box::new(SimpleContributor {
                messages: vec![Message::system("copied")],
                anchor: AttentionAnchor::Head(0),
                name: "copied".to_string(),
            }))
            .build();

        let builder_builder = ContextBuilderBuilder::new(10_000).add_contributors_from(&source);
        let cloned_builder_builder = builder_builder.clone();
        let builder = cloned_builder_builder.build();

        assert_eq!(builder.contributor_names(), vec!["copied"]);
        let output = builder.build().await.unwrap();
        assert_eq!(output.messages.len(), 1);
        assert_eq!(
            output.messages[0].content.as_ref().unwrap().as_str(),
            "copied"
        );
    }
}
