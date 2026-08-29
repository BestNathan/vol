---
type: concept
category: pattern
tags: [capability, discovery, sandbox, backend-agnostic]
created: 2026-08-27
updated: 2026-08-28
source_count: 1
---

# Capability Discovery

## Overview
Capability Discovery is a pattern for runtime discovery of backend capabilities. Each sandbox provider declares what operations it supports via `SandboxCapabilities`, so the orchestration layer and UI can expose the right lifecycle actions without hardcoding backend-specific knowledge.

> **Scope, as of 2026-08-28:** this describes the *intent*. In the current code the flags are **advisory only**. `SandboxManager` reads `capabilities()` in exactly one place — populating `SandboxInfo` for `list()` — and never to gate an operation: `stop()` and `destroy()` both proceed against providers declaring `stoppable: false` / `destroyable: false`. And because no lifecycle operation is exposed on the `sandbox.*` RPC surface, no UI can drive them regardless. Treat the flags as documentation of intent, not a guarantee. See [[sandbox-lifecycle]].

## How It Works

### Capability Declaration
Providers declare their capabilities via the `capabilities()` method:

```rust
#[async_trait]
pub trait SandboxProvider: Send + Sync {
    fn capabilities(&self) -> SandboxCapabilities;
    // ...
}
```

### Capability Structure
```rust
pub struct SandboxCapabilities {
    pub persistent: bool,    // Survives process restart
    pub pausable: bool,      // Supports pause/resume
    pub stoppable: bool,     // Supports stop (preserves workspace)
    pub destroyable: bool,   // Supports destroy (removes resources)
}
```

### Provider-Specific Capabilities

| Provider | Persistent | Pausable | Stoppable | Destroyable | Rationale |
|----------|------------|----------|-----------|-------------|-----------|
| Local | ✓ | ✗ | ✗ | ✗ | Local filesystem persists; cannot pause process; stopping implies cleanup |
| Tmp | ✗ | ✗ | ✗ | ✓ | Temp directory is ephemeral; can be destroyed |
| SSH | ✓ | ✗ | ✗ | ✗ | Remote host persists; cannot pause remote process; destroy would kill host |
| Firecracker | ✓ | ✓ | ✓ | ✓ | VM can be paused, stopped, destroyed |
| Wasm | ✗ | ✗ | ✗ | ✓ | Wasm module is ephemeral; can be destroyed |

## Usage

### Manager Queries Capabilities
```rust
pub async fn list(&self, filter: Option<SandboxFilter>) -> SandboxResult<Vec<SandboxInfo>> {
    let records = self.store.list(filter).await?;
    let mut infos = Vec::new();
    
    for record in records {
        // Query provider for capabilities
        let caps = {
            let providers = self.providers.read().await;
            providers
                .get(&record.provider_kind)
                .map(|p| p.capabilities())
                .unwrap_or(SandboxCapabilities {
                    persistent: false,
                    pausable: false,
                    stoppable: false,
                    destroyable: false,
                })
        };
        
        infos.push(SandboxInfo {
            id: record.id.to_string(),
            profile: record.profile,
            kind: record.provider_kind,
            status: format!("{:?}", record.status).to_lowercase(),
            root_path: /* ... */,
            capabilities: caps,
        });
    }
    
    Ok(infos)
}
```

### UI Exposes Available Actions
```rust
// Frontend code (conceptual)
for sandbox in sandboxes {
    let actions = vec![];
    
    if sandbox.status == "running" {
        if sandbox.capabilities.pausable {
            actions.push("Pause");
        }
        if sandbox.capabilities.stoppable {
            actions.push("Stop");
        }
    }
    
    if sandbox.status == "paused" && sandbox.capabilities.pausable {
        actions.push("Resume");
    }
    
    if sandbox.status == "stopped" && sandbox.capabilities.destroyable {
        actions.push("Destroy");
    }
    
    render_actions(sandbox.id, actions);
}
```

### RPC Response Includes Capabilities
```json
{
  "sandboxes": [
    {
      "id": "sb_01JXYZ...",
      "profile": "devbox",
      "kind": "ssh",
      "status": "running",
      "root_path": "/workspace",
      "capabilities": {
        "persistent": true,
        "pausable": false,
        "stoppable": false,
        "destroyable": false
      }
    }
  ]
}
```

## Benefits

### Backend Agnostic
Orchestration and UI don't need to know backend-specific details. They query capabilities at runtime.

### Extensibility
New backends declare their capabilities; no changes needed in orchestration or UI.

### Safety (intended, not yet realized)
The goal is that a UI only offers actions the backend supports. Nothing enforces this today — see the scope note above.

### Flexibility
Capabilities can be extended without breaking existing code.

## Implementation Example

### LocalSandboxProvider
```rust
impl SandboxProvider for LocalSandboxProvider {
    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            persistent: true,    // Local filesystem persists
            pausable: false,     // Cannot pause local process
            stoppable: false,    // Cannot stop without destroying
            destroyable: false,  // Cannot destroy host filesystem
        }
    }
}
```

### TmpSandboxProvider
```rust
impl SandboxProvider for TmpSandboxProvider {
    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            persistent: false,   // Temp directory is ephemeral
            pausable: false,     // Cannot pause
            stoppable: false,    // Cannot stop
            destroyable: true,   // Can destroy temp directory
        }
    }
}
```

### FirecrackerSandboxProvider (future)
```rust
impl SandboxProvider for FirecrackerSandboxProvider {
    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            persistent: true,    // VM state can be saved
            pausable: true,      // VM can be paused
            stoppable: true,     // VM can be stopped
            destroyable: true,   // VM can be destroyed
        }
    }
}
```

## Testing

Capabilities are tested per provider:
```rust
#[test]
fn test_local_provider_capabilities() {
    let provider = LocalSandboxProvider;
    let caps = provider.capabilities();
    
    assert!(caps.persistent);
    assert!(!caps.pausable);
    assert!(!caps.stoppable);
    assert!(!caps.destroyable);
}

#[test]
fn test_tmp_provider_capabilities() {
    let provider = TmpSandboxProvider;
    let caps = provider.capabilities();
    
    assert!(!caps.persistent);
    assert!(!caps.pausable);
    assert!(!caps.stoppable);
    assert!(caps.destroyable);
}
```

## Related Concepts
- [[sandbox-architecture]] — the four layers and provider matrix
- [[sandbox-lifecycle]] — overall lifecycle management, and why these flags are advisory
- [[provider-pattern]] — backend adapter pattern
- [[vol-llm-sandbox-crate]] — implementation details
