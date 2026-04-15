//! Unit tests for the coding module.

use crate::coding::*;
use std::sync::Arc;
use vol_llm_core::Sandbox;
use vol_llm_core::AgentPlugin;

// Dummy LLM client for builder/agent construction tests
use vol_llm_core::LLMClient;
use vol_llm_core::{ConversationRequest, ConversationResponse, StreamReceiver, SupportedParam};

struct DummyLlm;
#[async_trait::async_trait]
impl LLMClient for DummyLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
    fn model(&self) -> &str { "dummy" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }
    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> { unimplemented!() }
    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> { unimplemented!() }
}

// ========================
// config.rs tests
// ========================

#[test]
fn test_config_default() {
    let config = CodingAgentConfig::default();
    assert_eq!(config.agent_id, "coding-agent");
    assert_eq!(config.max_iterations, 10);
    assert_eq!(config.working_dir, std::path::PathBuf::from("."));
    assert_eq!(config.log_base_path, std::path::PathBuf::from("logs"));
    assert!(config.hitl_enabled);
    assert!(!config.verbose);
    assert!(config.html_report_path.is_none());
    assert!(config.llm.is_none());
}

#[test]
fn test_config_debug_impl() {
    let config = CodingAgentConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("<LLMClient>"));
    assert!(debug_str.contains("<PluginRegistry>"));
}

#[test]
fn test_config_clone() {
    let config = CodingAgentConfig::default();
    let cloned = config.clone();
    assert_eq!(cloned.agent_id, config.agent_id);
    assert_eq!(cloned.max_iterations, config.max_iterations);
    assert_eq!(cloned.working_dir, config.working_dir);
}

#[test]
fn test_config_custom_fields() {
    let config = CodingAgentConfig {
        agent_id: "custom_agent".to_string(),
        max_iterations: 20,
        ..Default::default()
    };
    assert_eq!(config.agent_id, "custom_agent");
    assert_eq!(config.max_iterations, 20);
}

// ========================
// error.rs tests
// ========================

#[test]
fn test_coding_agent_error_display() {
    let agent_err = CodingAgentError::Config("missing llm".to_string());
    assert!(agent_err.to_string().contains("missing llm"));

    let io_err = CodingAgentError::Io(std::io::Error::new(std::io::ErrorKind::Other, "io failed"));
    assert!(io_err.to_string().contains("io failed"));

    let task_err = CodingAgentError::TaskFailed("task failed".to_string());
    assert!(task_err.to_string().contains("task failed"));
}

#[test]
fn test_observer_error_display() {
    let record_err = ObserverError::RecordFailed("connection lost".to_string());
    assert!(record_err.to_string().contains("connection lost"));

    let report_err = ObserverError::ReportFailed("disk full".to_string());
    assert!(report_err.to_string().contains("disk full"));
}

#[test]
fn test_hitl_error_display() {
    let rejected = HITLError::Rejected("not allowed".to_string());
    assert!(rejected.to_string().contains("not allowed"));

    let timeout = HITLError::Timeout;
    assert!(timeout.to_string().contains("Timeout"));
}

#[test]
fn test_error_from_impls() {
    // Test From impls work (compile-time check + runtime)
    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "io");
    let coding_err: CodingAgentError = io_err.into();
    assert!(coding_err.to_string().contains("io"));
}

// ========================
// hitl.rs tests
// ========================

#[tokio::test]
async fn test_hitl_handler_disabled() {
    let handler = HITLHandler::new(false);
    let decision = handler.check_operation("bash", "rm -rf /").await.unwrap();
    assert!(matches!(decision, HITLDecision::Approve));
}

#[tokio::test]
async fn test_hitl_handler_safe_operation() {
    let handler = HITLHandler::new(true);
    let decision = handler.check_operation("bash", "ls -la").await.unwrap();
    assert!(matches!(decision, HITLDecision::Approve));
}

#[tokio::test]
async fn test_hitl_handler_dangerous_operation() {
    let handler = HITLHandler::new(true);
    let decision = handler.check_operation("bash", "rm -rf /tmp/foo").await.unwrap();
    assert!(matches!(decision, HITLDecision::Reject { .. }));
}

