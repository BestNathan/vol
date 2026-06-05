# Context Ordering Standard — Design Spec

**Date**: 2026-06-05
**Status**: design-approved
**Scope**: `vol-llm-context`, `vol-llm-agent`, `vol-llm-skill`, `vol-session`, `CLAUDE.md`

## Problem

`ContextBuilder` contributors are registered with ad-hoc `AttentionAnchor` positions. No standard governs which contributor goes where. Agent Prompt and SkillInjector both use `Head(0)`, SessionContributor uses `Middle(0)` incorrectly, and there is no mechanism for injecting custom files from agent definitions.

## Design

### Context Ordering Standard

Agent context is assembled by `ContextBuilder` in the following fixed, validated order:

| Zone | Position | Name | Source | Required |
|------|----------|------|--------|----------|
| Head | 0 | Agent Prompt | `AgentDef.prompt` | Yes (empty placeholder if unset) |
| Head | 1 | Skills | `SkillInjector` | Yes (empty block if no skills loaded) |
| Middle | 0..n | Custom Files | `AgentDef.context_files` (paths relative to work_dir) | No |
| Tail | 0 | Session | `SessionContributor` (conversation history) | Yes |

**Rules:**

- Head and Tail sections are fixed-position — always present, never dropped on budget overflow.
- Custom Files are loaded from disk in array order: first file → `Middle(0)`, second → `Middle(1)`, etc.
- Middle blocks are eligible for budget-driven truncation (highest position dropped first).
- All new contributors MUST declare their zone and position explicitly.

### Component Changes

#### 1. AgentDef — new field

```rust
/// Custom context files, relative to agent's working directory.
/// Loaded in array order into Middle(0..n) of context.
pub context_files: Vec<String>,
```

Builder method:
```rust
pub fn with_context_files(mut self, files: Vec<String>) -> Self {
    self.context_files = files;
    self
}
```

#### 2. SkillInjector — parameterized anchor

Constructor accepts `AttentionAnchor` instead of hard-coding `Head(0)`:

```rust
pub fn new(loader: Arc<SkillLoader>, anchor: AttentionAnchor) -> Self
```

Called as: `SkillInjector::new(skill_loader, AttentionAnchor::Head(1))`

#### 3. SessionContributor — parameterized anchor

Constructor accepts `AttentionAnchor` instead of hard-coding `Middle(0)`:

```rust
pub fn new(session: Arc<Mutex<Session>>, max_messages: usize, anchor: AttentionAnchor) -> Self
```

Called as: `SessionContributor::new(session, max_history, AttentionAnchor::Tail(0))`

#### 4. AgentConfigBuilder::build() — standardized assembly

```rust
// 1. Agent Prompt — always Head(0), empty if unset
let prompt = self.def.as_ref().map(|d| d.prompt.clone()).unwrap_or_default();
b = b.add_contributor(Box::new(SimpleContributor::system(prompt)));

// 2. Skills — always Head(1)
b = b.add_contributor(Box::new(SkillInjector::new(skill_loader, AttentionAnchor::Head(1))));

// 3. Custom Files — Middle(0..n) from AgentDef.context_files
if let Some(ref def) = self.def {
    let specs: Vec<FileSpec> = def.context_files.iter().enumerate()
        .map(|(i, path)| {
            let full_path = def.working_dir.as_ref()
                .map(|d| d.join(path))
                .unwrap_or_else(|| PathBuf::from(path));
            FileSpec::new(full_path, AttentionAnchor::Middle(i as u32))
        })
        .collect();
    if !specs.is_empty() {
        b = b.add_contributor(Box::new(FileContributor::new(specs)));
    }
}

// 4. Clone existing context_builder contributors (if any)
// 5. Manual contributors from with_system_prompt / with_contributor
// 6. Session — Tail(0)
b = b.add_contributor(Box::new(SessionContributor::new(
    session_clone, max_history, AttentionAnchor::Tail(0),
)));
```

#### 5. CLAUDE.md — documented standard

Add `## Context Ordering Standard` section with the table and rules above.

### Files Touched

| File | Change |
|------|--------|
| `crates/vol-llm-agent/src/agent_def.rs` | Add `context_files` field + `with_context_files()` |
| `crates/vol-llm-agent/src/react/config_builder.rs` | Reorder + fix context assembly |
| `crates/vol-llm-skill/src/injector.rs` | Parameterize `AttentionAnchor` in constructor |
| `crates/vol-session/src/session_contributor.rs` | Parameterize `AttentionAnchor` in constructor |
| `CLAUDE.md` | Add Context Ordering Standard section |

### Backward Compatibility

- `SkillInjector::new(loader)` without anchor parameter breaks. Fix: update all call sites to pass `AttentionAnchor::Head(1)` or the desired anchor.
- `SessionContributor::new(session, max)` without anchor parameter breaks. Fix: update all call sites.
- All other callers of `SkillInjector` and `SessionContributor` in the workspace must be updated.

Call sites to update:
- `crates/vol-llm-agent/src/react/config_builder.rs` — main call site
- `crates/vol-llm-agents/src/coding/agent.rs`
- `crates/vol-llm-wiki/src/agent.rs`
- `crates/vol-llm-yaml-agent/src/builder.rs`
- `crates/vol-llm-agents/tests/skill_session_integration.rs`
- `crates/vol-llm-agents/tests/observer_plugin_unit.rs`
- `crates/vol-llm-agent/tests/react_agent_integration.rs`
- `crates/vol-llm-agent/tests/compression_flow_test.rs`

### Testing

- **Unit tests**: Verify context zone ordering (head → middle → tail), verify empty prompt still produces Head(0) placeholder, verify custom files map to Middle(0..n) in array order.
- **Integration tests**: Build AgentConfig with context_files and verify the assembled message order.
- **Existing tests**: Update all call sites for the new constructor signatures.
