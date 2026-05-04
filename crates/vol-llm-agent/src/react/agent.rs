//! ReAct Agent implementation.

use super::{
    AgentResponse, AgentStreamEvent, PluginDecision, PluginRegistry, RunContext,
};
use crate::react::state::ToolCallRecord;
use vol_session::{InMemoryEntryStore, Session};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use vol_llm_context::{ContextBuilder, ContextBuilderBuilder};
use vol_llm_skill::{SkillInjector, SkillLoader, SkillTool};
use vol_llm_tool::ToolRegistry;
use vol_llm_core::{
    ConversationRequest, ConversationResponse, FinishReason, LLMClient, Message, SandboxRef, StreamEventData, StreamReceiver,
    ToolChoice,
};
use vol_llm_tool::ToolContext;

/// Agent configuration — single source of truth for ReActAgent.
#[derive(Clone)]
pub struct AgentConfig {
    // === Declarative definition (optional) ===
    pub def: Option<crate::agent_def::AgentDef>,

    // === Runtime components ===
    pub llm: Arc<dyn vol_llm_core::LLMClient>,
    pub tools: Arc<vol_llm_tool::ToolRegistry>,
    pub session: Arc<Session>,
    pub sandbox: Option<SandboxRef>,

    // === Context and plugins ===
    pub context_builder: ContextBuilder,
    pub plugin_registry: PluginRegistry,

    // === Runtime tuning (defaults; can be overridden by def) ===
    pub max_iterations: u32,
    pub max_history_messages: usize,

    // === Metadata ===
    pub agent_id: String,
    /// Working directory. Log paths derive from `{working_dir}/logs/agents/{agent_id}/`.
    pub working_dir: PathBuf,
}

/// Generate a short random agent ID if not provided
fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("agent_{:x}", timestamp % 0xFFFFFF)
}

impl AgentConfig {
    /// Create a new builder for AgentConfig.
    pub fn builder() -> super::config_builder::AgentConfigBuilder {
        super::config_builder::AgentConfigBuilder::new()
    }

    /// Convenience constructor for direct struct creation (backwards-compatible).
    pub fn new(
        llm: Arc<dyn vol_llm_core::LLMClient>,
        tools: Arc<vol_llm_tool::ToolRegistry>,
        session: Arc<Session>,
    ) -> Self {
        Self {
            def: None,
            llm,
            tools,
            session,
            sandbox: None,
            context_builder: ContextBuilderBuilder::new(128_000).build(),
            plugin_registry: PluginRegistry::new(),
            max_iterations: 5,
            max_history_messages: 20,
            agent_id: generate_agent_id(),
            working_dir: PathBuf::from("."),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            def: None,
            llm: Arc::new(DefaultLlm),
            tools: Arc::new(vol_llm_tool::ToolRegistry::new()),
            session: Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
            sandbox: None,
            context_builder: ContextBuilderBuilder::new(128_000).build(),
            plugin_registry: PluginRegistry::new(),
            max_iterations: 5,
            max_history_messages: 20,
            agent_id: generate_agent_id(),
            working_dir: PathBuf::from("."),
        }
    }
}

/// Dummy LLM for Default impl (tests only — will panic if used).
struct DefaultLlm;
#[async_trait::async_trait]
impl LLMClient for DefaultLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
    fn model(&self) -> &str { "default" }
    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("DefaultLlm::converse called — AgentConfig::default() is for struct defaults only")
    }
    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
        let (_tx, rx) = tokio::sync::mpsc::channel(10);
        Ok(StreamReceiver::new(rx))
    }
}

/// Holds a shared SkillLoader and provides helpers to register skills
/// into the tool registry and context builder.
pub struct SkillsConfig {
    loader: Arc<SkillLoader>,
}

impl SkillsConfig {
    /// Create from a working directory. The SkillLoader discovers skills
    /// lazily on first access (no I/O during construction).
    pub fn from_workdir(working_dir: &Path) -> Self {
        Self {
            loader: Arc::new(SkillLoader::new(Some(working_dir.to_path_buf()))),
        }
    }

