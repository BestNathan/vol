# vol-agent-manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone control plane service (`vol-agent-manager` crate) that manages agents across multiple hosts via WebSocket, exposes Prometheus metrics, and provides REST APIs for management and task dispatch.

**Architecture:** Single Rust binary using axum for HTTP + WebSocket server, in-memory state with RwLock, Prometheus crate for metrics, SSE for real-time event streaming. Agents connect as data plane nodes, register themselves, and report heartbeats/metrics/events.

**Tech Stack:** Rust, axum, tokio, serde_json, prometheus crate, tracing, SSE (axum::response::sse)

---

### Task 1: Crate scaffold, configuration, and message protocol types

**Files:**
- Modify: `Cargo.toml` (workspace root — add vol-agent-manager to members and deps)
- Create: `crates/vol-agent-manager/Cargo.toml`
- Create: `crates/vol-agent-manager/src/lib.rs`
- Create: `crates/vol-agent-manager/src/main.rs`
- Create: `crates/vol-agent-manager/src/config.rs`
- Create: `crates/vol-agent-manager/src/ws/protocol.rs`
- Create: `crates/vol-agent-manager/src/ws/mod.rs`

- [ ] **Step 1: Add vol-agent-manager to workspace Cargo.toml**

Add to `members` list in root `Cargo.toml`:
```toml
    "crates/vol-agent-manager",
```

Add workspace dependencies:
```toml
axum = { version = "0.7", features = ["ws"] }
tower = "0.5"
tower-http = { version = "0.5", features = ["cors", "fs"] }
prometheus = "0.13"
```

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "vol-agent-manager"
version.workspace = true
edition.workspace = true

[dependencies]
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
prometheus = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
futures = "0.3"

[dev-dependencies]
tokio-tungstenite = { version = "0.21", features = ["rustls-tls-webpki-roots"] }
http-body-util = "0.1"
http = "1.1"
```

- [ ] **Step 3: Create config.rs**

```rust
//! Configuration for vol-agent-manager.

use serde::{Deserialize, Serialize};

/// Top-level configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManagerConfig {
    pub server: ServerConfig,
    pub health: HealthConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Listen address, e.g. "0.0.0.0:8080"
    pub listen_addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthConfig {
    /// How often to check heartbeats (seconds)
    pub check_interval_secs: u64,
    /// Heartbeat timeout threshold (seconds)
    pub heartbeat_timeout_secs: u64,
    /// How long to retain disconnected agent state (seconds)
    pub disconnect_retention_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// Optional token for WebSocket and REST auth
    pub token: Option<String>,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: "0.0.0.0:8080".to_string(),
            },
            health: HealthConfig {
                check_interval_secs: 15,
                heartbeat_timeout_secs: 90,
                disconnect_retention_secs: 300,
            },
            security: SecurityConfig { token: None },
        }
    }
}

impl ManagerConfig {
    /// Load from a TOML file path.
    pub fn from_path(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: ManagerConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
```

- [ ] **Step 4: Create ws/protocol.rs — message types**

```rust
//! WebSocket message protocol types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unified message envelope for WebSocket communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub message_type: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub target_agent_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl WsMessage {
    /// Create an agent→control message.
    pub fn agent_report(message_type: &str, agent_id: &str, payload: serde_json::Value) -> Self {
        Self {
            message_type: message_type.to_string(),
            agent_id: Some(agent_id.to_string()),
            task_id: None,
            target_agent_id: None,
            timestamp: Some(Utc::now().to_rfc3339()),
            payload,
        }
    }

    /// Create a control→agent message.
    pub fn control_command(
        message_type: &str,
        target_agent_id: &str,
        task_id: &str,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            message_type: message_type.to_string(),
            agent_id: None,
            task_id: Some(task_id.to_string()),
            target_agent_id: Some(target_agent_id.to_string()),
            timestamp: Some(Utc::now().to_rfc3339()),
            payload,
        }
    }

    /// Create an error response to agent.
    pub fn error(agent_id: &str, error: &str) -> Self {
        Self {
            message_type: "error".to_string(),
            agent_id: Some(agent_id.to_string()),
            task_id: None,
            target_agent_id: None,
            timestamp: Some(Utc::now().to_rfc3339()),
            payload: serde_json::json!({"error": error}),
        }
    }
}

// --- Payload types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPayload {
    pub name: String,
    pub r#type: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub host_info: HostInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAckPayload {
    pub agent_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub status: String, // "Idle" | "Busy"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPayload {
    pub samples: Vec<MetricSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub name: String,
    pub value: f64,
    #[serde(default)]
    pub labels: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    #[serde(default)]
    pub run_id: Option<String>,
    pub event_name: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPayload {
    pub task_type: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultPayload {
    pub status: String, // "Completed" | "Failed"
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub ip: String,
}
```

- [ ] **Step 5: Create ws/mod.rs**

```rust
pub mod protocol;
```

- [ ] **Step 6: Create lib.rs**

```rust
pub mod config;
pub mod ws;
```

- [ ] **Step 7: Create main.rs (minimal stub)**

```rust
use anyhow::Result;
use tracing::info;

fn parse_args() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--config" || args[i] == "-c" {
            if i + 1 < args.len() {
                return Some(args[i + 1].clone());
            }
            eprintln!("Error: --config requires a file path");
            std::process::exit(1);
        }
        i += 1;
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vol_agent_manager=info".into()),
        )
        .init();

    let config_path = parse_args().unwrap_or_else(|| "config.toml".to_string());
    let config = vol_agent_manager::config::ManagerConfig::from_path(&config_path)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to load {}: {}, using defaults", config_path, e);
            vol_agent_manager::config::ManagerConfig::default()
        });

    info!("vol-agent-manager starting on {}", config.server.listen_addr);
    info!("Press Ctrl+C to stop");

    // TODO: start server
    tokio::signal::ctrl_c().await?;
    info!("Shutting down");
    Ok(())
}
```

- [ ] **Step 8: Verify compilation**

Run: `cargo check -p vol-agent-manager`
Expected: compiles successfully

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml crates/vol-agent-manager/
git commit -m "feat: scaffold vol-agent-manager crate with config and protocol types"
```

---

### Task 2: Agent State Manager and models

**Files:**
- Create: `crates/vol-agent-manager/src/state/mod.rs`
- Create: `crates/vol-agent-manager/src/state/models.rs`
- Create: `crates/vol-agent-manager/src/state/manager.rs`
- Modify: `crates/vol-agent-manager/src/lib.rs` (add state module)
- Test: `crates/vol-agent-manager/src/state/manager.rs` (inline tests)

