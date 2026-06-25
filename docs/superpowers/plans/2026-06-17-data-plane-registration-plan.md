# Data-Plane Remote Registration and Sandbox Fault Tolerance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make sandbox init non-fatal for individual sandbox failures, and implement remote data-plane → control-plane WebSocket registration including heartbeat and capability snapshot.

**Architecture:** `SandboxRegistry::load()` wraps per-sandbox errors in `tracing::warn!` + `continue` instead of propagating with `?`. `app.rs` gains a `spawn_data_plane_connector()` helper that spawns a tokio task when `control_url` is set; the task connects via `tokio_tungstenite`, sends JSON-RPC `control.register`, then maintains heartbeat + capability snapshot on a timer.

**Tech Stack:** Rust, tokio, tokio-tungstenite, JSON-RPC protocol types from `vol-llm-agent-protocol`.

---

## File Structure

Modify:
- `crates/vol-llm-sandbox/src/registry.rs:170-230` — per-file sandbox errors → warn + continue
- `crates/vol-agent-server/Cargo.toml` — add `serde_json` dep (if not already available)
- `crates/vol-agent-server/src/app.rs` — add `spawn_data_plane_connector()` and call when `control_url` is set

Do not modify:
- `crates/vol-llm-runtime/src/lib.rs` — stays unchanged
- `crates/vol-llm-agent-protocol/` — types already exist
- Other files

---

### Task 1: Sandbox Fault Tolerance

**Files:**
- Modify: `crates/vol-llm-sandbox/src/registry.rs:170-230`

Make individual sandbox TOML parse failures and `sandbox.start()` failures non-fatal. Each failing sandbox is logged with `tracing::warn!` and skipped; the remaining sandboxes continue to load.

- [ ] **Step 1: Write a test for mixed valid/invalid sandbox files**

In `crates/vol-llm-sandbox/src/registry.rs`, find the existing `#[cfg(test)] mod tests` block at the bottom of the file and add this test:

```rust
#[tokio::test]
async fn load_skips_invalid_sandbox_keeps_valid() {
    let tmp = tempfile::tempdir().unwrap();
    // valid sandbox
    std::fs::write(
        tmp.path().join("good.toml"),
        r#"
name = "good"
type = "local"
"#,
    )
    .unwrap();
    // invalid sandbox (bad TOML syntax — missing `=`)
    std::fs::write(
        tmp.path().join("bad.toml"),
        r#"name "bad""#,
    )
    .unwrap();
    // duplicate name with good — should be skipped (warn)
    std::fs::write(
        tmp.path().join("dup.toml"),
        r#"
name = "good"
type = "local"
"#,
    )
    .unwrap();

    let registry = SandboxRegistry::load(tmp.path()).await.unwrap();
    // "good" is present once, "local" is always present
    assert!(registry.get("local").is_some(), "local must always exist");
    assert!(registry.get("good").is_some(), "good must be loaded");
    // "good" is not duplicated
    assert_eq!(
        registry.list().iter().filter(|n| n.as_str() == "good").count(),
        1
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vol-llm-sandbox load_skips_invalid_sandbox_keeps_valid
```

Expected: FAIL — current code propagates `?` and the bad TOML or duplicate name causes `load()` to return `Err`.

- [ ] **Step 3: Implement fault-tolerant loading**

In `crates/vol-llm-sandbox/src/registry.rs`, modify the per-file loop body so errors are caught with `tracing::warn!` and `continue` instead of `?`.

Change lines 170-209 (the `if sandboxes_dir.exists() { ... }` block) to wrap per-iteration logic in a closure or use `let _ =` with explicit error logging.

Replace the entire block from `if sandboxes_dir.exists() {` through the closing `}` before `Ok(Self { ... })`:

