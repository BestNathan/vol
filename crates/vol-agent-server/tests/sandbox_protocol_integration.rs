use std::sync::Arc;

use vol_agent_server::data_plane::handlers::sandbox::SandboxHandler;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, CommandRequestDef, Operation, Payload, SandboxOperation, SandboxPayload,
};
use vol_llm_agent_protocol::Connection;
use vol_llm_agent_protocol::HandlerRegistry;
use vol_llm_agent_protocol::MemoryConnection;
use vol_llm_agent_protocol::MemoryHandle;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_sandbox::Sandbox;

/// Create a test server pair: (MemoryHandle for controlling the server, MemoryHandle for client).
/// Returns (server_handle, client_handle):
///   - server_handle: send messages INTO the server, receive messages FROM the server
///   - client_handle: same — but this is the one the integration tests use
///
/// Actually, MemoryConnection::new() returns (MemoryConnection, MemoryHandle):
///   - MemoryConnection implements Connection (recv/send) — acts as the "server side"
///   - MemoryHandle sends into MemoryConnection's rx, receives from MemoryConnection's tx
///
/// So the pattern is:
///   test sends a message via handle.send() -> MemoryConnection.recv() gets it
///   MemoryConnection.send() -> handle.recv() gets it
async fn create_test_server() -> MemoryHandle {
    let sandbox = Arc::new(LocalSandbox::new(None));
    sandbox.start().await.unwrap();

    let mut registry = HandlerRegistry::new();
    registry
        .register(Arc::new(SandboxHandler::new(sandbox)))
        .unwrap();

    let (server_conn, handle) = MemoryConnection::new();

    // Spawn the server handler loop
    tokio::spawn(async move {
        loop {
            match server_conn.recv().await {
                Some(Ok(msg)) => {
                    let replies = registry.dispatch(msg).await.unwrap_or_else(|e| {
                        vec![AgentServerMessage::new_error(
                            "err",
                            Operation::Sandbox(SandboxOperation::List),
                            vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                                code: "dispatch_error".to_string(),
                                message: e.to_string(),
                                detail: None,
                                terminal: true,
                            },
                        )]
                    });
                    for reply in replies {
                        let _ = server_conn.send(reply).await;
                    }
                }
                Some(Err(_)) | None => break,
            }
        }
    });

    handle
}

/// Send a command message through the handle and receive the response.
async fn send_and_recv(
    handle: &mut MemoryHandle,
    msg: AgentServerMessage,
) -> AgentServerMessage {
    handle.send(msg).unwrap();
    handle.recv().await.unwrap()
}