- [ ] **Step 1: Write tests for AgentState and AgentStatus**

Add to `state/models.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum AgentStatus {
    Connected,
    Idle,
    Busy,
    Disconnected,
    Dead,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentState {
    pub agent_id: String,
    pub name: String,
    pub r#type: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub host_info: HostInfo,
    pub status: AgentStatus,
    pub connected_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_serialization() {
        let status = AgentStatus::Connected;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Connected"));
    }

    #[test]
    fn test_agent_state_creation() {
        let state = AgentState {
            agent_id: "repo:test-agent".to_string(),
            name: "test-agent".to_string(),
            r#type: "test".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["Read".to_string()],
            host_info: HostInfo {
                hostname: "host1".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                ip: "10.0.0.1".to_string(),
            },
            status: AgentStatus::Connected,
            connected_at: Utc::now(),
            last_heartbeat: Utc::now(),
        };
        assert_eq!(state.agent_id, "repo:test-agent");
        assert_eq!(state.status, AgentStatus::Connected);
    }
}
```

- [ ] **Step 2: Implement models**

Create `state/models.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum AgentStatus {
    Connected,
    Idle,
    Busy,
    Disconnected,
    Dead,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentState {
    pub agent_id: String,
    pub name: String,
    pub r#type: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub host_info: HostInfo,
    pub status: AgentStatus,
    pub connected_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}
```

- [ ] **Step 3: Run tests for models**

Run: `cargo test -p vol-agent-manager state::models -- --nocapture`
Expected: 2 tests pass

- [ ] **Step 4: Write tests for AgentStateManager**

Add inline tests section in `state/manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::models::{AgentStatus, HostInfo};

    fn make_state(id: &str) -> AgentState {
        AgentState {
            agent_id: id.to_string(),
            name: id.to_string(),
            r#type: "test".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![],
            host_info: HostInfo {
                hostname: "h".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                ip: "127.0.0.1".to_string(),
            },
            status: AgentStatus::Connected,
            connected_at: Utc::now(),
            last_heartbeat: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let mgr = AgentStateManager::new();
        let state = make_state("agent-1");
        mgr.register(state).await;
        let got = mgr.get("agent-1").await;
        assert!(got.is_some());
        assert_eq!(got.unwrap().agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_list_all() {
        let mgr = AgentStateManager::new();
        mgr.register(make_state("a")).await;
        mgr.register(make_state("b")).await;
        let all = mgr.list_all().await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_update_heartbeat() {
        let mgr = AgentStateManager::new();
        mgr.register(make_state("agent-1")).await;
        let before = mgr.get("agent-1").await.unwrap().last_heartbeat;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        mgr.update_heartbeat("agent-1").await;
        let after = mgr.get("agent-1").await.unwrap().last_heartbeat;
        assert!(after > before);
    }

    #[tokio::test]
    async fn test_update_status() {
        let mgr = AgentStateManager::new();
        mgr.register(make_state("agent-1")).await;
        mgr.update_status("agent-1", AgentStatus::Busy).await;
        let state = mgr.get("agent-1").await.unwrap();
        assert_eq!(state.status, AgentStatus::Busy);
    }

    #[tokio::test]
    async fn test_register_overwrites_existing() {
        let mgr = AgentStateManager::new();
        let mut s1 = make_state("dup");
        s1.version = "v1".to_string();
        mgr.register(s1).await;

        let mut s2 = make_state("dup");
        s2.version = "v2".to_string();
        mgr.register(s2).await;

        let got = mgr.get("dup").await.unwrap();
        assert_eq!(got.version, "v2");
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let mgr = AgentStateManager::new();
        assert!(mgr.get("nope").await.is_none());
    }
}
```

- [ ] **Step 5: Implement AgentStateManager**

Create `state/manager.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::models::{AgentState, AgentStatus};

/// Thread-safe store for agent states.
pub struct AgentStateManager {
    agents: Arc<RwLock<HashMap<String, AgentState>>>,
}

impl AgentStateManager {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register or re-register an agent (overwrites existing).
    pub async fn register(&self, state: AgentState) {
        let mut guard = self.agents.write().await;
        guard.insert(state.agent_id.clone(), state);
    }

    /// Get agent state by ID.
    pub async fn get(&self, agent_id: &str) -> Option<AgentState> {
        let guard = self.agents.read().await;
        guard.get(agent_id).cloned()
    }

    /// Update heartbeat timestamp.
    pub async fn update_heartbeat(&self, agent_id: &str) {
        let mut guard = self.agents.write().await;
        if let Some(state) = guard.get_mut(agent_id) {
            state.last_heartbeat = chrono::Utc::now();
        }
    }

    /// Update agent status.
    pub async fn update_status(&self, agent_id: &str, status: AgentStatus) {
        let mut guard = self.agents.write().await;
        if let Some(state) = guard.get_mut(agent_id) {
            state.status = status;
        }
    }

    /// List all agents.
    pub async fn list_all(&self) -> Vec<AgentState> {
        let guard = self.agents.read().await;
        guard.values().cloned().collect()
    }
}

impl Default for AgentStateManager {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 6: Run tests for manager**

Run: `cargo test -p vol-agent-manager state::manager -- --nocapture`
Expected: 6 tests pass

- [ ] **Step 7: Update lib.rs and state/mod.rs**

Update `state/mod.rs`:

```rust
pub mod manager;
pub mod models;
```

Update `lib.rs`:

```rust
pub mod config;
pub mod state;
pub mod ws;
```

- [ ] **Step 8: Commit**

```bash
git add crates/vol-agent-manager/src/state/ crates/vol-agent-manager/src/lib.rs
git commit -m "feat: add AgentStateManager with models and tests"
```

---

### Task 3: Prometheus metrics collector

**Files:**
- Create: `crates/vol-agent-manager/src/metrics/mod.rs`
- Create: `crates/vol-agent-manager/src/metrics/collector.rs`
- Modify: `crates/vol-agent-manager/src/lib.rs` (add metrics module)
- Test: `crates/vol-agent-manager/src/metrics/collector.rs` (inline tests)

- [ ] **Step 1: Write tests for metrics collector**

Add to `metrics/collector.rs` inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let mc = MetricsCollector::new();
        let output = mc.gather();
        // Should contain metric family names
        assert!(!output.is_empty());
    }

    #[test]
    fn test_increment_connections() {
        let mc = MetricsCollector::new();
        mc.agent_connections_current.set(1.0);
        assert_eq!(mc.agent_connections_current.get() as i64, 1);
    }

    #[test]
    fn test_increment_messages() {
        let mc = MetricsCollector::new();
        mc.increment_messages("heartbeat", "agent-1", "react-agent");
        let output = mc.gather();
        let text = output.iter().next().unwrap().get_name().to_string();
        assert!(output.iter().any(|m| m.get_name() == "agent_messages_total"));
    }
}
```

