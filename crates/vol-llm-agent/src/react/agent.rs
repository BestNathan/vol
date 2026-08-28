//! ReAct Agent implementation.

use super::{
    AgentInput, AgentResponse, AgentStreamEvent, PluginDecision, PluginRegistry, RunContext,
};
use crate::react::state::ToolCallRecord;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use vol_llm_context::{
    AttentionAnchor, ContextBuilder, ContextBuilderBuilder, ContextContributor, ContextError,
    ContextMessage, ContributorInfo,
};
use vol_llm_core::capability_overlay::CapabilityOverlay;
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, Message, StreamEventData, StreamReceiver,
    ToolChoice,
};
use vol_llm_mcp::McpManager;
use vol_llm_sandbox::SandboxManager;
use vol_llm_sandbox::SandboxRef;
use vol_llm_skill::{SkillInjector, SkillLoader};
use vol_llm_tool::{ToolConfig, ToolContext, ToolRegistry};
use vol_session::{InMemoryEntryStore, Session, SessionContributor};

/// Agent configuration — single source of truth for ReActAgent.
///
/// Clone is intentionally NOT derived. After construction, config is shared
/// via Arc and external code only gets &AgentConfig references.
#[allow(clippy::type_complexity)]
pub struct AgentConfig {
    // === Declarative definition (optional) ===
    pub def: Option<crate::agent_def::AgentDef>,

    // === Runtime components ===
    pub llm: Arc<dyn vol_llm_core::LLMClient>,
    pub tools: Arc<vol_llm_tool::ToolRegistry>,
    /// Session handle with interior mutability. Read via agent.session(),
    /// write via agent.set_session() (gated by is_running).
    pub(crate) session: std::sync::RwLock<Arc<Session>>,
    pub sandbox: Option<SandboxRef>,
    pub sandbox_manager: Option<Arc<SandboxManager>>,
    pub default_sandbox: Option<String>,
    /// Per-tool configuration (includes sandbox overrides, tool-specific settings).
    pub tool_config: ToolConfig,

    // === Context and plugins ===
    pub(crate) context_builder: RwLock<ContextBuilder>,
    pub plugin_registry: PluginRegistry,

    // === Capability overlays ===
    /// Capability overlay map reference for runtime tool/skill/MCP adjustment.
    pub capability_overlays: Option<
        Arc<
            tokio::sync::RwLock<
                HashMap<(String, String), vol_llm_core::capability_overlay::CapabilityOverlay>,
            >,
        >,
    >,
    /// Shared skill loader — discovers and loads skills from working_dir.
    /// Used by SkillInjector (context) and SkillTool (tool registry).
    pub skill_loader: Arc<SkillLoader>,

    // === MCP ===
    pub mcp_manager: Option<Arc<McpManager>>,

    // === Agent identity ===
    pub agent_id: String,
    /// Working directory. Log paths derive from `{working_dir}/logs/agents/{agent_id}/`.
    pub working_dir: PathBuf,
}

impl AgentConfig {
    /// Create a new builder for AgentConfig.
    pub fn builder() -> super::config_builder::AgentConfigBuilder {
        super::config_builder::AgentConfigBuilder::new()
    }

    /// Add a context contributor.
    #[allow(clippy::unwrap_used)]
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>) {
        self.context_builder
            .write()
            .unwrap()
            .add_contributor(contributor);
    }

    /// List contributor info (for RPC / UI queries).
    #[allow(clippy::unwrap_used)]
    pub async fn contributor_infos(&self) -> Result<Vec<ContributorInfo>, ContextError> {
        let cb = self.context_builder.read().unwrap().clone();
        cb.contributor_infos().await
    }

    /// Get message snapshot from a specific contributor.
    #[allow(clippy::unwrap_used)]
    pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
        let cb = self.context_builder.read().unwrap().clone();
        cb.snapshot_by_name(name).await
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            def: None,
            llm: Arc::new(DefaultLlm),
            tools: Arc::new(vol_llm_tool::ToolRegistry::new()),
            session: std::sync::RwLock::new(Arc::new(Session::new(Arc::new(
                InMemoryEntryStore::new(),
            )))),
            sandbox: None,
            sandbox_manager: None,
            default_sandbox: None,
            tool_config: ToolConfig::new(),
            context_builder: RwLock::new(ContextBuilderBuilder::new(128_000).build()),
            plugin_registry: PluginRegistry::new(),
            capability_overlays: None,
            skill_loader: Arc::new(SkillLoader::new_empty()),
            mcp_manager: None,
            agent_id: generate_agent_id(),
            working_dir: PathBuf::from("."),
        }
    }
}

