# JSON-RPC Transport Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate all JSON-RPC transport code from scattered `jsonrpc/` and `gateway/` modules into a single `transport/jsonrpc/` directory.

**Architecture:** Git-move files into the new location, create `transport/jsonrpc/mod.rs` with submodule declarations and re-exports, update 4 internal imports in `connection.rs` and 1 test import, remove old module declarations from `lib.rs`, and clean up empty directories. No public API breakage — `JsonRpcServer` stays re-exported from crate root.

**Tech Stack:** Rust, no new dependencies

---

### Task 1: Move files and create new module structure

**Files:**
- Create: `crates/vol-llm-agent-channel/src/transport/jsonrpc/mod.rs`
- Move: `src/jsonrpc/server.rs` → `src/transport/jsonrpc/server.rs`
- Move: `src/jsonrpc/connection.rs` → `src/transport/jsonrpc/connection.rs`
- Move: `src/jsonrpc/serde_helpers.rs` → `src/transport/jsonrpc/serde_helpers.rs`
- Move: `src/gateway/jsonrpc_ws.rs` → `src/transport/jsonrpc/codec.rs`

- [ ] **Step 1: Create target directory and git-move files**

```bash
mkdir -p crates/vol-llm-agent-channel/src/transport/jsonrpc
git mv crates/vol-llm-agent-channel/src/jsonrpc/server.rs crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs
git mv crates/vol-llm-agent-channel/src/jsonrpc/connection.rs crates/vol-llm-agent-channel/src/transport/jsonrpc/connection.rs
git mv crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs crates/vol-llm-agent-channel/src/transport/jsonrpc/serde_helpers.rs
git mv crates/vol-llm-agent-channel/src/gateway/jsonrpc_ws.rs crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs
```

- [ ] **Step 2: Create `transport/jsonrpc/mod.rs`**

```rust
//! JSON-RPC transport: server, connection, codec, and serialization helpers.

pub mod codec;
pub mod connection;
pub mod server;
pub mod serde_helpers;

pub use codec::{decode_jsonrpc_frame, encode_jsonrpc_message};
pub use server::JsonRpcServer;
```

- [ ] **Step 3: Update `transport/mod.rs` — add jsonrpc submodule**

Add `pub mod jsonrpc;` to `transport/mod.rs`:

```rust
mod http;
mod memory;
pub mod jsonrpc;
mod ws;

pub use http::HttpTransport;
pub use memory::{MemoryConnection, MemoryHandle};
pub use ws::{WsConnection, WsServer};
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/jsonrpc/
git add crates/vol-llm-agent-channel/src/transport/mod.rs
git commit -m "refactor: move jsonrpc and codec files into transport/jsonrpc"
```

---

### Task 2: Update lib.rs — remove old module declarations

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/lib.rs`

- [ ] **Step 1: Remove `pub mod gateway;` and `pub mod jsonrpc;`**

Remove lines:
```rust
pub mod gateway;
pub mod jsonrpc;
```

- [ ] **Step 2: Update JsonRpcServer re-export**

Change:
```rust
pub use jsonrpc::JsonRpcServer;
```
To:
```rust
pub use transport::jsonrpc::JsonRpcServer;
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/lib.rs
git commit -m "refactor: remove gateway/jsonrpc module declarations, re-export from transport"
```

---

### Task 3: Update internal imports in connection.rs

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/transport/jsonrpc/connection.rs:54,134,147`

- [ ] **Step 1: Update the 3 import references**

Line 54 — change:
```rust
                    match crate::gateway::jsonrpc_ws::decode_jsonrpc_frame(&text) {
```
To:
```rust
                    match crate::transport::jsonrpc::codec::decode_jsonrpc_frame(&text) {
```

Line 134 — change:
```rust
                let text = crate::gateway::jsonrpc_ws::encode_jsonrpc_message(msg)
```
To:
```rust
                let text = crate::transport::jsonrpc::codec::encode_jsonrpc_message(msg)
```

Line 147 — change:
```rust
    use crate::jsonrpc::serde_helpers::to_jsonrpc_event;
```
To:
```rust
    use crate::transport::jsonrpc::serde_helpers::to_jsonrpc_event;
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/jsonrpc/connection.rs
git commit -m "refactor: update internal imports in connection.rs to transport/jsonrpc paths"
```

---

### Task 4: Remove old jsonrpc/ and gateway/ directories, update test import

**Files:**
- Delete: `crates/vol-llm-agent-channel/src/jsonrpc/mod.rs`
- Delete: `crates/vol-llm-agent-channel/src/gateway/mod.rs`
- Modify: `crates/vol-llm-agent-channel/tests/jsonrpc_ws_gateway_test.rs:5`

- [ ] **Step 1: Remove empty old module files**

```bash
git rm crates/vol-llm-agent-channel/src/jsonrpc/mod.rs
git rm crates/vol-llm-agent-channel/src/gateway/mod.rs
```

- [ ] **Step 2: Update test import**

Change line 5:
```rust
use vol_llm_agent_channel::gateway::jsonrpc_ws::{decode_jsonrpc_frame, encode_jsonrpc_message};
```
To:
```rust
use vol_llm_agent_channel::transport::jsonrpc::codec::{decode_jsonrpc_frame, encode_jsonrpc_message};
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/mod.rs crates/vol-llm-agent-channel/src/gateway/mod.rs crates/vol-llm-agent-channel/tests/jsonrpc_ws_gateway_test.rs
git commit -m "refactor: remove old jsonrpc/gateway mod.rs files, update test import"
```

---

### Task 5: Verify compilation and tests

**Files:**
- Verify: `crates/vol-llm-agent-channel/` (all)
- Verify: `crates/vol-agent-manager/` (downstream consumer)

- [ ] **Step 1: Check compilation of channel crate**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: clean compilation (only pre-existing warnings, if any)

- [ ] **Step 2: Run channel tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1`
Expected: all tests pass (~55 tests)

- [ ] **Step 3: Check downstream crate**

Run: `cargo check -p vol-agent-manager 2>&1`
Expected: clean compilation (no new errors)

- [ ] **Step 4: Fix any remaining issues**

If compilation or tests fail, fix the specific issues and re-run.

- [ ] **Step 5: Commit any fixes or proceed**

If no fixes needed, no commit. Otherwise:
```bash
git add -A && git commit -m "fix: remaining compilation issues from jsonrpc consolidation"
```