- [ ] **Step 2: Implement MetricsCollector**

Create `metrics/collector.rs`:

```rust
use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry,
    register_gauge_with_registry, register_histogram_vec_with_registry, HistogramVec, Registry,
};
use prometheus::{CounterVec, Gauge, GaugeVec};

/// Prometheus metrics for the agent manager.
pub struct MetricsCollector {
    pub registry: Registry,
    pub agent_connections_current: Gauge,
    pub agent_registered_total: Gauge,
    pub agent_messages_total: CounterVec,
    pub agent_status_count: GaugeVec,
    pub agent_metric_samples_total: CounterVec,
    pub agent_heartbeat_latency_seconds: HistogramVec,
    pub agent_task_duration_seconds: HistogramVec,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Registry::new_custom("agent_manager", None).unwrap();

        let agent_connections_current =
            register_gauge_with_registry!("agent_connections_current", "Current active WebSocket connections", registry).unwrap();

        let agent_registered_total =
            register_gauge_with_registry!("agent_registered_total", "Total registered agents", registry).unwrap();

        let agent_messages_total = register_counter_vec_with_registry!(
            "agent_messages_total",
            "Total messages received by type",
            &["message_type", "agent_id", "agent_type"],
            registry
        ).unwrap();

        let agent_status_count = register_gauge_vec_with_registry!(
            "agent_status_count",
            "Count of agents in each status",
            &["status"],
            registry
        ).unwrap();

        let agent_metric_samples_total = register_counter_vec_with_registry!(
            "agent_metric_samples_total",
            "Total metric samples received",
            &["agent_id"],
            registry
        ).unwrap();

        let agent_heartbeat_latency_seconds = register_histogram_vec_with_registry!(
            "agent_heartbeat_latency_seconds",
            "Heartbeat round-trip latency",
            &["agent_id", "agent_type"],
            registry
        ).unwrap();

        let agent_task_duration_seconds = register_histogram_vec_with_registry!(
            "agent_task_duration_seconds",
            "Task execution duration",
            &["task_type", "agent_id", "status"],
            registry
        ).unwrap();

        Self {
            registry,
            agent_connections_current,
            agent_registered_total,
            agent_messages_total,
            agent_status_count,
            agent_metric_samples_total,
            agent_heartbeat_latency_seconds,
            agent_task_duration_seconds,
        }
    }

    /// Increment message counter.
    pub fn increment_messages(&self, message_type: &str, agent_id: &str, agent_type: &str) {
        self.agent_messages_total
            .with_label_values(&[message_type, agent_id, agent_type])
            .inc();
    }

    /// Increment metric samples counter.
    pub fn increment_metric_samples(&self, agent_id: &str) {
        self.agent_metric_samples_total
            .with_label_values(&[agent_id])
            .inc();
    }

    /// Observe heartbeat latency.
    pub fn observe_heartbeat_latency(&self, agent_id: &str, agent_type: &str, seconds: f64) {
        self.agent_heartbeat_latency_seconds
            .with_label_values(&[agent_id, agent_type])
            .observe(seconds);
    }

    /// Observe task duration.
    pub fn observe_task_duration(&self, task_type: &str, agent_id: &str, status: &str, seconds: f64) {
        self.agent_task_duration_seconds
            .with_label_values(&[task_type, agent_id, status])
            .observe(seconds);
    }

    /// Gather all metrics for /metrics endpoint.
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-agent-manager metrics::collector -- --nocapture`
Expected: 3 tests pass

- [ ] **Step 4: Update lib.rs**

```rust
pub mod config;
pub mod metrics;
pub mod state;
pub mod ws;
```

- [ ] **Step 5: Update metrics/mod.rs**

```rust
pub mod collector;
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-manager/src/metrics/ crates/vol-agent-manager/src/lib.rs
git commit -m "feat: add Prometheus metrics collector"
```

---

### Task 4: SSE event stream and events module

**Files:**
- Create: `crates/vol-agent-manager/src/events/mod.rs`
- Create: `crates/vol-agent-manager/src/events/sse.rs`
- Modify: `crates/vol-agent-manager/src/lib.rs` (add events module)
- Test: `crates/vol-agent-manager/src/events/sse.rs` (inline tests)

- [ ] **Step 1: Write tests for event system**

Add to `events/sse.rs` inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_emit_and_receive() {
        let bus = EventBus::new();
        bus.emit(ManagerEvent::agent_registered("agent-1"));
        let events = bus.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "agent_registered");
    }

    #[tokio::test]
    async fn test_event_serialization() {
        let event = ManagerEvent::agent_registered("agent-1");
        let json = event.to_json_string();
        assert!(json.contains("agent_registered"));
        assert!(json.contains("agent-1"));
    }

    #[test]
    fn test_agent_dead_event() {
        let event = ManagerEvent::agent_dead("agent-1");
        assert_eq!(event.event_type, "agent_dead");
        assert_eq!(event.agent_id, "agent-1");
    }
}
```

- [ ] **Step 2: Implement event bus**

Create `events/sse.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;