    /// Register the SkillTool into the tool registry.
    /// Call this on a mutable reference before wrapping the registry in Arc.
    pub fn register_tool(&self, registry: &mut ToolRegistry) {
        registry.register(SkillTool::new(self.loader.clone()));
    }

    /// Build a new ContextBuilder from an existing one, adding the SkillInjector.
    pub fn enhance_context_builder(
        &self,
        existing: &ContextBuilder,
    ) -> ContextBuilder {
        let injector = SkillInjector::new(self.loader.clone());
        let budget = existing.token_budget();
        ContextBuilderBuilder::new(budget.total)
            .head_size(budget.head_size)
            .tail_size(budget.tail_size)
            .add_contributors_from(existing)
            .add_contributor(Box::new(injector))
            .build()
    }
}

impl AgentConfig {
    /// Enhance this config with skill injection in the context builder.
    pub fn with_skills(self, working_dir: &Path) -> Self {
        let skills = SkillsConfig::from_workdir(working_dir);
        let new_context = skills.enhance_context_builder(&self.context_builder);
        AgentConfig {
            context_builder: new_context,
            ..self
        }
    }
}

/// ReAct Agent
pub struct ReActAgent {
    config: AgentConfig,
}

impl ReActAgent {
    /// Create a new ReActAgent from config.
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    /// Set the sandbox for tool execution
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.config.sandbox = Some(sandbox);
        self
    }

    /// Create agent with new session
    pub fn with_new_session(&self, session_id: String) -> Self {
        use vol_session::InMemoryEntryStore;

        let entry_store = Arc::new(InMemoryEntryStore::new());
        let new_session = Arc::new(Session::new(entry_store));
        // Note: session.id is now self-generated; session_id param is ignored for the ID.
        // If the caller needs a specific ID, use Session::resume instead.
        Self {
            config: AgentConfig {
                session: new_session,
                ..self.config.clone()
            },
        }
    }

    /// Run ReAct loop and return the final response.
    ///
    /// All events are emitted via RunContext event bus.
    /// The returned AgentResponse contains the complete execution context including:
    /// - Final answer content and complete reasoning chain
    /// - run_id and session_id for correlation
    /// - All tool calls made during execution
    /// - Error information if any tool call failed
    pub async fn run(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
        // === Phase 1: Generate run_id, compute effective config from def ===

        // Tool filtering from AgentDef
        let effective_tools = if let Some(def) = &self.config.def {
            let allowed: Option<Vec<&str>> = def.tools.as_ref()
                .map(|t| t.iter().map(|s| s.as_str()).collect());
            let disallowed: Option<Vec<&str>> = def.disallowed_tools.as_ref()
                .map(|t| t.iter().map(|s| s.as_str()).collect());
            ToolRegistry::filter(&self.config.tools, allowed.as_deref(), disallowed.as_deref())
        } else {
            self.config.tools.clone()
        };

        // Read max_iterations and max_history from def if set, else use config defaults
        let max_iterations = self.config.def.as_ref()
            .and_then(|d| d.max_iterations)
            .unwrap_or(self.config.max_iterations);
        let max_history_messages = self.config.def.as_ref()
            .and_then(|d| d.max_history_messages)
            .unwrap_or(self.config.max_history_messages);

        // Clone config with computed values
        let config = AgentConfig {
            max_iterations,
            max_history_messages,
            tools: effective_tools.clone(),
            ..self.config.clone()
        };

        let run_id = uuid::Uuid::new_v4().simple().to_string();

        let session = self.config.session.clone();

        let (run_ctx, plugin_rx) = RunContext::new(
            run_id.clone(),
            user_input.to_string(),
            self.config.session.id.clone(),
            session.clone(),
            effective_tools,
            config.clone(),
        );

        // Persist user message to session so it's available via SessionContributor.
        // This replaces the old UserInputContributor which injected input directly
        // into the context without persisting it.
        let user_msg = Message::user(user_input.to_string());
        run_ctx.add_message(user_msg).await.map_err(|e| {
            crate::AgentError::SessionError(format!("Failed to persist user message: {}", e))
        })?;

        // === Phase 2: Context is built per-iteration via get_context ===

        // === Phase 2.6: Spawn listener and interceptor tasks ===
        use super::plugin_stream::{run_interceptor_loop, spawn_listener_task};

        // Spawn listener task - subscribes to event broadcast channel
        // Handle stored for graceful shutdown wait
        // Note: We create plugin_ctx and subscribe here to avoid cloning RunContext
        // (which would clone senders and prevent channel close)
        let listener_event_rx = run_ctx.event_tx.subscribe();
        let listener_handle = spawn_listener_task(
            config.plugin_registry.plugins().to_vec(),
            run_ctx.clone(),
            listener_event_rx,
        );

        // Spawn interceptor loop task - receives from plugin_rx channel
        // When plugin_rx is closed (agent drops run_ctx), interceptor exits
        let interceptor_event_tx = run_ctx.event_tx.clone();
        let interceptor_plugins = config.plugin_registry.plugins().to_vec();
        let interceptor_ctx = run_ctx.clone();
        let interceptor_handle = tokio::spawn(async move {
            run_interceptor_loop(
                plugin_rx,
                interceptor_plugins,
                interceptor_event_tx,
                interceptor_ctx,
            )
            .await;
        });

        // === Phase 3: Spawn agent loop task and await it ===
        let llm = self.config.llm.clone();
        let user_input = user_input.to_string();
        let sandbox = self.config.sandbox.clone();

        let agent_task = tokio::spawn(async move {
            // === Emit and intercept AgentStart ===
            let start_event = AgentStreamEvent::agent_start(user_input.clone());
            run_ctx.emit(start_event.clone()).await;

            match run_ctx.intercept(&start_event).await {
                Ok(PluginDecision::Continue) => {
                    // Continue with normal flow
                }
                Ok(PluginDecision::Skip) => {
                    // Skip only affects the current event, not the entire run
                }
                Ok(PluginDecision::Abort(reason)) => {
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                        .await;
                    return Err(crate::AgentError::Context(reason));
                }
                Err(e) => {
                    tracing::warn!(
                        "Plugin intercept error (plugins may not be wired up yet): {}",
                        e
                    );
                }
            }

            loop {
                // Increment iteration via ctx
                run_ctx.next_iteration();
                let iteration = run_ctx.current_iteration();

                if iteration > config.max_iterations {
                    run_ctx.emit(AgentStreamEvent::max_iterations_reached(
                        iteration,
                        config.max_iterations,
                    )).await;

                    match run_ctx.intercept(&AgentStreamEvent::iteration_complete(iteration, vec![], None)).await {
                        Ok(PluginDecision::Continue) => {
                            run_ctx.emit(AgentStreamEvent::iteration_continued(iteration)).await;
                            run_ctx.reset_iteration();
                            continue;
                        }
                        _ => {
                            let reason = format!("Max iterations ({}) reached", config.max_iterations);
                            run_ctx.emit(AgentStreamEvent::agent_aborted(reason.clone())).await;
                            return Err(crate::AgentError::MaxIterationsReached {
                                max: config.max_iterations,
                            });
                        }
                    }
                }

                // Reason phase - call LLM with streaming
                let tools_defs = config.tools.definitions();

                // Get messages from ctx (not local variable)
                let messages = run_ctx.get_context().await.map_err(|e| crate::AgentError::from(e))?;

                // Emit LLMCallStart with full message history
                run_ctx.emit(AgentStreamEvent::llm_call_start(iteration, messages.clone())).await;

                let request = ConversationRequest::with_history(None, messages)
                    .with_tools(tools_defs)
                    .with_tool_choice(ToolChoice::Auto);

                let llm_stream = match llm.converse_stream(request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
                        run_ctx.emit(AgentStreamEvent::agent_aborted(format!("LLM request failed: {}", e))).await;
                        return Err(crate::AgentError::Llm(e));
                    }
                };

                // Consume LLM stream — emits Thinking/Content streaming events internally
                let (thinking, tool_calls, content, model, usage) = match consume_llm_stream(llm_stream, &run_ctx).await {
                    Ok(data) => data,
                    Err(e) => {
                        run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
                        run_ctx.emit(AgentStreamEvent::agent_aborted(format!("LLM stream failed: {}", e))).await;
                        return Err(e);
                    }
                };

                // Record reasoning step
                if !thinking.is_empty() {
                    run_ctx.record_reasoning_step(thinking.clone(), None).await;
                }

                // Emit LLMCallComplete with real model and usage
                run_ctx.emit(AgentStreamEvent::llm_call_complete(model.clone(), usage)).await;

                // Check if tool calls
                if !tool_calls.is_empty() {

                    // IMPORTANT: Add assistant message with tool calls to history
                    let assistant_message = if !content.is_empty() {
                        Message::assistant_with_tools(content.clone(), tool_calls.clone())
                    } else {
                        Message::assistant_with_tools(
                            "Calling tools to get information.".to_string(),
                            tool_calls.clone(),
                        )
                    };
                    if let Err(e) = run_ctx.add_message(assistant_message).await {
                        return Err(crate::AgentError::from(e));
                    }

                    // Act phase - execute tools
                    for call in &tool_calls {
                        // === Emit and intercept ToolCallBegin ===
                        let tool_event = AgentStreamEvent::tool_call_begin(
                            call.id.clone(),
                            call.name.clone(),
                            call.arguments.clone(),
                        );
                        run_ctx.emit(tool_event.clone()).await;
                        let tool_begin = std::time::Instant::now();

                        let tool_decision = match run_ctx.intercept(&tool_event).await {
                            Ok(decision) => decision,
                            Err(e) => {
                                tracing::warn!("Plugin intercept error: {}", e);
                                PluginDecision::Continue
                            }
                        };

                        match tool_decision {
                            PluginDecision::Continue => {
                                // Execute tool directly — approval is handled by HitlPlugin via intercept()
                            }
                            PluginDecision::Skip => {
                                tracing::warn!("Plugin intercepted to skip tool: {}", call.name);
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;

                                run_ctx.emit(AgentStreamEvent::tool_call_skipped(
                                    call.id.clone(),
                                    call.name.clone(),
                                    "Plugin skipped".to_string(),
                                    Some(duration_ms),
                                )).await;

                                continue;
                            }
                            PluginDecision::Abort(reason) => {
                                run_ctx
                                    .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                                    .await;
                                return Err(crate::AgentError::Context(reason));
                            }
                        }

                        // Execute tool
                        let tool_ctx = match &sandbox {
                            Some(sandbox) => ToolContext::default().with_sandbox(sandbox.clone()),
                            None => ToolContext::default(),
                        };
                        let result = match config.tools.execute(call, &tool_ctx).await {
                            Ok(r) => r,
                            Err(e) => {
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;
                                run_ctx.emit(AgentStreamEvent::tool_call_error(
                                    call.id.clone(),
                                    call.name.clone(),
                                    e.to_string(),
                                    Some(duration_ms),
                                )).await;

                                run_ctx.record_tool_call(ToolCallRecord {
                                    tool_name: call.name.clone(),
                                    arguments: call.arguments.clone(),
                                    result: format!("Error: {}", e),
                                    iteration,
                                    success: false,
                                }).await;

                                // Add error message to session — LLM sees it on next turn
                                let error_content =
                                    format!("Tool '{}' error: {}", call.name, e);
                                if let Err(e) = run_ctx
                                    .add_message(Message::tool(error_content, call.id.clone()))
                                    .await
                                {
                                    return Err(crate::AgentError::from(e));
                                }

                                continue;
                            }
                        };

                        // Record tool call
                        run_ctx
                            .record_tool_call(ToolCallRecord {
                                tool_name: call.name.clone(),
                                arguments: call.arguments.clone(),
                                result: result.content.clone(),
                                iteration,
                                success: true,
                            })
                            .await;

                        // Emit ToolCallComplete
                        let duration_ms = tool_begin.elapsed().as_millis() as u64;
                        run_ctx
                            .emit(AgentStreamEvent::tool_call_complete(
                                call.id.clone(),
                                call.name.clone(),
                                result.content.clone(),
                                Some(duration_ms),
                            ))
                            .await;

                        // Add tool result to ctx
                        if let Err(e) = run_ctx
                            .add_message(Message::tool(result.content.clone(), call.id.clone()))
                            .await
                        {
                            return Err(crate::AgentError::from(e));
                        }

                        // Clear current tool calls for next iteration
                        run_ctx.clear_current_tool_calls().await;
                    }

                    // Emit IterationComplete
                    run_ctx
                        .emit(AgentStreamEvent::iteration_complete(
                            iteration,
                            tool_calls.clone(),
                            None,
                        ))
                        .await;

                    continue;
                }

                // No tool calls - we have final answer
                run_ctx
                    .emit(AgentStreamEvent::iteration_complete(
                        iteration,
                        Vec::new(),
                        Some(content.clone()),
                    ))
                    .await;

                // Save assistant response to session
                if let Err(e) = run_ctx
                    .add_message(Message::assistant(content.clone()))
                    .await
                {
                    return Err(crate::AgentError::from(e));
                }

                // Store final response data
                run_ctx.set_final_content(content.clone()).await;

                // === Emit AgentComplete with response data ===
                let response = run_ctx.finalize();
                let response_json = serde_json::json!({
                    "content": response.content,
                    "iterations": response.iterations,
                    "tool_calls": response.tool_calls.iter().map(|t| serde_json::json!({
                        "tool_name": t.tool_name,
                        "arguments": t.arguments,
                        "result": t.result,
                        "iteration": t.iteration,
                        "success": t.success,
                    })).collect::<Vec<_>>(),
                    "run_id": response.run_id,
                    "session_id": response.session_id,
                });
                run_ctx
                    .emit(AgentStreamEvent::agent_complete_with_response(response_json))
                    .await;

                return Ok(response);
            }
        });

        // Wait for agent loop to complete
        let agent_result = match agent_task.await {
            Ok(result) => result,
            Err(join_err) => {
                return Err(crate::AgentError::Context(format!(
                    "Agent task panicked: {}",
                    join_err
                )));
            }
        };

        // Wait for interceptor to finish with timeout
        let interceptor_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), interceptor_handle).await;

        // Wait for listener to finish with timeout
        let listener_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), listener_handle).await;

        match interceptor_result {
            Ok(Ok(())) => {}
            Ok(Err(join_err)) => {
                tracing::warn!(%join_err, "Interceptor task panicked");
            }
            Err(_timeout) => {
                tracing::warn!(
                    "Interceptor task timeout after 5s - task may be hanging, proceeding anyway"
                );
            }
        }

        match listener_result {
            Ok(Ok(())) => {}
            Ok(Err(join_err)) => {
                tracing::warn!(%join_err, "Listener task panicked");
            }
            Err(_timeout) => {
                tracing::warn!(
                    "Listener task timeout after 5s - task may be hanging, proceeding anyway"
                );
            }
        }

        agent_result
    }
}

