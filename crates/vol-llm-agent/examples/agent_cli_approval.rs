//! Example: Agent with CLI-based human-in-the-loop approval.
//!
//! This example demonstrates how to use the HITL plugin with CLI approval channel.
//! Run with: cargo run --example agent_cli_approval
//!
//! The agent will prompt for approval before executing tool calls.

use vol_llm_agent::react::*;
use vol_llm_agent::react::hitl::*;
use vol_llm_agent::plugins::CliApprovalChannel;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("═══════════════════════════════════════════════════════════");
    println!("  ReAct Agent with Human-in-the-Loop Approval");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("This example demonstrates the HITL plugin system.");
    println!("The agent will request approval before executing tools.");
    println!();

    // HITL configuration: require approval for all tool executions
    let config = HitlConfig {
        triggers: vec![
            // Require approval before executing any tool
            ApprovalTrigger::ToolExecution { tools: None },
        ],
        timeout_secs: 60,
        on_timeout: TimeoutBehavior::Reject {
            reason: "Timeout waiting for approval".to_string(),
        },
        timeout_message: None,
    };

    // Create CLI approval channel
    let channel = Arc::new(CliApprovalChannel);

    // Create HITL plugin
    let _hitl_plugin = HitlPlugin::new(config, channel);

    // Note: To run this example with a real LLM, you would need to:
    // 1. Set up an LLM provider (e.g., Anthropic via DashScope)
    // 2. Create a tool (e.g., get_weather, get_btc_price)
    // 3. Build the agent with the plugin
    //
    // Example:
    // ```
    // let llm = Arc::new(AnthropicClient::new(api_key, model));
    // let tool = get_weather_tool();
    // let agent = ReActAgent::builder()
    //     .with_llm(llm)
    //     .with_tool(tool)
    //     .with_plugin(hitl_plugin)
    //     .build()?;
    //
    // let mut stream = agent.run("What is the weather in Beijing?", ToolContext::default()).await?;
    // while let Some(event) = stream.recv().await {
    //     match event {
    //         Ok(e) => println!("Event: {:?}", e),
    //         Err(e) => eprintln!("Error: {}", e),
    //     }
    // }
    // ```

    println!("HITL Plugin configured:");
    println!("  - Triggers: ToolExecution (all tools)");
    println!("  - Timeout: 60 seconds");
    println!("  - On timeout: Auto-reject");
    println!();
    println!("Plugin ID: {}", _hitl_plugin.id());
    println!("Plugin Priority: {}", _hitl_plugin.priority());
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  To use this example with a real LLM:");
    println!("  1. Set ANTHROPIC_AUTH_TOKEN environment variable");
    println!("  2. Uncomment the LLM setup code in the example");
    println!("  3. Run: cargo run --example agent_cli_approval");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
