use std::sync::Arc;

use super::capability::CapabilityIndex;
use super::event::EventBus;
use super::registry::NodeRegistry;
use super::store::{CommandStore, RunStore};

#[derive(Clone)]
pub struct ControlPlaneState {
    pub nodes: Arc<NodeRegistry>,
    pub capabilities: Arc<CapabilityIndex>,
    pub events: EventBus,
    pub commands: Arc<CommandStore>,
    pub runs: Arc<RunStore>,
}

impl ControlPlaneState {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(NodeRegistry::new()),
            capabilities: Arc::new(CapabilityIndex::new()),
            events: EventBus::new(),
            commands: Arc::new(CommandStore::new()),
            runs: Arc::new(RunStore::new()),
        }
    }
}

impl Default for ControlPlaneState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_plane::store::CommandRecord;

    #[test]
    fn state_new_creates_all_fields() {
        let state = ControlPlaneState::new();
        assert!(state.nodes.list().is_empty());
        assert!(state.capabilities.list(None).is_empty());
        assert!(state.commands.get("none").is_none());
        assert!(state.runs.get("none").is_none());
    }

    #[test]
    fn state_default_creates_valid_state() {
        let state = ControlPlaneState::default();
        assert!(state.nodes.list().is_empty());
        assert!(state.capabilities.list(None).is_empty());
    }

    #[test]
    fn state_clone_produces_identical_shared_state() {
        let state = ControlPlaneState::new();
        let cloned = state.clone();
        // Both clones share the same Arcs
        assert!(Arc::ptr_eq(&state.nodes, &cloned.nodes));
        assert!(Arc::ptr_eq(&state.capabilities, &cloned.capabilities));
    }
}
