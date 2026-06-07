//! SSH session lifecycle: connect, authenticate, verify host key, disconnect.

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::info;

use crate::{SandboxError, SandboxResult};

/// Internal configuration passed from SSHSandbox to the session.
#[derive(Debug, Clone)]
pub struct SshSandboxConfig {
    pub name: String,
    pub work_dir: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identity_file: String,
    pub passphrase: Option<String>,
    pub known_hosts_file: Option<String>,
    pub host_key: Option<String>,
    pub idle_timeout_secs: u64,
    pub connect_timeout_secs: u64,
}

/// A managed SSH session with auto-reconnect capability.
pub struct SshSession {
    inner: tokio::sync::Mutex<Option<InnerSession>>,
    config: Arc<SshSandboxConfig>,
}

struct InnerSession {
    sess: ssh2::Session,
    _tcp: TcpStream, // Keep TCP stream alive
}

impl SshSession {
    pub fn new(config: Arc<SshSandboxConfig>) -> Self {
        Self {
            inner: Mutex::new(None),
            config,
        }
    }

    /// Ensure a session exists. Reconnects if disconnected.
    pub async fn ensure(&self) -> SandboxResult<()> {
        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let session = Self::connect(&self.config).await?;
        *guard = Some(session);
        Ok(())
    }

    /// Disconnect the session.
    pub async fn disconnect(&self) -> SandboxResult<()> {
        let mut guard = self.inner.lock().await;
        if let Some(inner) = guard.take() {
            let _ = inner.sess.disconnect(None, "cleanup", None);
        }
        Ok(())
    }

    /// Execute a command via channel_exec in a blocking context.
    /// Call this from within `tokio::task::spawn_blocking`.
    pub fn execute_blocking(
        &self,
        req: &crate::CommandRequest,
    ) -> SandboxResult<crate::CommandOutput> {
        use std::io::Read;
        use std::io::Write;

        // Open channel under lock, then release — channel operates independently
        let mut channel = {
            let guard = self.inner.blocking_lock();
            let inner = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
            inner
                .sess
                .channel_session()
                .map_err(|e| SandboxError::Ssh(e.to_string()))?
        };

        for (k, v) in &req.env {
            channel.setenv(k, v).ok();
        }

        // Shell-quote arguments to prevent re-parsing through sh -c
        let cmd_line = if req.args.is_empty() {
            req.program.clone()
        } else {
            let quoted_args: Vec<String> = req.args.iter()
                .map(|a| shell_quote(a))
                .collect();
            format!("{} {}", req.program, quoted_args.join(" "))
        };

        channel
            .exec(&cmd_line)
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;

        // Write stdin if provided
        if let Some(ref stdin_data) = req.stdin {
            channel.write_all(stdin_data)
                .map_err(|e| SandboxError::Ssh(format!("stdin write failed: {}", e)))?;
        }
        channel.send_eof()
            .map_err(|e| SandboxError::Ssh(format!("send_eof failed: {}", e)))?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        channel
            .read_to_end(&mut stdout)
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;
        channel
            .stderr()
            .read_to_end(&mut stderr)
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;
        channel.wait_close().ok();

        Ok(crate::CommandOutput {
            stdout,
            stderr,
            exit_code: channel.exit_status().unwrap_or(-1),
            killed_by_signal: None,
        })
    }

    /// Get a new SFTP channel from the session.
    pub async fn sftp(&self) -> SandboxResult<ssh2::Sftp> {
        self.ensure().await?;
        let guard = self.inner.lock().await;
        let inner = guard.as_ref().ok_or(SandboxError::NotStarted)?;
        inner
            .sess
            .sftp()
            .map_err(|e| SandboxError::Ssh(e.to_string()))
    }

    async fn connect(config: &SshSandboxConfig) -> SandboxResult<InnerSession> {
        let addr = format!("{}:{}", config.host, config.port);
        info!("SSH connecting to {}", addr);

        let tcp = TcpStream::connect_timeout(
            &addr
                .parse()
                .map_err(|e| SandboxError::Ssh(format!("bad address: {}", e)))?,
            Duration::from_secs(config.connect_timeout_secs),
        )
        .map_err(|e| SandboxError::Ssh(format!("connection failed: {}", e)))?;

        tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();

        let mut sess = ssh2::Session::new()
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;
        sess.set_tcp_stream(
            tcp.try_clone()
                .map_err(|e| SandboxError::Ssh(e.to_string()))?,
        );
        sess.handshake()
            .map_err(|e| SandboxError::Ssh(format!("handshake failed: {}", e)))?;

        // Host key verification — REQUIRED
        verify_host_key(&sess, config)?;

        // Authenticate
        authenticate(&sess, config)?;

        info!("SSH authenticated to {}", addr);
        Ok(InnerSession { sess, _tcp: tcp })
    }
}