/// Internal event emitted by the control plane.
#[derive(Debug, Clone, Serialize)]
pub struct ManagerEvent {
    pub event_type: String,
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ManagerEvent {
    pub fn agent_registered(agent_id: &str) -> Self {
        Self {
            event_type: "agent_registered".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: None,
        }
    }

    pub fn agent_disconnected(agent_id: &str) -> Self {
        Self {
            event_type: "agent_disconnected".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: None,
        }
    }

    pub fn agent_dead(agent_id: &str) -> Self {
        Self {
            event_type: "agent_dead".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: None,
        }
    }

    pub fn task_dispatched(task_id: &str, agent_id: &str) -> Self {
        Self {
            event_type: "task_dispatched".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id})),
        }
    }

    pub fn task_completed(task_id: &str, agent_id: &str) -> Self {
        Self {
            event_type: "task_completed".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id})),
        }
    }

    pub fn task_failed(task_id: &str, agent_id: &str, error: &str) -> Self {
        Self {
            event_type: "task_failed".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id, "error": error})),
        }
    }

    pub fn task_timeout(task_id: &str, agent_id: &str) -> Self {
        Self {
            event_type: "task_timeout".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id})),
        }
    }

    pub fn agent_event(agent_id: &str, event_name: &str, data: serde_json::Value) -> Self {
        Self {
            event_type: "agent_event".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"event_name": event_name, "data": data})),
        }
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Broadcast bus for manager events.
pub struct EventBus {
    tx: broadcast::Sender<ManagerEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn emit(&self, event: ManagerEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ManagerEvent> {
        self.tx.subscribe()
    }

    /// Drain all pending events (for testing).
    pub fn drain(&self) -> Vec<ManagerEvent> {
        let mut rx = self.tx.subscribe();
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-agent-manager events::sse -- --nocapture`
Expected: 3 tests pass

- [ ] **Step 4: Update lib.rs and events/mod.rs**

`events/mod.rs`:

```rust
pub mod sse;
pub use sse::{EventBus, ManagerEvent};
```

Update `lib.rs`:

```rust
pub mod config;
pub mod events;
pub mod metrics;
pub mod state;
pub mod ws;
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/src/events/ crates/vol-agent-manager/src/lib.rs
git commit -m "feat: add SSE event bus and event types"
```

---

### Task 5: Health Checker

**Files:**
- Create: `crates/vol-agent-manager/src/health/mod.rs`
- Create: `crates/vol-agent-manager/src/health/checker.rs`
- Modify: `crates/vol-agent-manager/src/lib.rs` (add health module)
- Test: `crates/vol-agent-manager/src/health/checker.rs` (inline tests)

- [ ] **Step 1: Write tests for HealthChecker**

Add to `health/checker.rs` inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::manager::AgentStateManager;
    use crate::state::models::{AgentState, AgentStatus, HostInfo};

    fn make_state(id: &str, last_hb: DateTime<Utc>) -> AgentState {
        AgentState {
            agent_id: id.to_string(),
            name: id.to_string(),
            r#type: "test".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![],
            host_info: HostInfo {
                hostname: "h".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                ip: "127.0.0.1".to_string(),
            },
            status: AgentStatus::Idle,
            connected_at: Utc::now(),
            last_heartbeat: last_hb,
        }
    }

    #[tokio::test]
    async fn test_checker_marks_stale_agents_as_dead() {
        let mgr = Arc::new(AgentStateManager::new());
        let stale_time = Utc::now() - chrono::Duration::seconds(100);
        mgr.register(make_state("stale-agent", stale_time)).await;

        let checker = HealthChecker::new(
            mgr.clone(),
            Duration::from_secs(15),
            Duration::from_secs(90),
            None,
        );
        checker.run_once().await;

        let state = mgr.get("stale-agent").await.unwrap();
        assert_eq!(state.status, AgentStatus::Dead);
    }

    #[tokio::test]
    async fn test_checker_ignores_fresh_agents() {
        let mgr = Arc::new(AgentStateManager::new());
        let fresh_time = Utc::now();
        mgr.register(make_state("fresh-agent", fresh_time)).await;

        let checker = HealthChecker::new(
            mgr.clone(),
            Duration::from_secs(15),
            Duration::from_secs(90),
            None,
        );
        checker.run_once().await;

        let state = mgr.get("fresh-agent").await.unwrap();
        assert_ne!(state.status, AgentStatus::Dead);
    }
}
```

- [ ] **Step 2: Implement HealthChecker**

Create `health/checker.rs`:

```rust
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::{debug, info};

use crate::events::{EventBus, ManagerEvent};
use crate::state::manager::AgentStateManager;
use crate::state::models::AgentStatus;

/// Periodic health checker that scans agent heartbeats.
pub struct HealthChecker {
    state_manager: Arc<AgentStateManager>,
    check_interval: Duration,
    heartbeat_timeout: Duration,
    event_bus: Option<Arc<EventBus>>,
}

impl HealthChecker {
    pub fn new(
        state_manager: Arc<AgentStateManager>,
        check_interval: Duration,
        heartbeat_timeout: Duration,
        event_bus: Option<Arc<EventBus>>,
    ) -> Self {
        Self {
            state_manager,
            check_interval,
            heartbeat_timeout,
            event_bus,
        }
    }

    /// Run a single scan of all agents.
    pub async fn run_once(&self) {
        let agents = self.state_manager.list_all().await;
        let now = Utc::now();

        for agent in &agents {
            let elapsed = now.signed_duration_since(agent.last_heartbeat);
            let timeout = chrono::Duration::from_std(self.heartbeat_timeout).unwrap();

            if elapsed > timeout && agent.status != AgentStatus::Dead {
                info!(agent_id = %agent.agent_id, "Agent heartbeat timed out, marking as dead");
                self.state_manager
                    .update_status(&agent.agent_id, AgentStatus::Dead)
                    .await;
                if let Some(ref bus) = self.event_bus {
                    bus.emit(ManagerEvent::agent_dead(&agent.agent_id));
                }
            } else if elapsed <= timeout && agent.status == AgentStatus::Dead {
                debug!(agent_id = %agent.agent_id, "Dead agent has recent heartbeat, restoring");
                self.state_manager
                    .update_status(&agent.agent_id, AgentStatus::Connected)
                    .await;
            }
        }
    }

    /// Run the checker in a loop.
    pub async fn run_loop(self: Arc<Self>) {
        loop {
            tokio::time::sleep(self.check_interval).await;
            self.run_once().await;
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-agent-manager health::checker -- --nocapture`
Expected: 2 tests pass

- [ ] **Step 4: Update lib.rs and health/mod.rs**

`health/mod.rs`:

```rust
pub mod checker;
pub use checker::HealthChecker;
```

Update `lib.rs`:

```rust
pub mod config;
pub mod events;
pub mod health;
pub mod metrics;
pub mod state;
pub mod ws;
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/src/health/ crates/vol-agent-manager/src/lib.rs
git commit -m "feat: add health checker with heartbeat timeout detection"
```

---

### Task 6: Command Dispatcher (task management)

**Files:**
- Create: `crates/vol-agent-manager/src/task/mod.rs`
- Create: `crates/vol-agent-manager/src/task/dispatcher.rs`
- Modify: `crates/vol-agent-manager/src/lib.rs` (add task module)
- Test: `crates/vol-agent-manager/src/task/dispatcher.rs` (inline tests)

- [ ] **Step 1: Write tests for TaskDispatcher**

Add to `task/dispatcher.rs` inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_create_task() {
        let td = TaskDispatcher::new();
        let task = td.create_task(
            "agent-1",
            "run-query",
            json!({"query": "select *"}),
            Some(Duration::from_secs(60)),
        ).await;
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.agent_id, "agent-1");
        assert_eq!(task.task_type, "run-query");

        let got = td.get_task(&task.id).await;
        assert!(got.is_some());
    }

    #[tokio::test]
    async fn test_update_task_status() {
        let td = TaskDispatcher::new();
        let task = td.create_task("agent-1", "test", json!({}), None).await;
        let id = task.id.clone();
        td.update_status(&id, TaskStatus::Dispatched).await;
        let got = td.get_task(&id).await.unwrap();
        assert_eq!(got.status, TaskStatus::Dispatched);
    }

    #[tokio::test]
    async fn test_complete_task() {
        let td = TaskDispatcher::new();
        let task = td.create_task("agent-1", "test", json!({}), None).await;
        let id = task.id.clone();
        td.complete_task(&id, Some(json!({"result": "ok"})), None).await;
        let got = td.get_task(&id).await.unwrap();
        assert_eq!(got.status, TaskStatus::Completed);
        assert!(got.result.is_some());
    }

    #[tokio::test]
    async fn test_fail_task() {
        let td = TaskDispatcher::new();
        let task = td.create_task("agent-1", "test", json!({}), None).await;
        let id = task.id.clone();
        td.fail_task(&id, "something went wrong").await;
        let got = td.get_task(&id).await.unwrap();
        assert_eq!(got.status, TaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let td = TaskDispatcher::new();
        td.create_task("a", "t1", json!({}), None).await;
        td.create_task("b", "t2", json!({}), None).await;
        let all = td.list_tasks().await;
        assert_eq!(all.len(), 2);

        let filtered = td.list_tasks_by_agent("a").await;
        assert_eq!(filtered.len(), 1);
    }
}
```

- [ ] **Step 2: Implement TaskDispatcher**

Create `task/dispatcher.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Dispatched,
    Running,
    Completed,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: String,
    pub agent_id: String,
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub timeout: Duration,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatched_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

pub struct TaskDispatcher {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl TaskDispatcher {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new task and return it.
    pub async fn create_task(
        &self,
        agent_id: &str,
        task_type: &str,
        parameters: serde_json::Value,
        timeout: Option<Duration>,
    ) -> Task {
        let id = Uuid::new_v4().to_string();
        let task = Task {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            task_type: task_type.to_string(),
            parameters,
            timeout: timeout.unwrap_or(Duration::from_secs(300)),
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: Utc::now(),
            dispatched_at: None,
            completed_at: None,
        };
        let mut guard = self.tasks.write().await;
        guard.insert(id, task.clone());
        task
    }

    /// Get a task by ID.
    pub async fn get_task(&self, task_id: &str) -> Option<Task> {
        let guard = self.tasks.read().await;
        guard.get(task_id).cloned()
    }

    /// Update task status.
    pub async fn update_status(&self, task_id: &str, status: TaskStatus) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = status;
            if status == TaskStatus::Dispatched {
                task.dispatched_at = Some(Utc::now());
            }
        }
    }

    /// Mark task as completed with optional result.
    pub async fn complete_task(&self, task_id: &str, result: Option<serde_json::Value>, duration_ms: Option<u64>) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = TaskStatus::Completed;
            task.result = result;
            task.completed_at = Some(Utc::now());
        }
    }

    /// Mark task as failed with error message.
    pub async fn fail_task(&self, task_id: &str, error: &str) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = TaskStatus::Failed;
            task.error = Some(error.to_string());
            task.completed_at = Some(Utc::now());
        }
    }

    /// Mark task as timed out.
    pub async fn timeout_task(&self, task_id: &str) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = TaskStatus::Timeout;
            task.completed_at = Some(Utc::now());
        }
    }

    /// List all tasks.
    pub async fn list_tasks(&self) -> Vec<Task> {
        let guard = self.tasks.read().await;
        guard.values().cloned().collect()
    }

    /// List tasks for a specific agent.
    pub async fn list_tasks_by_agent(&self, agent_id: &str) -> Vec<Task> {
        let guard = self.tasks.read().await;
        guard
            .values()
            .filter(|t| t.agent_id == agent_id)
            .cloned()
            .collect()
    }
}