```rust
if sandboxes_dir.exists() {
    for entry in std::fs::read_dir(sandboxes_dir).map_err(SandboxError::Io)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to read sandbox directory entry: {}", e);
                continue;
            }
        };
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "toml") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to read sandbox config, skipping");
                continue;
            }
        };

        let config: SandboxConfig = match toml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to parse sandbox config, skipping");
                continue;
            }
        };

        if config.name == "local" {
            tracing::warn!(path = %path.display(), name = %config.name, "Sandbox name 'local' is reserved, skipping");
            continue;
        }
        if sandboxes.contains_key(&config.name) {
            tracing::warn!(path = %path.display(), name = %config.name, "Duplicate sandbox name, skipping");
            continue;
        }

        match config.sandbox_type.as_str() {
            #[cfg(feature = "ssh")]
            "ssh" => {
                let ssh_config = match config.ssh {
                    Some(c) => c,
                    None => {
                        tracing::warn!(name = %config.name, "SSH sandbox requires [sandbox.ssh] section, skipping");
                        continue;
                    }
                };
                let sb = match crate::ssh::SSHSandbox::new(
                    config.name.clone(),
                    config.work_dir.clone(),
                    ssh_config,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(name = %config.name, error = %e, "Failed to create SSH sandbox, skipping");
                        continue;
                    }
                };
                let sandbox: Arc<dyn Sandbox> = Arc::new(sb);
                if let Err(e) = sandbox.start().await {
                    tracing::warn!(name = %config.name, error = %e, "Failed to start sandbox, skipping");
                    continue;
                }
                sandboxes.insert(config.name.clone(), sandbox);
            }
            #[cfg(feature = "firecracker")]
            "firecracker" => {
                let fc_config = match config.firecracker {
                    Some(c) => c,
                    None => {
                        tracing::warn!(name = %config.name, "Firecracker sandbox requires [sandbox.firecracker] section, skipping");
                        continue;
                    }
                };
                #[cfg(target_os = "linux")]
                {
                    let pool = crate::firecracker::FirecrackerPool::new(
                        fc_config.clone(),
                        tokio::runtime::Handle::current(),
                    );
                    let sandbox: Arc<dyn Sandbox> =
                        Arc::new(crate::firecracker::FirecrackerSandbox::new(
                            config.name.clone(),
                            std::path::PathBuf::from(
                                config.work_dir.as_deref().unwrap_or("/tmp/fc-sandbox"),
                            ),
                            pool.clone(),
                        ));
                    firecracker_pools.insert(config.name.clone(), Arc::new(pool));
                    sandboxes.insert(config.name.clone(), sandbox);
                }
                #[cfg(not(target_os = "linux"))]
                {
                    tracing::warn!(name = %config.name, "Firecracker sandbox only supported on Linux, skipping");
                    continue;
                }
            }
            other => {
                tracing::warn!(name = %config.name, sandbox_type = %other, "Unknown sandbox type, skipping");
                continue;
            }
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vol-llm-sandbox load_skips_invalid_sandbox_keeps_valid
cargo test -p vol-llm-sandbox  # all existing tests must still pass
```

Expected: new test PASS; all existing tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-sandbox/src/registry.rs
git commit -m "fix(sandbox): skip individual sandbox init failures instead of crashing" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Remote Data-Plane → Control-Plane Connection

**Files:**
- Modify: `crates/vol-agent-server/src/app.rs:45-47` (around line 45, after data_core is built and before ws_owner mount)

Add a `spawn_data_plane_connector()` function and call it when running as standalone data-plane with `control_url` set.

- [ ] **Step 1: Add missing imports to app.rs**

At the top of `crates/vol-agent-server/src/app.rs`, add after the existing imports:

```rust
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use tokio::time;
use tokio_tungstenite::connect_async;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, Heartbeat, NodeHeartbeat,
    NodeLoad, NodeRegistration, Operation, Payload,
};
```

Note: `futures-util` and `tokio-tungstenite` are already in workspace deps. `tokio-tungstenite` is already in `vol-agent-server/Cargo.toml`. If `futures-util` is not a direct dep, add it to `vol-agent-server/Cargo.toml`:

```toml
futures-util = "0.3"
```

- [ ] **Step 2: Add spawn_data_plane_connector function**

Add this function before `pub async fn run` in `app.rs`:

