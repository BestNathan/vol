# Docs-RS MCP Integration Example Design

> End-to-end example demonstrating ReActAgent connecting to docs-rs MCP server to search for the "dioxus" crate.

**Goal:** Create a runnable example (`docs_rs_mcp_example.rs`) that demonstrates the full MCP integration flow — config loading, server connection, tool discovery, and tool execution via `with_mcp_from_config()`.

## Architecture

```
.env (ANTHROPIC_AUTH_TOKEN)    docs-rs-mcp binary (vol-mcp-servers)
        │                              │
        ▼                              ▼
   LLM Provider ──→ ReActAgent ←── McpSession (STDIO)
                        │
                        ├── McpTool: mcp__docs_rs__search_crates
                        ├── McpTool: mcp__docs_rs__readme
                        └── McpTool: mcp__docs_rs__search_in_crate
```

The example creates a temporary directory containing a `.mcp.json` file, uses it as the working directory for `with_mcp_from_config()`, then runs the agent with a Chinese-language prompt to search for the dioxus crate.

## File Structure

**New file:**
- `crates/vol-llm-agents/examples/docs_rs_mcp_example.rs`

**No modifications needed** — all dependencies already exist in the workspace.

## Component Design

### 1. MCP Config in Temp Directory

Create a `tempfile::TempDir`, write `.mcp.json` into it:

```json
{
  "mcpServers": {
    "docs-rs": {
      "command": "<path-to-docs-rs-mcp-binary>",
      "args": [],
      "env": {}
    }
  }
}
```

Binary path resolved via `DOCS_RS_MCP_BIN` env var, defaulting to `"docs-rs-mcp"` (expected in PATH or built via `cargo build --bin docs-rs-mcp`).

### 2. Agent Construction

Pattern follows `agent_loki_example.rs`:

- LLM: Anthropic provider via DashScope (`qwen3.6-plus`)
- System prompt: "You are a documentation assistant. Use your tools to search for information about crates."
- MCP: `with_mcp_from_config(Some(&tmp_dir))` — discovers `docs-rs-mcp` from `.mcp.json`
- No additional plugins needed

### 3. Execution and Verification

Task prompt (Chinese): `"搜索 dioxus 这个 crate，获取它的 README 并返回简要介绍"`

The agent will:
1. Call `mcp__docs_rs__search_crates("dioxus")` to find the crate
2. Call `mcp__docs_rs__readme("dioxus")` to get the README
3. Return a summary

The example prints:
- Discovered MCP tool names
- Agent's final answer
- Whether execution succeeded or failed

### 4. Run Command

```bash
# First build the docs-rs-mcp binary
cargo build --bin docs-rs-mcp -p vol-mcp-servers

# Set env and run
export DOCS_RS_MCP_BIN=$(pwd)/target/debug/docs-rs-mcp
export ANTHROPIC_AUTH_TOKEN=your_token
cargo run --example docs_rs_mcp_example -p vol-llm-agents
```
