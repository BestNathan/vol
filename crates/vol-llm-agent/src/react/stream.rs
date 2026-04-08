//! Agent streaming events and receiver.

use vol_llm_core::ToolCall;
use super::response::AgentResponse;

/// Agent streaming event
#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    /// Agent started execution
    AgentStart { input: String },

    /// LLM thinking completed
    ThinkingComplete { thinking: String },

    /// About to call tool
    ToolCallBegin { tool_name: String, arguments: String },

    /// Tool call completed
    ToolCallComplete { tool_name: String, result: String },

    /// One iteration completed (Reason-Act-Observation)
    IterationComplete {
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },

    /// Agent execution completed
    AgentComplete { response: AgentResponse },
}

/// Agent stream receiver
pub struct AgentStreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, super::response::AgentError>>,
}

impl AgentStreamReceiver {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, super::response::AgentError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<AgentStreamEvent, super::response::AgentError>> {
        self.rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_stream_event_creation() {
        let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
        match event {
            AgentStreamEvent::AgentStart { input } => {
                assert_eq!(input, "test");
            }
            _ => panic!("Expected AgentStart"),
        }
    }

    #[test]
    fn test_agent_stream_event_tool_call() {
        let event = AgentStreamEvent::ToolCallBegin {
            tool_name: "get_weather".to_string(),
            arguments: r#"{"city": "Beijing"}"#.to_string(),
        };
        match event {
            AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
                assert_eq!(tool_name, "get_weather");
                assert_eq!(arguments, r#"{"city": "Beijing"}"#);
            }
            _ => panic!("Expected ToolCallBegin"),
        }
    }

    #[test]
    fn test_agent_stream_event_iteration_complete() {
        let event = AgentStreamEvent::IterationComplete {
            iteration: 1,
            tool_calls: Vec::new(),
            final_answer: Some("The answer".to_string()),
        };
        match event {
            AgentStreamEvent::IterationComplete { iteration, final_answer, .. } => {
                assert_eq!(iteration, 1);
                assert_eq!(final_answer, Some("The answer".to_string()));
            }
            _ => panic!("Expected IterationComplete"),
        }
    }
}
