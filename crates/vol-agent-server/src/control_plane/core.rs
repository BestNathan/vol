use std::sync::Arc;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, ErrorPayload, Operation, Payload,
    ProtocolError,
};
use vol_llm_agent_protocol::{Connection, HandlerRegistry, JsonRpcMessageService};

use crate::control_plane::handlers::capability::CapabilityHandler;
use crate::control_plane::handlers::client::ClientHandler;
use crate::control_plane::handlers::control::ControlHandler;
use crate::control_plane::handlers::node::NodeHandler;
use crate::control_plane::handlers::run::RunHandler;
use crate::data_plane::handlers::sandbox::SandboxHandler;
use crate::control_plane::state::ControlPlaneState;

pub(crate) fn make_result(
    message: AgentServerMessage,
    operation: ControlOperation,
    payload: ControlPayload,
) -> AgentServerMessage {
    let mut result = AgentServerMessage::new_result(
        message.message_id,
        Operation::Control(operation),
        Payload::Control(payload),
    );
    result.sender = "control".to_string();
    result.receiver = message.sender;
    result
}

pub struct ControlPlaneServerCore {
    pub state: Arc<ControlPlaneState>,
    handler_registry: HandlerRegistry,
}

impl ControlPlaneServerCore {
    pub async fn new(state: Arc<ControlPlaneState>) -> Result<Self, String> {
        let mut handler_registry = HandlerRegistry::new();
        handler_registry.register(Arc::new(ControlHandler::new(state.clone())))?;
        handler_registry.register(Arc::new(NodeHandler::new(state.clone())))?;
        handler_registry.register(Arc::new(CapabilityHandler::new(state.clone())))?;
        handler_registry.register(Arc::new(ClientHandler::new(state.clone())))?;
        handler_registry.register(Arc::new(RunHandler::new(state.clone())))?;

        let local_sandbox: Arc<dyn vol_llm_sandbox::Sandbox> =
            Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None));
        local_sandbox
            .start()
            .await
            .map_err(|e| format!("failed to start sandbox: {e}"))?;
        handler_registry
            .register(Arc::new(SandboxHandler::new(local_sandbox)))
            .map_err(|e| format!("failed to register SandboxHandler: {e}"))?;

        Ok(Self {
            state,
            handler_registry,
        })
    }

    pub async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        self.handler_registry.dispatch(message).await
    }
}

impl ControlPlaneServerCore {
    pub async fn serve_connection_with_role(
        &self,
        role: crate::control_plane::endpoint::ControlConnectionRole,
        conn: Arc<dyn Connection>,
    ) {
        let is_node = matches!(
            role,
            crate::control_plane::endpoint::ControlConnectionRole::DataPlaneNode
        );

        let dir_label = if is_node {
            "cp < dp"
        } else {
            "cp < client"
        };
        tracing::info!(dir = %dir_label, "control-plane accepted connection");

        while let Some(next) = conn.recv().await {
            match next {
                Ok(message) => {
                    let message_id = message.message_id.clone();
                    let operation = message.operation.clone();

                    if !role.allows(&message.operation) {
                        let err = AgentServerMessage::new_error(
                            message_id,
                            operation,
                            ErrorPayload {
                                code: "method_not_allowed_for_role".to_string(),
                                message: "method is not allowed on this endpoint".to_string(),
                                detail: None,
                                terminal: false,
                            },
                        );
                        let _ = conn.send(err).await;
                        continue;
                    }

                    let is_register = matches!(
                        (&message.operation, &message.payload),
                        (
                            Operation::Control(ControlOperation::Register),
                            Payload::Control(ControlPayload::Register(_)),
                        )
                    );

                    let replies = match self.handle(message).await {
                        Ok(replies) => replies,
                        Err(err) => {
                            tracing::warn!("control-plane handler error: {err}");
                            vec![AgentServerMessage::new_error(
                                message_id,
                                operation,
                                ErrorPayload {
                                    code: "dispatch_error".to_string(),
                                    message: err.to_string(),
                                    detail: None,
                                    terminal: false,
                                },
                            )]
                        }
                    };

                    // After successful register, store the connection for agent.submit forwarding
                    if is_node && is_register {
                        for reply in &replies {
                            if let Payload::Control(ControlPayload::RegisterAck(ref ack)) =
                                reply.payload
                            {
                                if ack.accepted {
                                    self.state
                                        .node_connections
                                        .write()
                                        .expect("node_connections lock poisoned")
                                        .insert(ack.node_id.clone(), conn.clone());
                                    tracing::info!(
                                        node_id = %ack.node_id,
                                        "stored node connection for agent forwarding"
                                    );
                                }
                            }
                        }
                    }

                    for reply in replies {
                        if let Err(err) = conn.send(reply).await {
                            tracing::warn!("control-plane send error: {err}");
                            break;
                        }
                    }
                }
                Err(err) => {
                    tracing::debug!("control-plane connection ended: {err}");
                    break;
                }
            }
        }

        // Clean up on disconnect
        if is_node {
            // Remove node connections by scanning for this conn pointer
            let conn_ptr = format!("{:p}", Arc::as_ptr(&conn));
            self.state
                .node_connections
                .write()
                .expect("node_connections lock poisoned")
                .retain(|_k, v| format!("{:p}", Arc::as_ptr(v)) != conn_ptr);
        }
        tracing::info!(dir = %dir_label, "control-plane connection closed");
    }
}

