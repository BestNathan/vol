//! SessionRecorderPlugin — records agent events to session via AgentPlugin::listen().
//!
//! This is a standalone plugin provided for future external registration.
//! Not registered by default in agent.rs or CodingAgent.

use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_core::{AgentPlugin, AgentStreamEvent, PluginContext};

use crate::entry::{SessionEntry, RUN_ID_KEY};
use crate::{Session, SessionEntryStore, SessionMessage};
use vol_llm_core::{Message, ToolCall};

/// Plugin that records key agent events to the session entry store.
///
/// Implements AgentPlugin::listen() to record events as SessionEntry.
/// Not registered by default — callers may register it externally.
pub struct SessionRecorderPlugin {
    session: Arc<Session>,
    entry_store: Arc<dyn SessionEntryStore>,
}

impl SessionRecorderPlugin {
    pub fn new(session: Arc<Session>, entry_store: Arc<dyn SessionEntryStore>) -> Self {
        Self { session, entry_store }
    }

    fn should_record(event: &AgentStreamEvent) -> bool {
        matches!(
            event,
            AgentStreamEvent::AgentStart { .. }
                | AgentStreamEvent::ThinkingComplete { .. }
                | AgentStreamEvent::ContentComplete { .. }
                | AgentStreamEvent::ToolCallBegin { .. }
                | AgentStreamEvent::ToolCallComplete { .. }
                | AgentStreamEvent::ToolCallError { .. }
                | AgentStreamEvent::ToolCallSkipped { .. }
                | AgentStreamEvent::IterationComplete { .. }
        )
    }

    fn event_to_session_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                Some(SessionMessage::new(self.session.id.clone(), Message::user(input.clone())))
            }
            AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                Some(SessionMessage::new(self.session.id.clone(), Message::assistant(thinking.clone())))
            }
            AgentStreamEvent::ContentComplete { content, .. } => {
                Some(SessionMessage::new(self.session.id.clone(), Message::assistant(content.clone())))
            }
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                let tool_call = ToolCall {
                    id: tool_call_id.clone(),
                    name: tool_name.clone(),
                    arguments: arguments.clone(),
                    r#type: "function".to_string(),
                };
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::assistant_with_tools("", vec![tool_call]),
                ))
            }
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
                ..
            } => {
                let content = format!("Tool '{}' returned: {}", tool_name, result);
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::tool(content, tool_call_id.clone()),
                ))
            }
            AgentStreamEvent::ToolCallError {
                tool_call_id,
                tool_name,
                error,
                ..
            } => {
                let content = format!("Tool '{}' error: {}", tool_name, error);
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::tool(content, tool_call_id.clone()),
                ))
            }
            AgentStreamEvent::ToolCallSkipped {
                tool_call_id,
                tool_name,
                reason,
                ..
            } => {
                let content = format!("Tool '{}' skipped: {}", tool_name, reason);
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::tool(content, tool_call_id.clone()),
                ))
            }
            AgentStreamEvent::IterationComplete { final_answer, .. } => {
                final_answer.as_ref().map(|answer| {
                    SessionMessage::new(
                        self.session.id.clone(),
                        Message::assistant(answer.clone()),
                    )
                })
            }
            _ => None,
        }
    }
}

#[async_trait]
impl AgentPlugin for SessionRecorderPlugin {
    fn id(&self) -> vol_llm_core::PluginId {
        "session_recorder".to_string()
    }

