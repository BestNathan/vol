//! RunContext - Unified run state management for ReAct Agent.
//!
//! Encapsulates all state and resources for a single `run()` invocation.

use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use vol_llm_core::Message;
use vol_llm_core::ToolCall;
use crate::session::Session;
use crate::session::SessionMessage;
use vol_llm_tool::ToolRegistry;
use super::AgentConfig;
use super::response::AgentError;

/// RunContext encapsulates all state and resources for a single run() invocation.
///
/// This replaces the old PluginContext with a more comprehensive context that includes:
/// - Mutable state (messages, tool calls, iteration count)
/// - Resource references (session, tools, config)
/// - Thread-safe access via Arc/RwLock for async operations
pub struct RunContext {
    // Immutable fields
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,

    // Mutable state (internal mutability via AtomicU32 and Arc<RwLock>)
    pub iteration: AtomicU32,
    pub messages: Arc<RwLock<Vec<Message>>>,
    pub all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,

    // Resource references
    pub session: Arc<Session>,
    pub tools: Arc<ToolRegistry>,
    pub config: AgentConfig,
}

impl RunContext {
    /// Create a new RunContext
    pub fn new(
        run_id: String,
        user_input: String,
        session_id: String,
        session: Arc<Session>,
        tools: Arc<ToolRegistry>,
        config: AgentConfig,
    ) -> Self {
        Self {
            run_id,
            user_input,
            session_id,
            iteration: AtomicU32::new(0),
            messages: Arc::new(RwLock::new(Vec::new())),
            all_tool_calls: Arc::new(RwLock::new(Vec::new())),
            current_tool_calls: Arc::new(RwLock::new(Vec::new())),
            data: Arc::new(RwLock::new(HashMap::new())),
            session,
            tools,
            config,
        }
    }

