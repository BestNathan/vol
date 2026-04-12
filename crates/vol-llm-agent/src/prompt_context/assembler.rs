//! Message assembler for building LLM messages from prompt context.

use vol_llm_core::Message;

use super::PromptContext;
use crate::rag::Document;

/// Message assembler for building LLM messages from prompt context.
///
/// Provides methods to combine `PromptContext` with user input,
/// optionally including conversation history or RAG context.
pub struct MessageAssembler;

impl MessageAssembler {
    /// Assemble basic messages (System + User).
    ///
    /// Creates a minimal message sequence with just the system prompt
    /// and the user's input.
    ///
    /// # Arguments
    /// * `ctx` - The prompt context containing system prompt configuration
    /// * `user_input` - The user's query/question
    ///
    /// # Returns
    /// A vector of messages starting with System followed by User
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{MessageAssembler, PromptContext, PromptTemplate};
    ///
    /// let template = PromptTemplate::new("test", "You are a helpful assistant.");
    /// let ctx = PromptContext::new(template);
    ///
    /// let messages = MessageAssembler::assemble(&ctx, "Hello");
    /// assert_eq!(messages.len(), 2);
    /// ```
    pub fn assemble(ctx: &PromptContext, user_input: &str) -> Vec<Message> {
        vec![
            Message::system(ctx.build_system()),
            Message::user(user_input.to_string()),
        ]
    }

    /// Assemble messages with conversation history.
    ///
    /// Creates a message sequence including system prompt, historical
    /// messages, and the current user input.
    ///
    /// # Arguments
    /// * `ctx` - The prompt context containing system prompt configuration
    /// * `user_input` - The user's current query/question
    /// * `history` - Previous conversation messages
    ///
    /// # Returns
    /// A vector of messages: System, history messages, then current User message
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{MessageAssembler, PromptContext, PromptTemplate};
    /// use vol_llm_core::Message;
    ///
    /// let template = PromptTemplate::new("test", "You are a helpful assistant.");
    /// let ctx = PromptContext::new(template);
    ///
    /// let history = vec![
    ///     Message::user("What is AI?"),
    ///     Message::assistant("AI stands for Artificial Intelligence."),
    /// ];
    ///
    /// let messages = MessageAssembler::assemble_with_history(&ctx, "How does it work?", &history);
    /// assert_eq!(messages.len(), 4); // System + 2 history + current user
    /// ```
    pub fn assemble_with_history(
        ctx: &PromptContext,
        user_input: &str,
        history: &[Message],
    ) -> Vec<Message> {
        let mut messages = Vec::new();

        // System message (only once)
        messages.push(Message::system(ctx.build_system()));

        // Historical messages (already limited by max_history_messages)
        messages.extend_from_slice(history);

        // Current User message (with optional RAG context)
        let user_msg = ctx.build_user(user_input, None);
        messages.push(Message::user(user_msg));

        messages
    }