#[async_trait::async_trait]
impl JsonRpcMessageService for ControlPlaneServerCore {
    async fn serve_connection(&self, conn: Arc<dyn Connection>) {
        self.serve_connection_with_role(
            crate::control_plane::endpoint::ControlConnectionRole::Client,
            conn,
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::control_plane::state::ControlPlaneState;
    use crate::control_plane::core::make_result;
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
    };

    #[test]
    fn make_result_sets_sender_receiver_and_operation() {
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "req-1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::NodeList),
            payload: Payload::Control(ControlPayload::NodeList(
                vol_llm_agent_protocol::agent_server_protocol::NodeListRequest {},
            )),
            meta: Default::default(),
        };

        let result = make_result(
            msg,
            ControlOperation::NodeList,
            ControlPayload::NodeListResult(
                vol_llm_agent_protocol::agent_server_protocol::NodeListResult { nodes: vec![] },
            ),
        );
        assert_eq!(result.sender, "control");
        assert_eq!(result.receiver, "client");
        assert_eq!(result.message_id, "req-1");
        match result.operation {
            Operation::Control(ControlOperation::NodeList) => {}
            _ => panic!("expected NodeList operation"),
        }
    }

    #[tokio::test]
    async fn control_plane_server_core_registers_all_handlers() {
        let state = Arc::new(ControlPlaneState::new());
        let core = super::ControlPlaneServerCore::new(state).await.unwrap();
        // Core doesn't expose handler_registry publicly, but construction succeeds
        // and we can verify state is wired
        assert!(core.state.nodes.list().is_empty());
    }

    #[tokio::test]
    async fn control_plane_server_core_handle_unknown_operation() {
        let state = Arc::new(ControlPlaneState::new());
        let core = super::ControlPlaneServerCore::new(state).await.unwrap();
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::Heartbeat),
            payload: Payload::Control(ControlPayload::Heartbeat(
                vol_llm_agent_protocol::agent_server_protocol::NodeHeartbeat {
                    node_id: "unknown".to_string(),
                    status: "online".to_string(),
                    load: vol_llm_agent_protocol::agent_server_protocol::NodeLoad::default(),
                },
            )),
            meta: Default::default(),
        };

        let result = core.handle(msg).await;
        assert!(result.is_err());
    }

    #[test]
    fn make_result_preserves_message_id() {
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "req-42".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::NodeGet),
            payload: Payload::Control(ControlPayload::NodeGet(
                vol_llm_agent_protocol::agent_server_protocol::NodeGetRequest {
                    node_id: "x".to_string(),
                },
            )),
            meta: Default::default(),
        };
        let result = make_result(
            msg,
            ControlOperation::NodeGet,
            ControlPayload::NodeGetResult(
                vol_llm_agent_protocol::agent_server_protocol::NodeGetResult { node: None },
            ),
        );
        assert_eq!(result.message_id, "req-42");
        match result.operation {
            Operation::Control(ControlOperation::NodeGet) => {}
            _ => panic!("wrong operation"),
        }
    }

    #[tokio::test]
    async fn control_plane_server_core_handle_node_list() {
        let state = Arc::new(ControlPlaneState::new());
        let core = super::ControlPlaneServerCore::new(state).await.unwrap();
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::NodeList),
            payload: Payload::Control(ControlPayload::NodeList(
                vol_llm_agent_protocol::agent_server_protocol::NodeListRequest {},
            )),
            meta: Default::default(),
        };

        let replies = core.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
    }
}
