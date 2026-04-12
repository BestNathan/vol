//! RunContext - Unified run state management for ReAct Agent.
//!
//! Encapsulates all state and resources for a single `run()` invocation.

use super::plugin::PluginDecision;
use super::state::{ReasoningStep, ToolCallRecord};
use super::stream::AgentStreamEvent;
use super::AgentConfig;
use crate::session::Session;
use crate::session::SessionMessage;
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use vol_llm_core::Message;
use vol_llm_core::ToolCall;
use vol_llm_tool::ToolRegistry;
use vol_tracing::TracedEvent;

/// PluginContext - Read-only context for plugin hooks.
///
/// This struct contains all the data plugins need for `intercept()` and `listen()`
/// hooks, but EXCLUDES the broadcast channel senders (`event_tx`, `plugin_event_tx`).
///
/// # Why This Exists
///
/// The `RunContext::Clone` implementation clones sender references, which increments
/// the broadcast channel sender count. This prevents the channel from ever closing
/// (sender count never reaches 0), causing listener tasks to hang.
///
/// `PluginContext` provides a cloneable context without senders, allowing:
/// 1. Plugins to access all necessary read-only data
/// 2. Sender count to remain accurate (only agent + interceptor hold senders)
/// 3. Graceful shutdown when agent drops its sender
///
/// # Usage
///
/// - `spawn_listener_task()` should create a `PluginContext` and clone it for each plugin
/// - `run_interceptor_loop()` should use `PluginContext` for plugin.intercept() calls
/// - Plugins receive `&PluginContext` instead of `&RunContext`
#[derive(Clone)]
pub struct PluginContext {
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    pub session: Arc<Session>,
    pub tools: Arc<ToolRegistry>,
    pub config: AgentConfig,
    pub messages: Arc<RwLock<Vec<Message>>>,
    pub all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    // Note: Internal state fields (reasoning_chain, tool_call_records, final_content, error)
    // are NOT exposed to plugins. Plugins access state via events, not direct field access.
}

impl PluginContext {
    /// Create a PluginContext from a RunContext (without cloning senders)
    ///
    /// Note: Internal state fields (reasoning_chain, tool_call_records, final_content, error)
    /// are NOT copied to PluginContext. Plugins access state via events, not direct field access.
    pub fn from_run_ctx(ctx: &RunContext) -> Self {
        Self {
            run_id: ctx.run_id.clone(),
            user_input: ctx.user_input.clone(),
            session_id: ctx.session_id.clone(),
            session: ctx.session.clone(),
            tools: ctx.tools.clone(),
            config: ctx.config.clone(),
            messages: ctx.messages.clone(),
            all_tool_calls: ctx.all_tool_calls.clone(),
            current_tool_calls: ctx.current_tool_calls.clone(),
            data: ctx.data.clone(),
        }
    }

    /// Get a clone of all messages
    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
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
        self.data
            .read()
            .await
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set a value in the data store
    pub async fn set<T: serde::Serialize>(
        &self,
        key: &str,
        value: T,
    ) -> Result<(), serde_json::Error> {
        self.data
            .write()
            .await
            .insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }
}