impl Default for TaskDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-agent-manager task::dispatcher -- --nocapture`
Expected: 5 tests pass

- [ ] **Step 4: Update lib.rs and task/mod.rs**

`task/mod.rs`:

```rust
pub mod dispatcher;
pub use dispatcher::{Task, TaskDispatcher, TaskStatus};
```

Update `lib.rs`:

```rust
pub mod config;
pub mod events;
pub mod health;
pub mod metrics;
pub mod state;
pub mod task;
pub mod ws;
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/src/task/ crates/vol-agent-manager/src/lib.rs
git commit -m "feat: add task dispatcher with CRUD operations"
```

---

### Task 7: WebSocket server and handler

This is the core of the system — connecting all components together.

**Files:**
- Create: `crates/vol-agent-manager/src/ws/server.rs`
- Create: `crates/vol-agent-manager/src/ws/handler.rs`
- Modify: `crates/vol-agent-manager/src/ws/mod.rs` (add server, handler)
- Modify: `crates/vol-agent-manager/src/main.rs` (wire up server)

- [ ] **Step 1: Write tests for WebSocket handler (protocol parsing)**

Add to `ws/handler.rs` inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::protocol::*;

    #[test]
    fn test_parse_register_message() {
        let json = serde_json::json!({
            "message_type": "register",
            "agent_id": "agent-1",
            "payload": {
                "name": "test-agent",
                "type": "react-agent",
                "version": "0.1.0",
                "capabilities": ["Read", "Bash"],
                "host_info": {
                    "hostname": "host1",
                    "os": "linux",
                    "arch": "x86_64",
                    "ip": "10.0.0.1"
                }
            }
        });
        let msg: WsMessage = serde_json::from_value(json).unwrap();
        assert_eq!(msg.message_type, "register");
        assert_eq!(msg.agent_id.as_deref(), Some("agent-1"));

        let payload: RegisterPayload = serde_json::from_value(msg.payload).unwrap();
        assert_eq!(payload.name, "test-agent");
        assert_eq!(payload.capabilities.len(), 2);
    }

    #[test]
    fn test_parse_heartbeat_message() {
        let json = serde_json::json!({
            "message_type": "heartbeat",
            "agent_id": "agent-1",
            "payload": {
                "status": "Idle"
            }
        });
        let msg: WsMessage = serde_json::from_value(json).unwrap();
        let payload: HeartbeatPayload = serde_json::from_value(msg.payload).unwrap();
        assert_eq!(payload.status, "Idle");
    }

    #[test]
    fn test_parse_invalid_message() {
        let result = serde_json::from_str::<WsMessage>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_ws_message_helpers() {
        let err = WsMessage::error("agent-1", "something broke");
        assert_eq!(err.message_type, "error");
        assert_eq!(err.agent_id.as_deref(), Some("agent-1"));
    }
}
```

