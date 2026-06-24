//! Command execution and output formatting.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use vol_llm_sandbox::{CommandOutput, CommandRequest, Sandbox};

use crate::config::CliToolConfig;
use crate::interpolate;

/// A resolved CLI tool: config + its backing sandbox.
pub struct CliTool {
    pub config: CliToolConfig,
    sandbox: Arc<dyn Sandbox>,
    /// Interpolated env (cached at load time, not per-call).
    env: HashMap<String, String>,
    /// Interpolated cwd (cached at load time).
    cwd: String,
}

impl CliTool {
    /// Build a CliTool from a config + sandbox.
    /// Performs placeholder interpolation on `env` and `cwd` once.
    pub fn new(config: CliToolConfig, sandbox: Arc<dyn Sandbox>) -> Self {
        let env = interpolate::interpolate_map(&config.env);
        let cwd = interpolate::interpolate(&config.cwd);
        Self { config, sandbox, env, cwd }
    }

    /// Decompose back into (config, sandbox) for wrapper layers.
    pub fn into_parts(self) -> (CliToolConfig, Arc<dyn Sandbox>) {
        (self.config, self.sandbox)
    }

    /// Run a command string through this tool.
    pub async fn run(
        &self,
        command: &str,
    ) -> Result<ToolOutput, crate::CliToolError> {
        // 1. Validate first token
        crate::validate::validate_first_token(command, &self.config.binaries)?;

        // 2. Build shell command with `cd <cwd> && <command>` so it works
        //    on every sandbox type (SSHSandbox ignores CommandRequest.cwd).
        let shell_body = if self.cwd.is_empty() {
            command.to_string()
        } else {
            format!("cd {} && {}", shell_quote(&self.cwd), command)
        };

        let req = CommandRequest {
            program: self.config.shell.clone(),
            args: vec!["-c".into(), shell_body],
            env: self.env.clone(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(self.config.timeout_secs),
        };

        // 3. Execute via sandbox
        let output = self.sandbox.execute(req).await.map_err(|e| {
            crate::CliToolError::SandboxFailed(e.to_string())
        })?;

        // 4. Format output
        Ok(format_output(&output, self.config.max_output_bytes))
    }
}

/// Result of a tool invocation, pre-formatted for the LLM.
#[derive(Debug)]
pub struct ToolOutput {
    pub success: bool,
    pub content: String,
}

/// Format a CommandOutput into LLM-readable text with per-stream truncation.
pub fn format_output(
    output: &CommandOutput,
    max_bytes: usize,
) -> ToolOutput {
    let mut text = String::new();
    text.push_str(&format!("exit_code: {}\n", output.exit_code));

    text.push_str("--- stdout ---\n");
    append_truncated(&mut text, &output.stdout, max_bytes);

    text.push_str("\n--- stderr ---\n");
    append_truncated(&mut text, &output.stderr, max_bytes);

    if let Some(sig) = output.killed_by_signal {
        text.push_str(&format!("\n--- killed by signal {sig} ---\n"));
    }

    ToolOutput {
        success: output.exit_code == 0 && output.killed_by_signal.is_none(),
        content: text,
    }
}

fn append_truncated(out: &mut String, bytes: &[u8], max_bytes: usize) {
    let s = String::from_utf8_lossy(bytes);
    if s.len() <= max_bytes {
        out.push_str(&s);
    } else {
        let truncated_len = s.len() - max_bytes;
        out.push_str(&s[..max_bytes]);
        out.push_str(&format!("\n... [truncated {truncated_len} bytes]"));
    }
}

/// Shell-quote a string for safe interpolation into `sh -c`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// Tests (with MockSandbox)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use vol_llm_sandbox::*;

    /// MockSandbox: records the last CommandRequest, returns a preset output.
    struct MockSandbox {
        output: CommandOutput,
        last_request: Arc<Mutex<Option<CommandRequest>>>,
    }

    impl MockSandbox {
        fn new(output: CommandOutput) -> (Self, Arc<Mutex<Option<CommandRequest>>>) {
            let last = Arc::new(Mutex::new(None));
            (Self { output, last_request: Arc::clone(&last) }, last)
        }
    }

