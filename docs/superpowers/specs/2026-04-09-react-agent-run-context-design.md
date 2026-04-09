# ReAct Agent RunContext Design

**Date:** 2026-04-09
**Status:** Draft
**Author:** Claude Code

## Executive Summary

This design introduces `RunContext` - a unified context structure that encapsulates all state and resources for a single ReAct Agent `run()` invocation. `RunContext` replaces `PluginContext` and provides a centralized way to manage run-time state including messages, tool calls, iteration count, and plugin data.

## Motivation

Currently, the ReAct Agent's `run()` method maintains scattered state across multiple local variables:
- `run_id`, `iteration`, `messages`, `tool_calls` are local variables in the spawned task
- `PluginContext` only contains basic metadata (`run_id`, `user_input`, `session_id`) and a data map
- Plugins cannot access or modify runtime state like `messages` or `tool_calls`

This design consolidates all run state into a single `RunContext` structure, enabling:
1. **Unified state management** - all run state in one place
2. **Plugin access to runtime state** - plugins can read/modify messages, intercept tool calls
3. **Cleaner code organization** - state passed via `RunContext` instead of scattered variables

## Design

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                       RunContext                                │
│                                                                 │
│  Immutable (fixed at run start):                               │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ run_id: String                                          │   │
│  │ user_input: String                                      │   │
│  │ session_id: String                                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  Mutable (internal mutability via RwLock/Atomic):              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ iteration: AtomicU32                                    │   │
│  │ messages: Arc<RwLock<Vec<Message>>>                     │   │
│  │ all_tool_calls: Arc<RwLock<Vec<ToolCall>>>              │   │
│  │ current_tool_calls: Arc<RwLock<Vec<ToolCall>>>          │   │
│  │ data: Arc<RwLock<HashMap<String, Value>>>               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  Resource References (Arc shared):                              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ session: Arc<Session>                                   │   │
│  │ tools: Arc<ToolRegistry>                                │   │
│  │ config: AgentConfig                                     │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### RunContext Structure

```rust
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use vol_llm_core::Message;
use vol_llm_tool::ToolCall;
use crate::session::Session;
use vol_llm_tool::ToolRegistry;
use super::AgentConfig;

pub struct RunContext {
    // === Immutable fields (fixed at run start) ===
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    
    // === Mutable fields (internal mutability) ===
    /// Current iteration count (0-based, incremented each loop)
    pub iteration: AtomicU32,
    
    /// All messages in the conversation (system, history, user, assistant, tool)
    pub messages: Arc<RwLock<Vec<Message>>>,
    
    /// All tool calls across all iterations (historical record)
    pub all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    
    /// Tool calls for the current iteration (cleared each iteration)
    pub current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    
    /// Plugin data storage (key-value pairs)
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    
    // === Resource references ===
    pub session: Arc<Session>,
    pub tools: Arc<ToolRegistry>,
    pub config: AgentConfig,
}
```

### Helper Methods

```rust
impl RunContext {
    /// Create a new RunContext for a run invocation
    pub fn new(
        run_id: String,
        user_input: String,
        session_id: String,
        session: Arc<Session>,
        tools: Arc<ToolRegistry>,
        config: AgentConfig,
    ) -> Self;
    
    /// Get current iteration (1-based for display)
    pub fn current_iteration(&self) -> u32;
    
    /// Increment iteration counter
    pub fn next_iteration(&self);
    
    /// Add a message to the conversation
    pub async fn add_message(&self, message: Message) -> Result<(), tokio::sync::TryLockError>;
    
    /// Get all messages (read lock)
    pub async fn get_messages(&self) -> Result<Vec<Message>, tokio::sync::TryLockError>;
    
    /// Add a tool call to current iteration
    pub async fn add_tool_call(&self, tool_call: ToolCall) -> Result<(), tokio::sync::TryLockError>;
    
    /// Clear current iteration's tool calls (called at start of new iteration)
    pub async fn clear_current_tool_calls(&self);
    
    /// Get current iteration's tool calls
    pub async fn get_current_tool_calls(&self) -> Result<Vec<ToolCall>, tokio::sync::TryLockError>;
    
    /// Get all tool calls
    pub async fn get_all_tool_calls(&self) -> Result<Vec<ToolCall>, tokio::sync::TryLockError>;
    
    /// Plugin data: get value
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T>;
    
    /// Plugin data: set value
    pub async fn set<T: serde::Serialize>(&self, key: &str, value: T) -> Result<(), serde_json::Error>;
}
```

### Plugin Trait Update

The `AgentPlugin` trait is updated to use `RunContext` instead of `PluginContext`:

```rust
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;
    fn priority(&self) -> u32 { 100 }
    
    /// Called before agent execution starts
    async fn on_start(&self, ctx: &RunContext) -> PluginAction<()>;
    
    /// Called for each event in the stream
    async fn intercept(
        &self,
        event: StreamEvent,
        ctx: &RunContext,
    ) -> PluginAction<Option<StreamEvent>>;
    
    /// Called when agent completes successfully
    async fn on_complete(
        &self,
        ctx: &RunContext,
        response: &AgentResponse,
    ) -> PluginAction<()>;
    
    /// Called when agent encounters an error
    async fn on_error(
        &self,
        ctx: &RunContext,
        error: &AgentError,
    ) -> PluginAction<()>;
}
```

### Usage in ReActAgent::run()

```rust
pub async fn run(
    &self,
    user_input: &str,
    context: ToolContext,
) -> Result<AgentStreamReceiver, crate::AgentError> {
    // Generate run_id
    let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
    
    // Create RunContext with all state and resources
    let ctx = RunContext::new(
        run_id.clone(),
        user_input.to_string(),
        self.session.id.clone(),
        self.session.clone(),
        self.tools.clone(),
        self.config.clone(),
    );
    
    // Execute on_start hooks with RunContext
    for plugin in self.config.plugin_registry.plugins() {
        match plugin.on_start(&ctx).await {
            PluginAction::Continue(()) => {}
            PluginAction::ShortCircuit(response) => {
                return create_shortcircuit_stream(response, ctx).await;
            }
            PluginAction::Skip => {
                return create_skip_stream(ctx).await;
            }
            PluginAction::Abort(error) => {
                return Err(error);
            }
        }
    }
    
    // Spawn task with RunContext
    let ctx_for_task = ctx.clone(); // Clone is cheap (all Arc)
    tokio::spawn(async move {
        // Use ctx_for_task throughout the loop
        // - Access messages via ctx_for_task.messages.read().await
        // - Access tool calls via ctx_for_task.current_tool_calls.read().await
        // - Increment iteration via ctx_for_task.next_iteration()
    });
    
    // Wrap with PluginStream (also uses RunContext)
    let plugin_stream = PluginStream::new(raw_receiver, plugins, ctx);
    Ok(plugin_stream.into_receiver())
}
```

### Migration Path

**Files to modify:**
| File | Change |
|------|--------|
| `src/react/plugin.rs` | Replace `PluginContext` with `RunContext` |
| `src/react/agent.rs` | Use `RunContext` instead of local variables |
| `src/react/plugin_stream.rs` | Update to use `RunContext` |
| `src/plugins/observability.rs` | Update plugin to use `RunContext` |
| `src/plugins/caching.rs` | Update plugin to use `RunContext` |
| `src/plugins/retry.rs` | Update plugin to use `RunContext` |
| `src/plugins/rate_limiter.rs` | Update plugin to use `RunContext` |
| `src/plugins/hitl_cli.rs` | Update plugin to use `RunContext` |
| `src/plugins/hitl_http.rs` | Update plugin to use `RunContext` |
| `tests/*.rs` | Update tests to use `RunContext` |

**Deprecation strategy:**
1. Add `RunContext` alongside existing `PluginContext`
2. Update all internal code to use `RunContext`
3. Mark `PluginContext` as `#[deprecated]`
4. Remove `PluginContext` in a future breaking release

OR (simpler):
1. Directly replace `PluginContext` with `RunContext` (breaking change)

**Recommendation:** Direct replacement - this is still early in the project lifecycle.

## Error Handling

- `RwLock` operations use `.await` and can fail if the lock is poisoned
- Helper methods return `Result` for lock acquisition failures
- Plugin code should handle potential lock failures gracefully

## Testing Strategy

**Unit tests:**
- `RunContext::new()` creates correct structure
- `add_message()` / `get_messages()` work correctly
- `add_tool_call()` / `get_current_tool_calls()` work correctly
- `get()` / `set()` for plugin data work correctly
- Concurrent access (multiple tasks accessing `RunContext`)

**Integration tests:**
- Existing plugin tests updated to use `RunContext`
- Verify plugins can access and modify state via `RunContext`

## Acceptance Criteria

- [ ] `RunContext` struct created with all fields
- [ ] Helper methods implemented and tested
- [ ] `PluginContext` replaced with `RunContext` in `AgentPlugin` trait
- [ ] `ReActAgent::run()` uses `RunContext` for state management
- [ ] All built-in plugins updated to use `RunContext`
- [ ] All existing tests pass
- [ ] Documentation updated

## Out of Scope

This design focuses on creating the `RunContext` structure and migrating existing code. Advanced features are left for future iterations:

- Plugin API for modifying messages (e.g., `ctx.messages.write().await.retain(...)`)
- Transaction support for atomic state modifications
- Event emission when state changes (e.g., `OnMessageAdded` hook)

## Next Steps

1. User review and approval of this design
2. Implementation plan creation
3. Execute implementation in phases:
   - Phase 1: Create `RunContext` structure
   - Phase 2: Update `ReActAgent::run()` to use `RunContext`
   - Phase 3: Update `AgentPlugin` trait and all plugins
   - Phase 4: Update tests and verify