- [ ] **Step 2: Implement handler**

Add to existing `ws/handler.rs`:

```rust
use std::sync::Arc;
use axum::extract::ws::{Message, WebSocket};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use tracing::{info, warn, error};

use super::protocol::*;
use crate::events::EventBus;
use crate::metrics::collector::MetricsCollector;
use crate::state::manager::AgentStateManager;
use crate::state::models::{AgentState, AgentStatus, HostInfo};

/// Handle a single WebSocket connection for an agent.
pub async fn handle_agent_connection(
    mut ws: WebSocket,
    token: Option<String>,
    state_manager: Arc<AgentStateManager>,
    metrics: Arc<MetricsCollector>,
    event_bus: Arc<EventBus>,
    expected_token: Option<String>,
) {
    // Auth check
    if let Some(expected) = &expected_token {
        if token.as_ref() != Some(expected) {
            let _ = ws
                .send(Message::Text(
                    serde_json::json!({"error": "invalid token"}).to_string(),
                ))
                .await;
            return;
        }
    }

    let mut agent_id: Option<String> = None;

    // Wait for register message
    match ws.recv().await {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<WsMessage>(&text) {
                Ok(msg) if msg.message_type == "register" => {
                    match serde_json::from_value::<RegisterPayload>(msg.payload) {
                        Ok(payload) => {
                            let id = msg.agent_id.clone().unwrap_or_else(|| {
                                format!("{}:{}", payload.r#type, payload.name)
                            });
                            agent_id = Some(id.clone());

                            let state = AgentState {
                                agent_id: id.clone(),
                                name: payload.name,
                                r#type: payload.r#type,
                                version: payload.version,
                                capabilities: payload.capabilities,
                                host_info: payload.host_info,
                                status: AgentStatus::Idle,
                                connected_at: Utc::now(),
                                last_heartbeat: Utc::now(),
                            };
                            state_manager.register(state).await;
                            metrics.agent_registered_total.set(
                                state_manager.list_all().await.len() as f64,
                            );
                            event_bus.emit(ManagerEvent::agent_registered(&id));
                            metrics.increment_messages("register", &id, &state_manager
                                .get(&id).await.map(|s| s.r#type.clone()).unwrap_or_default());

                            // Send ack
                            let ack = WsMessage {
                                message_type: "register_ack".to_string(),
                                agent_id: Some(id.clone()),
                                task_id: None,
                                target_agent_id: None,
                                timestamp: Some(Utc::now().to_rfc3339()),
                                payload: serde_json::json!({
                                    "agent_id": id,
                                    "status": "ok"
                                }),
                            };
                            let _ = ws
                                .send(Message::Text(serde_json::to_string(&ack).unwrap()))
                                .await;
                        }
                        Err(e) => {
                            warn!("Invalid register payload: {}", e);
                            return;
                        }
                    }
                }
                _ => {
                    warn!("First message was not register, closing connection");
                    return;
                }
            }
        }
        _ => {
            warn!("Connection closed before register");
            return;
        }
    }

    let id = agent_id.clone().unwrap();

    // Message loop
    loop {
        match ws.recv().await {
            Some(Ok(Message::Text(text))) => {
                if let Err(e) = handle_agent_message(
                    &text, &id, &state_manager, &metrics, &event_bus,
                ).await {
                    let err_msg = WsMessage::error(&id, &e.to_string());
                    let _ = ws
                        .send(Message::Text(serde_json::to_string(&err_msg).unwrap()))
                        .await;
                }
            }
            Some(Ok(Message::Close(_))) => {
                info!(agent_id = %id, "Agent disconnected");
                state_manager
                    .update_status(&id, AgentStatus::Disconnected)
                    .await;
                metrics
                    .agent_connections_current
                    .dec();
                event_bus.emit(ManagerEvent::agent_disconnected(&id));
                break;
            }
            Some(Err(e)) => {
                error!(agent_id = %id, "WebSocket error: {}", e);
                break;
            }
            None => {
                info!(agent_id = %id, "Agent connection closed");
                state_manager
                    .update_status(&id, AgentStatus::Disconnected)
                    .await;
                metrics.agent_connections_current.dec();
                event_bus.emit(ManagerEvent::agent_disconnected(&id));
                break;
            }
        }
    }
}

async fn handle_agent_message(
    text: &str,
    agent_id: &str,
    state_manager: &AgentStateManager,
    metrics: &MetricsCollector,
    event_bus: &EventBus,
) -> Result<(), anyhow::Error> {
    let msg: WsMessage = serde_json::from_str(text)?;
    let agent_type = state_manager
        .get(agent_id)
        .await
        .map(|s| s.r#type)
        .unwrap_or_else(|| "unknown".to_string());

    match msg.message_type.as_str() {
        "heartbeat" => {
            let payload: HeartbeatPayload = serde_json::from_value(msg.payload)?;
            state_manager.update_heartbeat(agent_id).await;
            if payload.status == "Busy" {
                state_manager
                    .update_status(agent_id, AgentStatus::Busy)
                    .await;
            } else {
                state_manager
                    .update_status(agent_id, AgentStatus::Idle)
                    .await;
            }
            metrics.increment_messages("heartbeat", agent_id, &agent_type);
        }
        "metric" => {
            let payload: MetricPayload = serde_json::from_value(msg.payload)?;
            metrics.increment_metric_samples(agent_id);
            let sample_count = payload.samples.len();
            tracing::debug!(agent_id, sample_count, "Received metric samples");
            metrics.increment_messages("metric", agent_id, &agent_type);
        }
        "event" => {
            let payload: EventPayload = serde_json::from_value(msg.payload)?;
            event_bus.emit(ManagerEvent::agent_event(
                agent_id,
                &payload.event_name,
                payload.data,
            ));
            metrics.increment_messages("event", agent_id, &agent_type);
        }
        "task_result" => {
            let payload: TaskResultPayload = serde_json::from_value(msg.payload)?;
            if let Some(task_id) = msg.task_id {
                if payload.status == "Completed" {
                    // TaskDispatcher will be wired in from main
                    tracing::info!(task_id, agent_id, "Task completed");
                } else {
                    tracing::warn!(task_id, agent_id, error = ?payload.error, "Task failed");
                }
            }
            metrics.increment_messages("task_result", agent_id, &agent_type);
        }
        unknown => {
            return Err(anyhow::anyhow!("Unknown message type: {}", unknown));
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Run handler tests**

Run: `cargo test -p vol-agent-manager ws::handler -- --nocapture`
Expected: 4 tests pass

- [ ] **Step 4: Implement WebSocket server routes**

Create `ws/server.rs`:

```rust
use std::sync::Arc;