/// Consume LLM stream response, emit streaming events, and accumulate into complete data.
///
/// Returns: (thinking, tool_calls, content, model, usage)
async fn consume_llm_stream(
    mut stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<(String, Vec<vol_llm_core::ToolCall>, String, String, Option<vol_llm_core::TokenUsage>), crate::AgentError> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();
    let mut model = String::new();
    let mut last_usage: Option<vol_llm_core::TokenUsage> = None;

    let mut thinking_started = false;
    let mut content_started = false;

    while let Some(result) = stream.recv().await {
        let event = result.map_err(|e| crate::AgentError::Llm(e))?;

        match event.data {
            StreamEventData::ThinkingDelta { thinking: delta } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::thinking_start()).await;
                    thinking_started = true;
                }
                thinking.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::thinking_delta(delta)).await;
            }
            StreamEventData::ThinkingComplete { thinking: t } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::thinking_start()).await;
                    run_ctx.emit(AgentStreamEvent::thinking_delta(t.clone())).await;
                }
                thinking = t;
                run_ctx.emit(AgentStreamEvent::thinking_complete(thinking.clone())).await;
            }
            StreamEventData::ContentDelta { delta } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::content_start()).await;
                    content_started = true;
                }
                content.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::content_delta(delta)).await;
            }
            StreamEventData::ContentComplete { content: c } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::content_start()).await;
                    run_ctx.emit(AgentStreamEvent::content_delta(c.clone())).await;
                }
                content = c;
                run_ctx.emit(AgentStreamEvent::content_complete(content.clone())).await;
            }
            StreamEventData::ToolCallComplete { tool_call } => {
                tool_calls.push(tool_call);
            }
            StreamEventData::UsageUpdate { usage } => {
                last_usage = Some(usage);
            }
            StreamEventData::ResponseStart { model: m } => {
                model = m;
            }
            StreamEventData::ToolCallArgumentDelta { tool_call_id, tool_name, delta } => {
                run_ctx
                    .emit(AgentStreamEvent::tool_call_argument_delta(
                        tool_call_id.clone(),
                        tool_name.clone(),
                        delta.clone(),
                    ))
                    .await;
            }
            StreamEventData::Error { code, message } => {
                tracing::warn!(%code, %message, "Stream error event received");
            }
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content, model, last_usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{ConversationResponse, FinishReason, Message as CoreMessage, StreamReceiver};

    use crate::react::plugin::PluginRegistry;
    use vol_session::InMemoryEntryStore;

    struct MockLlm;
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider {
            vol_llm_core::LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "mock"
        }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
            &[]
        }
        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
            Ok(ConversationResponse {
                message: CoreMessage::assistant("mock".to_string()),
                model: "mock".to_string(),
                usage: vol_llm_core::TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }
        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
            let (_tx, rx) = tokio::sync::mpsc::channel(10);
            Ok(StreamReceiver::new(rx))
        }
    }

    fn make_config() -> AgentConfig {
        AgentConfig::new(
            Arc::new(MockLlm),
            Arc::new(ToolRegistry::new()),
            Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
        )
    }

    #[test]
    fn test_agent_config_default() {
        let config = make_config();
        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.max_history_messages, 20);
        assert_eq!(config.plugin_registry.plugins().len(), 0);
    }

    #[test]
    fn test_agent_config_custom() {
        let mut config = make_config();
        config.max_iterations = 10;
        config.max_history_messages = 50;
        config.agent_id = "custom_agent".to_string();
        config.working_dir = PathBuf::from("/custom/project");
        assert_eq!(config.max_history_messages, 50);
        assert_eq!(config.agent_id, "custom_agent");
        assert_eq!(config.working_dir, PathBuf::from("/custom/project"));
    }

    #[test]
    fn test_agent_config_with_observability() {
        let mut config = make_config();
        config.agent_id = "test_agent".to_string();
        config.working_dir = PathBuf::from(".");

        assert_eq!(config.agent_id, "test_agent");
        assert_eq!(config.working_dir, PathBuf::from("."));
    }

    #[test]
    fn test_skills_config_register_tool() {
        let skills = SkillsConfig::from_workdir(Path::new("/tmp/test-project"));
        let mut registry = ToolRegistry::new();
        skills.register_tool(&mut registry);
        let defs = registry.definitions();
        assert!(defs.iter().any(|d| d.name == "skill"));
    }

    #[test]
    fn test_skills_config_enhance_context_builder() {
        let skills = SkillsConfig::from_workdir(Path::new("/tmp/test-project"));
        let existing = ContextBuilderBuilder::new(128_000).build();
        let enhanced = skills.enhance_context_builder(&existing);
        let names = enhanced.contributor_names();
        assert!(names.contains(&"skills"));
    }
}
