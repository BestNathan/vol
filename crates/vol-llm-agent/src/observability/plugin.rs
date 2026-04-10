//! ObservabilityPlugin implementation.

use crate::react::plugin::{AgentPlugin, PluginDecision, PluginId};
use crate::react::run_context::RunContext;
use crate::AgentStreamEvent;
use super::logger::ObservabilityLogger;
use std::sync::Arc;

pub struct ObservabilityPlugin {
    logger: Arc<ObservabilityLogger>,
}

impl ObservabilityPlugin {
    pub fn new(agent_id: String, log_base_path: std::path::PathBuf) -> Self {
        todo!()
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId {
        todo!()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        todo!()
    }
}
