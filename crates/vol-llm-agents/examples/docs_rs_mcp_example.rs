//! Docs-RS MCP Integration Example.
//!
//! Demonstrates ReActAgent connecting to the docs-rs MCP server to search
//! for the "dioxus" crate and return its README summary.
//!
//! Prerequisites:
//! 1. Build the docs-rs-mcp binary:
//!    cargo build --bin docs-rs-mcp -p vol-mcp-servers
//! 2. Set ANTHROPIC_AUTH_TOKEN environment variable.
//!
//! Run:
//!    export DOCS_RS_MCP_BIN=$(pwd)/target/debug/docs-rs-mcp
//!    export ANTHROPIC_AUTH_TOKEN=your_token
//!    cargo run --example docs_rs_mcp_example -p vol-llm-agents

use vol_llm_agent::react::{AgentConfig, ReActAgent};
use vol_llm_provider::{AnthropicProvider, LLMConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("═══════════════════════════════════════════════════════════");
    println!("  Docs-RS MCP Integration Example");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // --- Validate prerequisites ---

    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN").map_err(|_| {
        eprintln!("Error: ANTHROPIC_AUTH_TOKEN environment variable not set");
        eprintln!("Set it: export ANTHROPIC_AUTH_TOKEN=your_token");
        "Missing API token"
    })?;
    println!("✓ ANTHROPIC_AUTH_TOKEN is set");

    let docs_rs_bin = std::env::var("DOCS_RS_MCP_BIN")
        .unwrap_or_else(|_| "docs-rs-mcp".to_string());
    println!("✓ docs-rs-mcp binary: {docs_rs_bin}");

    // --- Create temp directory with .mcp.json ---

    let tmp_dir = tempfile::tempdir()?;
    let mcp_json_path = tmp_dir.path().join(".mcp.json");

    let mcp_config = serde_json::json!({
        "mcpServers": {
            "docs-rs": {
                "command": docs_rs_bin,
                "args": [],
                "env": {}
            }
        }
    });
    std::fs::write(&mcp_json_path, serde_json::to_string_pretty(&mcp_config)?)?;
    println!("✓ MCP config written to: {}", mcp_json_path.display());
    println!();

    // --- Initialize LLM provider ---

    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.6-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );
    let llm = AnthropicProvider::new(&llm_config)?;
    println!("✓ Anthropic provider initialized (qwen3.6-plus via DashScope)");
    println!();

    // --- Build ReActAgent with MCP ---

    println!("Building agent with MCP integration...");

    let agent_config = AgentConfig::builder()
        .with_llm(std::sync::Arc::new(llm))
        .with_system_prompt(
            "You are a documentation assistant. Use your tools to search for \
             information about crates on docs.rs. Return concise summaries."
                .to_string(),
        )
        .with_mcp_from_config(Some(tmp_dir.path()))
        .await
        .build()?;

    // Print discovered MCP tools
    let registry = &agent_config.tools;
    let mcp_tools: Vec<_> = registry
        .definitions()
        .into_iter()
        .filter(|d| d.name.starts_with("mcp__"))
        .collect();
    if mcp_tools.is_empty() {
        eprintln!("Warning: No MCP tools discovered. Check that docs-rs-mcp binary is available.");
        eprintln!("Build it: cargo build --bin docs-rs-mcp -p vol-mcp-servers");
        std::process::exit(1);
    }
    println!("✓ Discovered {} MCP tools:", mcp_tools.len());
    for tool in &mcp_tools {
        println!("    - {} ({})", tool.name, tool.description.as_deref().unwrap_or("no description"));
    }
    println!();

    let agent = ReActAgent::new(agent_config);
    println!("✓ ReActAgent built");
    println!();

    // --- Run the agent ---

    let query = "搜索 dioxus 这个 crate，获取它的 README 并返回简要介绍";

    println!("═══════════════════════════════════════════════════════════");
    println!("  Running Agent");
    println!("═══════════════════════════════════════════════════════════");
    println!("Query: {}", query);
    println!();

    let result = agent.run(query).await;

    // --- Print results ---

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Results");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    match result {
        Ok(response) => {
            println!("Agent completed successfully.");
            println!("Run ID: {}", response.run_id);
            println!("Iterations: {}, Tool calls: {}", response.iterations, response.tool_calls.len());
            println!();
            println!("Answer:");
            println!("{}", response.content);
        }
        Err(e) => {
            eprintln!("Agent run failed: {:?}", e);
            std::process::exit(1);
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Example Complete");
    println!("═══════════════════════════════════════════════════════════");

    // Temp dir is automatically cleaned up on drop

    Ok(())
}