fn generate_agent_id() -> String {
    format!("agent-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

/// Dummy LLM for Default impl (tests only — will panic if used).
struct DefaultLlm;
#[async_trait::async_trait]
impl LLMClient for DefaultLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider {
        vol_llm_core::LLMProvider::Anthropic
    }
    fn model(&self) -> &str {
        "default"
    }
    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }
    async fn converse(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!(
            "DefaultLlm::converse called — AgentConfig::default() is for struct defaults only"
        )
    }
    async fn converse_stream(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<StreamReceiver> {
        let (_tx, rx) = tokio::sync::mpsc::channel(10);
        Ok(StreamReceiver::new(rx))
    }
}

/// Shared running state — exposed for external status queries.
pub struct RunningState {
    /// true while run_input() is executing.
    pub is_running: std::sync::atomic::AtomicBool,
    /// Current input text (for status display).
    pub current_input: std::sync::RwLock<Option<String>>,
    /// Current run_id (for status display).
    pub current_run_id: std::sync::RwLock<Option<String>>,
}

impl RunningState {
    fn new() -> Self {
        Self {
            is_running: std::sync::atomic::AtomicBool::new(false),
            current_input: std::sync::RwLock::new(None),
            current_run_id: std::sync::RwLock::new(None),
        }
    }
}

/// RAII guard that clears running state on drop (even on panic).
struct RunningGuard<'a> {
    run_state: &'a RunningState,
}

impl Drop for RunningGuard<'_> {
    #[allow(clippy::unwrap_used)]
    fn drop(&mut self) {
        self.run_state
            .is_running
            .store(false, std::sync::atomic::Ordering::Release);
        *self.run_state.current_input.write().unwrap() = None;
        *self.run_state.current_run_id.write().unwrap() = None;
    }
}

/// ReAct Agent — owns config (Arc) and running state.
pub struct ReActAgent {
    config: Arc<AgentConfig>,
    base_tools: Arc<ToolRegistry>,
    skill_loader: Arc<SkillLoader>,
    run_state: Arc<RunningState>,
}

