use async_trait::async_trait;
use std::sync::Arc;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, SandboxInfo, SandboxOperation, SandboxPayload,
};
use vol_llm_agent_protocol::DomainHandler;
use vol_llm_agent_protocol::ProtocolError;
use vol_llm_sandbox::Sandbox;

/// Handler that dispatches sandbox protocol operations to a local Sandbox instance.
pub struct SandboxHandler {
    sandbox: Arc<dyn Sandbox>,
}

impl SandboxHandler {
    pub fn new(sandbox: Arc<dyn Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl DomainHandler for SandboxHandler {
    fn name(&self) -> &str {
        "sandbox"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Sandbox(SandboxOperation::List),
            Operation::Sandbox(SandboxOperation::Exec),
            Operation::Sandbox(SandboxOperation::ReadFile),
            Operation::Sandbox(SandboxOperation::WriteFile),
            Operation::Sandbox(SandboxOperation::CreateDir),
            Operation::Sandbox(SandboxOperation::ReadDir),
            Operation::Sandbox(SandboxOperation::Metadata),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Sandbox(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("sandbox")),
        };

        let mid = &message.message_id;

        match (op, message.payload) {
            (SandboxOperation::List, Payload::Sandbox(SandboxPayload::List)) => {
                let info = SandboxInfo {
                    name: self.sandbox.name().to_string(),
                    kind: self.sandbox.kind().to_string(),
                    root_path: self.sandbox.root_path().to_string_lossy().to_string(),
                };
                Ok(vec![AgentServerMessage::new_result(
                    mid,
                    Operation::Sandbox(SandboxOperation::List),
                    Payload::Sandbox(SandboxPayload::ListResult {
                        sandboxes: vec![info],
                    }),
                )])
            }

            (SandboxOperation::Exec, Payload::Sandbox(SandboxPayload::Exec { command })) => {
                let req: vol_llm_sandbox::CommandRequest = command.into();
                match self.sandbox.execute(req).await {
                    Ok(output) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Exec),
                        Payload::Sandbox(SandboxPayload::ExecResult {
                            output: output.into(),
                        }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Exec),
                        Payload::Sandbox(SandboxPayload::ExecResult {
                            output: vol_llm_sandbox::CommandOutput {
                                stdout: vec![],
                                stderr: e.to_string().into_bytes(),
                                exit_code: -1,
                                killed_by_signal: None,
                            }
                            .into(),
                        }),
                    )]),
                }
            }

            (
                SandboxOperation::ReadFile,
                Payload::Sandbox(SandboxPayload::ReadFile {
                    path,
                    offset,
                    limit,
                }),
            ) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.read_file(p, offset, limit).await {
                    Ok(content) => {
                        use base64::Engine;
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&content);
                        Ok(vec![AgentServerMessage::new_result(
                            mid,
                            Operation::Sandbox(SandboxOperation::ReadFile),
                            Payload::Sandbox(SandboxPayload::ReadFileResult { content: encoded }),
                        )])
                    }
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(format!(
                        "sandbox.read_file: {e}"
                    ))),
                }
            }

            (
                SandboxOperation::WriteFile,
                Payload::Sandbox(SandboxPayload::WriteFile { path, content }),
            ) => {
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(&content)
                    .map_err(|e| {
                        ProtocolError::PayloadDecodeFailedOwned(format!(
                            "sandbox.write_file base64: {e}"
                        ))
                    })?;
                let p = std::path::Path::new(&path);
                match self.sandbox.write_file(p, &data).await {
                    Ok(()) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::WriteFile),
                        Payload::Sandbox(SandboxPayload::WriteFileResult),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(format!(
                        "sandbox.write_file: {e}"
                    ))),
                }
            }

            (SandboxOperation::CreateDir, Payload::Sandbox(SandboxPayload::CreateDir { path })) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.create_dir_all(p).await {
                    Ok(()) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::CreateDir),
                        Payload::Sandbox(SandboxPayload::CreateDirResult),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(format!(
                        "sandbox.create_dir: {e}"
                    ))),
                }
            }

            (SandboxOperation::ReadDir, Payload::Sandbox(SandboxPayload::ReadDir { path })) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.read_dir(p).await {
                    Ok(entries) => {
                        let defs: Vec<_> =
                            entries.into_iter().map(std::convert::Into::into).collect();
                        Ok(vec![AgentServerMessage::new_result(
                            mid,
                            Operation::Sandbox(SandboxOperation::ReadDir),
                            Payload::Sandbox(SandboxPayload::ReadDirResult { entries: defs }),
                        )])
                    }
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(format!(
                        "sandbox.read_dir: {e}"
                    ))),
                }
            }

            (SandboxOperation::Metadata, Payload::Sandbox(SandboxPayload::Metadata { path })) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.metadata(p).await {
                    Ok(meta) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Metadata),
                        Payload::Sandbox(SandboxPayload::MetadataResult {
                            metadata: meta.into(),
                        }),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(format!(
                        "sandbox.metadata: {e}"
                    ))),
                }
            }

            // Catch-all for mismatched payload types
            (SandboxOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.list")),
            (SandboxOperation::Exec, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.exec")),
            (SandboxOperation::ReadFile, _) => {
                Err(ProtocolError::PayloadDecodeFailed("sandbox.read_file"))
            }
            (SandboxOperation::WriteFile, _) => {
                Err(ProtocolError::PayloadDecodeFailed("sandbox.write_file"))
            }
            (SandboxOperation::CreateDir, _) => {
                Err(ProtocolError::PayloadDecodeFailed("sandbox.create_dir"))
            }
            (SandboxOperation::ReadDir, _) => {
                Err(ProtocolError::PayloadDecodeFailed("sandbox.read_dir"))
            }
            (SandboxOperation::Metadata, _) => {
                Err(ProtocolError::PayloadDecodeFailed("sandbox.metadata"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use vol_llm_agent_protocol::agent_server_protocol::{
        CommandRequestDef, SandboxOperation, SandboxPayload,
    };
    use vol_llm_sandbox::local::LocalSandbox;
    use vol_llm_sandbox::Sandbox;

    async fn setup() -> SandboxHandler {
        let sandbox = Arc::new(LocalSandbox::new(None));
        sandbox.start().await.unwrap();
        SandboxHandler::new(sandbox)
    }

    #[test]
    fn test_handler_name() {
        let sb = LocalSandbox::new(None);
        let handler = SandboxHandler::new(Arc::new(sb));
        assert_eq!(handler.name(), "sandbox");
    }

    #[test]
    fn test_operations_count() {
        let sb = LocalSandbox::new(None);
        let handler = SandboxHandler::new(Arc::new(sb));
        let ops = handler.operations();
        assert_eq!(ops.len(), 7);
    }

    #[tokio::test]
    async fn test_list() {
        let handler = setup().await;
        let msg = AgentServerMessage::new_command(
            "1",
            Operation::Sandbox(SandboxOperation::List),
            Payload::Sandbox(SandboxPayload::List),
        );
        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ListResult { sandboxes }) => {
                assert_eq!(sandboxes.len(), 1);
                assert_eq!(sandboxes[0].name, "local");
            }
            _ => panic!("expected ListResult"),
        }
    }

    #[tokio::test]
    async fn test_exec_echo() {
        let handler = setup().await;
        let msg = AgentServerMessage::new_command(
            "2",
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
        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ExecResult { output }) => {
                assert_eq!(output.exit_code, 0);
                // stdout is base64 encoded
                let stdout = base64::engine::general_purpose::STANDARD
                    .decode(&output.stdout)
                    .unwrap_or_default();
                assert_eq!(stdout, b"hello");
            }
            _ => panic!("expected ExecResult"),
        }
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let handler = setup().await;

        // Write
        let write = AgentServerMessage::new_command(
            "3",
            Operation::Sandbox(SandboxOperation::WriteFile),
            Payload::Sandbox(SandboxPayload::WriteFile {
                path: "test.txt".into(),
                content: base64::engine::general_purpose::STANDARD.encode(b"hello world"),
            }),
        );
        let replies = handler.handle(write).await.unwrap();
        assert!(matches!(
            &replies[0].payload,
            Payload::Sandbox(SandboxPayload::WriteFileResult)
        ));

        // Read
        let read = AgentServerMessage::new_command(
            "4",
            Operation::Sandbox(SandboxOperation::ReadFile),
            Payload::Sandbox(SandboxPayload::ReadFile {
                path: "test.txt".into(),
                offset: None,
                limit: None,
            }),
        );
        let replies = handler.handle(read).await.unwrap();
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ReadFileResult { content }) => {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(content)
                    .unwrap();
                assert_eq!(decoded, b"hello world");
            }
            _ => panic!("expected ReadFileResult"),
        }
    }

    #[tokio::test]
    async fn test_create_dir() {
        let handler = setup().await;
        let msg = AgentServerMessage::new_command(
            "5",
            Operation::Sandbox(SandboxOperation::CreateDir),
            Payload::Sandbox(SandboxPayload::CreateDir {
                path: "subdir/nested".into(),
            }),
        );
        let replies = handler.handle(msg).await.unwrap();
        assert!(matches!(
            &replies[0].payload,
            Payload::Sandbox(SandboxPayload::CreateDirResult)
        ));

        // Verify the directory was actually created by reading dir
        let read = AgentServerMessage::new_command(
            "6",
            Operation::Sandbox(SandboxOperation::ReadDir),
            Payload::Sandbox(SandboxPayload::ReadDir {
                path: "subdir".into(),
            }),
        );
        let replies = handler.handle(read).await.unwrap();
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ReadDirResult { entries }) => {
                assert!(!entries.is_empty(), "expected nested dir entry");
                assert!(
                    entries.iter().any(|e| e.name == "nested"),
                    "expected 'nested' entry"
                );
            }
            _ => panic!("expected ReadDirResult"),
        }
    }

    #[tokio::test]
    async fn test_metadata() {
        let handler = setup().await;
        // First create a file so we have something to query
        let write = AgentServerMessage::new_command(
            "7",
            Operation::Sandbox(SandboxOperation::WriteFile),
            Payload::Sandbox(SandboxPayload::WriteFile {
                path: "meta_test.txt".into(),
                content: base64::engine::general_purpose::STANDARD.encode(b"data"),
            }),
        );
        handler.handle(write).await.unwrap();

        // Query metadata
        let meta = AgentServerMessage::new_command(
            "8",
            Operation::Sandbox(SandboxOperation::Metadata),
            Payload::Sandbox(SandboxPayload::Metadata {
                path: "meta_test.txt".into(),
            }),
        );
        let replies = handler.handle(meta).await.unwrap();
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::MetadataResult { metadata }) => {
                assert_eq!(metadata.file_type, "file");
                assert_eq!(metadata.size, 4);
                assert!(metadata.mtime > 0);
            }
            _ => panic!("expected MetadataResult"),
        }
    }

    #[tokio::test]
    async fn test_write_file_bad_base64() {
        let handler = setup().await;
        let msg = AgentServerMessage::new_command(
            "9",
            Operation::Sandbox(SandboxOperation::WriteFile),
            Payload::Sandbox(SandboxPayload::WriteFile {
                path: "bad.txt".into(),
                content: "not-valid-base64!!!".into(),
            }),
        );
        let err = handler.handle(msg).await.unwrap_err();
        assert!(err.to_string().contains("sandbox.write_file base64"));
    }

    #[tokio::test]
    async fn test_catch_all_mismatched_payload() {
        let handler = setup().await;

        // Send a List operation but with an Exec payload — should hit catch-all
        let msg = AgentServerMessage::new_command(
            "10",
            Operation::Sandbox(SandboxOperation::List),
            Payload::Sandbox(SandboxPayload::Exec {
                command: CommandRequestDef {
                    program: "echo".into(),
                    args: vec![],
                    env: vec![],
                    cwd: None,
                    stdin: None,
                    timeout_ms: 0,
                },
            }),
        );
        let err = handler.handle(msg).await.unwrap_err();
        assert_eq!(err.to_string(), "payload decode failed for sandbox.list");
    }

    #[tokio::test]
    async fn test_exec_error_returns_result_with_stderr() {
        let handler = setup().await;
        let msg = AgentServerMessage::new_command(
            "11",
            Operation::Sandbox(SandboxOperation::Exec),
            Payload::Sandbox(SandboxPayload::Exec {
                command: CommandRequestDef {
                    program: "nonexistent_cmd_xyz".into(),
                    args: vec![],
                    env: vec![],
                    cwd: None,
                    stdin: None,
                    timeout_ms: 5000,
                },
            }),
        );
        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ExecResult { output }) => {
                assert_eq!(output.exit_code, -1);
                assert!(
                    !output.stderr.is_empty(),
                    "expected stderr from failed exec"
                );
            }
            _ => panic!("expected ExecResult"),
        }
    }
}