#[tokio::test]
async fn test_hitl_handler_dangerous_patterns() {
    let handler = HITLHandler::new(true);

    let dangerous_cmds = [
        "rm -fr /",
        ":(){:|:&};:",
        "mkfs.ext4 /dev/sda",
        "dd of=/dev/zero",
        "> /dev/sda1",
    ];

    for cmd in dangerous_cmds {
        let decision = handler.check_operation("bash", cmd).await.unwrap();
        assert!(
            matches!(decision, HITLDecision::Reject { .. }),
            "Expected Reject for: {}",
            cmd
        );
    }
}

#[tokio::test]
async fn test_hitl_handler_delete_file() {
    let handler = HITLHandler::new(true);
    let decision = handler.check_operation("delete_file", "/tmp/foo").await.unwrap();
    assert!(matches!(decision, HITLDecision::Reject { .. }));
}

#[test]
fn test_hitl_decision_serialize() {
    let approve = HITLDecision::Approve;
    let json = serde_json::to_string(&approve).unwrap();
    assert!(json.contains("approve"));

    let reject = HITLDecision::Reject { reason: "no".to_string() };
    let json = serde_json::to_string(&reject).unwrap();
    assert!(json.contains("reject"));
}

// ========================
// html_reporter.rs tests
// ========================

#[tokio::test]
async fn test_html_reporter_generate() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let output_path = tmp_dir.path().join("report.html");

    let reporter = HTMLReporter::new(
        output_path.clone(),
        "Test task".to_string(),
    );

    // on_complete triggers report generation
    reporter.on_complete().await.unwrap();

    assert!(output_path.exists());
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("<!DOCTYPE html>"));
    assert!(content.contains("Test task"));
}

#[tokio::test]
async fn test_html_reporter_empty_description() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let output_path = tmp_dir.path().join("empty_report.html");

    let reporter = HTMLReporter::new(output_path.clone(), String::new());
    reporter.on_complete().await.unwrap();

    assert!(output_path.exists());
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("<!DOCTYPE html>"));
}

// ========================
// sandbox/local.rs tests
// ========================

#[test]
fn test_local_sandbox_new_no_path() {
    let sandbox = LocalSandbox::new(None);
    // Path should contain "sandbox_" prefix (temp directory pattern)
    assert!(sandbox.root_path().to_string_lossy().contains("sandbox_"));
}

#[test]
fn test_local_sandbox_new_with_path() {
    let path = std::path::PathBuf::from("/tmp/test_sandbox_path");
    let sandbox = LocalSandbox::new(Some(path.clone()));
    assert_eq!(sandbox.root_path(), path);
}

#[test]
fn test_local_sandbox_start() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let path = tmp_dir.path().join("new_sandbox_dir");
    let sandbox = LocalSandbox::new(Some(path.clone()));
    sandbox.start().unwrap();
    assert!(path.exists());
}

#[test]
fn test_local_sandbox_resolve_path() {
    let sandbox = LocalSandbox::new(Some(std::path::PathBuf::from("/tmp/test_sandbox")));
    // Relative path resolves within sandbox
    let resolved = sandbox.resolve_path("sub/file.txt").unwrap();
    assert!(resolved.ends_with("sub/file.txt"));
    // Absolute path is rejected
    let err = sandbox.resolve_path("/etc/passwd");
    assert!(err.is_err());
}

#[test]
fn test_local_sandbox_kind() {
    let sandbox = LocalSandbox::new(None);
    assert_eq!(sandbox.kind(), "local");
}

// ========================
// observer.rs tests
// ========================

#[test]
fn test_event_observer_trait_methods() {
    // Compile-time check: EventObserver trait has on_event and on_complete
    fn _assert_observer<O: EventObserver>() {}
    _assert_observer::<ChannelledEventObserver>();
    _assert_observer::<HTMLReporter>();
}

// ========================
// channelled_observer.rs tests
// ========================

