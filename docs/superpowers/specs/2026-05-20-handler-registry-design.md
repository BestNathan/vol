# Handler Registry Design

## Summary

Replace the 7 hardcoded handler fields on `AgentServerCore` with a trait-based
`HandlerRegistry` that allows built-in and external handlers to register under
a uniform interface. Type-safe `Operation`-based dispatch for built-in domains;
string-based fallback for custom handlers.

## Motivation

Adding a new domain handler today requires touching 5 places:
1. Add variant to `Operation` enum (`agent_server_protocol.rs`)
2. Add a payload enum
3. Create a handler struct in `domain/`
4. Add a field on `AgentServerCore`
5. Add a match arm in `AgentServerCore::handle()`

Handlers also have ad-hoc constructor signatures with no shared contract,
making uniform registration impossible. A trait formalizes the contract
and a registry makes dispatch a single data structure instead of a
sprawling match statement.

## Design

### `DomainHandler` trait

All handlers implement this trait. It lives in `crates/vol-llm-agent-channel/src/domain/handler.rs`.

```rust
#[async_trait]
pub trait DomainHandler: Send + Sync + 'static {
    /// Unique name for debugging and logging.
    fn name(&self) -> &str;

    /// Operations this handler exclusively owns for type-safe dispatch.
    /// Return an empty slice for handlers that only use string-based routing.
    fn operations(&self) -> &[Operation];

    /// Handle a message. The operation is embedded in `message.operation`.
    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError>;
}
```

Key decisions:
- `handle()` takes the full `AgentServerMessage` — no separate operation
  parameter needed since the operation lives inside the message. This
  avoids the split-signature anti-pattern the current handlers have
  (`handle(operation, message)` where operation came from message anyway).
- `operations()` returns a slice — handlers can own multiple `Operation`
  variants (e.g., `AgentHandler` owns all `AgentOperation::*` variants).
  The registry builds a reverse index from this at registration time.
- `Send + Sync + 'static` and `Arc` storage means handlers can hold
  their own `Arc`'d dependencies and spawn background work.

### `HandlerRegistry`

A dedicated struct in `crates/vol-llm-agent-channel/src/domain/registry.rs`.

```rust
pub struct HandlerRegistry {
    handlers: Vec<Arc<dyn DomainHandler>>,
    /// Method name string → handler index.
    /// Built from `operations()` at register time (via Operation::method_name()).
    method_index: HashMap<String, usize>,
}
```

**Registration:**

```rust
impl HandlerRegistry {
    /// Register a handler with type-safe Operation declarations.
    pub fn register(&mut self, handler: Arc<dyn DomainHandler>) -> Result<(), String> {
        let idx = self.handlers.len();
        for op in handler.operations() {
            let method = op.method_name().to_string();
            if self.method_index.contains_key(&method) {
                return Err(format!(
                    "method '{}' already claimed by handler '{}'",
                    method,
                    self.handlers[self.method_index[&method]].name()
                ));
            }
        }
        for op in handler.operations() {
            self.method_index.insert(op.method_name().to_string(), idx);
        }
        self.handlers.push(handler);
        Ok(())
    }

    /// Register a custom handler with explicit method name strings.
    /// For handlers that don't have Operation variants (external/custom domains).
    pub fn register_custom(
        &mut self,
        handler: Arc<dyn DomainHandler>,
        methods: &[&str],
    ) -> Result<(), String> {
        let idx = self.handlers.len();
        for method in methods {
            if self.method_index.contains_key(*method) {
                return Err(format!("method '{}' already registered", *method));
            }
            self.method_index.insert(method.to_string(), idx);
        }
        self.handlers.push(handler);
        Ok(())
    }
}
```

**Dispatch:**

```rust
impl HandlerRegistry {
    pub async fn dispatch(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let method = message.operation.method_name();
        if let Some(idx) = self.method_index.get(method) {
            return self.handlers[*idx].handle(message).await;
        }
        Err(ProtocolError::UnknownMethod(method.to_string()))
    }
}
```

Dispatch is a single lookup: `method_name()` → handler. Both built-in and custom handlers are in the same index. The "two-layer" distinction is at registration time — built-in handlers declare `Operation` variants for compile-time safety; custom handlers declare method name strings directly.

### Changes to `AgentServerCore`

| Before | After |
|--------|-------|
| 7 `pub` handler fields (`agent`, `file`, `session`, `mcp`, `skill`, `log`, `system`) | 1 private field: `handler_registry: HandlerRegistry` |
| `handle()` has 7-arm match | `handle()` delegates to `self.handler_registry.dispatch(msg)` |
| Public handler fields accessed directly (e.g., `core.mcp`) | No direct handler access from outside |
| `for_test()` manually constructs each handler | `for_test()` registers handlers through the registry |

