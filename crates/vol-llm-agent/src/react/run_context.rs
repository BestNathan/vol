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
use vol_llm_tool::ToolRegistry;
use super::AgentConfig;

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

    /// Get the current iteration number
    pub fn current_iteration(&self) -> u32 {
        self.iteration.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Increment the iteration counter
    pub fn next_iteration(&self) {
        self.iteration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Add a message to the messages list
    pub async fn add_message(&self, message: Message) {
        self.messages.write().await.push(message);
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
    use crate::session::{InMemorySessionStore, InMemoryMessageStore};

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
}
