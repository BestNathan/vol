//! ReAct Agent implementation.

use std::sync::Arc;
use tokio::sync::mpsc;
use vol_llm_core::{LLMClient, Message, ConversationRequest, ToolChoice, StreamEventData, StreamReceiver};
use vol_llm_tool::ToolContext;
use tracing::{info, debug};
use super::{
    AgentResponse, AgentStreamEvent, AgentStreamReceiver, PluginRegistry, PluginContext,
    PluginStream, PluginAction, create_shortcircuit_stream, create_skip_stream,
};
use crate::session::{Session, SessionMessage};

/// Agent configuration
#[derive(Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub system_prompt: String,
    pub verbose: bool,
    pub plugin_registry: PluginRegistry,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_history_messages: 20,
            system_prompt: super::default_system_prompt().to_string(),
            verbose: false,
            plugin_registry: PluginRegistry::new(),
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
        // === Phase 1: Generate run_id and create PluginContext ===
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());

        let mut plugin_ctx = PluginContext::new(
            run_id.clone(),
            user_input.to_string(),
            self.session.id.clone(),
        );

        // === Phase 2: Execute on_start hooks ===
        for plugin in self.config.plugin_registry.plugins() {
            match plugin.on_start(&mut plugin_ctx).await {
                PluginAction::Continue(()) => {
                    // Continue to next plugin
                }
                PluginAction::ShortCircuit(response) => {
                    tracing::info!(
                        run_id = %run_id,
                        plugin = %plugin.id(),
                        "Plugin short-circuited execution"
                    );
                    return create_shortcircuit_stream(response, plugin_ctx, run_id).await;
                }
                PluginAction::Skip => {
                    tracing::warn!(
                        run_id = %run_id,
                        plugin = %plugin.id(),
                        "Plugin requested skip"
                    );
                    return create_skip_stream(plugin_ctx, run_id).await;
                }
                PluginAction::Abort(error) => {
                    return Err(error);
                }
            }
        }

        // === Phase 3: Clone for spawned task ===
        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let config = self.config.clone();
        let session = self.session.clone();
        let user_input = user_input.to_string();
        let plugin_registry = config.plugin_registry.clone();
        let plugin_ctx_for_stream = plugin_ctx.clone();
        let _run_id_for_tracing = run_id.clone();

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            // Send AgentStart event
            let _ = tx.send(Ok(AgentStreamEvent::AgentStart {
                input: user_input.clone()
            })).await;

            let mut messages = Vec::new();
            let mut iteration = 0u32;

            messages.push(Message::system(config.system_prompt.clone()));

            // Get historical messages from session
            let history = session.get_messages(config.max_history_messages).await.unwrap_or_default();

            // Add history
            for session_msg in &history {
                messages.push(session_msg.message.clone());
            }

            messages.push(Message::user(user_input.clone()));

            loop {
                iteration += 1;

                if iteration > config.max_iterations {
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
                let request = ConversationRequest::with_history(None, messages.clone())
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

                // Send ThinkingComplete if we have thinking content
                if !thinking.is_empty() {
                    let _ = tx.send(Ok(AgentStreamEvent::ThinkingComplete { thinking })).await;
                }

                // Check if tool calls
                if !tool_calls.is_empty() {
                    debug!("Tool calls: {:?}", tool_calls);

                    // Act phase - execute tools
                    for call in &tool_calls {
                        info!("Executing tool: {} with args: {}", call.name, call.arguments);

                        // Send ToolCallBegin
                        let _ = tx.send(Ok(AgentStreamEvent::ToolCallBegin {
                            tool_name: call.name.clone(),
                            arguments: call.arguments.clone(),
                        })).await;

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

                        // Add tool result to messages
                        messages.push(Message::tool(result.content.clone(), call.id.clone()));

                        // Save tool result to session
                        let tool_msg = SessionMessage::new(session.id.clone(), Message::tool(result.content.clone(), call.id.clone()));
                        let _ = session.add_message(tool_msg).await;
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

                // Save user input and assistant response to session
                let user_msg = SessionMessage::new(session.id.clone(), Message::user(user_input.clone()));
                let _ = session.add_message(user_msg).await;

                let assistant_msg = SessionMessage::new(session.id.clone(), Message::assistant(content.clone()));
                let _ = session.add_message(assistant_msg).await;

                // Send AgentComplete
                let response = AgentResponse {
                    content,
                    reasoning: String::new(),
                    iterations: iteration,
                    tool_calls,
                };

                let _ = tx.send(Ok(AgentStreamEvent::AgentComplete { response })).await;
                break;
            }
        });

        // === Phase 4: Wrap with plugin stream for intercept hooks ===
        let raw_receiver = AgentStreamReceiver::new(rx);
        let plugins = plugin_registry.plugins().to_vec();
        let plugin_stream = PluginStream::new(raw_receiver, plugins, plugin_ctx_for_stream);

        Ok(plugin_stream.into_receiver())
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
        let config = AgentConfig {
            max_iterations: 10,
            max_history_messages: 50,
            system_prompt: "test".to_string(),
            verbose: true,
            plugin_registry: PluginRegistry::new(),
        };
        assert_eq!(config.max_history_messages, 50);
    }
}