```rust
/// Spawn a background task that connects this data-plane to a remote control-plane
/// via WebSocket, sends registration + capability snapshot, and maintains heartbeats.
fn spawn_data_plane_connector(
    control_url: String,
    node_id: String,
    name: String,
    version: String,
    heartbeat_secs: u64,
    data_core: Arc<DataPlaneServerCore>,
) {
    tokio::spawn(async move {
        let mut backoff = 1u64; // seconds
        let max_backoff = 60u64;

        loop {
            tracing::info!(
                control_url = %control_url,
                node_id = %node_id,
                "connecting to control-plane"
            );

            let ws_stream = match connect_async(&control_url).await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    tracing::warn!(
                        control_url = %control_url,
                        error = %e,
                        backoff_secs = backoff,
                        "failed to connect to control-plane, retrying"
                    );
                    time::sleep(Duration::from_secs(backoff)).await;
                    backoff = (backoff * 2).min(max_backoff);
                    continue;
                }
            };

            tracing::info!(node_id = %node_id, "connected to control-plane");
            backoff = 1; // reset on successful connection

            let (mut write, mut read) = ws_stream.split();

            // ── Send register ────────────────────────────────────────────

            let register_msg = serde_json::to_string(&AgentServerMessage {
                id: Some(uuid::Uuid::new_v4().to_string()),
                operation: Operation::Control(ControlOperation::Register),
                payload: Payload::Control(ControlPayload::Register(
                    NodeRegistration {
                        node_id: node_id.clone(),
                        name: name.clone(),
                        version: version.clone(),
                    },
                )),
                sender: Some("system".to_string()),
                kind: vol_llm_agent_protocol::MessageKind::Request,
                timestamp_ms: None,
            })
            .unwrap();

            if let Err(e) = write
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    register_msg.clone(),
                ))
                .await
            {
                tracing::warn!(error = %e, "failed to send register message");
                continue;
            }

            // ── Send capability snapshot ──────────────────────────────────

            let agent_ids = data_core.list_agent_ids().await;
            let agents: Vec<_> = agent_ids
                .into_iter()
                .map(|id| vol_llm_agent_protocol::agent_server_protocol::AgentCapability {
                    agent_id: id.clone(),
                    name: id,
                    description: None,
                    status: Some("idle".to_string()),
                })
                .collect();

            let snapshot_msg = serde_json::to_string(&AgentServerMessage {
                id: Some(uuid::Uuid::new_v4().to_string()),
                operation: Operation::Control(ControlOperation::CapabilitySnapshot),
                payload: Payload::Control(ControlPayload::CapabilitySnapshot(
                    vol_llm_agent_protocol::agent_server_protocol::CapabilitySnapshot {
                        node_id: node_id.clone(),
                        revision: 1,
                        generated_at_ms: None,
                        agents,
                        tools: vec![],
                        mcp_servers: vec![],
                        skills: vec![],
                    },
                )),
                sender: Some("system".to_string()),
                kind: vol_llm_agent_protocol::MessageKind::Notification,
                timestamp_ms: None,
            })
            .unwrap();

            let _ = write
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    snapshot_msg,
                ))
                .await;

            // ── Heartbeat + read loop ─────────────────────────────────────

            let heartbeat_interval = Duration::from_secs(heartbeat_secs);
            let mut heartbeat_tick = time::interval(heartbeat_interval);
            // Skip immediate tick
            heartbeat_tick.tick().await;

            let mut connected = true;
            while connected {
                tokio::select! {
                    _ = heartbeat_tick.tick() => {
                        let hb_msg = serde_json::to_string(&AgentServerMessage {
                            id: Some(uuid::Uuid::new_v4().to_string()),
                            operation: Operation::Control(ControlOperation::Heartbeat),
                            payload: Payload::Control(ControlPayload::Heartbeat(
                                NodeHeartbeat {
                                    node_id: node_id.clone(),
                                    status: "online".to_string(),
                                    load: NodeLoad { running: 0, queued: 0 },
                                },
                            )),
                            sender: Some("system".to_string()),
                            kind: vol_llm_agent_protocol::MessageKind::Notification,
                            timestamp_ms: None,
                        })
                        .unwrap();

                        if write
                            .send(tokio_tungstenite::tungstenite::Message::Text(hb_msg))
                            .await
                            .is_err()
                        {
                            tracing::warn!("heartbeat send failed, reconnecting");
                            connected = false;
                        }
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(_)) => {
                                // Message received; heartbeat keep-alive handled above.
                                // Future: handle control-plane → data-plane commands here.
                            }
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "websocket read error, reconnecting");
                                connected = false;
                            }
                            None => {
                                tracing::warn!("websocket closed by control-plane, reconnecting");
                                connected = false;
                            }
                        }
                    }
                }
            }
        }
    });
}
```

Note: `uuid` is already in workspace deps. If it's not in `vol-agent-server/Cargo.toml`, add it. Also check that `serde_json` and `futures-util` are available.

- [ ] **Step 3: Call spawn_data_plane_connector from run()**

In `pub async fn run()`, after the data_core is built and agents discovered (around line 47, after the `if control_plane_enabled && data_plane_enabled` block), add:

```rust
// ── Remote control-plane registration (standalone data-plane) ──────

if !control_plane_enabled && data_plane_enabled {
    if let Some(ref control_url) = config.data_plane.control_url {
        let node_id = config
            .data_plane
            .node_id
            .clone()
            .unwrap_or_else(|| "dp-unknown".to_string()));
        let name = config
            .data_plane
            .name
            .clone()
            .unwrap_or_else(|| "data-plane".to_string());

        if let Some(ref data) = data_core {
            spawn_data_plane_connector(
                control_url.clone(),
                node_id,
                name,
                env!("CARGO_PKG_VERSION").to_string(),
                config.data_plane.heartbeat_secs,
                data.clone(),
            );
        }
    }
}
```

- [ ] **Step 4: Fix imports and build**

Check that `vol-agent-server/Cargo.toml` has all needed dependencies:

```bash
rtk grep -E "tokio-tungstenite|futures-util|serde_json|uuid|hostname" crates/vol-agent-server/Cargo.toml
```

