//! Example: K8s Ops Agent loaded from file with Loki observability.
//!
//! This example demonstrates:
//! - Loading an agent definition from a .md file via AgentLoader
//! - Building a ReActAgent with the loaded AgentDef
//! - Registering LokiPlugin for remote log shipping
//! - Running a real task against the Anthropic/DashScope API
//!
//! Run with:
//! ```bash
//! export ANTHROPIC_AUTH_TOKEN=your_token_here
//! export LOKI_URL=http://localhost:3100
//! cargo run --example agent_loki_example
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use vol_llm_agent::agent_def::AgentScope;
use vol_llm_agent::agent_loader::AgentLoader;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_observability::loki::{LokiConfig, LokiPlugin};
use vol_llm_provider::{LLMConfig, anthropic::AnthropicProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

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

    let loki_url = std::env::var("LOKI_URL")
        .unwrap_or_else(|_| "http://localhost:3100".to_string());

    println!("Configuration:");
    println!("  ✓ ANTHROPIC_AUTH_TOKEN is set");
    println!("  ✓ LOKI_URL = {}", loki_url);
    println!();

    // Create LLM config for DashScope Anthropic endpoint
    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );

    // Create Anthropic provider
    let llm = AnthropicProvider::new(&llm_config)
        .map_err(|e| format!("Failed to create Anthropic provider: {}", e))?;

    println!("  ✓ Anthropic provider initialized (qwen3.5-plus via DashScope)");
    println!();

    // Step 1: Create the agent definition file in the examples directory
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let agents_dir = manifest_dir.join(".agents").join("agents");
    std::fs::create_dir_all(&agents_dir)?;

    let agent_file_path = agents_dir.join("k8s_ops_agent.md");
    let agent_file_content = r#"---
name: k8s_ops_agent
type: k8s_ops_agent
description: Kubernetes 运维智能体，可以查看和分析 k8s 集群状态
---
你是一个 Kubernetes 运维智能体。你可以查看和分析 k8s 集群状态，包括：
- 节点状态和资源使用情况
- 系统 Pod 的运行状态
- 集群整体健康状况
- 常见问题的诊断建议

请根据用户的问题，分析当前集群的运行状况并给出建议。
"#;
    std::fs::write(&agent_file_path, agent_file_content)?;

    println!("  ✓ Agent definition written to: {}", agent_file_path.display());
    println!();

    // Step 2: Load agent via AgentLoader
    let mut loader = AgentLoader::new_empty();
    loader.add_root(AgentScope::Repo, agents_dir.clone());
    loader.discover_all().await?;

    let def = loader
        .get("k8s_ops_agent")
        .await
        .expect("k8s_ops_agent should be loaded from examples/.agents/agents/");

    println!("  ✓ Agent loaded via AgentLoader:");
    println!("    - name: {}", def.name);
    println!("    - type: {}", def.r#type);
    println!("    - description: {}", def.description);
    println!();

    // Step 3: Build LokiPlugin
    let loki_config = LokiConfig::with_url(loki_url.clone());
    let loki_plugin = LokiPlugin::new(loki_config);

    let mut plugin_registry = PluginRegistry::new();
    plugin_registry.register(loki_plugin);

    println!("  ✓ LokiPlugin registered (Loki URL: {})", loki_url);
    println!();

    // Step 4: Build ReActAgent with loaded AgentDef
    let agent_config = AgentConfig::builder()
        .with_def((*def).clone())
        .with_llm(Arc::new(llm))
        .with_system_prompt(def.prompt.clone())
        .with_plugin_registry(plugin_registry)
        .build()?;

    let agent = ReActAgent::new(agent_config);

    println!("  ✓ ReActAgent built with AgentDef + LokiPlugin");
    println!();

    // Step 5: Run the agent with a k8s cluster status query
    let query = "请获取当前 k8s 集群的节点状态和系统 Pod 的运行状态，分析集群健康状况";

    println!("═══════════════════════════════════════════════════════════");
    println!("  Running Agent");
    println!("═══════════════════════════════════════════════════════════");
    println!("Query: {}", query);
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
            eprintln!("Agent run failed: {:?}", e);
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Observability");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Loki entries were sent to: {}", loki_url);
    println!("Loki labels:");
    println!("  - namespace: agent");
    println!("  - agent: k8s_ops_agent (from AgentDef.r#type)");
    println!("  - agent_id: k8s_ops_agent (from AgentDef.name)");
    println!();

    if loki_url == "http://localhost:3100" {
        println!("Note: LOKI_URL not set, using default http://localhost:3100");
        println!("If Loki is not running, HTTP errors will be logged via tracing but the agent still completes.");
    }
    println!();

    println!("═══════════════════════════════════════════════════════════");
    println!("  Example Complete");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Features demonstrated:");
    println!("  ✓ Agent definition loaded from .md file via AgentLoader");
    println!("  ✓ AgentDef used to configure ReActAgent identity");
    println!("  ✓ Real Anthropic API calls via DashScope");
    println!("  ✓ LokiPlugin registered for remote log shipping");
    println!("  ✓ Loki labels derived automatically from AgentDef");
    println!();

    Ok(())
}
