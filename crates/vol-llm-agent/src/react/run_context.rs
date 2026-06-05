//! RunContext - Unified run state management for ReAct Agent.
//!
//! Encapsulates all state and resources for a single `run()` invocation.

use vol_session::{Session, SessionMessage};
use super::plugin::PluginDecision;
use super::state::{ReasoningStep, ToolCallRecord};
use super::stream::AgentStreamEvent;
use super::AgentConfig;
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use vol_llm_core::Message;
use vol_llm_core::ToolCall;
use vol_llm_tool::ToolRegistry;
use vol_tracing::TracedEvent;


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
/// It provides:
/// - Mutable state (tool calls, iteration count)
/// - Resource references (session, tools, config)
/// - Thread-safe access via Arc/RwLock for async operations
pub struct RunContext {
    // Immutable fields
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    /// Model used for this run, from LLM config.
    pub model: String,

    // Mutable state (internal mutability via AtomicU32 and Arc<RwLock>)
    pub iteration: AtomicU32,
    pub all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,

    // Resource references
    pub session: Arc<Session>,
    pub tools: Arc<ToolRegistry>,
    pub config: Arc<AgentConfig>,

    // Event bus
    pub event_tx: Option<Arc<broadcast::Sender<TracedEvent<AgentStreamEvent>>>>,
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
    /// The `event_tx` is wrapped in `Arc`, so cloning `RunContext` only copies the
    /// Arc pointer. Plugin infrastructure contexts can remove sender handles to
    /// avoid keeping shutdown channels alive.
    pub plugin_event_tx: Option<Arc<mpsc::Sender<PluginRequest>>>,


