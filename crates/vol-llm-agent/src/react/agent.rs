//! ReAct Agent implementation.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use vol_llm_core::{LLMClient, Message, ConversationRequest, ToolChoice, StreamEventData, StreamReceiver};
use vol_llm_tool::ToolContext;
use tracing::{info, debug};
use super::{
    AgentResponse, AgentStreamEvent, AgentStreamReceiver, PluginRegistry, RunContext,
    PluginDecision,
};
use crate::session::Session;
use crate::prompt_context::PromptContext;

/// Guard struct that aborts a JoinHandle when dropped.
///
/// This ensures that spawned listener tasks are cleaned up on all exit paths,
/// including early returns, breaks, and panics.
struct ListenerGuard {
    handle: Option<JoinHandle<()>>,
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

/// Agent configuration
#[derive(Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub prompt_context: PromptContext,
    pub verbose: bool,
    pub plugin_registry: PluginRegistry,

    // Observability fields
    pub agent_id: String,
    pub log_base_path: PathBuf,
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

impl Default for AgentConfig {
    fn default() -> Self {
        use crate::prompt_context::PromptTemplate;

        let template = PromptTemplate::new("default", "You are a helpful assistant.");
        let prompt_context = PromptContext::new(template);

        Self {
            max_iterations: 5,
            max_history_messages: 20,
            prompt_context,
            verbose: false,
            plugin_registry: PluginRegistry::new(),
            agent_id: generate_agent_id(),
            log_base_path: PathBuf::from("logs/agents"),
        }
    }
}

/// ReAct Agent
pub struct ReActAgent {
    llm: Arc<dyn LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    config: AgentConfig,
    session: Arc<Session>,
}

impl ReActAgent {
    /// Create agent builder
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    pub fn new(llm: Arc<dyn LLMClient>, tools: Arc<vol_llm_tool::ToolRegistry>, config: AgentConfig, session: Arc<Session>) -> Self {
        Self { llm, tools, config, session }
    }

    /// Create agent with new session
    pub fn with_new_session(&self, session_id: String) -> Self {
        use crate::session::{InMemorySessionStore, InMemoryMessageStore};

        let new_session = Arc::new(Session::new(
            session_id,
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        ));
        Self {
            session: new_session,
            llm: self.llm.clone(),
            tools: self.tools.clone(),
            config: self.config.clone(),
        }
    }

    /// Run ReAct loop with streaming events
    pub async fn run(
        &self,
        user_input: &str,
        context: ToolContext,
    ) -> Result<AgentStreamReceiver, crate::AgentError> {
        // === Phase 1: Generate run_id and create RunContext ===
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());

        let tools = self.tools.clone();
        let config = self.config.clone();
        let session = self.session.clone();

        let (run_ctx, plugin_rx) = RunContext::new(
            run_id.clone(),
            user_input.to_string(),
            self.session.id.clone(),
            session,
            tools,
            config,
        );

        // === Phase 1.5: Run log cleanup (best effort, non-blocking) ===
        let log_base_path = self.config.log_base_path.clone();
        let agent_id = self.config.agent_id.clone();
        tokio::spawn(async move {
            let agent_path = log_base_path.join(&agent_id);
            if let Err(e) = crate::observability::cleanup_old_logs(&agent_path).await {
                tracing::warn!(agent_id = %agent_id, error = %e, "Log cleanup failed");
            }
        });

        // === Phase 2: Initialize messages (call once before loop) ===
        run_ctx.init_messages().await?;

        // === Phase 2.5: Spawn listener and interceptor tasks ===
        use super::plugin_stream::{spawn_listener_task, run_interceptor_loop};

        let listener_handle = spawn_listener_task(
            self.config.plugin_registry.plugins().to_vec(),
            run_ctx.clone(),
        );

        // Spawn interceptor loop task - receives from plugin_rx channel
        let interceptor_run_ctx = run_ctx.clone();
        let interceptor_plugins = self.config.plugin_registry.plugins().to_vec();
        tokio::spawn(async move {
            run_interceptor_loop(plugin_rx, interceptor_plugins, interceptor_run_ctx).await;
        });