use axum::{
    extract::{Query, WebSocketUpgrade, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::extract::ws::WebSocket;
use tower_http::cors::CorsLayer;

use crate::config::ManagerConfig;
use crate::events::EventBus;
use crate::metrics::collector::MetricsCollector;
use crate::state::manager::AgentStateManager;
use crate::AppRouterState;

#[derive(Debug, serde::Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// Create the full application router.
pub fn create_router(state: AppRouterState) -> Router {
    let ws_handler = get(upgrade_ws);

    Router::new()
        .route("/ws", ws_handler)
        .with_state(state)
        .layer(CorsLayer::permissive())
}

async fn upgrade_ws(
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
    State(state): State<AppRouterState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        crate::ws::handler::handle_agent_connection(
            socket,
            query.token,
            state.state_manager,
            state.metrics,
            state.event_bus,
            state.config.security.token.clone(),
        )
    })
}
```

- [ ] **Step 5: Wire up server in main.rs**

Replace the stub `main.rs`:

```rust
use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use prometheus::TextEncoder;
use tokio::sync::broadcast;
use tracing::info;
use vol_agent_manager::config::ManagerConfig;
use vol_agent_manager::events::{EventBus, ManagerEvent};
use vol_agent_manager::health::HealthChecker;
use vol_agent_manager::metrics::collector::MetricsCollector;
use vol_agent_manager::state::manager::AgentStateManager;
use vol_agent_manager::task::dispatcher::TaskDispatcher;
use vol_agent_manager::ws::server::create_router;

/// Shared state passed to axum handlers.
pub struct AppRouterState {
    pub state_manager: Arc<AgentStateManager>,
    pub metrics: Arc<MetricsCollector>,
    pub event_bus: Arc<EventBus>,
    pub task_dispatcher: Arc<TaskDispatcher>,
    pub config: ManagerConfig,
}

fn parse_args() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--config" || args[i] == "-c" {
            if i + 1 < args.len() {
                return Some(args[i + 1].clone());
            }
            eprintln!("Error: --config requires a file path");
            std::process::exit(1);
        }
        i += 1;
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vol_agent_manager=info,tower_http=info".into()),
        )
        .init();

    let config_path = parse_args().unwrap_or_else(|| "config.toml".to_string());
    let config = ManagerConfig::from_path(&config_path)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to load {}: {}, using defaults", config_path, e);
            ManagerConfig::default()
        });

    let state_manager = Arc::new(AgentStateManager::new());
    let metrics = Arc::new(MetricsCollector::new());
    let event_bus = Arc::new(EventBus::new());
    let task_dispatcher = Arc::new(TaskDispatcher::new());

    let app_state = AppRouterState {
        state_manager: state_manager.clone(),
        metrics: metrics.clone(),
        event_bus: event_bus.clone(),
        task_dispatcher: task_dispatcher.clone(),
        config: config.clone(),
    };

    // Build router with WS route from vol-agent-manager
    let mut app = create_router(app_state.clone());

    // Add HTTP routes (health, metrics, api)
    app = app
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/agents/:id", get(get_agent))
        .route("/api/v1/agents/:id/tasks", post(dispatch_task))
        .route("/api/v1/tasks/:id", get(get_task))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/events", get(events_handler))
        .with_state(app_state.clone());

    // Start health checker in background
    let checker = HealthChecker::new(
        state_manager.clone(),
        std::time::Duration::from_secs(config.health.check_interval_secs),
        std::time::Duration::from_secs(config.health.heartbeat_timeout_secs),
        Some(event_bus.clone()),
    );
    tokio::spawn(Arc::new(checker).run_loop());

    info!("vol-agent-manager listening on {}", config.server.listen_addr);

    let listener = tokio::net::TcpListener::bind(&config.server.listen_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    axum::Json(serde_json::json!({"status": "ok"}))
}

async fn metrics_handler(State(state): State<AppRouterState>) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

async fn list_agents(State(state): State<AppRouterState>) -> impl IntoResponse {
    let agents = state.state_manager.list_all().await;
    axum::Json(serde_json::json!({"agents": agents}))
}

async fn get_agent(
    State(state): State<AppRouterState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.state_manager.get(&id).await {
        Some(agent) => axum::Json(serde_json::json!({"agent": agent})),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "agent not found"})),
        ),
    }
}

async fn events_handler(State(state): State<AppRouterState>) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.event_bus.subscribe();
    let stream = async_stream::stream! {
        let mut rx = rx;
        while let Ok(event) = rx.recv().await {
            yield Ok(Event::default().data(event.to_json_string()));
        }
    };
    Sse::new(stream)
}

async fn dispatch_task(
    State(state): State<AppRouterState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let task_type = body.get("task_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let parameters = body.get("parameters").cloned().unwrap_or(serde_json::json!({}));
    let timeout_secs = body.get("timeout_seconds").and_then(|v| v.as_u64());
    let timeout = timeout_secs.map(std::time::Duration::from_secs);

    let task = state.task_dispatcher
        .create_task(&id, task_type, parameters, timeout)
        .await;
    let task_id = task.id.clone();
    state.task_dispatcher.update_status(&task_id, vol_agent_manager::task::dispatcher::TaskStatus::Dispatched).await;
    state.event_bus.emit(vol_agent_manager::events::ManagerEvent::task_dispatched(&task_id, &id));

    (axum::http::StatusCode::ACCEPTED, axum::Json(serde_json::json!({"task_id": task_id})))
}

async fn get_task(
    State(state): State<AppRouterState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.task_dispatcher.get_task(&id).await {
        Some(task) => axum::Json(serde_json::json!({"task": task})),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "task not found"})),
        ),
    }
}

async fn list_tasks(State(state): State<AppRouterState>) -> impl IntoResponse {
    let tasks = state.task_dispatcher.list_tasks().await;
    axum::Json(serde_json::json!({"tasks": tasks}))
}
```

- [ ] **Step 6: Add async-stream dependency**

Add to `Cargo.toml` dependencies:

```toml
async-stream = "0.3"
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p vol-agent-manager`
Expected: compiles successfully

- [ ] **Step 8: Commit**

```bash
git add crates/vol-agent-manager/src/ws/server.rs crates/vol-agent-manager/src/ws/handler.rs crates/vol-agent-manager/src/ws/mod.rs crates/vol-agent-manager/src/main.rs Cargo.toml
git commit -m "feat: wire up WebSocket server, HTTP routes, health/metrics/SSE endpoints"
```

---

### Task 8: Integration tests and end-to-end verification

**Files:**
- Create: `crates/vol-agent-manager/tests/integration.rs`
- Create: `crates/vol-agent-manager/config.toml` (example config)

- [ ] **Step 1: Write integration test**

Create `tests/integration.rs`:

```rust
use std::sync::Arc;
use std::time::Duration;