/// Request type for plugin event bus communication
pub enum PluginRequest {
    Intercept {
        event: TracedEvent<AgentStreamEvent>,
        tx: oneshot::Sender<PluginDecision>,
    },
    Emit {
        event: TracedEvent<AgentStreamEvent>,
    },
}

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

    // Event bus
    pub event_tx: broadcast::Sender<TracedEvent<AgentStreamEvent>>,
    /// Plugin event channel sender.
    ///
    /// # Important: Receiver is intentionally Dropped in `new()`
    ///
    /// The corresponding receiver (`mpsc::Receiver<PluginRequest>`) is created
    /// but immediately dropped in [`RunContext::new()`]. This is by design.
    ///
    /// ## Usage Pattern
    ///
    /// 1. **Creation**: `RunContext::new()` creates the channel and drops the receiver
    /// 2. **Listener Setup**: A plugin listener task (created later in `PluginStream`)
    ///    will create a NEW receiver to intercept events
    /// 3. **Interception**: Once the listener is wired up, calls to
    ///    [`RunContext::intercept()`] will route events through the plugin system
    ///
    /// ## Why This Pattern?
    ///
    /// The sender is stored here to allow `RunContext` to be created early in the
    /// initialization flow, while the plugin listener is set up later by the
    /// `PluginStream` component. This separation of concerns allows:
    ///
    /// - Early context creation before plugin infrastructure is ready
    /// - Flexible plugin listener lifecycle management
    /// - No blocking during `RunContext` initialization
    ///
    /// ## Behavior Before Listener is Wired
    ///
    /// If [`intercept()`](RunContext::intercept) is called before a plugin listener
    /// has created a receiver, the send will fail with `SendError` (converted to
    /// `AgentError::Context`). This is expected - plugins are optional and the
    /// caller should handle this gracefully.
    ///
    /// ## Broadcast Channel Close Sequence
    ///
    /// The `event_tx` broadcast channel is used as a shutdown signal:
    /// 1. When `agent_task` completes, it drops its `RunContext` (one sender dropped)
    /// 2. `interceptor_handle` then exits (plugin_rx closed) and drops its `RunContext`
    /// 3. `listener_handle` sees `RecvError` (all senders dropped) and exits
    ///
    /// This ensures listener tasks have time to complete their `plugin.listen()` calls
    /// before the agent run is considered complete.
    pub plugin_event_tx: mpsc::Sender<PluginRequest>,

    // Internal state collection
    pub(crate) reasoning_chain: Arc<RwLock<Vec<ReasoningStep>>>,
    pub(crate) tool_call_records: Arc<RwLock<Vec<ToolCallRecord>>>,
    pub(crate) final_content: Arc<RwLock<Option<String>>>,
    pub(crate) error: Arc<RwLock<Option<String>>>,
}

impl RunContext {
    /// Create a new RunContext.
    ///
    /// Returns `(RunContext, mpsc::Receiver<PluginRequest>)`.
    ///
    /// The receiver should be passed to `run_interceptor_loop()` to handle
    /// plugin interception requests.
    pub fn new(
        run_id: String,
        user_input: String,
        session_id: String,
        session: Arc<Session>,
        tools: Arc<ToolRegistry>,
        config: AgentConfig,
    ) -> (Self, mpsc::Receiver<PluginRequest>) {
        let (event_tx, _) = broadcast::channel(100);
        let (plugin_event_tx, plugin_event_rx) = mpsc::channel(100);

        let ctx = Self {
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
            event_tx,
            plugin_event_tx,
            reasoning_chain: Arc::new(RwLock::new(Vec::new())),
            tool_call_records: Arc::new(RwLock::new(Vec::new())),
            final_content: Arc::new(RwLock::new(None)),
            error: Arc::new(RwLock::new(None)),
        };

        (ctx, plugin_event_rx)
    }

