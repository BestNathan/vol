# mcps field — refactor: filter from full registry

**Date:** 2026-06-18
**Status:** draft

## Context

PR #ci/prune-old-tags added `mcps: [server-names]` to agent markdown frontmatter, so agents can select which MCP servers' tools to use. The initial implementation duplicated built-in tool registration in `register_agent()` — once in `build()` for the shared registry, and again in `register_agent()` when `mcps` is set.

This refactor eliminates the duplication by treating the shared registry as the "universe" and applying per-agent filters (MCP server allowlist, tool allowlist, tool blocklist) during registration.

## Design

### Principle

- **`build()`**: sole assembly point for the full ToolRegistry (builtins + all MCP tools)
- **`register_agent()`**: clone full → filter MCP servers → filter tools/disallowed → agent

### New method: `ToolRegistry::filter_mcp_servers`

```rust
/// Keep non-MCP tools (builtins) + only MCP tools from named servers.
/// Server names are matched after sanitization (same as McpManager uses).
pub fn filter_mcp_servers(&self, server_names: &[String]) -> Self
```

Matching logic:
- Non-MCP tools (no `mcp__` prefix) → always kept
- MCP tools (`mcp__{server}__{tool}`) → kept only if `server` is in the sanitized `server_names` set

### Simplified `register_agent()`

```rust
// Start from full registry
let mut tool_registry = (*self.tool_registry).clone();

// Filter MCP servers
if let Some(ref server_names) = def.mcps {
    tool_registry = tool_registry.filter_mcp_servers(server_names);
}

// Filter allowed/disallowed tools (existing mechanism)
let tool_registry = Arc::new(tool_registry.filter(
    tools_to_strs(def.tools.as_ref()).as_deref(),
    tools_to_strs(def.disallowed_tools.as_ref()).as_deref(),
));
```

### Removals

- Duplicated builtin registration in `register_agent()` (~8 lines)
- `ToolRegistry::register_from_mcp_filtered()` — no longer needed

## Files changed

| File | Change |
|------|--------|
| `crates/vol-llm-tool/src/registry.rs` | Add `filter_mcp_servers()`, remove `register_from_mcp_filtered()` |
| `crates/vol-llm-runtime/src/lib.rs` | Simplify `register_agent()` — remove duplicate assembly |

## Verification

```bash
cargo test -p vol-llm-tool -p vol-llm-runtime -p vol-llm-agent
```

Existing tests cover:
- `mcps` frontmatter parsing
- `mcps` passthrough in agent loader
- `with_mcps()` builder
- Agent registration with/without `mcps`

New tests needed:
- `filter_mcp_servers` — keeps non-MCP tools, filters MCP by server name