#[tokio::test]
async fn test_channelled_observer_collects_events() {
    let observer = ChannelledEventObserver::new();

    let event = vol_llm_core::AgentStreamEvent::agent_start("hello".to_string());
    observer.on_event(&event).await.unwrap();

    // Allow async channel to drain
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let events = observer.events().await;
    assert_eq!(events.len(), 1);
}

// ========================
// observer_plugin.rs tests
// ========================

#[tokio::test]
async fn test_observer_plugin_new() {
    let observer = Arc::new(ChannelledEventObserver::new());
    let plugin = ObserverPlugin::new(observer.clone());
    assert_eq!(plugin.id(), "observer");
}

#[tokio::test]
async fn test_observer_plugin_observer_method() {
    let observer = Arc::new(ChannelledEventObserver::new());
    let plugin = ObserverPlugin::new(observer.clone());
    let _ = plugin.observer();
}

#[tokio::test]
async fn test_observer_plugin_priority() {
    let observer = Arc::new(ChannelledEventObserver::new());
    let plugin = ObserverPlugin::new(observer);
    assert_eq!(plugin.priority(), 0);
}

// ========================
// agent.rs — Builder tests
// ========================

#[tokio::test]
async fn test_builder_default() {
    let builder = CodingAgentBuilder::new();
    // Build without LLM should fail
    let result = builder.build().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_builder_with_llm() {
    let llm = Arc::new(DummyLlm);
    let agent = CodingAgentBuilder::new()
        .llm(llm)
        .build()
        .await
        .unwrap();
    // Agent created successfully
    assert!(agent.config().llm.is_some());
}

#[tokio::test]
async fn test_builder_with_all_methods() {
    let llm = Arc::new(DummyLlm);
    let tmp_dir = tempfile::tempdir().unwrap();
    let agent = CodingAgentBuilder::new()
        .llm(llm)
        .working_dir(tmp_dir.path().to_path_buf())
        .hitl_enabled(true)
        .max_iterations(20)
        .build()
        .await
        .unwrap();

    assert_eq!(agent.config().max_iterations, 20);
    assert!(agent.config().hitl_enabled);
}

#[tokio::test]
async fn test_builder_consumable_default() {
    // Builder::new() returns a fresh instance each time
    let _b1 = CodingAgentBuilder::new();
    let _b2 = CodingAgentBuilder::new();
    // No interference between instances
}

// ========================
// agent.rs — CodingAgent tests
// ========================

#[tokio::test]
async fn test_agent_new_validation() {
    let llm = Arc::new(DummyLlm);
    let config = CodingAgentConfig {
        llm: Some(llm),
        ..Default::default()
    };
    let result = CodingAgent::new(config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_agent_new_missing_llm() {
    let config = CodingAgentConfig::default();
    let result = CodingAgent::new(config).await;
    assert!(result.is_err());
    if let Err(CodingAgentError::Config(msg)) = result {
        assert!(msg.contains("llm"));
    } else {
        panic!("Expected Config error");
    }
}

#[tokio::test]
async fn test_agent_with_observer() {
    let llm = Arc::new(DummyLlm);
    let config = CodingAgentConfig {
        llm: Some(llm),
        ..Default::default()
    };
    let observer = Arc::new(ChannelledEventObserver::new());
    let agent = CodingAgent::new(config).await.unwrap()
        .with_observer(observer);
    assert!(agent.observer().is_some());
}

#[tokio::test]
async fn test_agent_with_methods() {
    let llm = Arc::new(DummyLlm);
    let tmp_dir = tempfile::tempdir().unwrap();
    let config = CodingAgentConfig {
        llm: Some(llm),
        ..Default::default()
    };
    let agent = CodingAgent::new(config).await.unwrap()
        .with_agent_id("test_123".to_string())
        .with_log_base_path(tmp_dir.path().join("logs"));
    assert_eq!(agent.config().agent_id, "test_123");
}

#[tokio::test]
async fn test_coding_agent_response() {
    let response = CodingAgentResponse {
        success: true,
        summary: "done".to_string(),
        iterations: 3,
        tool_calls: 5,
    };
    assert!(response.success);
    assert_eq!(response.iterations, 3);
    assert_eq!(response.tool_calls, 5);
}
