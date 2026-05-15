//! Load LLM provider from TOML config file and make a simple chat call.
//!
//! Usage:
//! ```bash
//! # Set up a provider config file:
//! mkdir -p .agents/providers
//! cat > .agents/providers/my-llm.toml << 'TOML'
//! provider = "anthropic"
//! model = "qwen3.5-plus"
//! api_key = "${ANTHROPIC_AUTH_TOKEN}"
//! base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"
//!
//! [body]
//! max_tokens = 1024
//! temperature = 0.7
//! TOML
//!
//! # Run the example:
//! ANTHROPIC_AUTH_TOKEN=your-key cargo run --example provider_toml_chat -p vol-llm-provider
//! ```

use vol_llm_core::ConversationRequest;
use vol_llm_provider::{LLMProviderRegistry, ProviderLoader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load providers from .agents/providers/ (project-level) and
    // ~/.agents/providers/ (user-level)
    let loader =
        ProviderLoader::load(std::env::current_dir().ok().as_deref());

    if loader.is_empty() {
        eprintln!("No providers found in .agents/providers/ or ~/.agents/providers/");
        eprintln!();
        eprintln!("Create a file like .agents/providers/my-llm.toml:");
        eprintln!();
        eprintln!("  provider = \"anthropic\"");
        eprintln!("  model = \"qwen3.5-plus\"");
        eprintln!("  api_key = \"${{ANTHROPIC_AUTH_TOKEN}}\"");
        eprintln!("  base_url = \"https://coding.dashscope.aliyuncs.com/apps/anthropic\"");
        eprintln!();
        eprintln!("  [body]");
        eprintln!("  max_tokens = 1024");
        eprintln!();
        return Ok(());
    }

    println!("Loaded {} provider(s): {:?}", loader.len(), loader.ids());

    // Build registry from loader
    let registry = LLMProviderRegistry::from_loader(&loader)?;

    // Pick the first provider
    let provider_id = *loader.ids().first().unwrap();
    let client = registry
        .get(provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    println!("Using provider: {} (model: {})", provider_id, client.model());

    // Build a simple conversation
    let request = ConversationRequest::with_system(
        "You are a friendly assistant.",
        "Say hello in one sentence.",
    );

    // Call the LLM
    println!("Sending request...");
    let response = client.converse(request).await?;

    println!("\nResponse:");
    println!("  Model: {}", response.model);
    println!("  Finish: {:?}", response.finish_reason);
    println!("  Tokens: {} in, {} out", response.usage.prompt_tokens, response.usage.completion_tokens);
    println!();

    let content = response.message.content.as_ref()
        .map(|c| c.as_str())
        .unwrap_or("(no content)");
    println!("{}", content);

    Ok(())
}