fn verify_host_key(sess: &ssh2::Session, config: &SshSandboxConfig) -> SandboxResult<()> {
    let remote_key = sess
        .host_key()
        .ok_or_else(|| SandboxError::Ssh("no host key from server".to_string()))?;

    if let Some(ref fingerprint) = config.host_key {
        let hash = sess
            .host_key_hash(ssh2::HashType::Sha256)
            .ok_or_else(|| SandboxError::Ssh("failed to hash host key".to_string()))?;
        let fp = format!("SHA256:{}", base64_encode(hash));
        if fp.to_uppercase() != fingerprint.to_uppercase() {
            return Err(SandboxError::Ssh(format!(
                "host key mismatch: expected {}, got {}",
                fingerprint, fp
            )));
        }
    } else if let Some(ref known_hosts) = config.known_hosts_file {
        let known_hosts = shellexpand::tilde(known_hosts).to_string();
        let mut known = sess
            .known_hosts()
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;
        known
            .read_file(
                Path::new(&known_hosts),
                ssh2::KnownHostFileKind::OpenSSH,
            )
            .map_err(|e| SandboxError::Ssh(format!("failed to read known_hosts: {}", e)))?;
        let key = known.check(&config.host, remote_key.0);
        match key {
            ssh2::CheckResult::Match => {}
            ssh2::CheckResult::NotFound => {
                return Err(SandboxError::Ssh(format!(
                    "host {} not found in known_hosts file",
                    config.host
                )));
            }
            ssh2::CheckResult::Mismatch => {
                return Err(SandboxError::Ssh(format!(
                    "host key mismatch for {} in known_hosts",
                    config.host
                )));
            }
            ssh2::CheckResult::Failure => {
                return Err(SandboxError::Ssh(format!(
                    "host key check failed for {}",
                    config.host
                )));
            }
        }
    } else {
        return Err(SandboxError::Ssh(
            "host key verification not configured (set known_hosts_file or host_key)".to_string(),
        ));
    }

    Ok(())
}

fn authenticate(sess: &ssh2::Session, config: &SshSandboxConfig) -> SandboxResult<()> {
    let identity = shellexpand::tilde(&config.identity_file).to_string();
    let identity_path = PathBuf::from(&identity);

    // Try with passphrase if provided
    if let Some(ref passphrase) = config.passphrase {
        sess.userauth_pubkey_file(&config.user, None, &identity_path, Some(passphrase))
            .map_err(|e| SandboxError::Ssh(format!("auth failed: {}", e)))?;
        return Ok(());
    }

    // Try ssh-agent first, then key file without passphrase
    let agent_authed = {
        let mut agent = sess.agent().ok();
        if let Some(ref mut agent) = agent {
            agent.connect().is_ok()
                && agent.list_identities().is_ok()
                && agent.identities().ok().map_or(false, |ids| {
                    ids.iter()
                        .any(|id| agent.userauth(&config.user, id).is_ok())
                })
        } else {
            false
        }
    };

    if !agent_authed {
        sess.userauth_pubkey_file(&config.user, None, &identity_path, None)
            .map_err(|e| SandboxError::Ssh(format!("auth failed: {}", e)))?;
    }

    if !sess.authenticated() {
        return Err(SandboxError::Ssh("authentication failed".to_string()));
    }

    Ok(())
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Quote a string for safe use in a shell command line.
/// Wraps in single quotes, escaping any embedded single quotes.
fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    // If no special chars, return as-is
    if !s.chars().any(|c| matches!(c, ' ' | '\t' | '\n' | '"' | '\'' | '\\' | '$' | '`' | '!' | '*' | '?' | '[' | ']' | '{' | '}' | '|' | '&' | ';' | '<' | '>' | '(' | ')' | '#' | '~')) {
        return s.to_string();
    }
    // Wrap in single quotes, escape embedded single quotes: ' -> '\''
    let escaped = s.replace('\'', r"'\''");
    format!("'{}'", escaped)
}
