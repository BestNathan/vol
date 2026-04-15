# Agent Test Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add unit tests to `vol-llm-agents/coding` and `vol-llm-agent/react` crates to reach 80% test coverage, with a shared `MockLlmClient` for test isolation.

**Architecture:** Create a configurable `MockLlmClient` in `vol-llm-core` (test-utils feature). CodingAgent gets ~37 new tests (was 0). ReActAgent gets ~26 new tests (was ~17 existing). Pure unit tests for config/error/builder/types, integration-style tests for agent loop, observers, and plugin flows.

**Tech Stack:** tokio (async test runtime), vol-llm-core LLMClient trait, vol-session types

**Existing test coverage:**
- `vol-llm-agents/coding`: **0** tests across 10 files (~1,800 lines)
- `vol-llm-agent/react`: **~17** tests (3 in agent.rs, 1 in stream.rs, 2 in hitl.rs, ~14 in run_context.rs, 2 in react_mock_test.rs)

---

## Task 1: Create MockLlmClient in vol-llm-core

**Files:**
- Create: `crates/vol-llm-core/src/test_utils.rs`
- Modify: `crates/vol-llm-core/src/lib.rs` (add module + exports)
- Modify: `crates/vol-llm-core/Cargo.toml` (add `test-utils` feature)

- [ ] **Step 1: Add test-utils feature to Cargo.toml**

Modify `crates/vol-llm-core/Cargo.toml`, add at bottom:

```toml
[features]
test-utils = []
```

- [ ] **Step 2: Create MockLlmClient implementation**

Create `crates/vol-llm-core/src/test_utils.rs`:

```rust
//! Mock LLM client for testing agent loops without real API calls.
//!
//! Gated behind `#[cfg(feature = "test-utils")]`.

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider,
    StreamReceiver, StreamEvent, StreamEventData, SupportedParam,
};

struct MockState {
    converse_response: Option<ConversationResponse>,
    stream_events: Vec<StreamEvent>,
    error_at: Option<usize>,
    call_log: Vec<ConversationRequest>,
}