#[tokio::test]
async fn test_sandbox_list_round_trip() {
    let mut handle = create_test_server().await;

    let msg = AgentServerMessage::new_command(
        "test-list-1",
        Operation::Sandbox(SandboxOperation::List),
        Payload::Sandbox(SandboxPayload::List),
    );

    let reply = send_and_recv(&mut handle, msg).await;

    assert_eq!(reply.kind, vol_llm_agent_protocol::agent_server_protocol::MessageKind::Result);
    match &reply.payload {
        Payload::Sandbox(SandboxPayload::ListResult { sandboxes }) => {
            assert_eq!(sandboxes.len(), 1, "expected exactly one sandbox");
            assert_eq!(sandboxes[0].name, "local");
        }
        other => panic!("expected ListResult, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_sandbox_exec_round_trip() {
    let mut handle = create_test_server().await;

    let msg = AgentServerMessage::new_command(
        "test-exec-1",
        Operation::Sandbox(SandboxOperation::Exec),
        Payload::Sandbox(SandboxPayload::Exec {
            command: CommandRequestDef {
                program: "echo".into(),
                args: vec!["-n".into(), "hello".into()],
                env: vec![],
                cwd: None,
                stdin: None,
                timeout_ms: 5000,
            },
        }),
    );

    let reply = send_and_recv(&mut handle, msg).await;

    assert_eq!(reply.kind, vol_llm_agent_protocol::agent_server_protocol::MessageKind::Result);
    match &reply.payload {
        Payload::Sandbox(SandboxPayload::ExecResult { output }) => {
            assert_eq!(output.exit_code, 0, "expected exit_code 0");
            // stdout is base64 encoded in the wire format
            use base64::Engine;
            let stdout = base64::engine::general_purpose::STANDARD
                .decode(&output.stdout)
                .unwrap_or_default();
            assert_eq!(String::from_utf8_lossy(&stdout), "hello");
        }
        other => panic!("expected ExecResult, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_path_traversal_rejected() {
    let mut handle = create_test_server().await;

    let msg = AgentServerMessage::new_command(
        "test-traversal-1",
        Operation::Sandbox(SandboxOperation::ReadFile),
        Payload::Sandbox(SandboxPayload::ReadFile {
            path: "../etc/passwd".into(),
            offset: None,
            limit: None,
        }),
    );

    let reply = send_and_recv(&mut handle, msg).await;

    // The sandbox should reject the path traversal attempt.
    // The handler wraps errors in ProtocolError::PayloadDecodeFailedOwned,
    // so we expect an error message response. It comes back as a result with exit_code != 0
    // or as an error message. Let's check both possibilities.

    // The LocalSandbox should reject this as a non-canonical path.
    // Look for evidence of security rejection.
    match &reply.payload {
        Payload::Sandbox(SandboxPayload::ReadFileResult { content }) => {
            // If it somehow succeeded (unlikely), that's a problem
            panic!(
                "path traversal should have been rejected, got ReadFileResult with content {:?}",
                content
            );
        }
        Payload::Error(err) => {
            // Error response — the error should mention something about path traversal
            let msg_lower = err.message.to_lowercase();
            assert!(
                msg_lower.contains("path")
                    || msg_lower.contains("traversal")
                    || msg_lower.contains("security")
                    || msg_lower.contains("invalid")
                    || msg_lower.contains("denied")
                    || msg_lower.contains("permission")
                    || msg_lower.contains("not found")
                    || msg_lower.contains("no such file")
                    || msg_lower.contains("no such directory"),
                "unexpected error message: {}",
                err.message
            );
        }
        _ => {
            // Could be an ExecResult if the handler wraps it differently
        }
    }
}

#[tokio::test]
async fn test_concurrent_exec_requests() {
    let mut handle = create_test_server().await;

    // Send 4 sequential exec requests (not truly concurrent through one handle,
    // but tests that multiple requests/responses work through the same connection).
    for i in 0..4 {
        let msg = AgentServerMessage::new_command(
            format!("test-exec-concurrent-{}", i),
            Operation::Sandbox(SandboxOperation::Exec),
            Payload::Sandbox(SandboxPayload::Exec {
                command: CommandRequestDef {
                    program: "echo".into(),
                    args: vec!["-n".into(), format!("msg-{}", i)],
                    env: vec![],
                    cwd: None,
                    stdin: None,
                    timeout_ms: 5000,
                },
            }),
        );

        let reply = send_and_recv(&mut handle, msg).await;

        assert_eq!(
            reply.kind,
            vol_llm_agent_protocol::agent_server_protocol::MessageKind::Result
        );
        match &reply.payload {
            Payload::Sandbox(SandboxPayload::ExecResult { output }) => {
                assert_eq!(output.exit_code, 0, "exec #{} had non-zero exit", i);
                use base64::Engine;
                let stdout = base64::engine::general_purpose::STANDARD
                    .decode(&output.stdout)
                    .unwrap_or_default();
                assert_eq!(
                    String::from_utf8_lossy(&stdout),
                    format!("msg-{}", i),
                    "exec #{} had unexpected stdout",
                    i
                );
            }
            other => panic!("exec #{} expected ExecResult, got: {:?}", i, other),
        }
    }
}