impl ReActAgent {
    /// Create a new ReActAgent from config.
    pub fn new(
        config: AgentConfig,
        base_tools: Arc<ToolRegistry>,
        skill_loader: Arc<SkillLoader>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            base_tools,
            skill_loader,
            run_state: Arc::new(RunningState::new()),
        }
    }

    // ── Read-only access ──

    /// Immutable reference to config.
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    // ── Contributor API ──

    /// Add a context contributor at runtime.
    #[allow(clippy::unwrap_used)]
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>) {
        self.config
            .context_builder
            .write()
            .unwrap()
            .add_contributor(contributor);
    }

    /// List all contributors with metadata (SOT for external queries).
    pub async fn contributors(&self) -> Result<Vec<ContributorInfo>, ContextError> {
        self.config.contributor_infos().await
    }

    /// Get messages from a specific contributor by name.
    pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
        self.config.snapshot_by_name(name).await
    }

    /// Cheap clone of the shared session handle.
    #[allow(clippy::unwrap_used)]
    pub fn session(&self) -> Arc<Session> {
        self.config.session.read().unwrap().clone()
    }

    /// Whether agent is currently executing run_input().
    pub fn is_running(&self) -> bool {
        self.run_state
            .is_running
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Shared running state for external status queries.
    pub fn run_state(&self) -> &Arc<RunningState> {
        &self.run_state
    }

    // ── Public query methods ──

    /// The LLM client used by this agent.
    pub fn llm(&self) -> &Arc<dyn LLMClient> {
        &self.config.llm
    }

    /// Tools resolved for the current session (overlay > AgentDef > global).
    pub fn tools(&self) -> Arc<ToolRegistry> {
        self.resolve_tools(&self.current_session_id())
    }

    /// Skills resolved for the current session (overlay > AgentDef > global).
    pub fn skills(&self) -> SkillInjector {
        self.resolve_skills(&self.current_session_id())
    }

    /// MCP manager resolved for the current session (overlay > AgentDef > global).
    pub fn mcps(&self) -> Arc<McpManager> {
        self.resolve_mcps(&self.current_session_id())
    }

    // ── Internal resolve methods ──

    fn resolve_tools(&self, sid: &str) -> Arc<ToolRegistry> {
        let overlay = self.get_overlay(sid);

        // Tool allowlist: overlay non-empty > def.tools
        let allowed: Option<Vec<&str>> = overlay
            .as_ref()
            .and_then(|o| {
                if o.effective_tools.is_empty() {
                    None
                } else {
                    Some(o.effective_tools.as_slice())
                }
            })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.tools.as_deref()))
            .map(|v| v.iter().map(String::as_str).collect());

        // Blocklist: always from def
        let disallowed: Option<Vec<&str>> = self
            .config
            .def
            .as_ref()
            .and_then(|d| d.disallowed_tools.as_deref())
            .map(|v| v.iter().map(String::as_str).collect());

        let mut filtered = self
            .base_tools
            .filter(allowed.as_deref(), disallowed.as_deref());

        // MCP tool filter: overlay non-empty > def.mcps
        let mcps = overlay
            .as_ref()
            .and_then(|o| {
                if o.effective_mcp_servers.is_empty() {
                    None
                } else {
                    Some(o.effective_mcp_servers.as_slice())
                }
            })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.mcps.as_deref()));
        if let Some(mcp_names) = mcps {
            filtered = Arc::new(filtered.filter_mcp_servers(mcp_names));
        }

        filtered
    }

    fn resolve_skills(&self, sid: &str) -> SkillInjector {
        let overlay = self.get_overlay(sid);

        let filter: Option<Vec<String>> = overlay
            .as_ref()
            .and_then(|o| {
                if o.effective_skills.is_empty() {
                    None
                } else {
                    Some(o.effective_skills.clone())
                }
            })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.skills.clone()));

        SkillInjector::new(self.skill_loader.clone(), AttentionAnchor::Head(1), filter)
    }

    fn resolve_mcps(&self, sid: &str) -> Arc<McpManager> {
        let overlay = self.get_overlay(sid);

        let filter: Option<&[String]> = overlay
            .as_ref()
            .and_then(|o| {
                if o.effective_mcp_servers.is_empty() {
                    None
                } else {
                    Some(o.effective_mcp_servers.as_slice())
                }
            })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.mcps.as_deref()));

        self.config
            .mcp_manager
            .as_ref()
            .map(|m| Arc::new(m.filter(filter)))
            .unwrap_or_else(|| Arc::new(McpManager::empty()))
    }

    fn get_overlay(&self, sid: &str) -> Option<CapabilityOverlay> {
        self.config
            .capability_overlays
            .as_ref()
            .and_then(|map| map.try_read().ok())
            .and_then(|guard| {
                guard
                    .get(&(self.config.agent_id.clone(), sid.to_string()))
                    .cloned()
            })
    }

    #[allow(clippy::unwrap_used)]
    fn current_session_id(&self) -> String {
        self.config.session.read().unwrap().id.clone()
    }

    // ── Mutation (gated by is_running) ──

    /// Replace the session. Rejected if agent is running.
    /// Also replaces the SessionContributor with a new one pointing to the new session.
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub fn set_session(&self, session: Arc<Session>) -> Result<(), AgentBusyError> {
        if self.is_running() {
            return Err(AgentBusyError {
                agent_id: self.config.agent_id.clone(),
            });
        }
        let max_history = self
            .config
            .def
            .as_ref()
            .and_then(|d| d.max_history_messages)
            .unwrap_or(50);
        let session_contributor = Box::new(SessionContributor::new(
            Arc::new(tokio::sync::Mutex::new((*session).clone())),
            max_history,
            AttentionAnchor::Tail(0),
        ));
        *self
            .config
            .session
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = session;
        self.config
            .context_builder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace_contributor("session", session_contributor);
        Ok(())
    }

    // ── Builder-style (consuming self, initial setup only) ──

    /// Set the sandbox for tool execution (builder pattern, consumes self).
    #[allow(clippy::expect_used)]
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        Arc::get_mut(&mut self.config)
            .expect("with_sandbox called after config was shared")
            .sandbox = Some(sandbox);
        self
    }

    // ── Execution ──

    /// Run ReAct loop and return the final response.
    pub async fn run(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
        self.run_input(AgentInput::text(user_input)).await
    }

    #[tracing::instrument(skip(self, input), fields(agent.run_id))]
    #[allow(clippy::unwrap_used, clippy::cast_possible_truncation)]
    pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, crate::AgentError> {
        // Re-entrancy guard
        if self
            .run_state
            .is_running
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(crate::AgentError::AlreadyRunning);
        }

        // Ensure all MCP servers are connected before starting the run.
        if let Some(ref mcp) = self.config.mcp_manager {
            mcp.reconnect_all().await;
        }

        let user_content = input
            .to_message_content()
            .map_err(|e| crate::AgentError::InvalidInput(e.to_string()))?;
        let user_input = input.display_text();
        let run_id = input
            .run_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        tracing::Span::current().record("agent.run_id", &run_id);

        // Set status metadata
        *self
            .run_state
            .current_input
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(user_input.clone());
        *self
            .run_state
            .current_run_id
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(run_id.clone());

        // RAII guard: clears running state on drop (even on panic)
        let _guard = RunningGuard {
            run_state: &self.run_state,
        };

        let (mut run_ctx, plugin_rx) =
            RunContext::new(run_id.clone(), user_input.clone(), self.config.clone());

        // Resolve tools and skills once at run start (overlay > AgentDef > global).
        let sid = run_ctx.session_id.clone();
        let resolved_tools = self.resolve_tools(&sid);
        let resolved_skills = self.resolve_skills(&sid);

        // Set the pre-resolved (filtered) tool registry on the run context.
        run_ctx.tools = resolved_tools;

        // Replace the skills contributor with the resolved SkillInjector.
        run_ctx.replace_contributor("skills", Box::new(resolved_skills));

        for (key, value) in input.metadata {
            run_ctx.data.write().await.insert(key, value);
        }

        // Persist user message to session so it's available via SessionContributor.
        let user_msg = Message::user(user_content);
        run_ctx.add_message(user_msg).await.map_err(|e| {
            crate::AgentError::SessionError(format!("Failed to persist user message: {e}"))
        })?;

        // === Phase 2: Context is built per-iteration via get_context ===

        // === Phase 2.6: Spawn listener and interceptor tasks ===
        use super::plugin_stream::{run_interceptor_loop, spawn_listener_tasks};

        let plugins = self.config.plugin_registry.plugins().to_vec();
        let mut listener_set = spawn_listener_tasks(plugins, run_ctx.clone());

        let interceptor_plugins = self.config.plugin_registry.plugins().to_vec();
        let interceptor_ctx = run_ctx.without_plugin_event_tx();
        let interceptor_handle = tokio::spawn(async move {
            run_interceptor_loop(plugin_rx, interceptor_plugins, interceptor_ctx).await;
        });

        let mut shutdown_event_tx = run_ctx.event_tx.clone();

        // === Phase 3: Spawn agent loop task and await it ===
        let llm = self.config.llm.clone();
        let user_input = user_input.clone();
        let sandbox = self.config.sandbox.clone();
        let agent_def = self.config.def.clone();
        let agent_task = tokio::spawn(async move {
            let max_iterations = run_ctx.max_iterations();
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

                if iteration > max_iterations {
                    run_ctx
                        .emit(AgentStreamEvent::max_iterations_reached(
                            iteration,
                            max_iterations,
                        ))
                        .await;

                    let reason = format!("Max iterations ({max_iterations}) reached");
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                        .await;
                    return Err(crate::AgentError::MaxIterationsReached {
                        max: max_iterations,
                    });
                }

                // Reason phase - call LLM with streaming
                let tools_defs = run_ctx.tools.definitions();

                // Get messages from ctx (not local variable)
                let messages = run_ctx.get_context().await?;

                // Emit LLMCallStart before calling the LLM
                run_ctx
                    .emit(AgentStreamEvent::llm_call_start(
                        iteration,
                        messages.clone(),
                    ))
                    .await;

                let request = ConversationRequest::with_history(None, messages)
                    .with_tools(tools_defs)
                    .with_tool_choice(ToolChoice::Auto);

                let llm_stream = match llm.converse_stream(request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        run_ctx
                            .emit(AgentStreamEvent::llm_call_error(e.to_string()))
                            .await;
                        run_ctx
                            .emit(AgentStreamEvent::agent_aborted(format!(
                                "LLM request failed: {e}"
                            )))
                            .await;
                        return Err(crate::AgentError::Llm(e));
                    }
                };

                // Consume LLM stream — emits Thinking/Content streaming events internally
                let (thinking, tool_calls, content, model, usage) =
                    match consume_llm_stream(llm_stream, &run_ctx).await {
                        Ok(data) => data,
                        Err(e) => {
                            run_ctx
                                .emit(AgentStreamEvent::llm_call_error(e.to_string()))
                                .await;
                            run_ctx
                                .emit(AgentStreamEvent::agent_aborted(format!(
                                    "LLM stream failed: {e}"
                                )))
                                .await;
                            return Err(e);
                        }
                    };

                // Emit LLMCallComplete with the model and token usage
                run_ctx
                    .emit(AgentStreamEvent::llm_call_complete(model, usage))
                    .await;

                // Record reasoning step
                if !thinking.is_empty() {
                    run_ctx.record_reasoning_step(thinking.clone(), None).await;
                }

                // Check if tool calls
                if !tool_calls.is_empty() {
                    // IMPORTANT: Add assistant message with tool calls to history
                    let assistant_message = {
                        let msg = if !content.is_empty() {
                            Message::assistant_with_tools(content.clone(), tool_calls.clone())
                        } else {
                            Message::assistant_with_tools(
                                "Calling tools to get information.".to_string(),
                                tool_calls.clone(),
                            )
                        };
                        if !thinking.is_empty() {
                            msg.with_thinking(thinking.clone())
                        } else {
                            msg
                        }
                    };
                    run_ctx.add_message(assistant_message).await?;

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
                                #[allow(clippy::cast_possible_truncation)]
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;

                                run_ctx
                                    .emit(AgentStreamEvent::tool_call_skipped(
                                        call.id.clone(),
                                        call.name.clone(),
                                        "Plugin skipped".to_string(),
                                        Some(duration_ms),
                                    ))
                                    .await;

                                continue;
                            }
                            PluginDecision::Abort(reason) => {
                                run_ctx
                                    .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                                    .await;
                                return Err(crate::AgentError::Context(reason));
                            }
                        }

                        // Resolve sandbox:
                        //   1. ToolConfig.get_sandbox(tool_name) — per-tool override
                        //   2. AgentDef.sandbox — agent default
                        //   3. Manager default ("local")
                        let sandbox_ref = if let Some(ref manager) = run_ctx.config.sandbox_manager
                        {
                            let sandbox_name = run_ctx
                                .config
                                .tool_config
                                .get_sandbox(&call.name)
                                .or_else(|| run_ctx.config.default_sandbox.clone())
                                .unwrap_or_else(|| "local".to_string());
                            match manager.acquire_by_name(&sandbox_name).await {
                                Some(sb) => sb,
                                None => manager.default_tmp().await,
                            }
                        } else {
                            match &sandbox {
                                Some(sb) => sb.clone(),
                                None => {
                                    // Use the agent's working_dir so tools (glob,
                                    // read_file, etc.) can access actual project files
                                    // instead of an empty temp directory.
                                    let root = run_ctx.config.working_dir.clone();
                                    Arc::new(vol_llm_sandbox::local::LocalSandbox::new(Some(root)))
                                }
                            }
                        };
                        let mut tool_ctx = ToolContext::default().with_sandbox(sandbox_ref);
                        if let Some(ref def) = agent_def {
                            tool_ctx = tool_ctx.with_agent_def(def.clone());
                        }
                        let result = match run_ctx.execute_tool(call, &tool_ctx).await {
                            Ok(r) => r,
                            Err(e) => {
                                #[allow(clippy::cast_possible_truncation)]
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;
                                run_ctx
                                    .emit(AgentStreamEvent::tool_call_error(
                                        call.id.clone(),
                                        call.name.clone(),
                                        e.to_string(),
                                        Some(duration_ms),
                                    ))
                                    .await;

                                run_ctx
                                    .record_tool_call(ToolCallRecord {
                                        tool_name: call.name.clone(),
                                        arguments: call.arguments.clone(),
                                        result: format!("Error: {e}"),
                                        iteration,
                                        success: false,
                                    })
                                    .await;

                                // Add error message to session — LLM sees it on next turn
                                let error_content = format!("Tool '{}' error: {}", call.name, e);
                                run_ctx
                                    .add_message(Message::tool(error_content, call.id.clone()))
                                    .await?;

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
                        #[allow(clippy::cast_possible_truncation)]
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
                        run_ctx
                            .add_message(Message::tool(result.content.clone(), call.id.clone()))
                            .await?;

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
                let mut final_msg = Message::assistant(content.clone());
                if !thinking.is_empty() {
                    final_msg = final_msg.with_thinking(thinking.clone());
                }
                run_ctx.add_message(final_msg).await?;

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
                    .emit(AgentStreamEvent::agent_complete_with_response(
                        response_json,
                    ))
                    .await;

                return Ok(response);
            }
        });

        // Wait for agent loop to complete
        let agent_result = match agent_task.await {
            Ok(result) => result,
            Err(join_err) => {
                return Err(crate::AgentError::Context(format!(
                    "Agent task panicked: {join_err}"
                )));
            }
        };

        if let Err(join_err) = interceptor_handle.await {
            tracing::warn!(%join_err, "Interceptor task panicked");
        }

        shutdown_event_tx.take();

        while let Some(result) = listener_set.join_next().await {
            if let Err(e) = result {
                tracing::warn!(%e, "Listener task panicked");
            }
        }

        // Disconnect MCP manager
        if let Some(ref mcp_manager) = self.config.mcp_manager {
            mcp_manager.disconnect().await.ok();
        }

        agent_result
    }
}

