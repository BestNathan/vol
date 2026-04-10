# Agent Observability Plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a comprehensive observability plugin for the ReAct Agent with JSONL file logging, human-readable stdout output, and automatic log rotation/retention.

**Architecture:** Create new `observability` module with three components: `logger.rs` (async log writes), `cleanup.rs` (retention policy), and `plugin.rs` (AgentPlugin integration). Logs organized by agent_id with session logs rotated by date and run logs by run_id.

**Tech Stack:** Rust, tokio async runtime, serde_json for JSONL, chrono for timestamps, std::fs for cleanup.

---

### Task 1: Create Observability Module Structure

**Files:**
- Create: `crates/vol-llm-agent/src/observability/mod.rs`
- Create: `crates/vol-llm-agent/src/observability/logger.rs`
- Create: `crates/vol-llm-agent/src/observability/cleanup.rs`
- Create: `crates/vol-llm-agent/src/observability/plugin.rs`

- [ ] **Step 1: Create module directory**

```bash
mkdir -p crates/vol-llm-agent/src/observability
```

- [ ] **Step 2: Create mod.rs with module declarations**

```rust
//! Observability plugin for structured logging and log retention.

pub mod logger;
pub mod cleanup;
pub mod plugin;

pub use logger::{ObservabilityLogger, LogEntry, LogType};
pub use cleanup::{cleanup_old_logs, cleanup_session_logs, cleanup_run_logs};
pub use plugin::ObservabilityPlugin;
```

- [ ] **Step 3: Create logger.rs skeleton**

```rust
//! Async log writer for JSONL file logs and stdout output.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct ObservabilityLogger {
    agent_id: String,
    log_base_path: PathBuf,
}

impl ObservabilityLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        Self { agent_id, log_base_path }
    }
}

pub enum LogType {
    Session { session_id: String, date: String },
    Run { run_id: String },
}

pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: Value,
}
```

- [ ] **Step 4: Create cleanup.rs skeleton**

```rust
//! Log cleanup utilities for retention policy enforcement.

use std::path::Path;

pub async fn cleanup_old_logs(agent_path: &Path) -> Result<(), LogError> {
    todo!()
}

pub async fn cleanup_session_logs(sessions_path: &Path, retention_days: u32) -> Result<usize, LogError> {
    todo!()
}

pub async fn cleanup_run_logs(runs_path: &Path, max_runs: usize) -> Result<usize, LogError> {
    todo!()
}

pub enum LogError {
    Io(std::io::Error),
    Parse(String),
}

impl From<std::io::Error> for LogError {
    fn from(err: std::io::Error) -> Self {
        LogError::Io(err)
    }
}
```

- [ ] **Step 5: Create plugin.rs skeleton**

```rust
//! ObservabilityPlugin implementation.

use crate::react::plugin::{AgentPlugin, PluginDecision, PluginId};
use crate::react::run_context::RunContext;
use crate::AgentStreamEvent;
use super::logger::ObservabilityLogger;
use std::sync::Arc;

pub struct ObservabilityPlugin {
    logger: Arc<ObservabilityLogger>,
}

impl ObservabilityPlugin {
    pub fn new(agent_id: String, log_base_path: std::path::PathBuf) -> Self {
        todo!()
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId {
        todo!()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        todo!()
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/observability/
git commit -m "feat: create observability module skeleton"
```

---

### Task 2: Add agent_id and log_base_path to AgentConfig

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:33-57`
- Test: `crates/vol-llm-agent/src/react/agent.rs:376-404`

- [ ] **Step 1: Write failing test**

```rust
// In crates/vol-llm-agent/src/react/agent.rs, add to existing tests:

