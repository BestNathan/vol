//! Handler registry with operation-based dispatch.

use std::collections::HashMap;

use crate::agent_server_protocol::{AgentServerMessage, ProtocolError};
use crate::domain::handler::HandlerRef;

/// Registry of domain handlers, dispatched by method name string.
pub struct HandlerRegistry {
    handlers: Vec<HandlerRef>,
    /// method_name → handler index
    method_index: HashMap<String, usize>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            method_index: HashMap::new(),
        }
    }

    /// Register a handler with type-safe Operation declarations.
    pub fn register(&mut self, handler: HandlerRef) -> Result<(), String> {
        let idx = self.handlers.len();
        for op in &handler.operations() {
            let method = op.method_name().to_string();
            if self.method_index.contains_key(&method) {
                return Err(format!(
                    "method '{}' already claimed by handler '{}'",
                    method,
                    self.handlers[self.method_index[&method]].name()
                ));
            }
        }
        for op in handler.operations() {
            self.method_index.insert(op.method_name().to_string(), idx);
        }
        self.handlers.push(handler);
        Ok(())
    }

    /// Register a custom handler with explicit method name strings.
    pub fn register_custom(
        &mut self,
        handler: HandlerRef,
        methods: &[&str],
    ) -> Result<(), String> {
        let idx = self.handlers.len();
        for method in methods {
            if self.method_index.contains_key(*method) {
                return Err(format!(
                    "method '{}' already registered",
                    method
                ));
            }
            self.method_index.insert(method.to_string(), idx);
        }
        self.handlers.push(handler);
        Ok(())
    }

    /// Dispatch a message to the appropriate handler.
    pub async fn dispatch(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let method = message.operation.method_name();
        if let Some(idx) = self.method_index.get(method) {
            return self.handlers[*idx].handle(message).await;
        }
        Err(ProtocolError::UnknownMethod(method.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::agent_server_protocol::{
        AgentServerMessage, FileOperation, FilePayload, MessageKind, Operation, Payload,
    };
    use crate::domain::handler::DomainHandler;
    use async_trait::async_trait;

    struct TestHandler {
        name: &'static str,
        ops: Vec<Operation>,
    }

    #[async_trait]
    impl DomainHandler for TestHandler {
        fn name(&self) -> &str { self.name }
        fn operations(&self) -> Vec<Operation> { self.ops.clone() }
        async fn handle(
            &self,
            msg: AgentServerMessage,
        ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
            Ok(vec![AgentServerMessage::new_result(
                msg.message_id,
                Operation::File(FileOperation::Read),
                Payload::File(FilePayload::ReadResult {
                    content: format!("handled by {}", self.name),
                    metadata: serde_json::json!({}),
                }),
            )])
        }
    }

    fn make_msg(operation: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "server".to_string(),
            kind: MessageKind::Command,
            operation,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_register_and_dispatch() {
        let mut registry = HandlerRegistry::new();
        let handler = Arc::new(TestHandler {
            name: "test",
            ops: vec![
                Operation::File(FileOperation::List),
                Operation::File(FileOperation::Read),
            ],
        });
        registry.register(handler).unwrap();

        let msg = make_msg(
            Operation::File(FileOperation::List),
            Payload::File(FilePayload::List { path: ".".into() }),
        );

        let results = registry.dispatch(msg).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, MessageKind::Result);
    }

    #[tokio::test]
    async fn test_duplicate_operation_rejected() {
        let mut registry = HandlerRegistry::new();
        let h1 = Arc::new(TestHandler {
            name: "first",
            ops: vec![Operation::File(FileOperation::List)],
        });
        let h2 = Arc::new(TestHandler {
            name: "second",
            ops: vec![Operation::File(FileOperation::List)],
        });
        registry.register(h1).unwrap();
        let err = registry.register(h2).unwrap_err();
        assert!(err.contains("already claimed"));
    }

    #[tokio::test]
    async fn test_unknown_method_returns_error() {
        let registry = HandlerRegistry::new();
        let msg = make_msg(
            Operation::File(FileOperation::List),
            Payload::File(FilePayload::List { path: ".".into() }),
        );
        let err = registry.dispatch(msg).await.unwrap_err();
        assert!(matches!(err, ProtocolError::UnknownMethod(_)));
    }

    #[tokio::test]
    async fn test_register_custom() {
        let mut registry = HandlerRegistry::new();
        let handler = Arc::new(TestHandler {
            name: "custom",
            ops: vec![],
        });
        registry.register_custom(handler, &["custom.op"]).unwrap();

        // Verify the string index is populated.
        assert!(registry.method_index.contains_key("custom.op"));
    }
}
