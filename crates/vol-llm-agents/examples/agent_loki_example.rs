//! Example: K8s Ops Agent loaded from file with Loki observability and built-in tools.
//!
//! This example demonstrates:
//! - Loading an agent definition from a .md file via AgentLoader
//! - Registering built-in tools (Bash, Read, Write, Edit, Glob, Grep)
//! - Building a ReActAgent with the loaded AgentDef and tools
//! - Registering LokiPlugin for OTel log shipping via tracing
//! - Running a real task against the Anthropic/DashScope API
//!
//! Run with:
//! ```bash
//! export ANTHROPIC_AUTH_TOKEN=your_token_here
//! # Optional: override OTel collector endpoint (default: http://localhost:4317)
//! export OTEL_EXPORTER_OTLP_ENDPOINT=http://your-collector:4317
//! cargo run --example agent_loki_example
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use vol_llm_agent::agent_def::AgentScope;
use vol_llm_agent::agent_loader::AgentLoader;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_observability::{init_otel_logs, LokiPlugin};
use vol_llm_provider::{anthropic::AnthropicProvider, LLMConfig};
use vol_llm_tools_builtin::register_all;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with OTel log export
    let otel_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());
    init_otel_logs(&otel_endpoint, "k8s-ops-agent").map_err(|e| {
        Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
    })?;

    println!("═══════════════════════════════════════════════════════════");
    println!("  K8s Ops Agent — File-Loaded with Loki Observability");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // Check for required environment variables
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN").map_err(|_| {
        eprintln!("Error: ANTHROPIC_AUTH_TOKEN environment variable not set");
        eprintln!("Please set it: export ANTHROPIC_AUTH_TOKEN=your_token_here");
        "Missing API token"
    })?;

    println!("Configuration:");
    println!("  ✓ ANTHROPIC_AUTH_TOKEN is set");
    println!("  ✓ OTel endpoint: {otel_endpoint}");
    println!();

    // Create LLM config for DashScope Anthropic endpoint
    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.6-plus",
        api_key,
        "http://k8s.nhome.local:31693",
    );

    // Create Anthropic provider
    let llm = AnthropicProvider::new(&llm_config)
        .map_err(|e| format!("Failed to create Anthropic provider: {e}"))?;

    println!("  ✓ Anthropic provider initialized (qwen3.6-plus via DashScope)");
    println!();

    // Step 1: Load agent definition from examples/k8s_ops_agent.md
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let agents_dir = manifest_dir.join("examples").join(".agents");

    println!("  ✓ Agents dir: {}", agents_dir.display());
    println!();

    // Step 2: Load via AgentLoader
    let mut loader = AgentLoader::new_empty();
    loader.add_root(AgentScope::Repo, agents_dir.clone());
    loader.discover_all().await?;

    let def = loader
        .get("k8s_ops_agent")
        .await
        .expect("k8s_ops_agent should be loaded from examples/");

    println!("  ✓ Agent loaded via AgentLoader:");
    println!("    - name: {}", def.name);
    println!("    - type: {}", def.r#type);
    println!("    - description: {}", def.description);
    println!("    - tools: {:?}", def.tools);
    println!();

    // Step 3: Build tool registry with built-in tools
    let mut tool_registry = vol_llm_tool::ToolRegistry::new();
    register_all(&mut tool_registry);

    println!("  ✓ Built-in tools registered:");
    for tool_def in tool_registry.definitions() {
        println!("    - {}", tool_def.name);
    }
    println!();

    // Step 4: Build LokiPlugin
    let loki_plugin = LokiPlugin::new();

    let mut plugin_registry = PluginRegistry::new();
    plugin_registry.register(loki_plugin);

    println!("  ✓ LokiPlugin registered (OTel logs via tracing)");
    println!();

    // Step 5: Build ReActAgent with AgentDef + tools
    let agent_config = AgentConfig::builder()
        .with_def((*def).clone())
        .with_llm(Arc::new(llm))
        .with_tools(Arc::new(tool_registry))
        .with_system_prompt(def.prompt.clone())
        .with_plugin_registry(plugin_registry)
        .build()?;

    let agent = ReActAgent::new(agent_config);

    println!("  ✓ ReActAgent built with AgentDef + tools + LokiPlugin");
    println!();

    // Step 6: Run the agent with a k8s cluster status query
    let query = "请获取当前 k8s 集群的节点状态和系统 Pod 的运行状态，分析集群健康状况";

    println!("═══════════════════════════════════════════════════════════");
    println!("  Running Agent");
    println!("═══════════════════════════════════════════════════════════");
    println!("Query: {query}");
    println!();

    let result = agent.run(query).await;

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Agent Execution Results");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    match result {
        Ok(response) => {
            println!("Agent completed successfully.");
            println!("Run ID: {}", response.run_id);
            println!("Final answer: {}", response.content);
        }
        Err(e) => {
            eprintln!("Agent run failed: {e:?}");
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Observability");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Logs are sent to OTel Collector via tracing layer.");
    println!("Log attributes:");
    println!("  - namespace: agent");
    println!("  - agent: k8s_ops_agent");
    println!("  - agent_id: k8s_ops_agent");
    println!("  - model: qwen3.6-plus");
    println!();

    println!("═══════════════════════════════════════════════════════════");
    println!("  Example Complete");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Features demonstrated:");
    println!("  ✓ Agent definition loaded from examples/k8s_ops_agent.md via AgentLoader");
    println!("  ✓ Built-in tools (Bash, Read, Write, Edit, Glob, Grep) registered");
    println!("  ✓ Real Anthropic API calls via DashScope");
    println!("  ✓ LokiPlugin registered for OTel log shipping via tracing");
    println!("  ✓ Log attributes derived automatically from AgentDef");
    println!();

    Ok(())
}
