//! ReAct Agent implementation.

use std::sync::Arc;
use vol_llm_core::{LLMClient, Message, ConversationRequest, ToolChoice, StreamEventData, StreamReceiver};
use vol_llm_tool::ToolContext;
use tracing::{info, debug};
use crate::{AgentResponse, AgentError};

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

    /// Run ReAct loop
    pub async fn run(
        &self,
        user_input: &str,
        context: ToolContext,
    ) -> Result<AgentResponse, AgentError> {
        let mut messages = Vec::new();
        let mut iteration = 0;

        // Initialize with system prompt
        messages.push(Message::system(self.config.system_prompt.clone()));
        messages.push(Message::user(user_input));

        loop {
            iteration += 1;

            if iteration > self.config.max_iterations {
                return Err(AgentError::MaxIterationsReached {
                    max: self.config.max_iterations,
                });
            }

            if self.config.verbose {
                info!("Iteration {}", iteration);
            }

            // Reason phase - call LLM
            let tools = self.tools.definitions();
            let request = ConversationRequest::with_history(None, messages.clone())
                .with_tools(tools)
                .with_tool_choice(ToolChoice::Auto);

            let response = self.llm.converse(request).await?;

            // Check if tool calls
            if let Some(tool_calls) = &response.message.tool_calls {
                if !tool_calls.is_empty() {
                    debug!("Tool calls: {:?}", tool_calls);

                    // Act phase - execute tools
                    let mut observations = Vec::new();
                    for call in tool_calls {
                        info!("Executing tool: {} with args: {}", call.name, call.arguments);
                        let result = self.tools.execute(call, &context).await
                            .map_err(|e| AgentError::ToolExecution {
                                tool: call.name.clone(),
                                error: e,
                            })?;
                        info!("Tool {} returned: {}", call.name, result.content);
                        observations.push((call.id.clone(), result.content.clone()));
                    }

                    // Observation phase - add results to messages
                    messages.push(response.message.clone());
                    for (call_id, content) in observations {
                        messages.push(Message::tool(content, call_id));
                    }

                    continue;
                }
            }

            // Final response
            let content = response.message.content
                .unwrap_or(vol_llm_core::MessageContent::Text(String::new()))
                .as_str()
                .to_string();

            let tool_calls = response.message.tool_calls.clone().unwrap_or_default();

            info!("Agent completed in {} iterations", iteration);

            return Ok(AgentResponse {
                content,
                reasoning: String::new(),
                iterations: iteration,
                tool_calls,
            });
        }
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