    /// Initialize messages array - must be called once before the loop
    ///
    /// This method:
    /// 1. Builds System message from `config.prompt_context.build_system()`
    /// 2. Gets historical messages from session (only once, limited by `max_history_messages`)
    /// 3. Adds user input
    /// 4. Writes all to `self.messages`
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(AgentError)` if session access fails
    pub async fn init_messages(&self) -> Result<(), crate::AgentError> {
        let mut messages = Vec::new();

        // 1. System message from prompt_context
        let system_content = self.config.prompt_context.build_system();
        messages.push(Message::system(system_content));

        // 2. Historical messages from session (only once)
        let history = self.session
            .get_messages(self.config.max_history_messages)
            .await
            .unwrap_or_default();

        for session_msg in history {
            messages.push(session_msg.message);
        }

        // 3. User input
        messages.push(Message::user(self.user_input.clone()));

        // Write to shared state
        *self.messages.write().await = messages;

        Ok(())
    }

    /// Get the current iteration number
    pub fn current_iteration(&self) -> u32 {
        self.iteration.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Increment the iteration counter
    pub fn next_iteration(&self) {
        self.iteration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Add a message to the messages list and sync to session
    pub async fn add_message(&self, message: Message) -> Result<(), crate::AgentError> {
        // 1. Add to runtime messages array
        self.messages.write().await.push(message.clone());

        // 2. Persist to session
        let session_msg = SessionMessage::new(self.session_id.clone(), message);
        self.session.add_message(session_msg).await.map_err(|e| crate::AgentError::SessionError {
            source: e,
        })?;

        Ok(())
    }

    /// Get a clone of all messages
    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }

    /// Add a tool call to both current and all_tool_calls lists
    pub async fn add_tool_call(&self, tool_call: ToolCall) {
        self.current_tool_calls.write().await.push(tool_call.clone());
        self.all_tool_calls.write().await.push(tool_call);
    }

    /// Clear the current tool calls list (called at the start of each iteration)
    pub async fn clear_current_tool_calls(&self) {
        self.current_tool_calls.write().await.clear();
    }

    /// Get a clone of current tool calls
    pub async fn get_current_tool_calls(&self) -> Vec<ToolCall> {
        self.current_tool_calls.read().await.clone()
    }

    /// Get a clone of all tool calls
    pub async fn get_all_tool_calls(&self) -> Vec<ToolCall> {
        self.all_tool_calls.read().await.clone()
    }

    /// Get a value from the data store
    pub async fn get<T: for<'de> serde::Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data.read().await.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set a value in the data store
    pub async fn set<T: serde::Serialize>(&self, key: &str, value: T) -> Result<(), serde_json::Error> {
        self.data.write().await.insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }
}

impl Clone for RunContext {
    fn clone(&self) -> Self {
        Self {
            run_id: self.run_id.clone(),
            user_input: self.user_input.clone(),
            session_id: self.session_id.clone(),
            iteration: AtomicU32::new(self.current_iteration()),
            messages: self.messages.clone(),
            all_tool_calls: self.all_tool_calls.clone(),
            current_tool_calls: self.current_tool_calls.clone(),
            data: self.data.clone(),
            session: self.session.clone(),
            tools: self.tools.clone(),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{InMemorySessionStore, InMemoryMessageStore, SessionMessage};
    use vol_llm_core::MessageRole;

    fn create_test_context() -> RunContext {
        RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        )
    }

    #[tokio::test]
    async fn test_run_context_new() {
        let ctx = create_test_context();
        assert_eq!(ctx.run_id, "test-run");
        assert_eq!(ctx.user_input, "test input");
        assert_eq!(ctx.session_id, "session-1");
        assert_eq!(ctx.current_iteration(), 0);
    }

    #[tokio::test]
    async fn test_run_context_iteration() {
        let ctx = create_test_context();
        assert_eq!(ctx.current_iteration(), 0);
        ctx.next_iteration();
        assert_eq!(ctx.current_iteration(), 1);
        ctx.next_iteration();
        assert_eq!(ctx.current_iteration(), 2);
    }

    #[tokio::test]
    async fn test_run_context_messages() {
        let ctx = create_test_context();
        ctx.add_message(Message::system("test".to_string())).await;
        let msgs = ctx.get_messages().await;
        assert_eq!(msgs.len(), 1);
    }

    #[tokio::test]
    async fn test_run_context_tool_calls() {
        let ctx = create_test_context();
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
            r#type: "function".to_string(),
        };
        ctx.add_tool_call(tool_call.clone()).await;

        let current = ctx.get_current_tool_calls().await;
        assert_eq!(current.len(), 1);

        let all = ctx.get_all_tool_calls().await;
        assert_eq!(all.len(), 1);

        ctx.clear_current_tool_calls().await;
        assert_eq!(ctx.get_current_tool_calls().await.len(), 0);
        assert_eq!(ctx.get_all_tool_calls().await.len(), 1); // all_tool_calls still has it
    }

    #[tokio::test]
    async fn test_run_context_data() {
        let ctx = create_test_context();
        ctx.set("key1", "value1").await.unwrap();
        let val: Option<String> = ctx.get("key1").await;
        assert_eq!(val, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_init_messages_system_message() {
        use crate::prompt_context::{PromptTemplate, PromptContext};

        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            prompt_context,
            ..Default::default()
        };

        let ctx = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            config,
        );

        ctx.init_messages().await.unwrap();
        let messages = ctx.get_messages().await;

        // First message should be system message
        assert!(messages.len() >= 1);
        assert_eq!(messages[0].role, MessageRole::System);
        assert!(messages[0].content.as_ref().unwrap().as_str().contains("You are a helpful assistant."));
    }

    #[tokio::test]
    async fn test_init_messages_history() {
        use crate::prompt_context::{PromptTemplate, PromptContext};

        let template = PromptTemplate::new("test", "System");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            prompt_context,
            max_history_messages: 10,
            ..Default::default()
        };

        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());
        let session = Arc::new(Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        ));

        // Add a historical message to session
        let history_msg = SessionMessage::new(
            "session-1".to_string(),
            Message::user("Previous conversation"),
        );
        session.add_message(history_msg).await.unwrap();

        let ctx = RunContext::new(
            "test-run".to_string(),
            "new input".to_string(),
            "session-1".to_string(),
            session.clone(),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            config,
        );

        ctx.init_messages().await.unwrap();
        let messages = ctx.get_messages().await;

        // Should have: system + history + user input = 3 messages
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].role, MessageRole::User);
        assert!(messages[1].content.as_ref().unwrap().as_str().contains("Previous conversation"));
    }

    #[tokio::test]
    async fn test_init_messages_user_input() {
        use crate::prompt_context::{PromptTemplate, PromptContext};

        let template = PromptTemplate::new("test", "System");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            prompt_context,
            ..Default::default()
        };

        let ctx = RunContext::new(
            "test-run".to_string(),
            "analyze market volatility".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            config,
        );

        ctx.init_messages().await.unwrap();
        let messages = ctx.get_messages().await;

        // Last message should be user input
        assert!(messages.len() >= 1);
        let last_msg = messages.last().unwrap();
        assert_eq!(last_msg.role, MessageRole::User);
        assert!(last_msg.content.as_ref().unwrap().as_str().contains("analyze market volatility"));
    }

    #[tokio::test]
    async fn test_init_messages_only_once() {
        use crate::prompt_context::{PromptTemplate, PromptContext};

        let template = PromptTemplate::new("test", "System");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            prompt_context,
            ..Default::default()
        };

        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());
        let session = Arc::new(Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        ));

        // Add a historical message
        let history_msg = SessionMessage::new(
            "session-1".to_string(),
            Message::user("History"),
        );
        session.add_message(history_msg).await.unwrap();

        let ctx = RunContext::new(
            "test-run".to_string(),
            "input".to_string(),
            "session-1".to_string(),
            session.clone(),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            config,
        );

        // Call init_messages multiple times
        ctx.init_messages().await.unwrap();
        let messages_after_first = ctx.get_messages().await.len();

        ctx.init_messages().await.unwrap();
        let messages_after_second = ctx.get_messages().await.len();

        // Multiple calls should NOT duplicate - second call overwrites
        // This is the expected behavior: init_messages replaces, not appends
        assert_eq!(messages_after_first, messages_after_second);

        // Verify we have: system + history + user = 3 messages
        assert_eq!(messages_after_second, 3);
    }
}
