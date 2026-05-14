//! LokiPlugin - Sends agent events to OTel Collector via tracing macros.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{AgentPlugin, PluginDecision, RunContext};
use vol_llm_core::stream::AgentStreamEvent;

/// Plugin that sends agent events to OTel via tracing macros.
///
/// Stateless — no fields, no config. Clone is trivial.
pub struct LokiPlugin;

impl LokiPlugin {
    /// Create a new LokiPlugin.
    pub fn new() -> Self {
        Self
    }

    /// Whether an event should be sent to OTel.
    /// Skips high-frequency streaming delta events.
    pub fn should_send(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    /// Convert an event to a flat JSON object with metadata.
    fn create_event_json(event: &AgentStreamEvent, ctx: &RunContext) -> String {
        let def = ctx.config.def.as_ref();
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());
        let run_id = &ctx.run_id;
        let session_id = &ctx.session_id;

        let line_map = match serde_json::to_value(event) {
            Ok(Value::Object(mut map)) => {
                if map.len() == 1 {
                    let (variant, fields) = map.iter().next().unwrap();
                    let mut flat = serde_json::Map::new();
                    flat.insert("event".to_string(), json!(variant));
                    if let Value::Object(fields) = fields {
                        for (k, v) in fields {
                            flat.insert(k.clone(), v.clone());
                        }
                    }
                    flat.insert("run_id".to_string(), json!(run_id));
                    flat.insert("session_id".to_string(), json!(session_id));
                    flat.insert("agent_id".to_string(), json!(agent_id));
                    flat
                } else {
                    map.insert("run_id".to_string(), json!(run_id));
                    map.insert("session_id".to_string(), json!(session_id));
                    map.insert("agent_id".to_string(), json!(agent_id));
                    map
                }
            }
            _ => {
                let mut map = serde_json::Map::new();
                map.insert("event".to_string(), json!(event.event_name()));
                map.insert("run_id".to_string(), json!(run_id));
                map.insert("session_id".to_string(), json!(session_id));
                map.insert("agent_id".to_string(), json!(agent_id));
                map
            }
        };

        json!(line_map).to_string()
    }
}

#[async_trait]
impl AgentPlugin for LokiPlugin {
    fn id(&self) -> String {
        "loki".to_string()
    }

    fn priority(&self) -> u32 {
        20
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if !Self::should_send(event) {
            return;
        }
        let event_json = Self::create_event_json(event, ctx);
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());

        tracing::info!(
            session_id = ctx.session_id,
            agent_id = agent_id,
            agent_type = agent_type,
            agent_model = ctx.model,
            run_id = ctx.run_id,
            "{}", event_json
        );
    }
}
