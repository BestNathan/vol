---
type: concept
category: pattern
tags: [state-machine, lifecycle, validation, sandbox]
created: 2026-08-27
updated: 2026-08-27
source_count: 1
---

# Lifecycle State Machine

## Overview
The Lifecycle State Machine is a pattern for managing sandbox instance lifecycle with explicit states and validated transitions. The `SandboxManager` enforces state transition rules, preventing undefined behavior and ensuring consistent lifecycle management.

## State Diagram

```
    ┌──────────┐
    │ Creating │
    └────┬─────┘
         │
         ▼
    ┌──────────┐
    │ Created  │
    └────┬─────┘
         │ start
         ▼
    ┌──────────┐    pause    ┌────────┐
    │ Running  │────────────►│ Paused │
    └────┬─────┘             └────┬───┘
         │                        │
         │ stop                   │ resume
         │                        │
         ▼                        │
    ┌──────────┐                  │
    │ Stopping │◄─────────────────┘
    └────┬─────┘
         │
         ▼
    ┌──────────┐
    │ Stopped  │
    └────┬─────┘
         │ destroy
         ▼
    ┌───────────┐
    │ Destroying│
    └────┬──────┘
         │
         ▼
    ┌───────────┐
    │ Destroyed │
    └───────────┘

    Any state ──► Failed (on error)
```

## Valid Transitions

| From | To | Operation |
|------|-----|-----------|
| Creating | Created | System |
| Created | Starting | System |
| Starting | Running | System |
| Running | Pausing | pause() |
| Pausing | Paused | System |
| Paused | Starting | resume() |
| Running | Stopping | stop() |
| Paused | Stopping | stop() |
| Stopping | Stopped | System |
| Stopped | Destroying | destroy() |
| Destroying | Destroyed | System |
| Any | Failed | Error |

## Implementation

### State Validation
```rust
fn validate_transition(from: SandboxStatus, to: SandboxStatus) -> SandboxResult<()> {
    let valid = matches!(
        (from, to),
        (SandboxStatus::Created, SandboxStatus::Starting)
            | (SandboxStatus::Starting, SandboxStatus::Running)
            | (SandboxStatus::Running, SandboxStatus::Pausing)
            | (SandboxStatus::Running, SandboxStatus::Stopping)
            | (SandboxStatus::Running, SandboxStatus::Stopped)
            | (SandboxStatus::Pausing, SandboxStatus::Paused)
            | (SandboxStatus::Paused, SandboxStatus::Starting)
            | (SandboxStatus::Paused, SandboxStatus::Running)
            | (SandboxStatus::Paused, SandboxStatus::Stopping)
            | (SandboxStatus::Paused, SandboxStatus::Stopped)
            | (SandboxStatus::Stopping, SandboxStatus::Stopped)
            | (SandboxStatus::Stopped, SandboxStatus::Starting)
            | (SandboxStatus::Stopped, SandboxStatus::Running)
            | (SandboxStatus::Stopped, SandboxStatus::Destroying)
            | (SandboxStatus::Stopped, SandboxStatus::Destroyed)
            | (SandboxStatus::Destroying, SandboxStatus::Destroyed)
            | (_, SandboxStatus::Failed)
    );
    
    if valid {
        Ok(())
    } else {
        Err(SandboxError::InvalidTransition { from, to })
    }
}
```

### Usage in Manager
```rust
pub async fn stop(&self, id: &SandboxId) -> SandboxResult<()> {
    let record = self.store.get(id).await?;
    
    // Validate state transition
    Self::validate_transition(record.status, SandboxStatus::Stopped)?;
    
    // Delegate to provider
    let provider = self.get_provider(&record.provider_kind)?;
    provider.stop(&record.backend_id).await?;
    
    // Update state
    self.store.update_status(id, SandboxStatus::Stopped).await?;
    Ok(())
}
```

## Benefits

### Safety
Invalid transitions are rejected at runtime, preventing undefined behavior.

### Clarity
Explicit states make the lifecycle model clear and documentable.

### Debugging
State transitions can be logged and audited for troubleshooting.

### Flexibility
New states and transitions can be added without breaking existing code.

## Error Handling

Invalid transitions return `SandboxError::InvalidTransition`:
```rust
match manager.stop(&id).await {
    Ok(()) => println!("Stopped successfully"),
    Err(SandboxError::InvalidTransition { from, to }) => {
        eprintln!("Cannot transition from {:?} to {:?}", from, to);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Testing

State machine is thoroughly tested:
```rust
#[tokio::test]
async fn test_invalid_state_transitions() {
    let manager = SandboxManager::new();
    // ... setup ...
    
    let id = manager.create("test").await.unwrap();
    
    // Try invalid transition (Running → Running)
    let result = manager.start(&id).await;
    assert!(result.is_err());
    
    // Valid transition (Running → Stopped)
    manager.stop(&id).await.unwrap();
    
    // Valid transition (Stopped → Running)
    manager.start(&id).await.unwrap();
}
```

## Related Concepts
- [[sandbox-lifecycle]] — overall lifecycle management
- [[provider-pattern]] — backend adapter pattern
- [[vol-llm-sandbox-crate]] — implementation details
