use async_trait::async_trait;
use std::sync::Arc;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, SandboxOperation, SandboxPayload, SandboxInfo,
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

            (
                SandboxOperation::Exec,
                Payload::Sandbox(SandboxPayload::Exec { command }),
            ) => {
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
                            Payload::Sandbox(SandboxPayload::ReadFileResult {
                                content: encoded,
                            }),
                        )])
                    }
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(
                        format!("sandbox.read_file: {}", e),
                    )),
                }
            }

            (
                SandboxOperation::WriteFile,
                Payload::Sandbox(SandboxPayload::WriteFile { path, content }),
            ) => {
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD.decode(&content).map_err(
                    |e| {
                        ProtocolError::PayloadDecodeFailedOwned(format!(
                            "sandbox.write_file base64: {}",
                            e
                        ))
                    },
                )?;
                let p = std::path::Path::new(&path);
                match self.sandbox.write_file(p, &data).await {
                    Ok(()) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::WriteFile),
                        Payload::Sandbox(SandboxPayload::WriteFileResult),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(
                        format!("sandbox.write_file: {}", e),
                    )),
                }
            }

            (
                SandboxOperation::CreateDir,
                Payload::Sandbox(SandboxPayload::CreateDir { path }),
            ) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.create_dir_all(p).await {
                    Ok(()) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::CreateDir),
                        Payload::Sandbox(SandboxPayload::CreateDirResult),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(
                        format!("sandbox.create_dir: {}", e),
                    )),
                }
            }

            (
                SandboxOperation::ReadDir,
                Payload::Sandbox(SandboxPayload::ReadDir { path }),
            ) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.read_dir(p).await {
                    Ok(entries) => {
                        let defs: Vec<_> = entries.into_iter().map(|e| e.into()).collect();
                        Ok(vec![AgentServerMessage::new_result(
                            mid,
                            Operation::Sandbox(SandboxOperation::ReadDir),
                            Payload::Sandbox(SandboxPayload::ReadDirResult { entries: defs }),
                        )])
                    }
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(
                        format!("sandbox.read_dir: {}", e),
                    )),
                }
            }

            (
                SandboxOperation::Metadata,
                Payload::Sandbox(SandboxPayload::Metadata { path }),
            ) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.metadata(p).await {
                    Ok(meta) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Metadata),
                        Payload::Sandbox(SandboxPayload::MetadataResult {
                            metadata: meta.into(),
                        }),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailedOwned(
                        format!("sandbox.metadata: {}", e),
                    )),
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
