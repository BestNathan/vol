#[cfg(feature = "ssh")]
mod ssh_tests {
    use vol_llm_sandbox::{CommandRequest, Sandbox};
    use vol_llm_sandbox::registry::SshConfig;
    use std::path::Path;
    use std::time::Duration;

    fn test_config() -> SshConfig {
        SshConfig {
            host: "localhost".to_string(),
            port: 2222,
            user: "agent".to_string(),
            identity_file: "tests/ssh_test_host/id_ed25519".to_string(),
            passphrase: None,
            known_hosts_file: None,
            host_key: None, // Accept any (Docker test host)
            idle_timeout_secs: 300,
            connect_timeout_secs: 10,
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker SSH test host running on port 2222"]
    async fn test_ssh_execute_echo() {
        let config = test_config();
        let sb = vol_llm_sandbox::ssh::SSHSandbox::new(
            "test".to_string(),
            Some("/home/agent/sandbox".to_string()),
            config,
        )
        .expect("create SSHSandbox");
        sb.start().await.expect("start sandbox");

        let req = CommandRequest {
            program: "echo".to_string(),
            args: vec!["-n".to_string(), "hello from ssh".to_string()],
            env: Default::default(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sb.execute(req).await.expect("execute echo");
        assert_eq!(output.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from ssh");

        sb.cleanup().await.ok();
    }

    #[tokio::test]
    #[ignore = "requires Docker SSH test host running on port 2222"]
    async fn test_ssh_file_read_write() {
        let config = test_config();
        let sb = vol_llm_sandbox::ssh::SSHSandbox::new(
            "test".to_string(),
            Some("/home/agent/sandbox".to_string()),
            config,
        )
        .expect("create SSHSandbox");
        sb.start().await.expect("start sandbox");

        let path = Path::new("test_file.txt");
        sb.write_file(path, b"hello ssh file")
            .await
            .expect("write file");
        let content = sb.read_file(path, None, None).await.expect("read file");
        assert_eq!(content, b"hello ssh file");

        sb.cleanup().await.ok();
    }

    #[tokio::test]
    #[ignore = "requires Docker SSH test host running on port 2222"]
    async fn test_ssh_missing_host_key_rejected() {
        let mut config = test_config();
        // Set a deliberately wrong host key to verify verification is enforced
        config.host_key =
            Some("SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string());

        let sb = vol_llm_sandbox::ssh::SSHSandbox::new(
            "test".to_string(),
            Some("/home/agent/sandbox".to_string()),
            config,
        )
        .expect("create SSHSandbox");

        let result = sb.start().await;
        assert!(
            result.is_err(),
            "Should reject connection with wrong host key"
        );
    }
}
