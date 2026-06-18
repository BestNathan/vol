# mcps filter-from-full-registry — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate duplicated builtin tool registration in `register_agent()` — instead clone the full shared ToolRegistry and apply per-agent filters (mcps, tools, disallowed_tools).

**Architecture:** `build()` is the sole assembly point. `register_agent()` clones the full registry, then chains: `filter_mcp_servers()` → `filter(tools, disallowed)`. `register_from_mcp_filtered()` is deleted.

**Tech Stack:** Rust, existing `vol-llm-tool` / `vol-llm-runtime` crates

---

### Task 1: Replace `register_from_mcp_filtered` with `filter_mcp_servers` in ToolRegistry

**Files:**
- Modify: `crates/vol-llm-tool/src/registry.rs`

- [ ] **Step 1: Delete `register_from_mcp_filtered` method**

Remove the entire `register_from_mcp_filtered` method (lines 131-169 in the current file).

- [ ] **Step 2: Delete the two `register_from_mcp_filtered` tests**

Remove `test_register_from_mcp_filtered_empty_manager` and `test_register_from_mcp_filtered_with_empty_names` (lines 284-311).

- [ ] **Step 3: Add `filter_mcp_servers` method**

Add after `filter()` (after line 99):

```rust
/// Keep non-MCP tools (all builtins) + only MCP tools from named servers.
///
/// MCP tools follow the naming convention `mcp__{server}__{tool}`.
/// Server names are matched after sanitization (same normalisation McpManager uses).
/// Returns a new ToolRegistry — does not mutate self.
pub fn filter_mcp_servers(&self, server_names: &[String]) -> Self {
    let allowed: std::collections::HashSet<String> = server_names
        .iter()
        .map(|n| vol_llm_mcp::session::sanitize_name(n))
        .collect();

    let tools = self
        .tools
        .iter()
        .filter(|(name, _)| {
            // Keep non-MCP tools (no "mcp__" prefix)
            if !name.starts_with("mcp__") {
                return true;
            }
            // For MCP tools, extract the server name segment and check allowlist.
            // Format: mcp__{server}__{tool}
            let rest = &name[5..]; // strip "mcp__"
            if let Some(server_end) = rest.find("__") {
                let server = &rest[..server_end];
                allowed.contains(server)
            } else {
                // Malformed name — keep it (shouldn't happen, but be safe).
                true
            }
        })
        .map(|(name, tool)| (name.clone(), Arc::clone(tool)))
        .collect();

    Self { tools }
}
```

- [ ] **Step 4: Add tests for `filter_mcp_servers`**

Add after the existing `test_register_from_mcp_empty_manager` test:

```rust
#[test]
fn test_filter_mcp_servers_keeps_non_mcp_tools() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("bash"));
    registry.register(DummyTool::new("read_file"));
    registry.register(DummyTool::new("skill"));
    let filtered = registry.filter_mcp_servers(&["docs-rs".to_string()]);
    let names = filtered.tool_names();
    assert!(names.contains(&"bash"));
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"skill"));
}

#[test]
fn test_filter_mcp_servers_filters_mcp_by_name() {
    // Simulate MCP-named tools using the DummyTool with mcp__-prefixed names.
    // We need to register tools with mcp__ server names.  DummyTool uses its
    // constructor name, so we create tools whose `name()` returns the mcp__ form.
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("mcp__docs_rs__search"));
    registry.register(DummyTool::new("mcp__weather__forecast"));
    registry.register(DummyTool::new("bash"));
    registry.register(DummyTool::new("read_file"));

    // Filter to only docs-rs server
    let filtered = registry.filter_mcp_servers(&["docs-rs".to_string()]);
    let names = filtered.tool_names();
    assert!(names.contains(&"mcp__docs_rs__search"));
    assert!(!names.contains(&"mcp__weather__forecast"));
    assert!(names.contains(&"bash"));       // non-MCP kept
    assert!(names.contains(&"read_file"));   // non-MCP kept
}

#[test]
fn test_filter_mcp_servers_empty_list_removes_all_mcp() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("mcp__docs_rs__search"));
    registry.register(DummyTool::new("bash"));

    let filtered = registry.filter_mcp_servers(&[]);
    let names = filtered.tool_names();
    assert!(names.contains(&"bash"));
    assert!(!names.contains(&"mcp__docs_rs__search"));
}

#[test]
fn test_filter_mcp_servers_sanitizes_names() {
    // Server names from user input may contain dots etc. — they should be
    // matched against the sanitized form used by McpManager.
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("mcp__docs_rs__search")); // sanitized: docs_rs

    let filtered = registry.filter_mcp_servers(&["docs.rs".to_string()]);
    // "docs.rs" sanitizes to "docs_rs" — should match
    assert!(filtered.tool_names().contains(&"mcp__docs_rs__search"));
}
```

- [ ] **Step 5: Run vol-llm-tool tests**

Run: `cargo test -p vol-llm-tool --lib`
Expected: all tests pass (old filtered tests removed, new filter_mcp_servers tests pass)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tool/src/registry.rs
git commit -m "refactor(tool): replace register_from_mcp_filtered with filter_mcp_servers

filter_mcp_servers operates on an already-populated ToolRegistry
rather than connecting to McpManager.  This lets register_agent()
filter from the full shared registry instead of rebuilding from
scratch.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Simplify `register_agent()` to filter from full registry

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs:136-149`

- [ ] **Step 1: Replace the duplicated assembly with filter chain**

Replace lines 134-149 (the `let tool_registry = if let Some(...` block):

```rust
        // Clone the full shared registry, then apply per-agent filters.
        let mut tool_registry = (*self.tool_registry).clone();

        // Filter MCP servers if mcps is set
        if let Some(ref server_names) = def.mcps {
            tool_registry = tool_registry.filter_mcp_servers(server_names);
        }

        // Filter allowed/disallowed tools (existing mechanism)
        let allowed_refs: Option<Vec<&str>> = def
            .tools
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect());
        let disallowed_refs: Option<Vec<&str>> = def
            .disallowed_tools
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect());
        let tool_registry = tool_registry.filter(
            allowed_refs.as_deref(),
            disallowed_refs.as_deref(),
        );
        let tool_registry = Arc::new(tool_registry);
```

- [ ] **Step 2: Run vol-llm-runtime tests**

Run: `cargo test -p vol-llm-runtime --lib`
Expected: `register_agent_with_mcps_creates_filtered_tool_registry` and `register_agent_without_mcps_uses_shared_registry` both pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-runtime/src/lib.rs
git commit -m "refactor(runtime): filter agent tool registry from full shared registry

register_agent() no longer rebuilds builtin tools from scratch when
mcps is set.  Instead it clones the full shared ToolRegistry and
applies filter_mcp_servers() + filter() to produce the per-agent view.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Verify full test suite

- [ ] **Step 1: Run all affected crate tests**

Run: `cargo test -p vol-llm-tool -p vol-llm-runtime -p vol-llm-agent -p vol-llm-core --lib`
Expected: all tests pass, no regressions.

- [ ] **Step 2: Check server crate compiles**

Run: `cargo check -p vol-agent-server`
Expected: compiles cleanly.

- [ ] **Step 3: Commit (if any cleanup needed)**

Only if step 1 or 2 required fixes.