#[test]
fn test_agent_config_with_observability() {
    use std::path::PathBuf;
    
    let config = AgentConfig {
        agent_id: "test_agent".to_string(),
        log_base_path: PathBuf::from("logs/agents"),
        ..Default::default()
    };
    
    assert_eq!(config.agent_id, "test_agent");
    assert_eq!(config.log_base_path, PathBuf::from("logs/agents"));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vol-llm-agent --lib test_agent_config_with_observability
```
Expected: FAIL with "unknown field: agent_id"

- [ ] **Step 3: Add fields to AgentConfig**

```rust
// In crates/vol-llm-agent/src/react/agent.rs:

use std::path::PathBuf;

#[derive(Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub prompt_context: PromptContext,
    pub verbose: bool,
    pub plugin_registry: PluginRegistry,
    
    // Observability fields
    pub agent_id: String,
    pub log_base_path: PathBuf,
}
```

- [ ] **Step 4: Update Default impl**

```rust
impl Default for AgentConfig {
    fn default() -> Self {
        use crate::prompt_context::PromptTemplate;

        let template = PromptTemplate::new("default", "You are a helpful assistant.");
        let prompt_context = PromptContext::new(template);

        Self {
            max_iterations: 5,
            max_history_messages: 20,
            prompt_context,
            verbose: false,
            plugin_registry: PluginRegistry::new(),
            agent_id: generate_agent_id(),
            log_base_path: PathBuf::from("logs/agents"),
        }
    }
}

/// Generate a short random agent ID if not provided
fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("agent_{:x}", timestamp % 0xFFFFFF)
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test -p vol-llm-agent --lib test_agent_config_with_observability
```
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: add agent_id and log_base_path to AgentConfig"
```

---

