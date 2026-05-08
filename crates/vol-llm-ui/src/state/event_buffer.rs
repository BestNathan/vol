use crate::state::{UiState, UiEvent};
use vol_llm_core::AgentStreamEvent;

/// Bridges AgentStreamEvent (local agent) and UiEvent (remote/normalized) to UiState.
///
/// Local mode: AgentStreamEvent -> apply_stream() -> UiState mutation.
/// Remote mode: UiEvent -> apply_event() -> UiState mutation.
pub struct EventBuffer;

impl EventBuffer {
    pub fn new() -> Self {
        Self
    }

    /// Apply a raw AgentStreamEvent from a local agent, converting to UiState mutations.
    pub fn apply_stream(&mut self, event: &AgentStreamEvent, state: &mut UiState) {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                state.apply(UiEvent::AgentStart { input: input.clone() });
            }
            AgentStreamEvent::AgentComplete { response, .. } => {
                let response = response
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                state.apply(UiEvent::AgentComplete { response });
            }
            AgentStreamEvent::AgentAborted { reason, .. } => {
                state.apply(UiEvent::AgentAborted { reason: reason.clone() });
            }

            AgentStreamEvent::ThinkingStart { .. } => {
                state.apply(UiEvent::ThinkingStart);
            }
            AgentStreamEvent::ThinkingDelta { delta, .. } => {
                state.apply(UiEvent::ThinkingDelta { delta: delta.clone() });
            }
            AgentStreamEvent::ThinkingComplete { .. } => {
                state.apply(UiEvent::ThinkingComplete);
            }

            AgentStreamEvent::ContentStart { .. } => {
                state.apply(UiEvent::ContentStart);
            }
            AgentStreamEvent::ContentDelta { delta, .. } => {
                state.apply(UiEvent::ContentDelta { delta: delta.clone() });
            }
            AgentStreamEvent::ContentComplete { content, .. } => {
                state.apply(UiEvent::ContentComplete { content: content.clone() });
            }

            AgentStreamEvent::ToolCallBegin {
                tool_name,
                arguments,
                ..
            } => {
                state.apply(UiEvent::ToolCallBegin {
                    tool_name: tool_name.clone(),
                    arguments: arguments.clone(),
                });
            }
            AgentStreamEvent::ToolCallArgumentDelta { .. } => {
                // Invisible in UI
            }
            AgentStreamEvent::ToolCallComplete {
                tool_name,
                result,
                duration_ms,
                ..
            } => {
                state.apply(UiEvent::ToolCallComplete {
                    tool_name: tool_name.clone(),
                    result: result.clone(),
                    duration_ms: *duration_ms,
                });
                // Track modified files for Write/Edit tools
                let tool = tool_name.to_lowercase();
                if tool.contains("write") || tool.contains("edit") {
                    if let Some(path) = extract_file_path(result) {
                        state.modified_files.insert(path);
                    }
                }
            }
            AgentStreamEvent::ToolCallError {
                tool_name,
                error,
                duration_ms,
                ..
            } => {
                state.apply(UiEvent::ToolCallError {
                    tool_name: tool_name.clone(),
                    error: error.clone(),
                    duration_ms: *duration_ms,
                });
            }
            AgentStreamEvent::ToolCallSkipped {
                tool_name,
                reason,
                duration_ms,
                ..
            } => {
                state.apply(UiEvent::ToolCallSkipped {
                    tool_name: tool_name.clone(),
                    reason: reason.clone(),
                    duration_ms: *duration_ms,
                });
            }

            AgentStreamEvent::MaxIterationsReached {
                current_iteration,
                max_iterations,
                ..
            } => {
                state.apply(UiEvent::MaxIterationsReached {
                    current: *current_iteration,
                    max: *max_iterations,
                });
            }
            AgentStreamEvent::IterationContinued {
                from_iteration, ..
            } => {
                state.apply(UiEvent::IterationContinued {
                    from_iteration: *from_iteration,
                });
            }
            AgentStreamEvent::IterationComplete {
                iteration,
                final_answer,
                ..
            } => {
                state.apply(UiEvent::IterationComplete {
                    iteration: *iteration,
                    final_answer: final_answer.clone(),
                });
            }

            // LLM call meta events, plugin events — invisible in UI
            AgentStreamEvent::LLMCallStart { .. }
            | AgentStreamEvent::LLMCallComplete { .. }
            | AgentStreamEvent::LLMCallError { .. }
            | AgentStreamEvent::PluginEvent { .. } => {}
        }
    }

    /// Apply a UiEvent (from remote JSON-RPC or local conversion) to UiState.
    pub fn apply_event(&mut self, event: UiEvent, state: &mut UiState) {
        state.apply(event);
    }
}

