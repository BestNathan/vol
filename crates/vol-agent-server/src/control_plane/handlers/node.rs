use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, NodeGetResult, NodeListResult, Operation,
    Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

use crate::control_plane::core::make_result;
use crate::control_plane::state::ControlPlaneState;

pub struct NodeHandler {
    state: Arc<ControlPlaneState>,
}

impl NodeHandler {
    pub fn new(state: Arc<ControlPlaneState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl DomainHandler for NodeHandler {
    fn name(&self) -> &str {
        "node"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Control(ControlOperation::NodeList),
            Operation::Control(ControlOperation::NodeGet),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (message.operation.clone(), message.payload.clone()) {
            (
                Operation::Control(ControlOperation::NodeList),
                Payload::Control(ControlPayload::NodeList(_)),
            ) => Ok(vec![make_result(
                message,
                ControlOperation::NodeList,
                ControlPayload::NodeListResult(NodeListResult {
                    nodes: self.state.nodes.list(),
                }),
            )]),
            (
                Operation::Control(ControlOperation::NodeGet),
                Payload::Control(ControlPayload::NodeGet(req)),
            ) => Ok(vec![make_result(
                message,
                ControlOperation::NodeGet,
                ControlPayload::NodeGetResult(NodeGetResult {
                    node: self.state.nodes.get(&req.node_id),
                }),
            )]),
            _ => Err(ProtocolError::PayloadDecodeFailedOwned(
                "unsupported node operation".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, ControlOperation, ControlPayload, MessageKind, NodeGetRequest,
        NodeListRequest, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use crate::control_plane::handlers::node::NodeHandler;
    use crate::control_plane::state::ControlPlaneState;
    use vol_llm_agent_protocol::agent_server_protocol::NodeRegistration;

    fn make_msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: id.to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn node_list_returns_empty_when_no_nodes_registered() {
        let handler = NodeHandler::new(Arc::new(ControlPlaneState::new()));
        let replies = handler
            .handle(make_msg(
                "1",
                Operation::Control(ControlOperation::NodeList),
                Payload::Control(ControlPayload::NodeList(NodeListRequest {})),
            ))
            .await
            .unwrap();
        let nodes = replies[0].payload.data_json()["nodes"]
            .as_array()
            .unwrap()
            .clone();
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn node_list_returns_registered_nodes() {
        let state = Arc::new(ControlPlaneState::new());
        state
            .nodes
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth".to_string(),
                1000,
            )
            .unwrap();
        let handler = NodeHandler::new(state);
        let replies = handler
            .handle(make_msg(
                "1",
                Operation::Control(ControlOperation::NodeList),
                Payload::Control(ControlPayload::NodeList(NodeListRequest {})),
            ))
            .await
            .unwrap();
        let nodes = replies[0].payload.data_json()["nodes"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["node_id"], "node-a");
    }

    #[tokio::test]
    async fn node_get_returns_node_when_found() {
        let state = Arc::new(ControlPlaneState::new());
        state
            .nodes
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth".to_string(),
                1000,
            )
            .unwrap();
        let handler = NodeHandler::new(state);
        let replies = handler
            .handle(make_msg(
                "1",
                Operation::Control(ControlOperation::NodeGet),
                Payload::Control(ControlPayload::NodeGet(NodeGetRequest {
                    node_id: "node-a".to_string(),
                })),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        assert!(json["node"].is_object());
        assert_eq!(json["node"]["node_id"], "node-a");
    }

    #[tokio::test]
    async fn node_get_returns_null_when_not_found() {
        let handler = NodeHandler::new(Arc::new(ControlPlaneState::new()));
        let replies = handler
            .handle(make_msg(
                "1",
                Operation::Control(ControlOperation::NodeGet),
                Payload::Control(ControlPayload::NodeGet(NodeGetRequest {
                    node_id: "nonexistent".to_string(),
                })),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        assert!(json["node"].is_null());
    }

    #[tokio::test]
    async fn node_handler_returns_error_on_unknown_operation() {
        let handler = NodeHandler::new(Arc::new(ControlPlaneState::new()));
        let err = handler
            .handle(make_msg(
                "1",
                Operation::Control(ControlOperation::Register),
                Payload::Control(ControlPayload::Register(NodeRegistration {
                    node_id: "x".to_string(),
                    name: "x".to_string(),
                    version: "0".to_string(),
                })),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unsupported node operation"));
    }
}