use axum::{Router, extract::State, response::IntoResponse, routing::get};
use futures::SinkExt;
use serde_json::json;
use vol_agent_manager::config::ManagerConfig;
use vol_agent_manager::events::EventBus;
use vol_agent_manager::metrics::collector::MetricsCollector;
use vol_agent_manager::state::manager::AgentStateManager;
use vol_agent_manager::state::models::AgentStatus;
use vol_agent_manager::task::dispatcher::TaskDispatcher;
use vol_agent_manager::ws::protocol::WsMessage;
use vol_agent_manager::ws::server::create_router;

struct TestApp {
    state_manager: Arc<AgentStateManager>,
    metrics: Arc<MetricsCollector>,
    event_bus: Arc<EventBus>,
    task_dispatcher: Arc<TaskDispatcher>,
    config: ManagerConfig,
}

fn make_state() -> TestApp {
    TestApp {
        state_manager: Arc::new(AgentStateManager::new()),
        metrics: Arc::new(MetricsCollector::new()),
        event_bus: Arc::new(EventBus::new()),
        task_dispatcher: Arc::new(TaskDispatcher::new()),
        config: ManagerConfig::default(),
    }
}

// AgentManagerState for the test router — matching main AppRouterState
use vol_agent_manager::AppRouterState;

fn make_router() -> (TestApp, Router) {
    let app = make_state();
    let state = AppRouterState {
        state_manager: app.state_manager.clone(),
        metrics: app.metrics.clone(),
        event_bus: app.event_bus.clone(),
        task_dispatcher: app.task_dispatcher.clone(),
        config: app.config.clone(),
    };
    let router = create_router(state.clone())
        .route("/health", get(|| async { axum::Json(json!({"status": "ok"})) }))
        .route("/metrics", get(|| async { String::new() }))
        .with_state(state);
    (app, router)
}

#[tokio::test]
async fn test_health_endpoint() {
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let (_app, router) = make_router();
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["status"], "ok");
}

#[tokio::test]
async fn test_list_agents_empty() {
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let (app, router) = make_router();
    // Register an agent so the state has content
    // For REST test, check that empty list works
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_agent_state_transitions() {
    let mgr = AgentStateManager::new();

    // Register is done via direct API in tests
    let state = vol_agent_manager::state::models::AgentState {
        agent_id: "test-agent".to_string(),
        name: "test".to_string(),
        r#type: "test".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec![],
        host_info: vol_agent_manager::state::models::HostInfo {
            hostname: "h".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            ip: "127.0.0.1".to_string(),
        },
        status: AgentStatus::Idle,
        connected_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now(),
    };
    mgr.register(state).await;

    let got = mgr.get("test-agent").await.unwrap();
    assert_eq!(got.status, AgentStatus::Idle);

    mgr.update_status("test-agent", AgentStatus::Busy).await;
    let got = mgr.get("test-agent").await.unwrap();
    assert_eq!(got.status, AgentStatus::Busy);

    let all = mgr.list_all().await;
    assert_eq!(all.len(), 1);
}

#[test]
fn test_protocol_message_roundtrip() {
    let msg = WsMessage::agent_report(
        "heartbeat",
        "agent-1",
        json!({"status": "Idle"}),
    );
    let serialized = serde_json::to_string(&msg).unwrap();
    let parsed: WsMessage = serde_json::from_str(&serialized).unwrap();
    assert_eq!(parsed.message_type, "heartbeat");
    assert_eq!(parsed.agent_id.as_deref(), Some("agent-1"));
}
```

- [ ] **Step 2: Create example config file**

Create `crates/vol-agent-manager/config.toml`:

```toml
[server]
listen_addr = "0.0.0.0:8080"

[health]
check_interval_secs = 15
heartbeat_timeout_secs = 90
disconnect_retention_secs = 300

[security]
# token = "your-secret-token"
```

- [ ] **Step 3: Run all tests**

Run: `cargo test -p vol-agent-manager`
Expected: All tests pass (15+ tests)

- [ ] **Step 4: Manual test — start server and verify endpoints**

```bash
cargo run -p vol-agent-manager -- --config crates/vol-agent-manager/config.toml
```

In another terminal:
```bash
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/agents
curl http://localhost:8080/metrics
```

Expected:
- `/health` → `{"status":"ok"}`
- `/api/v1/agents` → `{"agents":[]}`
- `/metrics` → Prometheus-formatted metrics output

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/tests/ crates/vol-agent-manager/config.toml
git commit -m "feat: add integration tests and example config"
```

---

### Task 9: Export AppRouterState from lib.rs and final cleanup

**Files:**
- Modify: `crates/vol-agent-manager/src/lib.rs`
- Modify: `crates/vol-agent-manager/src/main.rs`

- [ ] **Step 1: Export AppRouterState from lib.rs**

The `main.rs` references `AppRouterState` but it needs to be defined properly. Since it's only used internally in main.rs and tests, define it in `lib.rs`:

```rust
use std::sync::Arc;
use config::ManagerConfig;
use events::EventBus;
use metrics::collector::MetricsCollector;
use state::manager::AgentStateManager;
use task::dispatcher::TaskDispatcher;

pub mod config;
pub mod events;
pub mod health;
pub mod metrics;
pub mod state;
pub mod task;
pub mod ws;

/// Shared state passed to axum handlers.
pub struct AppRouterState {
    pub state_manager: Arc<AgentStateManager>,
    pub metrics: Arc<MetricsCollector>,
    pub event_bus: Arc<EventBus>,
    pub task_dispatcher: Arc<TaskDispatcher>,
    pub config: ManagerConfig,
}
```

- [ ] **Step 2: Fix main.rs imports**

Ensure main.rs imports `AppRouterState` from the crate root instead of defining it locally. Remove the duplicate struct definition if any exists.

- [ ] **Step 3: Final compilation check**

Run: `cargo check -p vol-agent-manager`
Expected: no warnings, no errors

- [ ] **Step 4: Final test run**

Run: `cargo test -p vol-agent-manager`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/src/lib.rs crates/vol-agent-manager/src/main.rs
git commit -m "feat: finalize lib.rs exports and main.rs wiring"
```