### Task 3: Implement LogEntry and LogType Types

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/logger.rs`

- [ ] **Step 1: Write failing test**

```rust
// In crates/vol-llm-agent/src/observability/logger.rs:

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_log_entry_to_json() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "run_123".to_string(),
            agent_id: "test_agent".to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "test"}),
        };
        
        let json_line = entry.to_json_line();
        assert!(json_line.contains("run_123"));
        assert!(json_line.contains("test_agent"));
        assert!(json_line.contains("AgentStart"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vol-llm-agent --lib observability::logger::tests::test_log_entry_to_json
```
Expected: FAIL (method `to_json_line` not found)

- [ ] **Step 3: Implement LogEntry methods**

```rust
// In crates/vol-llm-agent/src/observability/logger.rs:

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::fmt;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: Value,
}

impl LogEntry {
    /// Serialize log entry as JSON line
    pub fn to_json_line(&self) -> String {
        json!({
            "timestamp": self.timestamp.to_rfc3339(),
            "run_id": self.run_id,
            "agent_id": self.agent_id,
            "event": self.event,
            "data": self.data,
        }).to_string()
    }
    
    /// Format log entry for stdout (human-readable)
    pub fn to_stdout_line(&self) -> String {
        let level = match self.event.as_str() {
            "AgentAborted" | "AgentError" => "ERROR",
            "ToolCallBegin" | "ToolCallComplete" => "INFO",
            _ => "INFO",
        };
        
        let data_str = self.format_data_for_stdout();
        format!(
            "[{}] [{}] [{}] {}{}",
            level,
            self.agent_id,
            self.run_id,
            self.format_event_summary(),
            if data_str.is_empty() { String::new() } else { format!(" - {}", data_str) }
        )
    }
    
    fn format_event_summary(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => "Agent started".to_string(),
            "ThinkingComplete" => "Thinking complete".to_string(),
            "ToolCallBegin" => format!("Tool call: {}", 
                self.data.get("tool_name").map(|v| v.as_str().unwrap_or("unknown")).unwrap_or("unknown")),
            "ToolCallComplete" => format!("Tool result: {}", 
                self.data.get("result").map(|v| v.as_str().unwrap_or("")).unwrap_or("")),
            "IterationComplete" => format!("Iteration {} complete", 
                self.data.get("iteration").map(|v| v.as_u64().unwrap_or(0)).unwrap_or(0)),
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => format!("Agent aborted: {}", 
                self.data.get("reason").map(|v| v.as_str().unwrap_or("unknown")).unwrap_or("unknown")),
            "PluginEvent" => format!("Plugin event: {}", 
                self.data.get("name").map(|v| v.as_str().unwrap_or("unknown")).unwrap_or("unknown")),
            _ => self.event.clone(),
        }
    }
    
    fn format_data_for_stdout(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => {
                self.data.get("input").map(|v| format!("input: {:?}", v.as_str().unwrap_or(""))).unwrap_or_default()
            }
            "ToolCallBegin" => {
                self.data.get("arguments").map(|v| v.to_string()).unwrap_or_default()
            }
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LogType {
    Session { session_id: String, date: String },
    Run { run_id: String },
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vol-llm-agent --lib observability::logger::tests::test_log_entry_to_json
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/observability/logger.rs
git commit -m "feat: implement LogEntry serialization and formatting"
```

---

### Task 4: Implement ObservabilityLogger Core

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/logger.rs`
- Test: `crates/vol-llm-agent/src/observability/logger.rs`

- [ ] **Step 1: Write failing test for directory creation**

```rust
#[tokio::test]
async fn test_logger_creates_directories() {
    use std::path::PathBuf;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let log_base = temp_dir.path().join("logs");
    let agent_id = "test_agent";
    
    let logger = ObservabilityLogger::new(agent_id.to_string(), log_base.clone());
    
    // Logger should create directory structure
    let agent_path = log_base.join(agent_id);
    assert!(agent_path.exists());
    assert!(agent_path.join("sessions").exists());
    assert!(agent_path.join("runs").exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vol-llm-agent --lib observability::logger::tests::test_logger_creates_directories
```
Expected: FAIL (directories not created)

- [ ] **Step 3: Implement ObservabilityLogger::new() with directory creation**

```rust
use std::path::{Path, PathBuf};
use std::fs;
use tokio::sync::mpsc;
use tracing;

pub struct ObservabilityLogger {
    agent_id: String,
    log_base_path: PathBuf,
    agent_path: PathBuf,
}

impl ObservabilityLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        let agent_path = log_base_path.join(&agent_id);
        
        // Create directory structure (best effort, don't fail if can't)
        if let Err(e) = fs::create_dir_all(agent_path.join("sessions")) {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to create sessions directory");
        }
        if let Err(e) = fs::create_dir_all(agent_path.join("runs")) {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to create runs directory");
        }
        
        Self {
            agent_id,
            log_base_path,
            agent_path,
        }
    }
    
    fn get_session_log_path(&self, session_id: &str, date: &str) -> PathBuf {
        self.agent_path
            .join("sessions")
            .join(format!("session_{}_{}.jsonl", session_id, date))
    }
    
    fn get_run_log_path(&self, run_id: &str) -> PathBuf {
        self.agent_path
            .join("runs")
            .join(format!("run_{}.jsonl", run_id))
    }
}
```

- [ ] **Step 4: Implement log() method**

```rust
impl ObservabilityLogger {
    // ... existing code ...
    
    /// Log an event to both file and stdout
    pub async fn log(&self, entry: LogEntry, log_type: LogType) {
        let json_line = entry.to_json_line();
        let stdout_line = entry.to_stdout_line();
        
        // Always print to stdout
        println!("{}", stdout_line);
        
        // Write to file (best effort)
        let file_path = match log_type {
            LogType::Session { session_id, date } => self.get_session_log_path(&session_id, &date),
            LogType::Run { run_id } => self.get_run_log_path(&run_id),
        };
        
        if let Err(e) = self.append_to_file(&file_path, &json_line).await {
            tracing::warn!(
                agent_id = %self.agent_id,
                run_id = %entry.run_id,
                file = %file_path.display(),
                error = %e,
                "Failed to write log entry"
            );
        }
    }
    
    async fn append_to_file(&self, path: &Path, line: &str) -> Result<(), std::io::Error> {
        use tokio::fs::OpenOptions;
        use tokio::io::AsyncWriteExt;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        
        Ok(())
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test -p vol-llm-agent --lib observability::logger::tests::test_logger_creates_directories
```
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/observability/logger.rs
git commit -m "feat: implement ObservabilityLogger with async file writes"
```

---

### Task 5: Implement Log Cleanup Functions

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/cleanup.rs`
- Test: `crates/vol-llm-agent/src/observability/cleanup.rs`

- [ ] **Step 1: Write failing test for session log cleanup**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_cleanup_session_logs_removes_old_files() {
        let temp_dir = TempDir::new().unwrap();
        let sessions_path = temp_dir.path().join("sessions");
        fs::create_dir_all(&sessions_path).unwrap();
        
        // Create old session log (10 days ago)
        let old_date = format!("{}", chrono::Utc::now().date_naive() - chrono::Duration::days(10));
        let old_file = sessions_path.join(format!("session_abc_{}.jsonl", old_date));
        fs::write(&old_file, "old log").unwrap();
        
        // Create recent session log (2 days ago)
        let recent_date = format!("{}", chrono::Utc::now().date_naive() - chrono::Duration::days(2));
        let recent_file = sessions_path.join(format!("session_xyz_{}.jsonl", recent_date));
        fs::write(&recent_file, "recent log").unwrap();
        
        // Cleanup should remove only old file
        let deleted = cleanup_session_logs(&sessions_path, 7).await.unwrap();
        assert_eq!(deleted, 1);
        assert!(!old_file.exists());
        assert!(recent_file.exists());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vol-llm-agent --lib observability::cleanup::tests::test_cleanup_session_logs_removes_old_files
```
Expected: FAIL (todo!() panics)

- [ ] **Step 3: Implement cleanup_session_logs**

```rust
use std::path::Path;
use std::fs;
use chrono::{Utc, Duration, NaiveDate};
use regex::Regex;

pub async fn cleanup_session_logs(sessions_path: &Path, retention_days: u32) -> Result<usize, LogError> {
    if !sessions_path.exists() {
        return Ok(0);
    }
    
    let cutoff_date = Utc::now().date_naive() - Duration::days(retention_days as i64);
    let session_pattern = Regex::new(r"session_(.+)_([0-9]{8})\.jsonl")
        .map_err(|e| LogError::Parse(format!("Invalid regex: {}", e)))?;
    
    let mut deleted_count = 0;
    
    for entry in fs::read_dir(sessions_path)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy();
        
        if let Some(captures) = session_pattern.captures(&filename) {
            if let Some(date_str) = captures.get(2) {
                if let Ok(file_date) = NaiveDate::parse_from_str(date_str.as_str(), "%Y%m%d") {
                    if file_date < cutoff_date {
                        fs::remove_file(entry.path())?;
                        deleted_count += 1;
                    }
                }
            }
        }
    }
    
    Ok(deleted_count)
}
```

- [ ] **Step 4: Implement cleanup_run_logs**

```rust
pub async fn cleanup_run_logs(runs_path: &Path, max_runs: usize) -> Result<usize, LogError> {
    if !runs_path.exists() {
        return Ok(0);
    }
    
    let mut run_files: Vec<_> = fs::read_dir(runs_path)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("run_")
        })
        .collect();
    
    // Sort by filename (run_id includes timestamp, so alphabetical = chronological)
    run_files.sort_by(|a, b| {
        let a_name = a.file_name().to_string_lossy();
        let b_name = b.file_name().to_string_lossy();
        a_name.cmp(&b_name)
    });
    
    // Delete oldest files if over limit
    let mut deleted_count = 0;
    while run_files.len() - deleted_count > max_runs {
        let file_to_delete = run_files[deleted_count].path();
        fs::remove_file(&file_to_delete)?;
        deleted_count += 1;
    }
    
    Ok(deleted_count)
}
```

- [ ] **Step 5: Implement cleanup_old_logs (combined)**

```rust
pub async fn cleanup_old_logs(agent_path: &Path) -> Result<(), LogError> {
    let sessions_path = agent_path.join("sessions");
    let runs_path = agent_path.join("runs");
    
    // Clean session logs older than 7 days
    match cleanup_session_logs(&sessions_path, 7).await {
        Ok(count) => tracing::debug!(path = %sessions_path.display(), count, "Cleaned up old session logs"),
        Err(e) => tracing::warn!(path = %sessions_path.display(), error = %e, "Failed to cleanup session logs"),
    }
    
    // Keep only last 10 run logs
    match cleanup_run_logs(&runs_path, 10).await {
        Ok(count) => tracing::debug!(path = %runs_path.display(), count, "Cleaned up excess run logs"),
        Err(e) => tracing::warn!(path = %runs_path.display(), error = %e, "Failed to cleanup run logs"),
    }
    
    Ok(())
}
```

- [ ] **Step 6: Add regex dependency to Cargo.toml**

```toml
# In crates/vol-llm-agent/Cargo.toml, under [dependencies]:
regex = "1.10"
```

- [ ] **Step 7: Run tests to verify they pass**

```bash
cargo test -p vol-llm-agent --lib observability::cleanup::tests
```
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent/src/observability/cleanup.rs crates/vol-llm-agent/Cargo.toml
git commit -m "feat: implement log cleanup with retention policy"
```

---

### Task 6: Implement ObservabilityPlugin

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs`
- Test: `crates/vol-llm-agent/src/observability/plugin.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::react::run_context::RunContext;
    use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};
    use crate::react::AgentConfig;
    use std::sync::Arc;
    use tempfile::TempDir;
    
    fn create_test_context() -> RunContext {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        );
        ctx
    }
    
    #[tokio::test]
    async fn test_observability_plugin_logs_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_context();
        
        let event = AgentStreamEvent::AgentStart {
            input: "test".to_string(),
        };
        
        plugin.listen(&event, &ctx).await;
        
        // Verify log file was created
        let agent_path = temp_dir.path().join("test_agent");
        let runs_path = agent_path.join("runs");
        assert!(runs_path.exists());
        
        // Check run log contains expected entry
        let run_log_path = runs_path.join("run_test-run.jsonl");
        let content = std::fs::read_to_string(&run_log_path).unwrap();
        assert!(content.contains("AgentStart"));
        assert!(content.contains("test input"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vol-llm-agent --lib observability::plugin::tests::test_observability_plugin_logs_event
```
Expected: FAIL (todo!() panics)

- [ ] **Step 3: Implement ObservabilityPlugin::new()**

```rust
use crate::react::plugin::{AgentPlugin, PluginDecision, PluginId};
use crate::react::run_context::RunContext;
use crate::AgentStreamEvent;
use super::logger::{ObservabilityLogger, LogEntry, LogType};
use std::sync::Arc;
use std::path::PathBuf;
use chrono::Utc;

pub struct ObservabilityPlugin {
    logger: Arc<ObservabilityLogger>,
}

impl ObservabilityPlugin {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        let logger = Arc::new(ObservabilityLogger::new(agent_id, log_base_path));
        Self { logger }
    }
}
```

- [ ] **Step 4: Implement AgentPlugin trait**

```rust
use serde_json::json;
use async_trait::async_trait;

#[async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId {
        "observability".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        let entry = self.create_log_entry(event, ctx);
        
        // Determine log type based on event
        let log_type = match event {
            AgentStreamEvent::AgentStart { .. } |
            AgentStreamEvent::AgentComplete { .. } |
            AgentStreamEvent::AgentAborted { .. } |
            AgentStreamEvent::ThinkingComplete { .. } => {
                LogType::Run { run_id: ctx.run_id.clone() }
            }
            _ => {
                // Session logs use session_id + date
                let date = Utc::now().format("%Y%m%d").to_string();
                LogType::Session {
                    session_id: ctx.session_id.clone(),
                    date,
                }
            }
        };
        
        self.logger.log(entry, log_type).await;
    }
}