fn extract_file_path(result: &str) -> Option<String> {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
        if let Some(path) = parsed.get("file_path").and_then(|v| v.as_str()) {
            return Some(path.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_buffer_new() {
        let buffer = EventBuffer::new();
        // EventBuffer is stateless now, just verify construction
        let _ = buffer;
    }

    #[test]
    fn test_apply_event_direct() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".");
        buffer.apply_event(UiEvent::AgentStart { input: "hello".into() }, &mut state);
        assert!(state.is_running);
        assert_eq!(state.conversation.len(), 1);
    }

    #[test]
    fn test_apply_stream_thinking_flow() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".");

        buffer.apply_stream(
            &AgentStreamEvent::ThinkingStart {
                timestamp: chrono::Utc::now(),
            },
            &mut state,
        );
        buffer.apply_stream(
            &AgentStreamEvent::ThinkingDelta {
                timestamp: chrono::Utc::now(),
                delta: "thinking ".into(),
            },
            &mut state,
        );
        buffer.apply_stream(
            &AgentStreamEvent::ThinkingDelta {
                timestamp: chrono::Utc::now(),
                delta: "about it".into(),
            },
            &mut state,
        );
        buffer.apply_stream(
            &AgentStreamEvent::ThinkingComplete {
                timestamp: chrono::Utc::now(),
                thinking: String::new(),
            },
            &mut state,
        );

        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            crate::state::ConversationEntry::Thinking { content } => {
                assert_eq!(content, "thinking about it");
            }
            _ => panic!("Expected Thinking entry"),
        }
    }

    #[test]
    fn test_apply_stream_modified_files() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".");

        buffer.apply_stream(
            &AgentStreamEvent::ToolCallComplete {
                timestamp: chrono::Utc::now(),
                tool_call_id: "call-1".into(),
                tool_name: "WriteFile".into(),
                result: r#"{"file_path":"src/main.rs"}"#.into(),
                duration_ms: Some(10),
            },
            &mut state,
        );

        assert!(state.modified_files.contains("src/main.rs"));
    }

    #[test]
    fn test_apply_stream_ignores_llm_meta_events() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".");

        buffer.apply_stream(
            &AgentStreamEvent::LLMCallStart {
                timestamp: chrono::Utc::now(),
                iteration: 1,
                messages: vec![],
            },
            &mut state,
        );
        buffer.apply_stream(
            &AgentStreamEvent::LLMCallComplete {
                timestamp: chrono::Utc::now(),
                model: "test".into(),
                usage: None,
            },
            &mut state,
        );

        // No conversation entries should be added
        assert!(state.conversation.is_empty());
    }

    #[test]
    fn test_apply_stream_agent_start_and_complete() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".");

        buffer.apply_stream(
            &AgentStreamEvent::AgentStart {
                timestamp: chrono::Utc::now(),
                input: "fix the bug".into(),
            },
            &mut state,
        );
        assert!(state.is_running);
        assert_eq!(state.run_count, 1);

        buffer.apply_stream(
            &AgentStreamEvent::AgentComplete {
                timestamp: chrono::Utc::now(),
                response: Some(serde_json::json!({"answer": "done"})),
            },
            &mut state,
        );
        assert!(!state.is_running);
    }

    #[test]
    fn test_apply_stream_tool_call_lifecycle() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".");

        buffer.apply_stream(
            &AgentStreamEvent::ToolCallBegin {
                timestamp: chrono::Utc::now(),
                tool_call_id: "call-1".into(),
                tool_name: "bash".into(),
                arguments: r#"{"command":"ls"}"#.into(),
            },
            &mut state,
        );
        assert_eq!(state.tool_calls.len(), 1);

        buffer.apply_stream(
            &AgentStreamEvent::ToolCallComplete {
                timestamp: chrono::Utc::now(),
                tool_call_id: "call-1".into(),
                tool_name: "bash".into(),
                result: "file.txt\n".into(),
                duration_ms: Some(42),
            },
            &mut state,
        );
        assert_eq!(state.tool_calls[0].duration_ms, Some(42));
    }

    #[test]
    fn test_apply_stream_max_iterations() {
        let mut buffer = EventBuffer::new();
        let mut state = UiState::new("sess-1".into(), ".");

        buffer.apply_stream(
            &AgentStreamEvent::MaxIterationsReached {
                timestamp: chrono::Utc::now(),
                current_iteration: 5,
                max_iterations: 10,
            },
            &mut state,
        );

        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            crate::state::ConversationEntry::Error { message } => {
                assert!(message.contains("5"));
                assert!(message.contains("10"));
            }
            _ => panic!("Expected Error entry"),
        }
    }
}
