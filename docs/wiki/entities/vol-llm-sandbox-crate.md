---
type: entity
category: service
tags: [sandbox, container, ssh, firecracker, fault-tolerance]
created: 2026-06-17
updated: 2026-06-17
source_count: 1
---

# vol-llm-sandbox Crate

## Overview
`vol-llm-sandbox` is the sandbox abstraction and lifecycle management crate. It defines the `Sandbox` trait and provides implementations for local execution, SSH sandboxes, and Firecracker VM sandboxes. `SandboxRegistry` manages named sandbox instances loaded from TOML configuration files.

## Key Facts
- Defines the `Sandbox` trait with `start()`, `stop()`, `execute()`, and file-system operations.
- `SandboxRegistry::load()` reads sandbox configs from a directory, creates and starts each valid sandbox.
- Now tolerates individual sandbox failures: invalid TOML, missing SSH configs, failed `sandbox.start()`, or duplicate names are logged and skipped instead of crashing the server.
- `SandboxRegistry` always has a built-in `"local"` sandbox — this name is reserved and cannot be overridden by config files.
- Supports SSH sandboxes with known_hosts (feature-gated behind `feature = "ssh"`).
- Supports Firecracker micro-VM sandboxes (feature-gated behind `feature = "firecracker"`, Linux-only).

## Modules
- `sandbox.rs` — `Sandbox` trait and `SandboxResult`/`SandboxError` types
- `local.rs` — `LocalSandbox` implementation via `tokio::process::Command`
- `ssh.rs` — SSH sandbox with session pooling (feature = "ssh")
- `firecracker.rs` — Firecracker VM sandbox with pool (feature = "firecracker")
- `registry.rs` — `SandboxRegistry` with fault-tolerant loading

## Fault-Tolerant Loading
Source: [[data-plane-registration-sandbox-tolerance]]

`SandboxRegistry::load()` now wraps each per-file operation in error logging + `continue` instead of propagating errors. The following failures are handled gracefully:
- Directory entry read errors
- File read failures
- TOML parse errors
- Reserved name `"local"`
- Duplicate sandbox names
- Missing required config sections
- SSH sandbox creation failures
- `sandbox.start()` failures
- Unknown sandbox types

## Related
- [[vol-agent-server-crate]] — uses `SandboxRegistry` for sandbox management
