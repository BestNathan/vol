use vol_llm_sandbox::{SandboxCapabilities, SandboxId, SandboxStatus};

#[test]
fn test_sandbox_id_generation() {
    let id1 = SandboxId::new();
    let id2 = SandboxId::new();
    assert_ne!(id1, id2);
    assert!(id1.to_string().starts_with("sb_"));
}

#[test]
fn test_sandbox_status_variants() {
    let status = SandboxStatus::Running;
    assert_eq!(status, SandboxStatus::Running);
}

#[test]
fn test_sandbox_capabilities() {
    let caps = SandboxCapabilities {
        persistent: true,
        pausable: false,
        stoppable: false,
        destroyable: false,
    };
    assert!(caps.persistent);
    assert!(!caps.pausable);
}
