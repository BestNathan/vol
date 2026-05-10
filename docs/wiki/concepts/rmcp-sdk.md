---
type: concept
category: framework
tags: [mcp, rmcp, rust, sdk]
created: 2026-05-10
updated: 2026-05-10
source_count: 1
---

# rmcp SDK

**Category:** Rust library — Model Context Protocol
**Related:** [[vol-mcp-servers-crate]], [[mcp-transport-pattern]], [[docs-rs-tools]]

## Overview

`rmcp` (v1.6.0) is the official Rust SDK for the Model Context Protocol. Provides server/client implementations, tool/prompt/prompt macros, and pluggable transport layer.

## Key Features Used

### Tool Macros

```rust
#[tool_router(server_handler)]
impl MyServer {
    #[tool(description = "Tool description")]
    async fn my_tool(&self, Parameters(params): Parameters<MyParams>)
        -> Result<String, String> { ... }
}
```

- `#[tool_router(server_handler)]` generates the `Service` impl for the server
- `#[tool(description = "...")]` registers the method as an MCP tool
- `Parameters<T>` wrapper deserializes from MCP tool call arguments
- `schemars::JsonSchema` derive on params structs generates input schemas automatically

### Transports

- `rmcp::transport::stdio` — stdio-based transport (default for CLI servers)
- `rmcp::transport::streamable_http_server::tower::StreamableHttpService` — HTTP/SSE server transport
- Session management via `LocalSessionManager` (default, stateful)

### Service Trait

- `ServiceExt::serve(transport)` — starts the server on a transport
- `ServerHandler` — trait that MCP server implementations satisfy

### Feature Flags

| Feature | Purpose |
|---------|---------|
| `server` | Server functionality (default) |
| `macros` | `#[tool]` / `#[prompt]` macros (default) |
| `schemars` | JSON Schema generation for tool definitions |
| `transport-io` | stdio transport |
| `transport-streamable-http-server` | Streamable HTTP server transport |
