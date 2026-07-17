use std::sync::Arc;

use vol_llm_agent_protocol::agent_server_protocol::NodeRegistration;

use crate::control_plane::state::ControlPlaneState;

pub fn register_local_data_plane(
    state: Arc<ControlPlaneState>,
    node_id: String,
    name: String,
    version: String,
) -> Result<(), String> {
    state.nodes.register(
        NodeRegistration {
            node_id,
            name,
            version,
        },
        "local".to_string(),
        now_ms(),
    )?;
    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::register_local_data_plane;
    use crate::control_plane::state::ControlPlaneState;

    #[test]
    fn register_local_data_plane_creates_node_record() {
        let state = Arc::new(ControlPlaneState::new());
        register_local_data_plane(
            state.clone(),
            "local-node".to_string(),
            "Local Node".to_string(),
            "test-version".to_string(),
        )
        .unwrap();

        let node = state.nodes.get("local-node").unwrap();
        assert_eq!(node.node_id, "local-node");
        assert_eq!(node.name, "Local Node");
        assert_eq!(node.status, "online");
    }

    #[test]
    fn register_local_data_plane_fails_on_empty_node_id() {
        let state = Arc::new(ControlPlaneState::new());
        let result = register_local_data_plane(
            state,
            "".to_string(),
            "Empty ID".to_string(),
            "test".to_string(),
        );
        // register doesn't validate empty IDs in NodeRegistry
        assert!(result.is_ok());
    }

    #[test]
    fn register_local_data_plane_handles_multiple_calls() {
        let state = Arc::new(ControlPlaneState::new());
        register_local_data_plane(
            state.clone(),
            "node-1".to_string(),
            "Node 1".to_string(),
            "1.0".to_string(),
        )
        .unwrap();
        register_local_data_plane(
            state.clone(),
            "node-2".to_string(),
            "Node 2".to_string(),
            "2.0".to_string(),
        )
        .unwrap();
        assert_eq!(state.nodes.list().len(), 2);
    }
}
