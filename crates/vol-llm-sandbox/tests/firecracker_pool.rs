//! Unit tests for FirecrackerConfig — tests config deserialization.
//!
//! These do NOT spawn real firecracker processes. Integration tests
//! that require KVM would be separate (and #[ignore]d by default).

use vol_llm_sandbox::registry::SandboxConfig;

#[test]
fn test_config_minimal() {
    let toml_str = r#"
name = "fc"
type = "firecracker"
work_dir = "/tmp/fc"

[firecracker]
kernel_image = "/opt/vmlinux"
rootfs_image = "/opt/rootfs.ext4"
tap_device = "fc-tap0"
ssh_identity_file = "/opt/key"
"#;
    let config: SandboxConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.name, "fc");
    assert_eq!(config.sandbox_type, "firecracker");
    let fc = config.firecracker.unwrap();
    assert_eq!(fc.pool_size, 1); // default
    assert_eq!(fc.idle_timeout_secs, 300); // default
    assert_eq!(fc.guest_ip, "172.16.0.2"); // default
    assert_eq!(fc.rootfs_readonly, false); // default
    assert_eq!(fc.guest_ssh_port, 22); // default
    assert_eq!(fc.firecracker_binary, None); // default
    assert_eq!(fc.ssh_passphrase, None); // default
}

#[test]
fn test_config_full() {
    let toml_str = r#"
name = "fc"
type = "firecracker"
work_dir = "/tmp/fc"

[firecracker]
kernel_image = "/opt/vmlinux"
rootfs_image = "/opt/rootfs.ext4"
rootfs_readonly = true
pool_size = 4
idle_timeout_secs = 120
connect_timeout_secs = 30
firecracker_binary = "/usr/local/bin/firecracker"
guest_ip = "10.0.0.1"
guest_ssh_port = 2222
tap_device = "fc-tap0"
ssh_identity_file = "/opt/key"
ssh_passphrase = "secret"
"#;
    let config: SandboxConfig = toml::from_str(toml_str).unwrap();
    let fc = config.firecracker.unwrap();
    assert_eq!(fc.pool_size, 4);
    assert_eq!(fc.idle_timeout_secs, 120);
    assert_eq!(fc.connect_timeout_secs, 30);
    assert_eq!(
        fc.firecracker_binary,
        Some("/usr/local/bin/firecracker".to_string())
    );
    assert_eq!(fc.guest_ip, "10.0.0.1");
    assert_eq!(fc.guest_ssh_port, 2222);
    assert_eq!(fc.rootfs_readonly, true);
    assert_eq!(fc.ssh_passphrase, Some("secret".to_string()));
}

#[test]
fn test_firecracker_config_missing_required() {
    // Missing kernel_image, rootfs_image, tap_device, ssh_identity_file
    let toml_str = r#"
name = "fc"
type = "firecracker"

[firecracker]
"#;
    let result: Result<SandboxConfig, _> = toml::from_str(toml_str);
    assert!(
        result.is_err(),
        "Should fail: missing required fields (kernel_image, rootfs_image, tap_device, ssh_identity_file)"
    );
}
