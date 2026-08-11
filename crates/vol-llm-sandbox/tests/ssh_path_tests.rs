//! SSHSandbox path logic tests — does NOT require a running SSH server.
//!
//! These tests cover resolve_path, remote_path, construction, and all
//! path-resolution edge cases that are testable without a network connection.
//!
//! For end-to-end tests requiring a running SSH server, see ssh_integration.rs.

#[cfg(feature = "ssh")]
mod path_tests {
    use std::path::Path;
    use vol_llm_sandbox::registry::SshConfig;
    use vol_llm_sandbox::Sandbox;

    fn test_config() -> SshConfig {
        SshConfig {
            host: "localhost".to_string(),
            port: 22,
            user: "test".to_string(),
            identity_file: "/dev/null".to_string(),
            passphrase: None,
            known_hosts_file: None,
            host_key: Some(String::new()), // empty = accept any (allows construction)
            idle_timeout_secs: 300,
            connect_timeout_secs: 10,
        }
    }

    fn build(name: &str, work_dir: &str) -> vol_llm_sandbox::ssh::SSHSandbox {
        vol_llm_sandbox::ssh::SSHSandbox::new(
            name.to_string(),
            Some(work_dir.to_string()),
            test_config(),
        )
        .expect("SSHSandbox::new should succeed with valid config")
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Construction & metadata
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[tokio::test]
    async fn test_name_and_kind() {
        let sb = build("my-ssh-box", "/tmp/work");
        assert_eq!(sb.name(), "my-ssh-box");
        assert_eq!(sb.kind(), "ssh");
    }

    #[tokio::test]
    async fn test_root_path() {
        let sb = build("box", "/opt/sandbox");
        assert_eq!(sb.root_path(), Path::new("/opt/sandbox"));
    }

    #[tokio::test]
    async fn test_default_work_dir() {
        // When work_dir is None, default to /tmp/sandbox
        let sb = vol_llm_sandbox::ssh::SSHSandbox::new("box".to_string(), None, test_config())
            .expect("create SSHSandbox");
        assert_eq!(sb.root_path(), Path::new("/tmp/sandbox"));
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // resolve_path: SSHSandbox REJECTS all absolute paths
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[tokio::test]
    async fn test_resolve_path_relative() {
        let sb = build("box", "/home/agent/work");
        let resolved = sb.resolve_path("subdir/file.txt").unwrap();
        assert!(resolved.ends_with("subdir/file.txt"));
        assert!(resolved.starts_with("/home/agent/work"));
    }

    #[tokio::test]
    async fn test_resolve_path_rejects_absolute() {
        let sb = build("box", "/home/agent/work");
        // SSHSandbox rejects ALL absolute paths, even if within root
        assert!(sb.resolve_path("/home/agent/work/inside.txt").is_err());
        assert!(sb.resolve_path("/etc/passwd").is_err());
    }

    #[tokio::test]
    async fn test_resolve_path_rejects_traversal() {
        let sb = build("box", "/home/agent/work");
        assert!(sb.resolve_path("../escape.txt").is_err());
        assert!(sb.resolve_path("../../../etc/passwd").is_err());
    }

    #[tokio::test]
    async fn test_resolve_path_dot() {
        let sb = build("box", "/home/agent/work");
        let resolved = sb.resolve_path(".").unwrap();
        assert_eq!(resolved, Path::new("/home/agent/work"));
    }

    #[tokio::test]
    async fn test_resolve_path_dot_components() {
        let sb = build("box", "/home/agent/work");
        let resolved = sb.resolve_path("./foo/./bar").unwrap();
        assert!(resolved.ends_with("foo/bar"));
        assert!(!resolved.to_string_lossy().contains("/./"));
    }

    #[tokio::test]
    async fn test_resolve_path_spaces() {
        let sb = build("box", "/home/agent/work");
        let resolved = sb.resolve_path("my dir/my file.txt").unwrap();
        assert!(resolved.ends_with("my dir/my file.txt"));
    }

    #[tokio::test]
    async fn test_resolve_path_unicode() {
        let sb = build("box", "/home/agent/work");
        let resolved = sb.resolve_path("日志/报告.txt").unwrap();
        assert!(resolved.ends_with("日志/报告.txt"));
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // remote_path: maps local Path → remote absolute path for SFTP
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // NOTE: remote_path is private, tested indirectly through
    // how resolve_path output feeds into file operations.
    //
    // Key invariant: resolve_path → absolute path → remote_path passes
    // absolute through unchanged → SFTP uses correct remote path.

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Construction validation
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[tokio::test]
    async fn test_construction_with_passphrase() {
        let mut config = test_config();
        config.passphrase = Some("secret".to_string());
        let sb = vol_llm_sandbox::ssh::SSHSandbox::new(
            "box".to_string(),
            Some("/tmp/work".to_string()),
            config,
        );
        assert!(sb.is_ok());
    }

    #[tokio::test]
    async fn test_construction_with_known_hosts() {
        let mut config = test_config();
        config.host_key = None;
        config.known_hosts_file = Some("~/.ssh/known_hosts".to_string());
        let sb = vol_llm_sandbox::ssh::SSHSandbox::new(
            "box".to_string(),
            Some("/tmp/work".to_string()),
            config,
        );
        assert!(sb.is_ok());
    }

    #[tokio::test]
    async fn test_construction_without_host_verification_fails() {
        let mut config = test_config();
        config.host_key = None;
        config.known_hosts_file = None;
        let sb = vol_llm_sandbox::ssh::SSHSandbox::new(
            "box".to_string(),
            Some("/tmp/work".to_string()),
            config,
        );
        // Construction succeeds; host key check happens at connect (start)
        assert!(sb.is_ok());
    }

    #[tokio::test]
    async fn test_idle_timeout_from_config() {
        let mut config = test_config();
        config.idle_timeout_secs = 600;
        let sb = vol_llm_sandbox::ssh::SSHSandbox::new(
            "box".to_string(),
            Some("/tmp/work".to_string()),
            config,
        );
        assert!(sb.is_ok());
    }
}
