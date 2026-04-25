# CodingAgent + LoggerPlugin Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `with_logger()` to `CodingAgentBuilder` and migrate TUI's `spawn_agent` from direct config construction to the builder pattern.

**Architecture:** `CodingAgentBuilder` gets three new builder methods: `with_logger()` (registers LoggerPlugin), `session()` (passes shared session), and `llm_provider_id()` (sets provider ID). TUI's `spawn_agent` migrates from manual `CodingAgentConfig { ..Default::default() }` to fluent builder calls ending in `.with_logger().build().await`.

**Tech Stack:** Rust, cargo, vol-llm-observability::LoggerPlugin

---

### Task 1: Add `with_logger()`, `session()`, `llm_provider_id()` to CodingAgentBuilder

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`

- [ ] **Step 1: Add three builder methods**

Add these methods to `CodingAgentBuilder` impl (insert between `tool_config()` and `build()`):

```rust
    /// Register LoggerPlugin to write JSONL event logs to store_dir/logs/.
    pub fn with_logger(mut self) -> Self {
        let logger = vol_llm_observability::LoggerPlugin::new(self.config.store_dir.clone());
        self.config.plugin_registry.register(logger);
        self
    }

    /// Set the shared session for conversation history.
    pub fn session(mut self, session: Arc<vol_session::Session>) -> Self {
        self.config.session = Some(session);
        self
    }

    /// Set the LLM provider ID (used when `llm` is None).
    pub fn llm_provider_id(mut self, id: String) -> Self {
        self.config.llm_provider_id = id;
        self
    }
```

- [ ] **Step 2: Verify build**

Run: `cargo check -p vol-llm-agents`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: add with_logger, session, llm_provider_id builder methods to CodingAgentBuilder"
```

---

### Task 2: Migrate TUI spawn_agent to builder + with_logger()

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs`
- Test: `cargo test -p vol-llm-tui`

- [ ] **Step 1: Update imports**

In `main.rs`, update the `vol_llm_agents` import. Change the existing import line:

```rust
// Remove this line:
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, EventObserver, ObserverError};

// Replace with:
use vol_llm_agents::coding::{CodingAgentBuilder, EventObserver, ObserverError};
```

Also add the LoggerPlugin import at the top of the file (near other vol imports):

```rust
use vol_llm_observability::LoggerPlugin;
```

Note: The `CodingAgentConfig` struct is still used internally by the builder, so the build will use it transitively. We only remove the direct `CodingAgentConfig` import since we no longer construct it manually.

- [ ] **Step 2: Rewrite spawn_agent to use builder**

Replace the entire `spawn_agent` function body (lines 336-422) with:

```rust
fn spawn_agent(
    input: String,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<Session>,
) {
    tokio::spawn(async move {
        // Set running flag and clear approval state
        {
            let mut state = state.lock().await;
            state.is_running = true;
            state.approval_state.clear().await;
        }

        // Configure tools
        let mut tool_config = ToolConfig::new();
        if let Ok(tavily_key) = std::env::var("TAVILY_API_KEY") {
            tool_config.set("web_search", vol_llm_tools_builtin::WebSearchConfig {
                provider: "tavily".to_string(),
                api_key: tavily_key,
                proxy: ProxyConfig::default(),
            });
        }
        if let Ok(max_len) = std::env::var("WEB_FETCH_MAX_LENGTH") {
            tool_config.set("web_fetch", vol_llm_tools_builtin::WebFetchConfig {
                max_content_length: max_len.parse().ok(),
                proxy: ProxyConfig::default(),
            });
        }

        let (store_dir, _sessions_dir) = derive_store_paths();
        let working_dir = std::env::current_dir().unwrap_or_default();

        let unsafe_mode = {
            let state_guard = state.lock().await;
            state_guard.unsafe_mode
        };

        // Get approval state for handler — unsafe_mode is shared via AtomicBool
        let approval_state = {
            let state_guard = state.lock().await;
            state_guard.approval_state.unsafe_mode.store(unsafe_mode, std::sync::atomic::Ordering::Relaxed);
            state_guard.approval_state.clone()
        };

        let agent = match CodingAgentBuilder::new()
            .working_dir(working_dir)
            .store_dir(store_dir)
            .max_iterations(10)
            .session(session)
            .hitl_enabled(true)
            .unsafe_mode(unsafe_mode)
            .approval_handler(approval_state.into_handler())
            .tool_config(tool_config)
            .with_logger()
            .build()
            .await
        {
            Ok(a) => a,
            Err(e) => {
                let mut state = state.lock().await;
                state.conversation.push(app::ConversationEntry::Error {
                    message: format!("Error creating agent: {}", e),
                });
                state.is_running = false;
                return;
            }
        };

        let observer = Arc::new(RatatuiObserver::new(state.clone()));
        let agent = agent.with_observer(observer);

        match agent.run(&input).await {
            Ok(_response) => {
                // All events handled via observer
            }
            Err(e) => {
                let mut state = state.lock().await;
                state.conversation.push(app::ConversationEntry::Error {
                    message: format!("Error: {}", e),
                });
                state.is_running = false;
            }
        }
    });
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo check -p vol-llm-tui`
Expected: No errors

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-tui -p vol-llm-agents`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs
git commit -m "refactor: migrate TUI spawn_agent to CodingAgentBuilder with logger plugin"
```

---

### Task 3: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Full workspace build**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 2: Full test suite**

Run: `cargo test -p vol-llm-observability -p vol-llm-agent -p vol-llm-agents -p vol-llm-tui -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 3: Commit (if any fixes needed)**

No commit needed if everything passes — changes were committed in prior tasks.
