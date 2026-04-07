//! ReAct Agent implementation.

use std::sync::Arc;
use tokio::sync::mpsc;
use vol_llm_core::{LLMClient, Message, ConversationRequest, ToolChoice, StreamEventData, StreamReceiver};
use vol_llm_tool::ToolContext;
use tracing::{info, debug};
use crate::{AgentResponse, AgentStreamEvent, AgentStreamReceiver};

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub system_prompt: String,
    pub verbose: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            system_prompt: crate::default_system_prompt().to_string(),
            verbose: false,
        }
    }
}

/// ReAct Agent
pub struct ReActAgent {
    llm: Arc<dyn LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    config: AgentConfig,
}

impl ReActAgent {
    pub fn new(llm: Arc<dyn LLMClient>, tools: Arc<vol_llm_tool::ToolRegistry>, config: AgentConfig) -> Self {
        Self { llm, tools, config }
    }

    /// Run ReAct loop with streaming events
    pub async fn run(
        &self,
        user_input: &str,
        context: ToolContext,
    ) -> Result<AgentStreamReceiver, crate::AgentError> {
        let (tx, rx) = mpsc::channel(100);

        // Clone necessary data for the spawned task
        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let config = self.config.clone();
        let user_input = user_input.to_string();

        tokio::spawn(async move {
            // Send AgentStart event
            let _ = tx.send(Ok(AgentStreamEvent::AgentStart {
                input: user_input.clone()
            })).await;

            let mut messages = Vec::new();
            let mut iteration = 0;

            messages.push(Message::system(config.system_prompt.clone()));
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