    #[async_trait]
    impl Sandbox for MockSandbox {
        fn kind(&self) -> &str { "mock" }
        fn name(&self) -> &str { "mock" }
        async fn start(&self) -> SandboxResult<()> { Ok(()) }
        async fn cleanup(&self) -> SandboxResult<()> { Ok(()) }
        fn root_path(&self) -> &Path { Path::new("/") }
        fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
            Ok(PathBuf::from(rel))
        }
        async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
            *self.last_request.lock().unwrap() = Some(req);
            Ok(self.output.clone())
        }
        async fn read_file(&self, _p: &Path, _o: Option<u64>, _l: Option<u64>) -> SandboxResult<Vec<u8>> { Ok(vec![]) }
        async fn write_file(&self, _p: &Path, _c: &[u8]) -> SandboxResult<()> { Ok(()) }
        async fn create_dir_all(&self, _p: &Path) -> SandboxResult<()> { Ok(()) }
        async fn read_dir(&self, _p: &Path) -> SandboxResult<Vec<DirEntry>> { Ok(vec![]) }
        async fn metadata(&self, _p: &Path) -> SandboxResult<FileMetadata> {
            Err(SandboxError::Io(std::io::Error::new(std::io::ErrorKind::Other, "mock")))
        }
    }

    fn minimal_config() -> CliToolConfig {
        CliToolConfig {
            name: "ansible".into(),
            description: "Ansible suite".into(),
            binaries: vec!["ansible".into(), "ansible-playbook".into()],
            sandbox: None,
            sandbox_ref: Some("mock".into()),
            cwd: "/opt/ansible".into(),
            shell: "/bin/bash".into(),
            timeout_secs: 60,
            max_output_bytes: 65536,
            env: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn success_flow_builds_correct_request() {
        let output = CommandOutput {
            stdout: b"ok\n".to_vec(),
            stderr: vec![],
            exit_code: 0,
            killed_by_signal: None,
        };
        let (sandbox, last_req) = MockSandbox::new(output);
        let mut cfg = minimal_config();
        cfg.env.insert("ANSIBLE_CONFIG".into(), "/etc/ansible.cfg".into());
        let tool = CliTool::new(cfg, Arc::new(sandbox));

        let out = tool.run("ansible all -m ping").await.unwrap();
        assert!(out.success);
        assert!(out.content.contains("exit_code: 0"));
        assert!(out.content.contains("ok\n"));

        let req = last_req.lock().unwrap().clone().unwrap();
        assert_eq!(req.program, "/bin/bash");
        assert_eq!(req.args, vec!["-c", "cd '/opt/ansible' && ansible all -m ping"]);
        assert_eq!(req.env.get("ANSIBLE_CONFIG").map(String::as_str), Some("/etc/ansible.cfg"));
        assert!(req.cwd.is_none());
    }

    #[tokio::test]
    async fn invalid_first_token_rejected() {
        let (sandbox, _) = MockSandbox::new(CommandOutput {
            stdout: vec![], stderr: vec![], exit_code: 0, killed_by_signal: None,
        });
        let tool = CliTool::new(minimal_config(), Arc::new(sandbox));
        let err = tool.run("rm -rf /").await.unwrap_err();
        assert!(matches!(err, crate::CliToolError::BinaryNotAllowed { .. }));
    }

    #[tokio::test]
    async fn non_zero_exit_reports_failure() {
        let output = CommandOutput {
            stdout: vec![],
            stderr: b"ERROR: playbook not found\n".to_vec(),
            exit_code: 4,
            killed_by_signal: None,
        };
        let (sandbox, _) = MockSandbox::new(output);
        let tool = CliTool::new(minimal_config(), Arc::new(sandbox));
        let out = tool.run("ansible-playbook missing.yml").await.unwrap();
        assert!(!out.success);
        assert!(out.content.contains("exit_code: 4"));
        assert!(out.content.contains("ERROR: playbook not found"));
    }

    #[tokio::test]
    async fn stdout_is_truncated_at_max_bytes() {
        let big = vec![b'A'; 1000];
        let output = CommandOutput {
            stdout: big,
            stderr: vec![],
            exit_code: 0,
            killed_by_signal: None,
        };
        let (sandbox, _) = MockSandbox::new(output);
        let mut cfg = minimal_config();
        cfg.max_output_bytes = 100;
        let tool = CliTool::new(cfg, Arc::new(sandbox));
        let out = tool.run("ansible --version").await.unwrap();
        let a_count = out.content.matches('A').count();
        assert_eq!(a_count, 100);
        assert!(out.content.contains("[truncated 900 bytes]"));
    }

    #[tokio::test]
    async fn killed_by_signal_appended() {
        let output = CommandOutput {
            stdout: vec![],
            stderr: vec![],
            exit_code: -1,
            killed_by_signal: Some(9),
        };
        let (sandbox, _) = MockSandbox::new(output);
        let tool = CliTool::new(minimal_config(), Arc::new(sandbox));
        let out = tool.run("ansible-playbook slow.yml").await.unwrap();
        assert!(!out.success);
        assert!(out.content.contains("killed by signal 9"));
    }

    #[tokio::test]
    async fn env_placeholders_interpolated_at_construction() {
        std::env::set_var("CLI_TOOL_TEST_TOKEN", "secret-token");
        let (sandbox, last_req) = MockSandbox::new(CommandOutput {
            stdout: vec![], stderr: vec![], exit_code: 0, killed_by_signal: None,
        });
        let mut cfg = minimal_config();
        cfg.env.insert("TOKEN".into(), "{{env.CLI_TOOL_TEST_TOKEN}}".into());
        let tool = CliTool::new(cfg, Arc::new(sandbox));
        let _ = tool.run("ansible --version").await.unwrap();
        let req = last_req.lock().unwrap().clone().unwrap();
        assert_eq!(req.env.get("TOKEN").map(String::as_str), Some("secret-token"));
        std::env::remove_var("CLI_TOOL_TEST_TOKEN");
    }
}