Constructor flow in `AgentServerCoreBuilder::build()`:

```rust
let mut registry = HandlerRegistry::new();
registry.register(Arc::new(AgentHandler::new(router.clone(), holders.clone())))?;
registry.register(Arc::new(FileHandler::new(working_dir.clone())))?;
registry.register(Arc::new(SessionHandler::new(agents_root.clone())))?;
registry.register(Arc::new(McpHandler::new(Some(mcp_manager.clone()))))?;
registry.register(Arc::new(SkillHandler::new(Some(skill_loader.clone()))))?;
registry.register(Arc::new(LogHandler))?;
registry.register(Arc::new(SystemHandler))?;
```

### Changes to Each Built-in Handler

Each handler file in `domain/` changes minimally:
- Implements `DomainHandler` trait instead of ad-hoc `handle()` method
- `handle()` signature changes from `fn handle(&self, operation: XxxOperation, message: AgentServerMessage)` to `fn handle(&self, message: AgentServerMessage)` — the operation is extracted inside the method from `message.operation`

Example for `FileHandler`:

```rust
impl DomainHandler for FileHandler {
    fn name(&self) -> &str { "file" }

    fn operations(&self) -> &[Operation] {
        &[
            Operation::File(FileOperation::List),
            Operation::File(FileOperation::Read),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::File(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("file")),
        };
        match (op, message.payload) {
            // ... existing match arms unchanged
        }
    }
}
```

Constructors stay the same — each handler is still built with its
specific dependencies (`working_dir`, `agents_root`, etc.) before
being registered.

### Builder: External Registration

`AgentServerCoreBuilder` gains a `register_handler` method:

```rust
impl AgentServerCoreBuilder {
    pub fn register_handler(mut self, handler: Arc<dyn DomainHandler>) -> Self {
        self.extra_handlers.push(handler);
        self
    }
}
```

Extra handlers are registered last in `build()`, after built-in handlers.
This means built-in operations can't be shadowed (registration would fail
with a conflict error).

### What Stays the Same

- **Protocol** (`agent_server_protocol.rs`): No changes. `Operation` enum,
  `Payload` enum, and all variant types remain as-is. This is the contract
  between client and server — it shouldn't change.
- **JSON-RPC layer** (`jsonrpc/connection.rs`): `handle_core_dispatch()`
  keeps calling `core.handle(msg)`. From its perspective, the dispatch
  mechanism is an implementation detail.
- **Handler constructors**: Each handler is constructed outside the registry
  with its own dependencies. The registry only owns `Arc<dyn DomainHandler>`
  references.
- **Handlers are in `domain/`**: Existing handlers stay in the same files.

### Files Changed

| File | Change |
|------|--------|
| `src/domain/handler.rs` | **New**: `DomainHandler` trait |
| `src/domain/registry.rs` | **New**: `HandlerRegistry` |
| `src/domain/mod.rs` | Add `pub mod handler; pub mod registry;` |
| `src/domain/agent.rs` | Replace ad-hoc impl with `DomainHandler` impl |
| `src/domain/file.rs` | Same |
| `src/domain/session.rs` | Same |
| `src/domain/mcp.rs` | Same |
| `src/domain/skill.rs` | Same |
| `src/domain/log.rs` | Same |
| `src/domain/system.rs` | Same |
| `src/server_core.rs` | Replace 7 handler fields with `HandlerRegistry`, update `handle()`, `build()`, `for_test()` |
| `src/lib.rs` | Export `DomainHandler` trait and `HandlerRegistry` |
| `examples/jsonrpc_agent_service.rs` | No change needed (uses `core.handle()` indirectly) |
| Tests (`tests/*.rs`) | Use `for_test()` — update if tests access handler fields directly |

### Error Handling

- **Registration conflict**: If two handlers claim the same `Operation`,
  `register()` returns `Err`. This is a programmer error caught at startup.
- **Unknown method**: If no handler matches, `dispatch()` returns
  `ProtocolError::UnknownMethod(method_name)` — the JSON-RPC layer
  converts this to a `-32601 Method not found` error.
- **Payload mismatch**: Each handler still validates `message.payload`
  against the expected type and returns `ProtocolError::PayloadDecodeFailed`
  on mismatch. Same behavior as today.

### Testing

- `AgentServerCore::for_test()` registers handlers through the same
  `HandlerRegistry::register()` path — tests cover the registry implicitly.
- Add unit tests for `HandlerRegistry`: register, duplicate detection,
  dispatch to correct handler, unknown method fallback, custom string-based
  handler registration.
- Add a test for external handler registration via builder.