/// Returned when mutation is attempted while agent is running.
#[derive(Debug, thiserror::Error)]
#[error("agent {agent_id} is currently running — state mutation not allowed")]
pub struct AgentBusyError {
    pub agent_id: String,
}

/// Consume LLM stream response, emit streaming events, and accumulate into complete data.
///
/// Returns: (thinking, tool_calls, content, model, usage)
async fn consume_llm_stream(
    mut stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<
    (
        String,
        Vec<vol_llm_core::ToolCall>,
        String,
        String,
        Option<vol_llm_core::TokenUsage>,
    ),
    crate::AgentError,
> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();
    let mut model = String::new();
    let mut last_usage: Option<vol_llm_core::TokenUsage> = None;

    let mut thinking_started = false;
    let mut content_started = false;

    while let Some(result) = stream.recv().await {
        let event = result.map_err(crate::AgentError::Llm)?;

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
                    run_ctx
                        .emit(AgentStreamEvent::thinking_delta(t.clone()))
                        .await;
                }
                thinking = t;
                run_ctx
                    .emit(AgentStreamEvent::thinking_complete(thinking.clone()))
                    .await;
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
                    run_ctx
                        .emit(AgentStreamEvent::content_delta(c.clone()))
                        .await;
                }
                content = c;
                run_ctx
                    .emit(AgentStreamEvent::content_complete(content.clone()))
                    .await;
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
            StreamEventData::ToolCallArgumentDelta {
                tool_call_id,
                tool_name,
                delta,
            } => {
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
    use vol_llm_core::{
        ConversationResponse, FinishReason, Message as CoreMessage, StreamReceiver,
    };

    use crate::agent_def::AgentDef;
    use vol_llm_skill::SkillDef;
    use vol_llm_tool::{ExecutableTool, ToolRegistry, ToolResult, ToolResultType, ToolSensitivity};
    use vol_session::InMemoryEntryStore;

    /// Minimal tool for registry tests.
    struct DummyTool {
        name: &'static str,
    }
    impl DummyTool {
        fn new(name: &'static str) -> Self {
            Self { name }
        }
    }
    #[async_trait::async_trait]
    impl ExecutableTool for DummyTool {
        fn name(&self) -> &'static str {
            self.name
        }
        fn description(&self) -> &'static str {
            "dummy"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
            ToolSensitivity::Safe
        }
        async fn execute(
            &self,
            _args: &serde_json::Value,
            _context: &vol_llm_tool::ToolContext,
        ) -> ToolResultType<ToolResult> {
            Ok(ToolResult::success("ok"))
        }
    }

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
        async fn converse(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            Ok(ConversationResponse {
                message: CoreMessage::assistant("mock".to_string()),
                model: "mock".to_string(),
                usage: vol_llm_core::TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }
        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<StreamReceiver> {
            let (_tx, rx) = tokio::sync::mpsc::channel(10);
            Ok(StreamReceiver::new(rx))
        }
    }

    fn make_config() -> AgentConfig {
        AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(Arc::new(ToolRegistry::new()))
            .with_session(Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))))
            .build()
            .expect("Test config build failed")
    }

    #[test]
    fn test_agent_config_default() {
        let config = make_config();
        assert!(config.def.is_none());
        assert_eq!(config.plugin_registry.plugins().len(), 0);
    }

    #[test]
    fn test_agent_config_with_def() {
        let def = AgentDef::new("test-agent", "You are a test agent.")
            .with_type("test-runner")
            .with_max_iterations(10)
            .with_max_history_messages(50);
        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_def(def)
            .build()
            .unwrap();
        assert_eq!(config.def.as_ref().unwrap().name, "test-agent");
        assert_eq!(config.def.as_ref().unwrap().max_iterations, Some(10));
        assert_eq!(config.def.as_ref().unwrap().max_history_messages, Some(50));
    }

    #[test]
    fn test_agent_config_fields() {
        let config = AgentConfig {
            agent_id: "test_agent".to_string(),
            working_dir: PathBuf::from("."),
            ..Default::default()
        };

        assert_eq!(config.agent_id, "test_agent");
        assert_eq!(config.working_dir, PathBuf::from("."));
    }

    #[tokio::test]
    async fn test_resolve_tools_no_def_no_overlay_returns_all() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("read"));
        let base_tools = Arc::new(registry);

        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .build()
            .unwrap();
        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let tools = agent.resolve_tools("any-session");
        let names = tools
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect::<Vec<_>>();
        assert!(names.contains(&"bash".to_string()));
        assert!(names.contains(&"read".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_tools_with_def_allowlist() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("read"));
        let base_tools = Arc::new(registry);

        let def = AgentDef::new("test", "prompt").with_tools(vec!["bash".into()]);

        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();
        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let tools = agent.resolve_tools("any-session");
        let names: Vec<String> = tools.definitions().iter().map(|d| d.name.clone()).collect();
        assert_eq!(names, vec!["bash"]);
    }

    #[tokio::test]
    async fn test_resolve_tools_def_disallowed_blocklist() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("read"));
        let base_tools = Arc::new(registry);

        let def = AgentDef::new("test", "prompt").with_disallowed_tools(vec!["read".into()]);

        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();
        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let tools = agent.resolve_tools("any-session");
        let names: Vec<String> = tools.definitions().iter().map(|d| d.name.clone()).collect();
        assert_eq!(names, vec!["bash"]);
    }

    #[tokio::test]
    async fn test_resolve_tools_overlay_takes_precedence_over_def() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("read"));
        registry.register(DummyTool::new("write"));
        let base_tools = Arc::new(registry);

        let def = AgentDef::new("test", "prompt").with_tools(vec!["bash".into(), "read".into()]);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();

        // Overlay narrows to ["read"] — must win over def allowlist ["bash", "read"].
        let overlay = CapabilityOverlay::new(vec!["read".into()], vec![], vec![]);
        let mut map = HashMap::new();
        map.insert(
            (config.agent_id.clone(), "overlay-session".to_string()),
            overlay,
        );
        config.capability_overlays = Some(Arc::new(tokio::sync::RwLock::new(map)));

        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let tools = agent.resolve_tools("overlay-session");
        let names: Vec<String> = tools.definitions().iter().map(|d| d.name.clone()).collect();
        assert_eq!(names, vec!["read"]);
    }

    #[tokio::test]
    async fn test_resolve_skills_no_def_no_overlay_returns_all() {
        let loader = SkillLoader::new_empty();
        let mut skill = SkillDef::new("test-skill", "# T").with_description("Test");
        skill.id = "user:test-skill".into();
        loader.register(skill).await;
        let skill_loader = Arc::new(loader);

        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .build()
            .unwrap();
        let agent = ReActAgent::new(config, base_tools, skill_loader);

        let injector = agent.resolve_skills("any-session");
        let names = injector.skill_names().await;
        assert!(names.contains(&"test-skill".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_skills_def_allowlist_filters() {
        let loader = SkillLoader::new_empty();
        let mut skill = SkillDef::new("test-skill", "# T").with_description("Test");
        skill.id = "user:test-skill".into();
        loader.register(skill).await;
        let skill_loader = Arc::new(loader);

        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let mut def = AgentDef::new("test", "prompt");
        def.skills = Some(vec!["other-skill".into()]);

        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();
        let agent = ReActAgent::new(config, base_tools, skill_loader);

        let injector = agent.resolve_skills("any-session");
        let names = injector.skill_names().await;
        assert!(!names.contains(&"test-skill".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_mcps_no_manager_returns_empty() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .build()
            .unwrap();
        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let mcps = agent.resolve_mcps("any-session");
        assert!(mcps.server_status().is_empty());
    }

    fn mcp_manager_with(names: &[&str]) -> Arc<McpManager> {
        Arc::new(McpManager::new(
            names
                .iter()
                .map(|name| vol_llm_mcp::config::McpServerConfig {
                    name: (*name).to_string(),
                    transport: vol_llm_mcp::config::McpTransport::Http {
                        url: "http://127.0.0.1:1".to_string(),
                        headers: None,
                        env: Default::default(),
                    },
                })
                .collect(),
        ))
    }

    #[tokio::test]
    async fn test_resolve_tools_overlay_empty_tools_falls_back_to_def() {
        // Overlay exists but carries no tool restriction -> the AgentDef
        // allowlist applies instead of the full global registry.
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("read"));
        let base_tools = Arc::new(registry);

        let def = AgentDef::new("test", "prompt").with_tools(vec!["bash".into()]);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();

        let overlay = CapabilityOverlay::new(vec![], vec![], vec![]);
        let mut map = HashMap::new();
        map.insert(
            (config.agent_id.clone(), "overlay-session".to_string()),
            overlay,
        );
        config.capability_overlays = Some(Arc::new(tokio::sync::RwLock::new(map)));

        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let names: Vec<String> = agent
            .resolve_tools("overlay-session")
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        assert_eq!(names, vec!["bash"]);
    }

    #[tokio::test]
    async fn test_resolve_tools_mcp_filter_uses_overlay_over_def() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("mcp__docs_rs__search"));
        registry.register(DummyTool::new("mcp__weather__forecast"));
        let base_tools = Arc::new(registry);

        let mut def = AgentDef::new("test", "prompt");
        def.mcps = Some(vec!["docs_rs".into()]);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();

        // Overlay narrows MCP servers to weather — must win over def's docs_rs.
        let overlay = CapabilityOverlay::new(vec![], vec![], vec!["weather".into()]);
        let mut map = HashMap::new();
        map.insert(
            (config.agent_id.clone(), "overlay-session".to_string()),
            overlay,
        );
        config.capability_overlays = Some(Arc::new(tokio::sync::RwLock::new(map)));

        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let mut names: Vec<String> = agent
            .resolve_tools("overlay-session")
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["bash", "mcp__weather__forecast"]);
    }

    #[tokio::test]
    async fn test_resolve_tools_def_mcps_filter_mcp_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("mcp__docs_rs__search"));
        registry.register(DummyTool::new("mcp__weather__forecast"));
        let base_tools = Arc::new(registry);

        let mut def = AgentDef::new("test", "prompt");
        def.mcps = Some(vec!["docs_rs".into()]);

        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();
        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let mut names: Vec<String> = agent
            .resolve_tools("any-session")
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        names.sort();
        // Non-MCP tools are kept; only the allowed MCP server's tools remain.
        assert_eq!(names, vec!["bash", "mcp__docs_rs__search"]);
    }

    #[tokio::test]
    async fn test_resolve_skills_overlay_takes_precedence_over_def() {
        let loader = SkillLoader::new_empty();
        let mut skill = SkillDef::new("test-skill", "# T").with_description("Test");
        skill.id = "user:test-skill".into();
        loader.register(skill).await;
        let skill_loader = Arc::new(loader);

        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let mut def = AgentDef::new("test", "prompt");
        def.skills = Some(vec!["test-skill".into()]);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();

        // Overlay's skill allowlist ["other-skill"] replaces the def allowlist.
        let overlay = CapabilityOverlay::new(vec![], vec!["other-skill".into()], vec![]);
        let mut map = HashMap::new();
        map.insert(
            (config.agent_id.clone(), "overlay-session".to_string()),
            overlay,
        );
        config.capability_overlays = Some(Arc::new(tokio::sync::RwLock::new(map)));

        let agent = ReActAgent::new(config, base_tools, skill_loader);

        let names = agent.resolve_skills("overlay-session").skill_names().await;
        assert!(!names.contains(&"test-skill".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_mcps_def_allowlist_filters_servers() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let mut def = AgentDef::new("test", "prompt");
        def.mcps = Some(vec!["docs_rs".into()]);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();
        config.mcp_manager = Some(mcp_manager_with(&["docs_rs", "weather"]));

        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let mcps = agent.resolve_mcps("any-session");
        let mut names: Vec<String> = mcps.server_status().into_keys().collect();
        names.sort();
        assert_eq!(names, vec!["docs_rs"]);
    }

    #[tokio::test]
    async fn test_resolve_mcps_overlay_takes_precedence_over_def() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let mut def = AgentDef::new("test", "prompt");
        def.mcps = Some(vec!["docs_rs".into()]);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();
        config.mcp_manager = Some(mcp_manager_with(&["docs_rs", "weather"]));

        // Overlay narrows MCP servers to weather — must win over def's docs_rs.
        let overlay = CapabilityOverlay::new(vec![], vec![], vec!["weather".into()]);
        let mut map = HashMap::new();
        map.insert(
            (config.agent_id.clone(), "overlay-session".to_string()),
            overlay,
        );
        config.capability_overlays = Some(Arc::new(tokio::sync::RwLock::new(map)));

        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let mcps = agent.resolve_mcps("overlay-session");
        let mut names: Vec<String> = mcps.server_status().into_keys().collect();
        names.sort();
        assert_eq!(names, vec!["weather"]);
    }

    #[tokio::test]
    async fn test_resolve_mcps_overlay_empty_falls_back_to_def() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let mut def = AgentDef::new("test", "prompt");
        def.mcps = Some(vec!["docs_rs".into()]);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_def(def)
            .build()
            .unwrap();
        config.mcp_manager = Some(mcp_manager_with(&["docs_rs", "weather"]));

        // Overlay with empty MCP list -> falls through to the def allowlist.
        let overlay = CapabilityOverlay::new(vec![], vec![], vec![]);
        let mut map = HashMap::new();
        map.insert(
            (config.agent_id.clone(), "overlay-session".to_string()),
            overlay,
        );
        config.capability_overlays = Some(Arc::new(tokio::sync::RwLock::new(map)));

        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let mcps = agent.resolve_mcps("overlay-session");
        let mut names: Vec<String> = mcps.server_status().into_keys().collect();
        names.sort();
        assert_eq!(names, vec!["docs_rs"]);
    }

    #[tokio::test]
    async fn test_session_accessors_apply_overlay_for_current_session() {
        // tools()/skills()/mcps() resolve against the CURRENT session id —
        // an overlay keyed by that session must apply through the accessor.
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        registry.register(DummyTool::new("read"));
        let base_tools = Arc::new(registry);

        let mut config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(base_tools.clone())
            .with_session(Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))))
            .build()
            .unwrap();
        config.mcp_manager = Some(mcp_manager_with(&["docs_rs", "weather"]));

        let session_id = config.session.read().unwrap().id.clone();

        // Overlay keyed by the session id narrows tools to ["read"] and MCPs to ["weather"].
        let overlay = CapabilityOverlay::new(vec!["read".into()], vec![], vec!["weather".into()]);
        let mut map = HashMap::new();
        map.insert((config.agent_id.clone(), session_id), overlay);
        config.capability_overlays = Some(Arc::new(tokio::sync::RwLock::new(map)));

        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        let tool_names: Vec<String> = agent
            .tools()
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        assert_eq!(tool_names, vec!["read"]);

        let mcp_names: Vec<String> = agent.mcps().server_status().into_keys().collect();
        assert_eq!(mcp_names, vec!["weather"]);

        // Skills accessor works too (no skills registered -> empty).
        assert!(agent.skills().skill_names().await.is_empty());
    }

    #[tokio::test]
    async fn test_llm_returns_config_llm() {
        let llm: Arc<dyn LLMClient> = Arc::new(MockLlm);
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("bash"));
        let base_tools = Arc::new(registry);

        let config = AgentConfig::builder()
            .with_llm(llm.clone())
            .with_tools(base_tools.clone())
            .build()
            .unwrap();
        let skills = Arc::new(SkillLoader::new_empty());
        let agent = ReActAgent::new(config, base_tools, skills);

        assert!(Arc::ptr_eq(agent.llm(), &llm));
    }
}
