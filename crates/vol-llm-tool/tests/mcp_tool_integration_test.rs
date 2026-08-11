//! Integration tests for MCP tool proxying.
//!
//! Tests that McpTool instances can be created, registered in a ToolRegistry,
//! filtered by server, and that the naming convention is consistent.

use std::sync::Arc;
use vol_llm_mcp::McpManager;
use vol_llm_tool::mcp_tool::McpTool;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolRegistry};

#[test]
fn test_mcp_tool_name_convention() {
    let manager = Arc::new(McpManager::new(vec![]));
    let tool = McpTool::new(
        manager,
        "docs-rs-http",
        "search_crates",
        "Search Rust crates",
        serde_json::json!({"type": "object"}),
    );
    // Name must follow mcp__{sanitized_server}__{sanitized_tool} format
    let name = tool.name();
    assert!(
        name.starts_with("mcp__"),
        "Expected mcp__ prefix, got: {name}"
    );
    assert!(
        name.contains("__"),
        "Expected double-underscore separator, got: {name}"
    );
    // Server and tool names appear in the display name (hyphens preserved by sanitize_name)
    assert!(
        name.contains("docs-rs-http") || name.contains("docs_rs_http"),
        "Expected sanitized server name, got: {name}"
    );
    assert!(
        name.contains("search_crates"),
        "Expected sanitized tool name, got: {name}"
    );
}

#[test]
fn test_mcp_tool_registered_in_registry() {
    let manager = Arc::new(McpManager::new(vec![]));
    let tool = McpTool::new(
        manager,
        "test-server",
        "test_tool",
        "A test tool",
        serde_json::json!({"type": "object", "properties": {"x": {"type": "string"}}}),
    );
    let tool_name = tool.name().to_string();

    let mut registry = ToolRegistry::new();
    registry.register_boxed(Box::new(tool));

    assert!(registry.contains(&tool_name));
    let defs = registry.definitions();
    assert!(defs.iter().any(|d| d.name == tool_name));
}

#[test]
fn test_mcp_tool_filter_mcp_servers() {
    let manager1 = Arc::new(McpManager::new(vec![]));
    let manager2 = Arc::new(McpManager::new(vec![]));

    let tool_a = McpTool::new(
        manager1,
        "server-a",
        "tool_1",
        "Tool from server A",
        serde_json::json!({}),
    );
    let tool_b = McpTool::new(
        manager2,
        "server-b",
        "tool_2",
        "Tool from server B",
        serde_json::json!({}),
    );

    let mut registry = ToolRegistry::new();
    registry.register_boxed(Box::new(tool_a));
    registry.register_boxed(Box::new(tool_b));

    // Filter to only allow server-a
    let filtered = registry.filter_mcp_servers(&["server-a".to_string()]);
    let names = filtered.tool_names();

    // Should contain server-a's tool but not server-b's
    let a_name = names.iter().find(|n| n.contains("server-a")).unwrap();
    assert!(a_name.contains("tool_1"));
    assert!(!names.iter().any(|n| n.contains("server-b")));
}

#[tokio::test]
async fn test_mcp_tool_metadata_preserved() {
    let manager = Arc::new(McpManager::new(vec![]));
    let params = serde_json::json!({
        "type": "object",
        "properties": {
            "query": {"type": "string", "description": "Search query"}
        },
        "required": ["query"]
    });
    let tool = McpTool::new(
        manager,
        "search-svc",
        "find",
        "Search for items",
        params.clone(),
    );

    assert_eq!(tool.description(), "Search for items");
    assert_eq!(tool.parameters(), params);

    // Sensitivity should always be Safe for MCP tools
    match tool.sensitivity(&serde_json::json!({})) {
        vol_llm_tool::ToolSensitivity::Safe => {}
        _ => panic!("McpTool should always be Safe"),
    }
}

#[tokio::test]
async fn test_mcp_tool_execute_with_no_server_errors() {
    let manager = Arc::new(McpManager::new(vec![]));
    // Don't connect — the tool will fail when trying to call the server
    let tool = McpTool::new(
        manager,
        "nonexistent",
        "noop",
        "desc",
        serde_json::json!({}),
    );
    let result = tool
        .execute(&serde_json::json!({}), &ToolContext::for_test())
        .await;
    // Should fail because the manager has no connected server with that name
    assert!(result.is_err());
}