    fn priority(&self) -> u32 {
        0
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        if !Self::should_record(event) {
            return;
        }

        let Some(msg) = self.event_to_session_message(event) else {
            return;
        };
        let msg = msg.with_metadata(RUN_ID_KEY, &ctx.run_id);
        let entry = SessionEntry::from_message(msg);

        if let Err(e) = self.entry_store.save(entry).await {
            tracing::error!("Failed to save session entry: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InMemoryEntryStore;
    use crate::entry::{SessionEntryData, SessionEntryType};
    use vol_llm_core::{AgentStreamEvent, PluginContext};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn make_plugin() -> (SessionRecorderPlugin, PluginContext) {
        let entry_store: Arc<dyn crate::SessionEntryStore> = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());
        let plugin = SessionRecorderPlugin::new(Arc::new(session), entry_store);

        let ctx = PluginContext {
            run_id: "test-run".to_string(),
            user_input: "test".to_string(),
            session_id: "test-session".to_string(),
            all_tool_calls: Arc::new(RwLock::new(vec![])),
            current_tool_calls: Arc::new(RwLock::new(vec![])),
            data: Arc::new(RwLock::new(HashMap::new())),
        };

        (plugin, ctx)
    }

    #[tokio::test]
    async fn test_plugin_id() {
        let (plugin, _) = make_plugin();
        assert_eq!(plugin.id(), "session_recorder");
    }

    #[tokio::test]
    async fn test_plugin_records_thinking_complete() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ThinkingComplete {
            timestamp: chrono::Utc::now(),
            thinking: "Let me think...".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].r#type, SessionEntryType::Message);
        if let SessionEntryData::Message { message } = &entries[0].data {
            assert_eq!(message.message.role, vol_llm_core::MessageRole::Assistant);
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_records_tool_call_begin() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "get_weather".to_string(),
            arguments: r#"{"city": "Beijing"}"#.to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert_eq!(entries.len(), 1);
        if let SessionEntryData::Message { message } = &entries[0].data {
            let tool_calls = message.message.tool_calls.as_ref().unwrap();
            assert_eq!(tool_calls[0].id, "call_123");
            assert_eq!(tool_calls[0].name, "get_weather");
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_records_tool_call_complete() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ToolCallComplete {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "get_weather".to_string(),
            result: "25°C".to_string(),
            duration_ms: None,
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert_eq!(entries.len(), 1);
        if let SessionEntryData::Message { message } = &entries[0].data {
            assert_eq!(message.message.role, vol_llm_core::MessageRole::Tool);
            assert_eq!(message.message.tool_call_id, Some("call_123".to_string()));
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_records_tool_call_error() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ToolCallError {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            error: "command failed".to_string(),
            duration_ms: None,
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert_eq!(entries.len(), 1);
        if let SessionEntryData::Message { message } = &entries[0].data {
            assert_eq!(message.message.role, vol_llm_core::MessageRole::Tool);
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_records_tool_call_skipped() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ToolCallSkipped {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            reason: "User rejected".to_string(),
            duration_ms: None,
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert_eq!(entries.len(), 1);
        if let SessionEntryData::Message { message } = &entries[0].data {
            assert_eq!(message.message.role, vol_llm_core::MessageRole::Tool);
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_records_agent_start() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::AgentStart {
            timestamp: chrono::Utc::now(),
            input: "Hello".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert_eq!(entries.len(), 1);
        if let SessionEntryData::Message { message } = &entries[0].data {
            assert_eq!(message.message.role, vol_llm_core::MessageRole::User);
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_records_iteration_complete_with_final_answer() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::IterationComplete {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            tool_calls: Vec::new(),
            final_answer: Some("The answer is 42".to_string()),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert_eq!(entries.len(), 1);
        if let SessionEntryData::Message { message } = &entries[0].data {
            assert_eq!(message.message.role, vol_llm_core::MessageRole::Assistant);
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_records_run_id_in_metadata() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ThinkingComplete {
            timestamp: chrono::Utc::now(),
            thinking: "test".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        if let SessionEntryData::Message { message } = &entries[0].data {
            assert_eq!(
                message.metadata.get(RUN_ID_KEY),
                Some(&"test-run".to_string())
            );
        } else {
            panic!("Expected message entry");
        }
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_thinking_start() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ThinkingStart {
            timestamp: chrono::Utc::now(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert!(entries.is_empty(), "ThinkingStart should not be recorded");
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_thinking_delta() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ThinkingDelta {
            timestamp: chrono::Utc::now(),
            delta: "test".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert!(entries.is_empty(), "ThinkingDelta should not be recorded");
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_llm_call_start() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::LLMCallStart {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            messages: vec![],
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert!(entries.is_empty(), "LLMCallStart should not be recorded");
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_content_start() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ContentStart {
            timestamp: chrono::Utc::now(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert!(entries.is_empty(), "ContentStart should not be recorded");
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_content_delta() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ContentDelta {
            timestamp: chrono::Utc::now(),
            delta: "test".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert!(entries.is_empty(), "ContentDelta should not be recorded");
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_llm_call_complete() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: chrono::Utc::now(),
            model: "test".to_string(),
            usage: None,
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert!(entries.is_empty(), "LLMCallComplete should not be recorded");
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_llm_call_error() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::LLMCallError {
            timestamp: chrono::Utc::now(),
            error: "test".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&plugin.session.id).await.unwrap();
        assert!(entries.is_empty(), "LLMCallError should not be recorded");
    }
}
