//! Coding Agent - Write a CLI word count tool (fixed output path).

use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter, LocalSandbox};
use vol_llm_core::Sandbox;
use vol_llm_provider::{LLMConfig, LLMProviderConfig, LLMProviderRegistry, Secret};
use vol_llm_core::LLMProvider;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let work_dir = std::path::PathBuf::from("/tmp/wordcount-work");
    std::fs::create_dir_all(&work_dir)?;

    // Create a sample text file
    std::fs::write(
        work_dir.join("sample.txt"),
        "the quick brown fox jumps over the lazy dog\n\
         the fox is quick and the dog is lazy\n\
         brown bears are not as quick as foxes\n",
    )?;

    let report_path = work_dir.join("report.html");

    // Construct LLM externally
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");
    let llm_config = LLMProviderConfig {
        id: "anthropic-main".to_string(),
        config: LLMConfig {
            provider: LLMProvider::Anthropic,
            model: "qwen3.5-plus".to_string(),
            api_key: Secret::literal(api_key),
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
        },
    };
    let registry = LLMProviderRegistry::from_configs(&[llm_config])?;
    let llm = registry.get("anthropic-main").expect("LLM provider not found").clone();

    let config = CodingAgentConfig {
        max_iterations: 20,
        working_dir: work_dir.clone(),
        hitl_enabled: false,
        html_report_path: Some(report_path.clone()),
        llm: Some(llm),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        ..Default::default()
    };

    let sandbox = Arc::new(LocalSandbox::new(Some(work_dir.clone())));
    sandbox.start()?;

    let agent = CodingAgent::new(config).await?;

    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Write a Rust CLI word count tool".to_string(),
    ));

    let agent = agent.with_observer(observer).with_sandbox(sandbox.clone());

    let task = r#"Create a Rust CLI tool called "wordcount" that:
1. Takes a file path as a command-line argument
2. Reads the file and counts: total words, total lines, total characters
3. Prints the results in this exact format:
   Lines: <count>
   Words: <count>
   Chars: <count>
4. Use clap for CLI argument parsing
5. Create Cargo.toml and src/main.rs in the current directory

Use only standard library (no external crates except clap). Create the files using write_file tool, then build it using bash tool."#;

    println!("Task: {}", task);
    println!("Working directory: {}", work_dir.display());
    println!("Report path: {}", report_path.display());
    println!("---");

    let result = agent.run(task).await;

    sandbox.cleanup()?;

    let result = result?;

    println!("\n=== Task Complete ===");
    println!("Iterations: {}, Tool calls: {}", result.iterations, result.tool_calls);
    println!("Summary: {}", result.summary);
    println!("\nReport: {}", report_path.display());

    // Try to build and run the tool
    let cargo_toml = work_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        println!("\n--- Building the generated tool ---");
        let output = std::process::Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(&work_dir)
            .output()?;

        if output.status.success() {
            println!("Build successful!");
            println!("\n--- Running on sample.txt ---");
            let run_output = std::process::Command::new(
                work_dir.join("target/release/wordcount"),
            )
            .arg(work_dir.join("sample.txt"))
            .output()?;

            if run_output.status.success() {
                println!("{}", String::from_utf8_lossy(&run_output.stdout));
            } else {
                println!("Run failed: {}", String::from_utf8_lossy(&run_output.stderr));
            }
        } else {
            println!("Build failed:\n{}", String::from_utf8_lossy(&output.stderr));
        }
    }

    Ok(())
}