    /// Initialize messages array - must be called once before the loop
    ///
    /// This method:
    /// 1. Builds System message from `config.prompt_context.build_system()`
    /// 2. Gets historical messages from session (only once, limited by `max_history_messages`)
    /// 3. Adds user input
    /// 4. Writes all to `self.messages`
    ///
    /// Note: User input is NOT persisted to session here - it's only added to the
    /// runtime messages array. Callers should persist user input separately if needed.
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(AgentError)` if session access fails
    pub async fn init_messages(&self) -> Result<(), crate::AgentError> {
        let mut messages = Vec::new();

        // 1. System message from prompt_context (not persisted to session)
        let system_content = self.config.prompt_context.build_system();
        messages.push(Message::system(system_content));

        // 2. Historical messages from session (only once)
        let history = self
            .session
            .get_messages(self.config.max_history_messages)
            .await
            .unwrap_or_default();

        for session_msg in history {
            messages.push(session_msg.message);
        }

        // 3. User input (not persisted to session here)
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
        self.iteration
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Add a message to the messages list and sync to session
    pub async fn add_message(&self, message: Message) -> Result<(), crate::AgentError> {
        // 1. Add to runtime messages array
        self.messages.write().await.push(message.clone());

        // 2. Persist to session
        let session_msg = SessionMessage::new(self.session_id.clone(), message);
        self.session.add_message(session_msg).await.map_err(|e| {
            crate::AgentError::SessionError(format!("Failed to save message: {}", e))
        })?;

        Ok(())
    }

    /// Get a clone of all messages
    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }

    /// Add a tool call to both current and all_tool_calls lists
    pub async fn add_tool_call(&self, tool_call: ToolCall) {
        self.current_tool_calls
            .write()
            .await
            .push(tool_call.clone());
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
        self.data
            .read()
            .await
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set a value in the data store
    pub async fn set<T: serde::Serialize>(
        &self,
        key: &str,
        value: T,
    ) -> Result<(), serde_json::Error> {
        self.data
            .write()
            .await
            .insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }

    /// Emit an event to the event bus (non-blocking, fire-and-forget)
    ///
    /// This sends the event to all subscribers via the broadcast channel.
    /// Used by plugins to emit custom events.
    pub async fn emit(&self, event: AgentStreamEvent) {
        let traced_event = TracedEvent::without_span(event);
        let _ = self.event_tx.send(traced_event);
    }

    /// Intercept an event for plugin processing (blocking, returns decision).
    ///
    /// This sends the event to the plugin channel and waits for a decision.
    /// Returns `PluginDecision::Continue` to proceed, `Skip` to skip the event,
    /// or `Abort` to stop the entire agent execution.
    ///
    /// # Note
    ///
    /// This method requires a plugin listener to be active (created by `PluginStream`).
    /// If no listener has set up a receiver, this will return an error indicating
    /// the channel is closed. See [`plugin_event_tx`](RunContext::plugin_event_tx)
    /// for the full usage pattern.
    pub async fn intercept(
        &self,
        event: &AgentStreamEvent,
    ) -> Result<PluginDecision, crate::AgentError> {
        let (tx, rx) = oneshot::channel();
        let traced_event = TracedEvent::without_span(event.clone());
        self.plugin_event_tx
            .send(PluginRequest::Intercept {
                event: traced_event,
                tx,
            })
            .await
            .map_err(|e| crate::AgentError::Context(format!("Plugin channel error: {}", e)))?;

        rx.await
            .map_err(|e| crate::AgentError::Context(format!("Plugin response error: {}", e)))
    }

    /// Record a reasoning step
    pub async fn record_reasoning_step(&self, thinking: String, duration_ms: Option<u64>) {
        let iteration = self.iteration.load(std::sync::atomic::Ordering::SeqCst);
        let step = ReasoningStep::new(iteration, thinking, duration_ms);
        self.reasoning_chain.write().await.push(step);
    }

    /// Record a tool call
    pub async fn record_tool_call(&self, record: ToolCallRecord) {
        self.tool_call_records.write().await.push(record);
    }

    /// Set final answer content
    pub async fn set_final_content(&self, content: String) {
        *self.final_content.write().await = Some(content);
    }

    /// Set error information
    pub async fn set_error(&self, error: String) {
        *self.error.write().await = Some(error);
    }

    /// Get a clone of the reasoning chain
    pub async fn get_reasoning_chain(&self) -> Vec<ReasoningStep> {
        self.reasoning_chain.read().await.clone()
    }

    /// Get a clone of tool call records
    pub async fn get_tool_call_records(&self) -> Vec<ToolCallRecord> {
        self.tool_call_records.read().await.clone()
    }

    /// Build AgentResponse from collected state
    pub fn finalize(&self) -> super::response::AgentResponse {
        use super::response::{AgentResponse, ToolCallRecord as ResponseToolCallRecord};
        use futures::executor::block_on;

        let reasoning_chain = block_on(self.reasoning_chain.read());
        let tool_call_records = block_on(self.tool_call_records.read());
        let final_content = block_on(self.final_content.read());
        let error = block_on(self.error.read());

        // Convert state::ToolCallRecord to response::ToolCallRecord
        let response_tool_calls = tool_call_records
            .iter()
            .map(|record| ResponseToolCallRecord {
                tool_name: record.tool_name.clone(),
                arguments: record.arguments.clone(),
                result: record.result.clone(),
                iteration: record.iteration,
                success: record.success,
            })
            .collect();

        AgentResponse {
            content: final_content.clone().unwrap_or_default(),
            reasoning: reasoning_chain.clone(),
            run_id: self.run_id.clone(),
            session_id: self.session_id.clone(),
            iterations: self.iteration.load(std::sync::atomic::Ordering::SeqCst),
            tool_calls: response_tool_calls,
            error: error.clone(),
        }
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
            event_tx: self.event_tx.clone(),
            plugin_event_tx: self.plugin_event_tx.clone(),
            reasoning_chain: self.reasoning_chain.clone(),
            tool_call_records: self.tool_call_records.clone(),
            final_content: self.final_content.clone(),
            error: self.error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{InMemoryMessageStore, InMemorySessionStore, SessionMessage};
    use vol_llm_core::{MessageContent, MessageRole};

    fn create_test_context() -> RunContext {
        let (ctx, _rx) = RunContext::new(
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
        );
        ctx
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
        ctx.add_message(Message::system("test".to_string()))
            .await
            .unwrap();
        let msgs = ctx.get_messages().await;
        assert_eq!(msgs.len(), 1);
    }

    #[tokio::test]
    async fn test_add_message_syncs_to_session() {
        let ctx = create_test_context();

        // Add a message via add_message (should sync to session)
        let message = Message::user("test message".to_string());
        ctx.add_message(message.clone()).await.unwrap();

        // Verify message is in runtime messages
        let msgs = ctx.get_messages().await;
        assert_eq!(msgs.len(), 1);
        if let Some(MessageContent::Text(content)) = &msgs[0].content {
            assert_eq!(content, "test message");
        } else {
            panic!("Expected Text content");
        }

        // Verify message is persisted to session
        let session_msgs = ctx.session.get_messages(10).await.unwrap();
        assert_eq!(session_msgs.len(), 1);
        if let Some(MessageContent::Text(content)) = &session_msgs[0].message.content {
            assert_eq!(content, "test message");
        } else {
            panic!("Expected Text content");
        }
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
        use crate::prompt_context::{PromptContext, PromptTemplate};

        let template = PromptTemplate::new("test", "You are a helpful assistant.");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            prompt_context,
            ..Default::default()
        };

        let (ctx, _rx) = RunContext::new(
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
        assert!(messages[0]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("You are a helpful assistant."));
    }

    #[tokio::test]
    async fn test_init_messages_history() {
        use crate::prompt_context::{PromptContext, PromptTemplate};

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

        let (ctx, _rx) = RunContext::new(
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
        assert!(messages[1]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("Previous conversation"));
    }

    #[tokio::test]
    async fn test_init_messages_user_input() {
        use crate::prompt_context::{PromptContext, PromptTemplate};

        let template = PromptTemplate::new("test", "System");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            prompt_context,
            ..Default::default()
        };

        let (ctx, _rx) = RunContext::new(
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
        assert!(last_msg
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("analyze market volatility"));
    }

    #[tokio::test]
    async fn test_init_messages_only_once() {
        use crate::prompt_context::{PromptContext, PromptTemplate};

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
        let history_msg = SessionMessage::new("session-1".to_string(), Message::user("History"));
        session.add_message(history_msg).await.unwrap();

        let (ctx, _rx) = RunContext::new(
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

    #[tokio::test]
    async fn test_record_reasoning_step() {
        let (ctx, _rx) = RunContext::new(
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
        );

        ctx.record_reasoning_step("First thought".to_string(), Some(100))
            .await;
        ctx.record_reasoning_step("Second thought".to_string(), None)
            .await;

        let chain = ctx.get_reasoning_chain().await;
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].thinking, "First thought");
        assert_eq!(chain[0].duration_ms, Some(100));
        assert_eq!(chain[1].thinking, "Second thought");
    }

    #[tokio::test]
    async fn test_record_tool_call() {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        );

        let record = ToolCallRecord {
            tool_name: "test_tool".to_string(),
            arguments: "{}".to_string(),
            result: "result".to_string(),
            iteration: 1,
            success: true,
        };
        ctx.record_tool_call(record).await;

        let records = ctx.get_tool_call_records().await;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool_name, "test_tool");
    }

    #[tokio::test]
    async fn test_set_final_content() {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        );

        ctx.set_final_content("Final answer".to_string()).await;

        // Use finalize to verify
        let response = ctx.finalize();
        assert_eq!(response.content, "Final answer");
    }

    #[tokio::test]
    async fn test_finalize() {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        );

        ctx.record_reasoning_step("thought".to_string(), None).await;
        ctx.set_final_content("answer".to_string()).await;

        let response = ctx.finalize();

        assert_eq!(response.content, "answer");
        assert_eq!(response.reasoning.len(), 1);
        assert_eq!(response.reasoning[0].thinking, "thought");
        assert_eq!(response.run_id, "test-run");
        assert_eq!(response.session_id, "session-1");
    }
}