        // === Phase 3: Clone for spawned task ===
        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let config = self.config.clone();
        let _session = self.session.clone();
        let user_input = user_input.to_string();
        let _run_id_for_tracing = run_id.clone();

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            // Use a Drop guard to ensure the listener is cleaned up on all exit paths
            let _guard = ListenerGuard { handle: Some(listener_handle) };

            // === Emit and intercept AgentStart ===
            let start_event = AgentStreamEvent::AgentStart {
                input: user_input.clone()
            };
            run_ctx.emit(start_event.clone()).await;

            match run_ctx.intercept(&start_event).await {
                Ok(PluginDecision::Continue) => {
                    // Continue with normal flow
                }
                Ok(PluginDecision::Skip) => {
                    // Skip AgentStart event but continue with normal flow
                    // Skip only affects the current event, not the entire run
                }
                Ok(PluginDecision::Abort(reason)) => {
                    run_ctx.emit(AgentStreamEvent::AgentAborted { reason: reason.clone() }).await;
                    let _ = tx.send(Err(crate::AgentError::Context(reason))).await;
                    return;
                }
                Err(e) => {
                    // Plugin channel error - log and continue (plugins are optional)
                    debug!("Plugin intercept error (plugins may not be wired up yet): {}", e);
                }
            }

            loop {
                // Increment iteration via ctx
                run_ctx.next_iteration();
                let iteration = run_ctx.current_iteration();

                if iteration > config.max_iterations {
                    // Emit max iterations reached event
                    let reason = format!("Max iterations ({}) reached", config.max_iterations);
                    run_ctx.emit(AgentStreamEvent::AgentAborted { reason: reason.clone() }).await;

                    let _ = tx.send(Err(crate::AgentError::MaxIterationsReached {
                        max: config.max_iterations
                    })).await;
                    break;
                }

                if config.verbose {
                    info!("Iteration {}", iteration);
                }

                // Reason phase - call LLM with streaming
                let tools_defs = tools.definitions();

                // Get messages from ctx (not local variable)
                let messages = run_ctx.get_messages().await;

                let request = ConversationRequest::with_history(None, messages)
                    .with_tools(tools_defs)
                    .with_tool_choice(ToolChoice::Auto);

                let llm_stream = match llm.converse_stream(request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        let _ = tx.send(Err(crate::AgentError::Llm(e))).await;
                        break;
                    }
                };

                // Consume LLM stream and accumulate events
                let (thinking, tool_calls, content) = match consume_llm_stream(llm_stream).await {
                    Ok(data) => data,
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                };

                // Emit ThinkingComplete if we have thinking content
                if !thinking.is_empty() {
                    let thinking_event = AgentStreamEvent::ThinkingComplete { thinking };
                    run_ctx.emit(thinking_event.clone()).await;
                    let _ = tx.send(Ok(thinking_event)).await;
                }

                // Check if tool calls
                if !tool_calls.is_empty() {
                    debug!("Tool calls: {:?}", tool_calls);

                    // Act phase - execute tools
                    for call in &tool_calls {
                        info!("Executing tool: {} with args: {}", call.name, call.arguments);

                        // === Emit and intercept ToolCallBegin ===
                        let tool_event = AgentStreamEvent::ToolCallBegin {
                            tool_name: call.name.clone(),
                            arguments: call.arguments.clone(),
                        };
                        run_ctx.emit(tool_event.clone()).await;

                        let tool_decision = match run_ctx.intercept(&tool_event).await {
                            Ok(decision) => decision,
                            Err(e) => {
                                debug!("Plugin intercept error: {}", e);
                                PluginDecision::Continue
                            }
                        };

                        match tool_decision {
                            PluginDecision::Continue => {
                                // Execute tool
                            }
                            PluginDecision::Skip => {
                                // Skip this tool, continue to next
                                debug!("Plugin intercepted to skip tool: {}", call.name);
                                continue;
                            }
                            PluginDecision::Abort(reason) => {
                                run_ctx.emit(AgentStreamEvent::AgentAborted { reason: reason.clone() }).await;
                                let _ = tx.send(Err(crate::AgentError::Context(reason))).await;
                                break;
                            }
                        }

                        // Execute tool
                        let result = match tools.execute(call, &context).await {
                            Ok(r) => r,
                            Err(e) => {
                                let _ = tx.send(Err(crate::AgentError::ToolExecution {
                                    tool: call.name.clone(),
                                    error: e.to_string(),
                                })).await;
                                break;
                            }
                        };

                        info!("Tool {} returned: {}", call.name, result.content);

                        // Send ToolCallComplete
                        let _ = tx.send(Ok(AgentStreamEvent::ToolCallComplete {
                            tool_name: call.name.clone(),
                            result: result.content.clone(),
                        })).await;

                        // Add tool result to ctx (syncs to session automatically)
                        if let Err(e) = run_ctx.add_message(Message::tool(result.content.clone(), call.id.clone())).await {
                            let _ = tx.send(Err(crate::AgentError::from(e))).await;
                            break;
                        }

                        // Clear current tool calls for next iteration
                        run_ctx.clear_current_tool_calls().await;
                    }

                    // Send IterationComplete
                    let _ = tx.send(Ok(AgentStreamEvent::IterationComplete {
                        iteration,
                        tool_calls: tool_calls.clone(),
                        final_answer: None,
                    })).await;

                    // Continue to next iteration
                    continue;
                }