impl ObservabilityPlugin {
    fn create_log_entry(&self, event: &AgentStreamEvent, ctx: &RunContext) -> LogEntry {
        let (event_name, data) = match event {
            AgentStreamEvent::AgentStart { input } => {
                ("AgentStart", json!({"input": input}))
            }
            AgentStreamEvent::ThinkingComplete { thinking } => {
                ("ThinkingComplete", json!({"thinking_length": thinking.len()}))
            }
            AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
                ("ToolCallBegin", json!({"tool_name": tool_name, "arguments": arguments}))
            }
            AgentStreamEvent::ToolCallComplete { tool_name, result } => {
                ("ToolCallComplete", json!({"tool_name": tool_name, "result": result}))
            }
            AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer } => {
                ("IterationComplete", json!({
                    "iteration": iteration,
                    "tool_calls_count": tool_calls.len(),
                    "has_final_answer": final_answer.is_some(),
                }))
            }
            AgentStreamEvent::AgentComplete { response } => {
                ("AgentComplete", json!({
                    "iterations": response.iterations,
                    "tool_calls_count": response.tool_calls.len(),
                }))
            }
            AgentStreamEvent::AgentAborted { reason } => {
                ("AgentAborted", json!({"reason": reason}))
            }
            AgentStreamEvent::PluginEvent { name, data } => {
                ("PluginEvent", json!({"name": name, "data": data}))
            }
        };
        
        LogEntry {
            timestamp: Utc::now(),
            run_id: ctx.run_id.clone(),
            agent_id: self.logger.agent_id.clone(),
            event: event_name.to_string(),
            data,
        }
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test -p vol-llm-agent --lib observability::plugin::tests::test_observability_plugin_logs_event
```
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/observability/plugin.rs
git commit -m "feat: implement ObservabilityPlugin with event logging"
```