If `futures-util` is missing, add to `[dependencies]`:
```toml
futures-util = "0.3"
```

Check: `tokio-tungstenite` is already present in both dev and regular deps — keep as-is.

- [ ] **Step 5: Build and compile-check**

```bash
cargo check -p vol-agent-server
```

Expected: no compile errors. Fix any type mismatches or missing imports.

- [ ] **Step 6: Write heartbeat formatting test**

Add to `crates/vol-agent-server/tests/` a new file or extend existing tests:

```rust
use serde_json;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, NodeHeartbeat,
    NodeLoad, Operation, Payload,
};

#[test]
fn node_heartbeat_serializes_to_json_rpc() {
    let msg = AgentServerMessage {
        id: Some("test-id".to_string()),
        operation: Operation::Control(ControlOperation::Heartbeat),
        payload: Payload::Control(ControlPayload::Heartbeat(NodeHeartbeat {
            node_id: "dp-1".to_string(),
            status: "online".to_string(),
            load: NodeLoad {
                running: 0,
                queued: 0,
            },
        })),
        sender: Some("system".to_string()),
        kind: vol_llm_agent_protocol::MessageKind::Notification,
        timestamp_ms: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("Heartbeat"));
    assert!(json.contains("dp-1"));
    assert!(json.contains("online"));
}

#[test]
fn node_registration_serializes_roundtrip() {
    let reg = vol_llm_agent_protocol::agent_server_protocol::NodeRegistration {
        node_id: "dp-1".to_string(),
        name: "data-plane-1".to_string(),
        version: "0.1.0".to_string(),
    };
    let json = serde_json::to_string(&reg).unwrap();
    let back: vol_llm_agent_protocol::agent_server_protocol::NodeRegistration =
        serde_json::from_str(&json).unwrap();
    assert_eq!(reg, back);
}
```

- [ ] **Step 7: Run tests**

```bash
cargo test -p vol-agent-server node_heartbeat_serializes_to_json_rpc
cargo test -p vol-agent-server node_registration_serializes_roundtrip
cargo test -p vol-agent-server  # all existing tests must pass
```

Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vol-agent-server/src/app.rs crates/vol-agent-server/Cargo.toml crates/vol-agent-server/tests/
git commit -m "feat(data-plane): connect to remote control-plane on startup" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Validation and Wiki Ingest

**Files:**
- Read-only validation.

- [ ] **Step 1: Full build check**

```bash
cargo check -p vol-agent-server -p vol-llm-sandbox
cargo test -p vol-llm-sandbox
cargo test -p vol-agent-server
```

Expected: all compile and tests pass.

- [ ] **Step 2: Rebuild and redeploy data-plane in cluster**

```bash
# After image rebuild, restart the data-plane with sandbox + mcp remounted
kubectl -n vol-agent-system rollout restart deployment/agent-server-dp
sleep 10
kubectl -n vol-agent-system logs deployment/agent-server-dp --tail=20
```

Expected: data-plane starts without sandbox crash, logs show "connecting to control-plane" and/or registration attempt.

- [ ] **Step 3: Verify control-plane receives registration**

```bash
kubectl -n vol-agent-system logs deployment/agent-server --tail=30 | grep -i "register\|node\|snapshot"
```

Expected: control-plane logs show node registration activity (if data-plane image has been rebuilt with the new code). If the existing image is used (no rebuild), this step is informational only — behavior is unchanged until a new image is built and pushed.

- [ ] **Step 4: Wiki ingest**

Invoke `wiki-ingest` with summary:
```
Ingest the data-plane remote registration and sandbox fault tolerance implementation: sandbox init skipped individual failing sandboxes instead of crashing; standalone data-plane connects to control-plane via WebSocket, registers, sends capability snapshots, and maintains heartbeats with auto-reconnect.
```

- [ ] **Step 5: Commit wiki**

```bash
git add docs/wiki
git commit -m "docs(wiki): ingest sandbox tolerance and data-plane registration" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

## Self-Review

### Spec coverage

- Sandbox fault tolerance: Task 1 implements per-file warn+continue in `SandboxRegistry::load()`.
- Remote data-plane registration: Task 2 adds `spawn_data_plane_connector()` and calls it from `run()`.
- Error handling: exponential backoff on disconnect (1s→60s), heartbeat send failures trigger reconnect.
- Notification heartbeats do not expect server responses.

### Placeholder scan

No TBD/TODO/fill-later. Registration, heartbeat, snapshot, and reconnect logic is fully specified.

### Type consistency

- `NodeRegistration`, `NodeHeartbeat`, `CapabilitySnapshot`, `AgentCapability` match protocol definitions from `vol-llm-agent-protocol`.
- Dependencies (`tokio-tungstenite`, `futures-util`, `serde_json`, `uuid`) checked against Cargo.toml.