                // No tool calls - we have final answer
                // Send IterationComplete with final answer
                let _ = tx.send(Ok(AgentStreamEvent::IterationComplete {
                    iteration,
                    tool_calls: Vec::new(),
                    final_answer: Some(content.clone()),
                })).await;

                // Save assistant response to session via ctx (user input already saved in init_messages)
                if let Err(e) = run_ctx.add_message(Message::assistant(content.clone())).await {
                    let _ = tx.send(Err(crate::AgentError::from(e))).await;
                    break;
                }

                // === Emit AgentComplete ===
                let response = AgentResponse {
                    content,
                    reasoning: String::new(),
                    iterations: iteration,
                    tool_calls,
                };

                let complete_event = AgentStreamEvent::AgentComplete { response: response.clone() };
                run_ctx.emit(complete_event.clone()).await;
                let _ = tx.send(Ok(complete_event)).await;

                // Guard will abort listener on drop - no manual cleanup needed
                break;
            }
        });

        // Return the raw stream receiver - all plugin intercept/listen logic
        // is already handled via RunContext event bus in the spawned task above
        Ok(AgentStreamReceiver::new(rx))
    }
}

/// Consume LLM stream response and accumulate into complete data
async fn consume_llm_stream(
    mut stream: StreamReceiver,
) -> Result<(String, Vec<vol_llm_core::ToolCall>, String), crate::AgentError> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();

    while let Some(result) = stream.recv().await {
        match result?.data {
            StreamEventData::ThinkingComplete { thinking: t } => {
                thinking = t;
            }
            StreamEventData::ToolCallComplete { tool_call } => {
                tool_calls.push(tool_call);
            }
            StreamEventData::ContentComplete { content: c } => {
                content = c;
            }
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.max_history_messages, 20);
        assert_eq!(config.verbose, false);
        assert_eq!(config.plugin_registry.plugins().len(), 0);
    }

    #[test]
    fn test_agent_config_custom() {
        use crate::prompt_context::{PromptTemplate, PromptContext};

        let template = PromptTemplate::new("test", "test prompt");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            max_iterations: 10,
            max_history_messages: 50,
            prompt_context,
            verbose: true,
            plugin_registry: PluginRegistry::new(),
            agent_id: "custom_agent".to_string(),
            log_base_path: PathBuf::from("custom/logs"),
        };
        assert_eq!(config.max_history_messages, 50);
        assert_eq!(config.agent_id, "custom_agent");
        assert_eq!(config.log_base_path, PathBuf::from("custom/logs"));
    }

    #[test]
    fn test_agent_config_with_observability() {
        use std::path::PathBuf;

        let config = AgentConfig {
            agent_id: "test_agent".to_string(),
            log_base_path: PathBuf::from("logs/agents"),
            ..Default::default()
        };

        assert_eq!(config.agent_id, "test_agent");
        assert_eq!(config.log_base_path, PathBuf::from("logs/agents"));
    }
}