---

### Task 7: Run Cleanup on Agent Startup

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Add cleanup call in ReActAgent::run()**

```rust
// In crates/vol-llm-agent/src/react/agent.rs, in run() method:

// === Phase 1: Generate run_id and create RunContext ===
let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());

let tools = self.tools.clone();
let config = self.config.clone();
let session = self.session.clone();

let (run_ctx, plugin_rx) = RunContext::new(
    run_id.clone(),
    user_input.to_string(),
    self.session.id.clone(),
    session,
    tools,
    config,
);

// === Phase 1.5: Run log cleanup (best effort, non-blocking) ===
let log_base_path = self.config.log_base_path.clone();
let agent_id = self.config.agent_id.clone();
tokio::spawn(async move {
    let agent_path = log_base_path.join(&agent_id);
    if let Err(e) = crate::observability::cleanup_old_logs(&agent_path).await {
        tracing::warn!(agent_id = %agent_id, error = %e, "Log cleanup failed");
    }
});
```

- [ ] **Step 2: Run cargo check to verify it compiles**

```bash
cargo check -p vol-llm-agent
```
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: run log cleanup on agent startup"
```

---

### Task 8: Update Module Exports

**Files:**
- Modify: `crates/vol-llm-agent/src/react/mod.rs`
- Modify: `crates/vol-llm-agent/src/lib.rs`

- [ ] **Step 1: Export observability module from lib.rs**

```rust
// In crates/vol-llm-agent/src/lib.rs, add:

pub mod observability;
```

- [ ] **Step 2: Update AgentConfig usage example in agent.rs if needed**

- [ ] **Step 3: Run cargo check to verify exports work**

```bash
cargo check --workspace
```
Expected: Success

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/lib.rs crates/vol-llm-agent/src/react/mod.rs
git commit -m "feat: export observability module"
```

---

### Task 9: Add Unit Tests for All Log Events

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs`

- [ ] **Step 1: Add comprehensive test for all event types**

```rust
#[cfg(test)]
mod tests {
    // ... existing imports ...
    
    #[tokio::test]
    async fn test_observability_plugin_logs_all_event_types() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_context();
        
        // Test all event types
        let events = vec![
            AgentStreamEvent::AgentStart { input: "test".to_string() },
            AgentStreamEvent::ThinkingComplete { thinking: "thought".to_string() },
            AgentStreamEvent::ToolCallBegin { tool_name: "test_tool".to_string(), arguments: "{}".to_string() },
            AgentStreamEvent::ToolCallComplete { tool_name: "test_tool".to_string(), result: "result".to_string() },
            AgentStreamEvent::IterationComplete { iteration: 1, tool_calls: vec![], final_answer: None },
            AgentStreamEvent::AgentComplete { response: crate::AgentResponse { content: "done".to_string(), reasoning: String::new(), iterations: 1, tool_calls: vec![] } },
            AgentStreamEvent::AgentAborted { reason: "test".to_string() },
            AgentStreamEvent::PluginEvent { name: "test".to_string(), data: serde_json::Map::new() },
        ];
        
        for event in events {
            plugin.listen(&event, &ctx).await;
        }
        