    /// Assemble messages with RAG context.
    ///
    /// Creates a message sequence with system prompt and user input
    /// augmented with retrieved documents as reference context.
    ///
    /// # Arguments
    /// * `ctx` - The prompt context containing system prompt configuration
    /// * `user_input` - The user's query/question
    /// * `rag_docs` - Retrieved documents to include as context
    ///
    /// # Returns
    /// A vector of messages: System and User (with RAG context embedded)
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{MessageAssembler, PromptContext, PromptTemplate};
    /// use vol_llm_agent::rag::Document;
    ///
    /// let template = PromptTemplate::new("test", "You are a helpful assistant.");
    /// let ctx = PromptContext::new(template);
    ///
    /// let docs = vec![
    ///     Document::new("AI stands for Artificial Intelligence.".to_string()),
    /// ];
    ///
    /// let messages = MessageAssembler::assemble_with_rag(&ctx, "What is AI?", &docs);
    /// assert_eq!(messages.len(), 2);
    /// ```
    pub fn assemble_with_rag(
        ctx: &PromptContext,
        user_input: &str,
        rag_docs: &[Document],
    ) -> Vec<Message> {
        let rag_context = rag_docs
            .iter()
            .map(|d| d.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let user_msg = ctx.build_user(user_input, Some(&rag_context));

        vec![Message::system(ctx.build_system()), Message::user(user_msg)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt_context::{FragmentType, PromptFragment, PromptTemplate};

    #[test]
    fn test_assemble_basic() {
        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let ctx = PromptContext::new(template);

        let messages = MessageAssembler::assemble(&ctx, "Hello, how are you?");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, vol_llm_core::MessageRole::System);
        assert_eq!(messages[1].role, vol_llm_core::MessageRole::User);
        assert!(messages[0]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("helpful assistant"));
        assert!(messages[1]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("Hello, how are you?"));
    }

    #[test]
    fn test_assemble_with_custom_context() {
        let template = PromptTemplate::new("test", "Role: {role}");
        let ctx = PromptContext::new(template).with_fragment(PromptFragment::new(
            "role",
            "You are a financial analyst.",
            FragmentType::Role,
        ));

        let messages = MessageAssembler::assemble(&ctx, "What is the market outlook?");

        assert_eq!(messages.len(), 2);
        assert!(messages[0]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("financial analyst"));
    }

    #[test]
    fn test_assemble_with_history() {
        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let ctx = PromptContext::new(template);

        let history = vec![
            Message::user("What is AI?"),
            Message::assistant("AI stands for Artificial Intelligence."),
        ];

        let messages = MessageAssembler::assemble_with_history(&ctx, "How does it work?", &history);

        assert_eq!(messages.len(), 4); // System + 2 history + current user
        assert_eq!(messages[0].role, vol_llm_core::MessageRole::System);
        assert_eq!(messages[1].role, vol_llm_core::MessageRole::User);
        assert_eq!(messages[2].role, vol_llm_core::MessageRole::Assistant);
        assert_eq!(messages[3].role, vol_llm_core::MessageRole::User);
    }

    #[test]
    fn test_assemble_with_history_empty() {
        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let ctx = PromptContext::new(template);

        let history: Vec<Message> = vec![];

        let messages = MessageAssembler::assemble_with_history(&ctx, "Hello", &history);

        assert_eq!(messages.len(), 2); // System + user only
    }

    #[test]
    fn test_assemble_with_rag() {
        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let ctx = PromptContext::new(template);

        let docs = vec![
            Document::new("AI stands for Artificial Intelligence.".to_string()),
            Document::new("Machine learning is a subset of AI.".to_string()),
        ];

        let messages = MessageAssembler::assemble_with_rag(&ctx, "What is AI?", &docs);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, vol_llm_core::MessageRole::System);
        assert_eq!(messages[1].role, vol_llm_core::MessageRole::User);

        // Check RAG context is included
        let user_content = messages[1].content.as_ref().unwrap().as_str();
        assert!(user_content.contains("参考资料:"));
        assert!(user_content.contains("AI stands for Artificial Intelligence."));
        assert!(user_content.contains("---"));
        assert!(user_content.contains("Machine learning is a subset of AI."));
        assert!(user_content.contains("问题：What is AI?"));
    }

    #[test]
    fn test_assemble_with_rag_empty_docs() {
        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let ctx = PromptContext::new(template);

        let docs: Vec<Document> = vec![];

        let messages = MessageAssembler::assemble_with_rag(&ctx, "What is AI?", &docs);

        assert_eq!(messages.len(), 2);
        // With empty docs, RAG context will be empty string
        let user_content = messages[1].content.as_ref().unwrap().as_str();
        assert!(user_content.contains("问题：What is AI?"));
    }

    #[test]
    fn test_assemble_with_rag_single_doc() {
        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let ctx = PromptContext::new(template);

        let doc = Document::new("The answer is 42.".to_string());

        let messages = MessageAssembler::assemble_with_rag(&ctx, "What is the answer?", &[doc]);

        assert_eq!(messages.len(), 2);
        let user_content = messages[1].content.as_ref().unwrap().as_str();
        assert!(user_content.contains("参考资料:"));
        assert!(user_content.contains("The answer is 42."));
        assert!(user_content.contains("问题：What is the answer?"));
    }
}