    // Internal state collection
    pub(crate) reasoning_chain: Arc<RwLock<Vec<ReasoningStep>>>,
    pub(crate) tool_call_records: Arc<RwLock<Vec<ToolCallRecord>>>,
    pub(crate) final_content: Arc<RwLock<Option<String>>>,
    pub(crate) error: Arc<RwLock<Option<String>>>,
    /// Tracks the ID of the last message added, for auto-setting parent_id.
    pub last_message_id: Arc<std::sync::Mutex<Option<String>>>,
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
        config: Arc<AgentConfig>,
    ) -> (Self, mpsc::Receiver<PluginRequest>) {
        let (event_tx, _) = broadcast::channel::<TracedEvent<AgentStreamEvent>>(1024);
        let event_tx = Arc::new(event_tx);
        let (plugin_event_tx, plugin_event_rx) = mpsc::channel(100);

        let session = config.session.read().unwrap().clone();

        let ctx = Self {
            run_id,
            user_input,
            session_id: session.id.clone(),
            model: if config.llm.model().is_empty() { "unknown".to_string() } else { config.llm.model().to_string() },
            iteration: AtomicU32::new(0),
            all_tool_calls: Arc::new(RwLock::new(Vec::new())),
            current_tool_calls: Arc::new(RwLock::new(Vec::new())),
            data: Arc::new(RwLock::new(HashMap::new())),
            session,
            tools: Arc::clone(&config.tools),
            config,
            event_tx: Some(event_tx),
            plugin_event_tx: Some(Arc::new(plugin_event_tx)),
            reasoning_chain: Arc::new(RwLock::new(Vec::new())),
            tool_call_records: Arc::new(RwLock::new(Vec::new())),
            final_content: Arc::new(RwLock::new(None)),
            error: Arc::new(RwLock::new(None)),
            last_message_id: Arc::new(std::sync::Mutex::new(None)),
        };

        (ctx, plugin_event_rx)
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

    /// Reset the iteration counter to 0 (called after user approves continuation).
    pub fn reset_iteration(&self) {
        self.iteration.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get max iterations from AgentDef, defaulting to 5.
    pub fn max_iterations(&self) -> u32 {
        self.config.def.as_ref()
            .and_then(|d| d.max_iterations)
            .unwrap_or(5)
    }

    /// Get max history messages from AgentDef, defaulting to 20.
    pub fn max_history_messages(&self) -> usize {
        self.config.def.as_ref()
            .and_then(|d| d.max_history_messages)
            .unwrap_or(20)
    }

    /// Add a message to the session.
    /// Automatically sets parent_id to the previous message's ID.
    pub async fn add_message(&self, message: Message) -> Result<(), crate::AgentError> {
        let session_msg = {
            let mut last_id = self.last_message_id.lock().unwrap();
            let mut msg = SessionMessage::new(self.session.id.clone(), message)
                .with_metadata(vol_session::RUN_ID_KEY, &self.run_id);
            if let Some(id) = last_id.as_ref() {
                msg = msg.with_parent_id(id.clone());
            }
            let new_id = msg.id.clone();
            *last_id = Some(new_id);
            msg
        };

        self.session.add_message(session_msg).await.map_err(|e| {
            crate::AgentError::SessionError(format!("Failed to save message: {}", e))
        })?;

        Ok(())
    }

    /// Build the full LLM context for a run iteration.
    ///
    /// SessionContributor is a permanent contributor on config.context_builder,
    /// so we clone the builder and build directly.
    pub async fn get_context(&self) -> Result<Vec<Message>, crate::AgentError> {
        let cb = self.config.context_builder.read().unwrap().clone();
        let output = cb.build().await.map_err(|e| {
            crate::AgentError::Context(format!("Failed to build context: {}", e))
        })?;
        Ok(output.messages)
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
        let _ = self.event_tx.as_ref().unwrap().send(traced_event);
    }

    /// Get effective (filtered) tool definitions for LLM request.
    ///
    /// Filters tools based on AgentDef's `tools` (allowlist) and
    /// `disallowed_tools` (blocklist). If no def is present, returns all tools.
    pub fn effective_tools(&self) -> Vec<vol_llm_core::ToolDefinition> {
        self.effective_registry().definitions()
    }

    /// Execute a tool by its call specification.
    ///
    /// Only executes tools that are in the effective (filtered) set.
    /// Returns an error if the tool is not in the allowed set.
    pub async fn execute_tool(
        &self,
        call: &vol_llm_core::ToolCall,
        ctx: &vol_llm_tool::ToolContext,
    ) -> vol_llm_tool::Result<vol_llm_tool::ToolResult> {
        self.effective_registry()
            .execute(call, ctx)
            .await
            .map_err(vol_llm_tool::ToolError::ExecutionFailed)
    }

    /// Build a filtered ToolRegistry based on AgentDef configuration.
    ///
    /// Returns a registry containing only the allowed tools, minus any
    /// disallowed tools. If no def is present, returns the full registry.
    fn effective_registry(&self) -> Arc<ToolRegistry> {
        if let Some(def) = &self.config.def {
            let allowed: Option<Vec<&str>> = def.tools.as_ref()
                .map(|t| t.iter().map(|s| s.as_str()).collect());
            let disallowed: Option<Vec<&str>> = def.disallowed_tools.as_ref()
                .map(|t| t.iter().map(|s| s.as_str()).collect());
            ToolRegistry::filter(&self.tools, allowed.as_deref(), disallowed.as_deref())
        } else {
            self.tools.clone()
        }
    }

    /// Emit a traced event to the event bus (non-blocking, fire-and-forget).
    pub async fn emit_traced(&self, event: TracedEvent<AgentStreamEvent>) {
        let _ = self.event_tx.as_ref().unwrap().send(event);
    }

    /// Return a cloned RunContext with plugin_event_tx set to None.
    pub fn without_plugin_event_tx(&self) -> Self {
        let mut ctx = self.clone();
        ctx.plugin_event_tx = None;
        ctx
    }

    /// Return a cloned RunContext without event or plugin request senders.
    pub fn without_event_senders(&self) -> Self {
        let mut ctx = self.without_plugin_event_tx();
        ctx.event_tx = None;
        ctx
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
        let sender = self
            .plugin_event_tx
            .as_ref()
            .ok_or_else(|| crate::AgentError::Context("Plugin channel closed".to_string()))?;
        sender
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
            model: self.model.clone(),
            iteration: AtomicU32::new(self.current_iteration()),
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
            last_message_id: self.last_message_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_context::{AttentionAnchor, ContextBuilderBuilder};
    use vol_session::{InMemoryEntryStore, SessionMessage};
    use vol_llm_core::{MessageContent, MessageRole};

    fn create_test_context() -> RunContext {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            Arc::new(AgentConfig::default()),
        );
        ctx
    }

    #[tokio::test]
    async fn test_run_context_new() {
        let ctx = create_test_context();
        assert_eq!(ctx.run_id, "test-run");
        assert_eq!(ctx.user_input, "test input");
        assert_eq!(ctx.session_id, ctx.session.id);
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
        let msgs = ctx.session.get_messages().await.unwrap();
        assert_eq!(msgs.len(), 1);
    }

    #[tokio::test]
    async fn test_add_message_syncs_to_session() {
        let ctx = create_test_context();

        // Add a message via add_message (should sync to session)
        let message = Message::user("test message".to_string());
        ctx.add_message(message).await.unwrap();

        // Verify message is persisted to session
        let session_msgs = ctx.session.get_messages().await.unwrap();
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
    async fn test_get_context_system_message() {
        use vol_llm_context::builtin::SimpleContributor;

        let context_builder = ContextBuilderBuilder::new(128_000)
            .add_contributor(Box::new(SimpleContributor::system(
                "You are a helpful assistant.".to_string(),
            )))
            .build();

        let config = Arc::new(AgentConfig {
            context_builder: std::sync::RwLock::new(context_builder),
            ..Default::default()
        });

        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            config,
        );

        let messages = ctx.get_context().await.unwrap();

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
    async fn test_get_context_history() {
        use vol_llm_context::builtin::SimpleContributor;
        use vol_session::SessionContributor;

        let session = Arc::new(Session::new(
            Arc::new(InMemoryEntryStore::new()),
        ));

        // Add a historical message to session
        let history_msg = SessionMessage::new(
            session.id.clone(),
            Message::user("Previous conversation"),
        );
        session.add_message(history_msg).await.unwrap();

        // Build context_builder with SessionContributor after session has messages
        let context_builder = ContextBuilderBuilder::new(128_000)
            .add_contributor(Box::new(SimpleContributor::system("System".to_string())))
            .add_contributor(Box::new(SessionContributor::new(
                Arc::new(tokio::sync::Mutex::new((*session).clone())),
                50,
                AttentionAnchor::Middle(0),
            )))
            .build();

        let config = Arc::new(AgentConfig {
            context_builder: std::sync::RwLock::new(context_builder),
            session: std::sync::RwLock::new(session),
            ..Default::default()
        });

        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "new input".to_string(),
            config,
        );

        let messages = ctx.get_context().await.unwrap();

        // Should have: system + history = 2 messages (user input comes from session, not parameter)
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, MessageRole::User);
        assert!(messages[1]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("Previous conversation"));
    }

    #[tokio::test]
    async fn test_get_context_user_message_from_session() {
        use vol_llm_context::builtin::SimpleContributor;
        use vol_session::SessionContributor;

        let session = Arc::new(Session::new(
            Arc::new(InMemoryEntryStore::new()),
        ));

        // Build context_builder with SessionContributor (session is empty for now)
        let context_builder = ContextBuilderBuilder::new(128_000)
            .add_contributor(Box::new(SimpleContributor::system("System".to_string())))
            .add_contributor(Box::new(SessionContributor::new(
                Arc::new(tokio::sync::Mutex::new((*session).clone())),
                50,
                AttentionAnchor::Middle(0),
            )))
            .build();

        let config = Arc::new(AgentConfig {
            context_builder: std::sync::RwLock::new(context_builder),
            session: std::sync::RwLock::new(session.clone()),
            ..Default::default()
        });

        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "analyze market volatility".to_string(),
            config,
        );

        // Persist user message to session (simulating what agent.rs does at run start)
        ctx.add_message(Message::user("analyze market volatility")).await.unwrap();

        // get_context should now pick up the user message from the session
        let messages = ctx.get_context().await.unwrap();

        // Should have: system + user message from session = 2 messages
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[1].role, MessageRole::User);
        assert!(messages[1]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("analyze market volatility"));
    }

    #[tokio::test]
    async fn test_get_context_consistent() {
        use vol_llm_context::builtin::SimpleContributor;
        use vol_session::SessionContributor;

        let session = Arc::new(Session::new(
            Arc::new(InMemoryEntryStore::new()),
        ));

        // Add a historical message
        let history_msg = SessionMessage::new(session.id.clone(), Message::user("History"));
        session.add_message(history_msg).await.unwrap();

        // Build context_builder with SessionContributor after session has messages
        let context_builder = ContextBuilderBuilder::new(128_000)
            .add_contributor(Box::new(SimpleContributor::system("System".to_string())))
            .add_contributor(Box::new(SessionContributor::new(
                Arc::new(tokio::sync::Mutex::new((*session).clone())),
                50,
                AttentionAnchor::Middle(0),
            )))
            .build();

        let config = Arc::new(AgentConfig {
            context_builder: std::sync::RwLock::new(context_builder),
            session: std::sync::RwLock::new(session),
            ..Default::default()
        });

        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "input".to_string(),
            config,
        );

        // Call get_context multiple times - each builds fresh
        let messages_first = ctx.get_context().await.unwrap();
        let messages_second = ctx.get_context().await.unwrap();

        // Same count since no new messages were added between calls
        assert_eq!(messages_first.len(), messages_second.len());

        // Verify we have: system + history = 2 messages (user input from session, not parameter)
        assert_eq!(messages_second.len(), 2);
    }

    #[tokio::test]
    async fn test_record_reasoning_step() {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            Arc::new(AgentConfig::default()),
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
            Arc::new(AgentConfig::default()),
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
            Arc::new(AgentConfig::default()),
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
            Arc::new(AgentConfig::default()),
        );

        ctx.record_reasoning_step("thought".to_string(), None).await;
        ctx.set_final_content("answer".to_string()).await;

        let response = ctx.finalize();

        assert_eq!(response.content, "answer");
        assert_eq!(response.reasoning.len(), 1);
        assert_eq!(response.reasoning[0].thinking, "thought");
        assert_eq!(response.run_id, "test-run");
        assert_eq!(response.session_id, ctx.session.id);
    }

    #[tokio::test]
    async fn test_add_message_auto_sets_parent_id() {
        let ctx = create_test_context();

        // Add first message
        ctx.add_message(Message::user("first")).await.unwrap();

        // Add second message
        ctx.add_message(Message::assistant("second")).await.unwrap();

        // Verify second message has parent_id set
        let session_msgs = ctx.session.get_messages().await.unwrap();
        assert_eq!(session_msgs.len(), 2);
        assert!(session_msgs[0].parent_id.is_none()); // first message, no parent
        assert!(session_msgs[1].parent_id.is_some()); // second message has parent
        assert_eq!(session_msgs[1].parent_id.as_ref().unwrap(), &session_msgs[0].id);
    }

    #[tokio::test]
    async fn test_run_context_reset_iteration() {
        let ctx = create_test_context();
        ctx.next_iteration();
        ctx.next_iteration();
        assert_eq!(ctx.current_iteration(), 2);
        ctx.reset_iteration();
        assert_eq!(ctx.current_iteration(), 0);
    }
}