        // Verify logs were created
        let agent_path = temp_dir.path().join("test_agent");
        assert!(agent_path.exists());
        assert!(agent_path.join("sessions").exists());
        assert!(agent_path.join("runs").exists());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-agent --lib observability
```
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/observability/plugin.rs
git commit -m "test: add comprehensive tests for all event types"
```

---

### Task 10: Integration Test with Full Agent Run

**Files:**
- Create: `crates/vol-llm-agent/tests/observability_integration.rs`

- [ ] **Step 1: Create integration test**

```rust
//! Observability plugin integration test.
//!
//! Run with: cargo test -p vol-llm-agent --test observability_integration

use vol_llm_agent::{ReActAgent, AgentConfig, AgentStreamEvent, observability::ObservabilityPlugin};
use vol_llm_agent::session::{Session, InMemorySessionStore, InMemoryMessageStore};
use vol_llm_tool::ToolContext;
use vol_llm_core::{LLMClient, ConversationRequest, LLMProvider, StreamEvent, StreamEventData};
use async_trait::async_trait;
use std::sync::Arc;
use tempfile::TempDir;

struct MockLlm;

#[async_trait]
impl LLMClient for MockLlm {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;

        let (tx, rx) = mpsc::channel(10);
        tokio::spawn(async move {
            let _ = tx.send(Ok(StreamEvent {
                id: "event_1".to_string(),
                data: StreamEventData::ContentComplete {
                    content: "Mock response".to_string(),
                },
            })).await;
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_full_agent_run_with_observability() {
    let temp_dir = TempDir::new().unwrap();
    let log_base = temp_dir.path().to_path_buf();
    
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Arc::new(Session::new(
        "test-session".to_string(),
        session_store.clone(),
        message_store.clone(),
    ));
    
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(MockLlm))
        .with_session(session)
        .with_agent_id("test_agent".to_string())
        .with_log_base_path(log_base.clone())
        .build()
        .unwrap();
    
    let context = ToolContext::default();
    let mut stream = agent.run("Test query", context).await.unwrap();
    
    // Consume stream
    while let Some(event) = stream.recv().await {
        match event.unwrap() {
            AgentStreamEvent::AgentComplete { .. } => break,
            _ => {}
        }
    }
    
    // Verify logs were created
    let agent_path = log_base.join("test_agent");
    assert!(agent_path.exists());
    assert!(agent_path.join("sessions").exists());
    assert!(agent_path.join("runs").exists());
}
```

- [ ] **Step 2: Run integration test**

```bash
cargo test -p vol-llm-agent --test observability_integration
```
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/tests/observability_integration.rs
git commit -m "test: add integration test for full agent run"
```

---

### Task 11: Update Builder for agent_id and log_base_path

**Files:**
- Modify: `crates/vol-llm-agent/src/react/builder.rs`

- [ ] **Step 1: Read existing builder structure**

```bash
cat crates/vol-llm-agent/src/react/builder.rs
```

- [ ] **Step 2: Add builder methods**

```rust
// In crates/vol-llm-agent/src/react/builder.rs:

impl AgentBuilder {
    // ... existing methods ...
    
    pub fn with_agent_id(mut self, agent_id: String) -> Self {
        self.config.agent_id = agent_id;
        self
    }
    
    pub fn with_log_base_path(mut self, path: std::path::PathBuf) -> Self {
        self.config.log_base_path = path;
        self
    }
    
    pub fn with_observability_plugin(mut self) -> Self {
        let plugin = crate::observability::ObservabilityPlugin::new(
            self.config.agent_id.clone(),
            self.config.log_base_path.clone(),
        );
        self.config.plugin_registry = self.config.plugin_registry.with_plugin(Arc::new(plugin));
        self
    }
}
```

- [ ] **Step 3: Run tests to verify builder works**

```bash
cargo test -p vol-llm-agent --lib react::builder
```
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/builder.rs
git commit -m "feat: add builder methods for observability config"
```

---

### Task 12: Final Verification and Documentation

**Files:**
- Verify: All existing tests pass
- Update: README or docs if applicable

- [ ] **Step 1: Run full test suite**

```bash
cargo test -p vol-llm-agent
```
Expected: All 106+ unit tests pass, integration tests pass

- [ ] **Step 2: Verify workspace builds**

```bash
cargo build --release
```
Expected: Success

- [ ] **Step 3: Commit final changes**

```bash
git add -A
git commit -m "feat: complete observability plugin implementation"
```

---

## Spec Coverage Check

| Spec Requirement | Task |
|-----------------|------|
| R1: Log all events | Task 6 |
| R2: JSONL format | Task 3 |
| R3: Stdout output | Task 3 |
| R4: Organize by agent_id | Task 4 |
| R5: Session rotation | Task 4, 5 |
| R6: Run logs by run_id | Task 4 |
| R7: 7-day retention | Task 5 |
| R8: Last 10 runs | Task 5 |
| R9: Startup cleanup | Task 7 |
| N1: Async writes | Task 4 |
| N2: Graceful errors | Task 4, 5, 7 |
| N3: Low overhead | Task 7 (spawn) |

## Type Consistency Check

- `AgentConfig.agent_id: String` - consistent throughout
- `AgentConfig.log_base_path: PathBuf` - consistent throughout
- `ObservabilityPlugin::new(agent_id: String, log_base_path: PathBuf)` - matches spec
- `LogEntry` fields match JSONL spec exactly
- `LogType` enum variants match file naming patterns

---

**Plan complete and saved to `docs/superpowers/plans/2026-04-10-agent-observability-implementation.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