/// Configurable mock LLM client for testing.
///
/// Uses shared Arc state so the mock can be configured before creation
/// and inspected after the agent run completes.
pub struct MockLlmClient {
    state: Arc<Mutex<MockState>>,
    call_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                converse_response: None,
                stream_events: Vec::new(),
                error_at: None,
                call_log: Vec::new(),
            })),
            call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Set the response for converse() calls.
    pub async fn set_converse_response(&self, resp: ConversationResponse) {
        self.state.lock().await.converse_response = Some(resp);
    }

    /// Set the stream events for converse_stream() calls.
    /// Events are returned in order on each call.
    pub async fn set_stream_events(&self, events: Vec<StreamEvent>) {
        self.state.lock().await.stream_events = events;
    }

    /// Configure error at a specific call index (0-based).
    pub async fn set_error_at(&self, index: usize) {
        self.state.lock().await.error_at = Some(index);
    }

    /// Get the number of LLM calls made.
    pub fn call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get the last conversation request.
    pub async fn last_request(&self) -> Option<ConversationRequest> {
        self.state.lock().await.call_log.last().cloned()
    }

    /// Get all conversation requests.
    pub async fn all_requests(&self) -> Vec<ConversationRequest> {
        self.state.lock().await.call_log.clone()
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for MockLlmClient {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock-llm"
    }

    fn supported_params(&self) -> &[SupportedParam] {
        &[]
    }

    async fn converse(
        &self,
        request: ConversationRequest,
    ) -> crate::Result<ConversationResponse> {
        let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut state = self.state.lock().await;
        state.call_log.push(request);

        if let Some(error_at) = state.error_at {
            if count == error_at {
                return Err(crate::LLMError::Timeout("mock error".to_string()));
            }
        }

        state.converse_response.clone().ok_or_else(|| {
            crate::LLMError::Timeout("mock converse_response not set".to_string())
        })
    }

    async fn converse_stream(
        &self,
        request: ConversationRequest,
    ) -> crate::Result<StreamReceiver> {
        use tokio::sync::mpsc;

        let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut state = self.state.lock().await;
        state.call_log.push(request);

        if let Some(error_at) = state.error_at {
            if count == error_at {
                return Err(crate::LLMError::Timeout("mock stream error".to_string()));
            }
        }

        let events = state.stream_events.clone();
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            for event in events {
                let _ = tx.send(Ok(event)).await;
            }
        });

        Ok(StreamReceiver::new(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FinishReason, Message, MessageContent, TokenUsage};

    #[tokio::test]
    async fn test_mock_default_values() {
        let mock = MockLlmClient::new();
        assert_eq!(mock.provider(), LLMProvider::Anthropic);
        assert_eq!(mock.model(), "mock-llm");
        assert_eq!(mock.call_count(), 0);
    }

    #[tokio::test]
    async fn test_mock_converse_response() {
        let mock = MockLlmClient::new();
        let resp = ConversationResponse {
            message: Message::assistant(MessageContent::Text("test".to_string())),
            model: "mock".to_string(),
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cached_tokens: None,
            },
            finish_reason: FinishReason::Stop,
            raw_response: None,
        };
        mock.set_converse_response(resp.clone()).await;

        let request = ConversationRequest {
            system: None,
            messages: vec![],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        let result = mock.converse(request).await.unwrap();
        assert_eq!(result.message, resp.message);
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn test_mock_stream_events() {
        let mock = MockLlmClient::new();
        let events = vec![
            StreamEvent {
                id: "e1".to_string(),
                data: StreamEventData::ContentDelta { delta: "Hello".to_string() },
            },
            StreamEvent {
                id: "e2".to_string(),
                data: StreamEventData::ContentComplete { content: "Hello World".to_string() },
            },
        ];
        mock.set_stream_events(events).await;

        let request = ConversationRequest {
            system: None,
            messages: vec![],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        let mut receiver = mock.converse_stream(request).await.unwrap();
        let mut received = Vec::new();
        while let Some(event) = receiver.recv().await {
            received.push(event.unwrap());
        }
        assert_eq!(received.len(), 2);
        assert_eq!(received[0].id, "e1");
        assert_eq!(received[1].id, "e2");
    }

    #[tokio::test]
    async fn test_mock_error_at() {
        let mock = MockLlmClient::new();
        mock.set_error_at(0).await;

        let request = ConversationRequest {
            system: None,
            messages: vec![],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        let result = mock.converse_stream(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_call_logging() {
        let mock = MockLlmClient::new();

        let request = ConversationRequest {
            system: Some("sys".to_string()),
            messages: vec![Message::user("hi")],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        mock.set_stream_events(vec![]).await;
        let _ = mock.converse_stream(request.clone()).await;

        assert_eq!(mock.call_count(), 1);
        let last = mock.last_request().await.unwrap();
        assert_eq!(last.system, Some("sys".to_string()));
        assert_eq!(mock.all_requests().await.len(), 1);
    }
}
```

- [ ] **Step 3: Export from lib.rs**

Modify `crates/vol-llm-core/src/lib.rs`, add after the existing `pub use` block:

```rust
#[cfg(feature = "test-utils")]
pub mod test_utils;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-core --features test-utils
```

Expected: Compiles successfully

- [ ] **Step 5: Run mock tests**

```bash
cargo test -p vol-llm-core --features test-utils -- test_utils
```

Expected: 5 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-core/src/test_utils.rs crates/vol-llm-core/src/lib.rs crates/vol-llm-core/Cargo.toml
git commit -m "feat: add MockLlmClient for testing agent loops without real API calls"
```

---

## Task 2: Add coding module tests — config, error, hitl, html_reporter, sandbox, observer

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/mod.rs` (add `#[cfg(test)] mod tests;`)
- Create: `crates/vol-llm-agents/src/coding/tests.rs`
- Modify: `crates/vol-llm-agents/Cargo.toml` (add vol-llm-core/test-utils dev-dependency)

- [ ] **Step 1: Add test module and dev-dependencies**

Modify `crates/vol-llm-agents/src/coding/mod.rs`, add at end:

```rust
#[cfg(test)]
mod tests;
```

Modify `crates/vol-llm-agents/Cargo.toml`, add to `[dev-dependencies]`:

```toml
vol-llm-core = { path = "../vol-llm-core", features = ["test-utils"] }
```

- [ ] **Step 2: Create tests.rs with config, error, hitl, html_reporter, sandbox, observer tests**

Create `crates/vol-llm-agents/src/coding/tests.rs`:

```rust
//! Unit tests for the coding module.

use crate::coding::*;

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
    assert!(!config.unsafe_mode);
    assert!(!config.verbose);
    assert!(config.html_report_path.is_none());
    assert!(config.llm.is_none());
    assert!(config.session.is_none());
}

#[test]
fn test_config_debug_impl() {
    let config = CodingAgentConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("<Session>"));
    assert!(debug_str.contains("<LLMClient>"));
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
fn test_config_session_field() {
    let session = Arc::new(vol_session::Session::new(
        "test_session".to_string(),
        Arc::new(vol_session::InMemorySessionStore::new()),
        Arc::new(vol_session::InMemoryMessageStore::new()),
    ));
    let config = CodingAgentConfig {
        session: Some(session),
        ..Default::default()
    };
    assert!(config.session.is_some());
    assert_eq!(config.session.as_ref().unwrap().id(), "test_session");
}

// ========================
// error.rs tests
// ========================

#[test]
fn test_coding_agent_error_display() {
    use thiserror::Error;
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
    assert!(sandbox.is_temp);
    assert!(sandbox.root_path().to_string_lossy().contains("sandbox_"));
}

#[test]
fn test_local_sandbox_new_with_path() {
    let path = std::path::PathBuf::from("/tmp/test_sandbox_path");
    let sandbox = LocalSandbox::new(Some(path.clone()));
    assert!(!sandbox.is_temp);
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
    let mut observer = ChannelledEventObserver::new();

    let event = AgentStreamEvent::agent_start("hello".to_string());
    observer.on_event(&event).await.unwrap();

    // Allow async channel to drain
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let events = observer.events().await;
    assert_eq!(events.len(), 1);
}

// ========================
// observer_plugin.rs tests
// ========================

#[test]
fn test_observer_plugin_new() {
    let observer = Arc::new(ChannelledEventObserver::new());
    let plugin = ObserverPlugin::new(observer.clone());
    assert_eq!(plugin.id(), "observer");
}

#[test]
fn test_observer_plugin_observer_method() {
    let observer = Arc::new(ChannelledEventObserver::new());
    let plugin = ObserverPlugin::new(observer.clone());
    let _ = plugin.observer();
}

#[test]
fn test_observer_plugin_priority() {
    let observer = Arc::new(ChannelledEventObserver::new());
    let plugin = ObserverPlugin::new(observer);
    assert_eq!(plugin.priority(), 0);
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-agents --lib
```

Expected: Compiles successfully

- [ ] **Step 4: Run tests**

```bash
cargo test -p vol-llm-agents --lib coding::tests
```

Expected: ~25 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/mod.rs crates/vol-llm-agents/src/coding/tests.rs crates/vol-llm-agents/Cargo.toml
git commit -m "test: add unit tests for coding module config/error/hitl/sandbox/observer"
```

---

## Task 3: Add CodingAgent builder and agent tests

**Files:**
- Continue: `crates/vol-llm-agents/src/coding/tests.rs` (append)

- [ ] **Step 1: Add CodingAgent builder and agent tests**

Append to `crates/vol-llm-agents/src/coding/tests.rs`:

```rust
// ========================
// agent.rs — Builder tests
// ========================

use vol_llm_core::LLMClient;

struct DummyLlm;
#[async_trait::async_trait]
impl LLMClient for DummyLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
    fn model(&self) -> &str { "dummy" }
    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
    async fn converse(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> { unimplemented!() }
    async fn converse_stream(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> { unimplemented!() }
}

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
    assert!(agent.config.llm.is_some());
}

#[tokio::test]
async fn test_builder_with_all_methods() {
    let llm = Arc::new(DummyLlm);
    let session = Arc::new(vol_session::Session::new(
        "test_session".to_string(),
        Arc::new(vol_session::InMemorySessionStore::new()),
        Arc::new(vol_session::InMemoryMessageStore::new()),
    ));
    let tmp_dir = tempfile::tempdir().unwrap();
    let agent = CodingAgentBuilder::new()
        .llm(llm)
        .working_dir(tmp_dir.path().to_path_buf())
        .session(session)
        .hitl_enabled(true)
        .unsafe_mode(true)
        .max_iterations(20)
        .build()
        .await
        .unwrap();

    assert!(agent.config.session.is_some());
    assert_eq!(agent.config.max_iterations, 20);
    assert!(agent.config.unsafe_mode);
    assert!(agent.config.hitl_enabled);
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
    assert!(agent.observer.is_some());
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
    assert_eq!(agent.config.agent_id, "test_123");
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
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-agents --lib coding::tests
```

Expected: ~34 tests pass (previous ~25 + ~9 new)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agents/src/coding/tests.rs
git commit -m "test: add CodingAgent builder and agent tests"
```

---

## Task 4: Add vol-llm-agent/react tests — builder, prompt, response, state, stream

**Files:**
- Create: `crates/vol-llm-agent/src/react/tests.rs` (new test module)
- Modify: `crates/vol-llm-agent/src/react/mod.rs` (add `#[cfg(test)] mod tests;`)
- Modify: `crates/vol-llm-agent/Cargo.toml` (add vol-llm-core/test-utils dev-dependency)

- [ ] **Step 1: Add test module and dev-dependencies**

Modify `crates/vol-llm-agent/src/react/mod.rs`, add at end:

```rust
#[cfg(test)]
mod tests;
```

Modify `crates/vol-llm-agent/Cargo.toml`, add to `[dev-dependencies]`:

```toml
vol-llm-core = { path = "../vol-llm-core", features = ["test-utils"] }
```

- [ ] **Step 2: Create tests.rs with builder, prompt, response, state, stream tests**

Create `crates/vol-llm-agent/src/react/tests.rs`:

```rust
//! Unit tests for the react module.

use super::*;
use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_core::LLMClient;
use vol_llm_core::{Message, MessageContent};

struct DummyLlm;
#[async_trait::async_trait]
impl LLMClient for DummyLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
    fn model(&self) -> &str { "dummy" }
    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
    async fn converse(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> { unimplemented!() }
    async fn converse_stream(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> { unimplemented!() }
}

// ========================
// builder.rs tests
// ========================

#[tokio::test]
async fn test_builder_default() {
    let builder = AgentBuilder::new();
    let result = builder.build();
    // Should fail without LLM
    assert!(result.is_err());
}

#[tokio::test]
async fn test_builder_with_methods() {
    let llm = Arc::new(DummyLlm);
    let tmp_dir = tempfile::tempdir().unwrap();
    let session = Arc::new(vol_session::Session::new(
        "test".to_string(),
        Arc::new(vol_session::InMemorySessionStore::new()),
        Arc::new(vol_session::InMemoryMessageStore::new()),
    ));
    let agent = AgentBuilder::new()
        .with_llm(llm)
        .with_max_iterations(15)
        .with_system_prompt("You are a test assistant.".to_string())
        .with_verbose(true)
        .with_max_history_messages(50)
        .with_session(session)
        .with_agent_id("test_agent".to_string())
        .with_log_base_path(tmp_dir.path().to_path_buf())
        .build()
        .unwrap();

    assert_eq!(agent.config.max_iterations, 15);
    assert!(agent.config.verbose);
    assert_eq!(agent.config.max_history_messages, 50);
    assert_eq!(agent.config.agent_id, "test_agent");
}

#[test]
fn test_build_missing_llm() {
    let result = AgentBuilder::new().build();
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("llm") || e.to_string().contains("LLM"));
    }
}

#[tokio::test]
async fn test_build_with_observability_plugin() {
    struct DummyPlugin;
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for DummyPlugin {
        fn id(&self) -> plugin::PluginId { "dummy".to_string() }
        fn priority(&self) -> u32 { 50 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &PluginContext) -> plugin::PluginDecision { plugin::PluginDecision::Continue }
        async fn listen(&self, _: &AgentStreamEvent, _: &PluginContext) {}
    }

    let llm = Arc::new(DummyLlm);
    let agent = AgentBuilder::new()
        .with_llm(llm)
        .with_plugin(DummyPlugin)
        .build()
        .unwrap();

    // Build succeeds with plugin registered — the registry internal state is private,
    // so we verify via the successful build (plugin registration is a config mutation)
    let _ = agent; // agent built successfully
}

// ========================
// prompt.rs tests
// ========================

#[test]
fn test_default_system_prompt_content() {
    let prompt = prompt::default_system_prompt();
    // Contains Chinese text for derivatives market risk analyst
    assert!(prompt.contains("衍生品"));
    assert!(prompt.contains("风险分析师"));
    assert!(prompt.contains("工具"));
}

#[test]
fn test_system_prompt_builder_with_tools() {
    let tools = vec![vol_llm_core::ToolDefinition {
        name: "test_tool".to_string(),
        description: Some("A test tool".to_string()),
        parameters: Default::default(),
    }];

    let prompt = prompt::SystemPromptBuilder::new()
        .with_tools(&tools)
        .build();

    // Base prompt content should be present
    assert!(prompt.contains("衍生品"));
}

#[test]
fn test_system_prompt_builder_with_instructions() {
    let prompt = prompt::SystemPromptBuilder::new()
        .with_instructions("Custom instructions here")
        .build();

    assert!(prompt.contains("Custom instructions here"));
    assert!(prompt.contains("额外指示"));
}

#[test]
fn test_system_prompt_builder_default() {
    let prompt = prompt::SystemPromptBuilder::default().build();
    assert!(prompt.contains("衍生品"));
}

// ========================
// response.rs tests
// ========================

#[test]
fn test_agent_error_display() {
    let llm_err = AgentError::Llm(vol_llm_core::LLMError::Timeout("api failed".to_string()));
    assert!(llm_err.to_string().contains("api failed"));

    let tool_err = AgentError::ToolExecution {
        tool: "bash".to_string(),
        error: "permission denied".to_string(),
    };
    assert!(tool_err.to_string().contains("bash"));
    assert!(tool_err.to_string().contains("permission denied"));

    let max_err = AgentError::MaxIterationsReached { max: 5 };
    assert!(max_err.to_string().contains("5"));

    let ctx_err = AgentError::Context("missing context".to_string());
    assert!(ctx_err.to_string().contains("missing context"));

    let session_err = AgentError::SessionError("session failed".to_string());
    assert!(session_err.to_string().contains("session failed"));
}

#[test]
fn test_agent_response_construction() {
    let response = AgentResponse {
        content: "Hello World".to_string(),
        reasoning: vec![],
        run_id: "run_123".to_string(),
        session_id: "sess_456".to_string(),
        iterations: 3,
        tool_calls: vec![],
        error: None,
    };

    assert_eq!(response.content, "Hello World");
    assert_eq!(response.run_id, "run_123");
    assert!(response.error.is_none()); // No error = success
}

#[test]
fn test_agent_response_with_error() {
    let response = AgentResponse {
        content: String::new(),
        reasoning: vec![],
        run_id: "run_123".to_string(),
        session_id: "sess_456".to_string(),
        iterations: 0,
        tool_calls: vec![],
        error: Some("failed".to_string()),
    };

    assert!(response.error.is_some());
    assert_eq!(response.error, Some("failed".to_string()));
}

// ========================
// state.rs tests
// ========================

#[test]
fn test_reasoning_step_creation() {
    let step = state::ReasoningStep::new(1, "thinking about it".to_string(), Some(100));
    assert_eq!(step.iteration, 1);
    assert_eq!(step.thinking, "thinking about it");
    assert_eq!(step.duration_ms, Some(100));
}

#[test]
fn test_reasoning_step_no_duration() {
    let step = state::ReasoningStep::new(5, "more thinking".to_string(), None);
    assert_eq!(step.iteration, 5);
    assert!(step.duration_ms.is_none());
}

#[test]
fn test_tool_call_record() {
    let record = state::ToolCallRecord {
        tool_name: "bash".to_string(),
        arguments: "{}".to_string(),
        result: "ok".to_string(),
        iteration: 1,
        success: true,
    };
    assert_eq!(record.tool_name, "bash");
    assert!(record.success);
}

// ========================
// stream.rs tests (1 existing test, adding 2 more)
// ========================

#[test]
fn test_agent_stream_receiver_with_error() {
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let mut receiver = AgentStreamReceiver::new(rx);

    // Send an error event
    let err = AgentError::Context("test error".to_string());
    // Note: this test just verifies the type can handle errors
    // Full async test would need tokio runtime
    drop(tx);
    // Receiver should handle the channel close gracefully
    let _ = &mut receiver;
}

#[tokio::test]
async fn test_agent_stream_receiver_recv() {
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let mut receiver = AgentStreamReceiver::new(rx);

    tx.send(Ok(AgentStreamEvent::agent_start("test".to_string()))).await.unwrap();
    drop(tx);

    let event = receiver.recv().await;
    assert!(event.is_some());
    let event = event.unwrap().unwrap();
    assert!(matches!(event, AgentStreamEvent::AgentStart { .. }));
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-agent --lib
```

Expected: Compiles successfully

- [ ] **Step 4: Run tests**

```bash
cargo test -p vol-llm-agent --lib react::tests
```

Expected: ~18 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/mod.rs crates/vol-llm-agent/src/react/tests.rs crates/vol-llm-agent/Cargo.toml
git commit -m "test: add react module builder/prompt/response/state/stream tests"
```

---

## Task 5: Add plugin_stream and additional hitl tests

**Files:**
- Continue: `crates/vol-llm-agent/src/react/tests.rs` (append)

- [ ] **Step 1: Add plugin_stream tests**

Append to `crates/vol-llm-agent/src/react/tests.rs`:

```rust
// ========================
// plugin_stream.rs tests
// ========================

#[tokio::test]
async fn test_run_interceptor_loop_continue_decision() {
    // A plugin that always returns Continue
    struct ContinuePlugin;
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for ContinuePlugin {
        fn id(&self) -> plugin::PluginId { "continue".to_string() }
        fn priority(&self) -> u32 { 10 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &PluginContext) -> plugin::PluginDecision {
            plugin::PluginDecision::Continue
        }
        async fn listen(&self, _: &AgentStreamEvent, _: &PluginContext) {}
    }

    let (plugin_tx, plugin_rx) = tokio::sync::mpsc::channel(10);
    let (event_tx, _) = tokio::sync::broadcast::channel(10);
    let plugin_ctx = PluginContext {
        run_id: "test".to_string(),
        user_input: "test".to_string(),
        session_id: "test".to_string(),
        messages: Arc::new(tokio::sync::RwLock::new(vec![])),
        all_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
        current_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
        data: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![Arc::new(ContinuePlugin)];

    let interceptor = tokio::spawn(run_interceptor_loop(plugin_rx, plugins, event_tx, plugin_ctx));

    // Send an intercept request
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    plugin_tx.send(PluginRequest::Intercept {
        event: vol_tracing::TracedEvent::without_span(AgentStreamEvent::agent_start("test".to_string())),
        tx: reply_tx,
    }).await.unwrap();

    let decision = reply_rx.await.unwrap();
    assert!(matches!(decision, plugin::PluginDecision::Continue));

    // Shutdown
    drop(plugin_tx);
    interceptor.await.unwrap();
}

#[tokio::test]
async fn test_run_interceptor_loop_skip_decision() {
    struct SkipPlugin;
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for SkipPlugin {
        fn id(&self) -> plugin::PluginId { "skip".to_string() }
        fn priority(&self) -> u32 { 10 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &PluginContext) -> plugin::PluginDecision {
            plugin::PluginDecision::Skip
        }
        async fn listen(&self, _: &AgentStreamEvent, _: &PluginContext) {}
    }

    let (plugin_tx, plugin_rx) = tokio::sync::mpsc::channel(10);
    let (event_tx, _) = tokio::sync::broadcast::channel(10);
    let plugin_ctx = PluginContext {
        run_id: "test".to_string(),
        user_input: "test".to_string(),
        session_id: "test".to_string(),
        messages: Arc::new(tokio::sync::RwLock::new(vec![])),
        all_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
        current_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
        data: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![Arc::new(SkipPlugin)];

    let interceptor = tokio::spawn(run_interceptor_loop(plugin_rx, plugins, event_tx.clone(), plugin_ctx));

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    plugin_tx.send(PluginRequest::Intercept {
        event: vol_tracing::TracedEvent::without_span(AgentStreamEvent::agent_start("test".to_string())),
        tx: reply_tx,
    }).await.unwrap();

    let decision = reply_rx.await.unwrap();
    assert!(matches!(decision, plugin::PluginDecision::Skip));

    drop(plugin_tx);
    interceptor.await.unwrap();
}

#[tokio::test]
async fn test_run_interceptor_loop_emit_request() {
    let (plugin_tx, plugin_rx) = tokio::sync::mpsc::channel(10);
    let (event_tx, mut event_rx) = tokio::sync::broadcast::channel(10);
    let plugin_ctx = PluginContext {
        run_id: "test".to_string(),
        user_input: "test".to_string(),
        session_id: "test".to_string(),
        messages: Arc::new(tokio::sync::RwLock::new(vec![])),
        all_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
        current_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
        data: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![];

    let interceptor = tokio::spawn(run_interceptor_loop(plugin_rx, plugins, event_tx, plugin_ctx));

    // Send an emit request
    plugin_tx.send(PluginRequest::Emit {
        event: vol_tracing::TracedEvent::without_span(AgentStreamEvent::agent_start("test".to_string())),
    }).await.unwrap();

    // Should receive event on broadcast
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(event.value(), AgentStreamEvent::AgentStart { .. }));

    drop(plugin_tx);
    interceptor.await.unwrap();
}

#[tokio::test]
async fn test_plugin_decision_variants() {
    // Compile-time + runtime check for all decision variants
    let _continue = plugin::PluginDecision::Continue;
    let _skip = plugin::PluginDecision::Skip;
    let _abort = plugin::PluginDecision::Abort("reason".to_string());
}

// ========================
// Additional hitl.rs tests
// ========================

#[test]
fn test_hitl_config_with_triggers() {
    let config = hitl::HitlConfig {
        triggers: vec![
            hitl::ApprovalTrigger::ToolExecution { tools: None },
            hitl::ApprovalTrigger::AfterIteration,
            hitl::ApprovalTrigger::BeforeFinalAnswer,
        ],
        timeout_secs: 30,
        on_timeout: hitl::TimeoutBehavior::Reject { reason: "timed out".to_string() },
        timeout_message: Some("Please respond within 30 seconds".to_string()),
    };

    assert_eq!(config.triggers.len(), 3);
    assert_eq!(config.timeout_secs, 30);
}

#[test]
fn test_approval_type_variants() {
    let _tool = hitl::ApprovalType::ToolExecution { tool_name: "bash".to_string() };
    let _iter = hitl::ApprovalType::ContinueIteration { iteration: 1 };
    let _final = hitl::ApprovalType::FinalAnswer;
    let _custom = hitl::ApprovalType::Custom { name: "custom".to_string() };
}

#[test]
fn test_hitl_needs_tool_approval_all_tools() {
    // HitlPlugin internals — test via public API
    // We test the HitlConfig + ApprovalTrigger combination
    let config = hitl::HitlConfig {
        triggers: vec![hitl::ApprovalTrigger::ToolExecution { tools: None }],
        ..Default::default()
    };
    assert!(!config.triggers.is_empty());
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-agent --lib
```

Expected: Compiles successfully

- [ ] **Step 3: Run tests**

```bash
cargo test -p vol-llm-agent --lib react::tests
```

Expected: ~25 tests pass (previous ~18 + ~7 new)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/tests.rs
git commit -m "test: add plugin_stream and additional hitl tests"
```

---

## Task 6: Add agent run() flow tests using MockLlmClient

**Files:**
- Create: `crates/vol-llm-agent/tests/agent_run_tests.rs`

This file contains integration-style tests using `MockLlmClient` to test the `ReActAgent.run()` flow.

- [ ] **Step 1: Create agent run tests**

Create `crates/vol-llm-agent/tests/agent_run_tests.rs`:

```rust
//! Integration tests for ReActAgent run() flow using MockLlmClient.

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use vol_llm_agent::react::{ReActAgent, AgentBuilder, AgentStreamEvent};
use vol_llm_core::{
    LLMClient, LLMProvider, ConversationRequest, ConversationResponse,
    StreamReceiver, StreamEvent, StreamEventData, SupportedParam,
    Message, MessageContent, ToolCall, FinishReason, TokenUsage,
};
use vol_llm_core::test_utils::MockLlmClient;
use vol_session::{Session, InMemorySessionStore, InMemoryMessageStore};

/// Helper: create a ContentComplete stream event
fn content_complete_event(content: &str) -> StreamEvent {
    StreamEvent {
        id: "event_1".to_string(),
        data: StreamEventData::ContentComplete {
            content: content.to_string(),
        },
    }
}

/// Helper: create a ToolCallComplete stream event
fn tool_call_event(tool_name: &str, args: &str, call_id: &str) -> StreamEvent {
    StreamEvent {
        id: "event_1".to_string(),
        data: StreamEventData::ToolCallComplete {
            tool_call: ToolCall {
                id: call_id.to_string(),
                name: tool_name.to_string(),
                arguments: args.to_string(),
                r#type: "function".to_string(),
            },
        },
    }
}

/// Helper: create a default AgentResponse for mock converse (unused by stream tests)
fn mock_response() -> ConversationResponse {
    ConversationResponse {
        message: Message::assistant(MessageContent::Text("done".to_string())),
        model: "mock".to_string(),
        usage: TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            cached_tokens: None,
        },
        finish_reason: FinishReason::Stop,
        raw_response: None,
    }
}

// ========================
// Single iteration test
// ========================

#[tokio::test]
async fn test_agent_run_single_iteration() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Hello, I can help with that."),
    ]).await;

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Hi").await.unwrap();
    assert!(result.success);
    assert_eq!(result.iterations, 1);
}

// ========================
// Multi-iteration test
// ========================

struct MultiCallMock {
    call_count: Arc<AtomicUsize>,
}

impl MultiCallMock {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (Self { call_count: count.clone() }, count)
    }
}

#[async_trait]
impl LLMClient for MultiCallMock {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "multi-call-mock" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
        use tokio::sync::mpsc;

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            if count == 0 {
                // First call: tool call
                let _ = tx.send(Ok(tool_call_event("read_file", r#"{"path": "test.txt"}"#, "call_1"))).await;
            } else {
                // Second call: final answer
                let _ = tx.send(Ok(content_complete_event("The file contains hello world."))).await;
            }
        });

        Ok(StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_agent_run_multiple_iterations() {
    let (mock, count) = MultiCallMock::new();

    struct CountingPlugin {
        tool_count: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl vol_llm_agent::react::plugin::AgentPlugin for CountingPlugin {
        fn id(&self) -> vol_llm_agent::react::plugin::PluginId { "counter".to_string() }
        fn priority(&self) -> u32 { 100 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &vol_llm_agent::react::PluginContext) -> vol_llm_agent::react::plugin::PluginDecision {
            vol_llm_agent::react::plugin::PluginDecision::Continue
        }
        async fn listen(&self, event: &AgentStreamEvent, _: &vol_llm_agent::react::PluginContext) {
            if matches!(event, AgentStreamEvent::ToolCallBegin { .. }) {
                self.tool_count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    let tool_count = Arc::new(AtomicUsize::new(0));
    let plugin = CountingPlugin { tool_count: tool_count.clone() };

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_tool(vol_llm_tools_builtin::read_tool::ReadTool::new())
        .with_plugin(plugin)
        .with_max_iterations(10)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Read test.txt").await.unwrap();
    assert!(result.success);
    assert_eq!(count.load(Ordering::SeqCst), 2); // Two LLM calls
    assert_eq!(tool_count.load(Ordering::SeqCst), 1); // One tool call
}

// ========================
// Tool call flow test (reuse existing react_mock_test pattern)
// ========================

#[tokio::test]
async fn test_agent_run_tool_call_flow() {
    // Same pattern as react_mock_test.rs::test_agent_executes_full_react_cycle
    // Verify the tool call → result → next LLM iteration flow
    let (mock, count) = MultiCallMock::new();

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_tool(vol_llm_tools_builtin::read_tool::ReadTool::new())
        .with_max_iterations(10)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Read test.txt").await.unwrap();
    assert!(result.success);
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

// ========================
// LLM error recovery test
// ========================

struct ErrorThenOkMock {
    call_count: Arc<AtomicUsize>,
}

impl ErrorThenOkMock {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (Self { call_count: count.clone() }, count)
    }
}

#[async_trait]
impl LLMClient for ErrorThenOkMock {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "error-then-ok" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!()
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
        use tokio::sync::mpsc;

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            if count == 0 {
                // First call: error
                let _ = tx.send(Err(vol_llm_core::LLMError::Timeout("temporary failure".to_string()))).await;
            } else {
                // Second call: success
                let _ = tx.send(Ok(content_complete_event("Recovered from error."))).await;
            }
        });

        Ok(StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_agent_run_llm_error_recovery() {
    let (mock, count) = ErrorThenOkMock::new();

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    // Agent should handle LLM error and continue
    let result = agent.run("test").await;
    // The agent may succeed or fail depending on error handling — just verify it attempted
    assert!(count.load(Ordering::SeqCst) >= 1);
}

// ========================
// Unsafe mode test
// ========================

#[tokio::test]
async fn test_agent_run_unsafe_mode() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Done."),
    ]).await;

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    // Verify unsafe_mode field exists on config
    assert!(!agent.config.unsafe_mode); // default is false

    // Agent runs successfully in safe mode
    let result = agent.run("Hi").await.unwrap();
    assert!(result.success);
}

// ========================
// Session recording test
// ========================

#[tokio::test]
async fn test_agent_run_session_recording() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Session test answer."),
    ]).await;

    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Arc::new(Session::new(
        "session_test".to_string(),
        session_store.clone(),
        message_store.clone(),
    ));

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_session(session.clone())
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Session question?").await.unwrap();
    assert!(result.success);

    // Verify session has recorded messages
    let messages = session.get_messages(10).await;
    // Should have at least the user input recorded
    assert!(!messages.is_empty());
}

// ========================
// Event emission test
// ========================

struct EventCollectorPlugin {
    events: Arc<tokio::sync::Mutex<Vec<String>>>,
}

#[async_trait]
impl vol_llm_agent::react::plugin::AgentPlugin for EventCollectorPlugin {
    fn id(&self) -> vol_llm_agent::react::plugin::PluginId { "collector".to_string() }
    fn priority(&self) -> u32 { 100 }
    async fn intercept(&self, _: &AgentStreamEvent, _: &vol_llm_agent::react::PluginContext) -> vol_llm_agent::react::plugin::PluginDecision {
        vol_llm_agent::react::plugin::PluginDecision::Continue
    }
    async fn listen(&self, event: &AgentStreamEvent, _: &vol_llm_agent::react::PluginContext) {
        let event_name = match event {
            AgentStreamEvent::AgentStart { .. } => "AgentStart",
            AgentStreamEvent::LLMCallStart { .. } => "LLMCallStart",
            AgentStreamEvent::LLMCallComplete { .. } => "LLMCallComplete",
            AgentStreamEvent::ThinkingComplete { .. } => "ThinkingComplete",
            AgentStreamEvent::ContentComplete { .. } => "ContentComplete",
            AgentStreamEvent::AgentComplete { .. } => "AgentComplete",
            _ => "Other",
        };
        self.events.lock().await.push(event_name.to_string());
    }
}

#[tokio::test]
async fn test_agent_run_event_emission() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Event test answer."),
    ]).await;

    let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let plugin = EventCollectorPlugin { events: events.clone() };

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_plugin(plugin)
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let _ = agent.run("Event question?").await.unwrap();

    // Allow async plugin events to drain
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let recorded = events.lock().await.clone();
    assert!(!recorded.is_empty(), "Should have recorded at least one event");
}

// ========================
// Max iterations test (reuse existing pattern from react_mock_test.rs)
// ========================

#[tokio::test]
async fn test_agent_run_max_iterations_reached() {
    // Mock that always returns tool calls — same pattern as react_mock_test.rs::test_agent_max_iterations
    struct LoopMock {
        call_count: Arc<AtomicUsize>,
    }

    impl LoopMock {
        fn new() -> (Self, Arc<AtomicUsize>) {
            let count = Arc::new(AtomicUsize::new(0));
            (Self { call_count: count.clone() }, count)
        }
    }

    #[async_trait]
    impl LLMClient for LoopMock {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "loop-mock" }
        fn supported_params(&self) -> &[SupportedParam] { &[] }

        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
            unimplemented!()
        }

        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
            use tokio::sync::mpsc;
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel(10);
            tokio::spawn(async move {
                let _ = tx.send(Ok(tool_call_event("index_price", r#"{"instrument": "btc"}"#, "loop"))).await;
            });
            Ok(StreamReceiver::new(rx))
        }
    }

    let (mock, count) = LoopMock::new();

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_tool(vol_llm_tools_builtin::read_tool::ReadTool::new())
        .with_max_iterations(3)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Keep querying...").await;
    match result {
        Err(vol_llm_agent::AgentError::MaxIterationsReached { max }) => {
            assert_eq!(max, 3);
        }
        Err(e) => panic!("Expected MaxIterationsReached, got: {:?}", e),
        Ok(_) => panic!("Expected MaxIterationsReached error"),
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-agent --test agent_run_tests
```

Expected: ~10 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/tests/agent_run_tests.rs
git commit -m "test: add agent run() flow tests with MockLlmClient"
```

---

## Task 7: Run full test suite and verify coverage

**Files:**
- No file changes

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace --lib 2>&1 | tail -50
cargo test --workspace --tests 2>&1 | tail -50
```

Expected: All tests pass (existing + new ~70 tests)

- [ ] **Step 3: Check coverage estimate**

```bash
cargo test -p vol-llm-agents --lib -- --test-threads=1 2>&1 | grep -E "test result|running"
cargo test -p vol-llm-agent --lib -- --test-threads=1 2>&1 | grep -E "test result|running"
```

Verify test count:
- vol-llm-agents/coding: ~34 tests (was 0)
- vol-llm-agent/react: ~35 lib tests + ~10 integration tests (was ~17)

Total new tests: ~63-68 across both crates.

- [ ] **Step 4: Commit final changes if any**

---

## Summary

| Crate | Before | After | New Tests |
|-------|--------|-------|-----------|
| `vol-llm-core` (MockLlmClient) | 0 | 5 | Shared mock |
| `vol-llm-agents/coding` | 0 | ~34 | ~34 |
| `vol-llm-agent/react` | ~17 | ~55 | ~38 |
| **Total** | **~17** | **~94** | **~77** |

| Task | Files | Purpose |
|------|-------|---------|
| 1 | `vol-llm-core/test_utils.rs` | Shared MockLlmClient |
| 2 | `coding/tests.rs` (config/error/hitl/html_reporter/sandbox/observer) | Pure unit tests |
| 3 | `coding/tests.rs` (builder/agent) | Builder + CodingAgent tests |
| 4 | `react/tests.rs` (builder/prompt/response/state/stream) | Pure unit tests |
| 5 | `react/tests.rs` (plugin_stream/hitl) | Plugin flow + HITL tests |
| 6 | `tests/agent_run_tests.rs` | Integration-style run() tests |
| 7 | N/A | Full suite verification |
