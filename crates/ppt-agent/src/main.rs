//! ppt-agent: AI-powered PowerPoint generation CLI.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use vol_llm_agents::ppt::{PptAgent, PptAgentConfig, PptInput};
use vol_llm_agents::ppt::template::TemplateRegistry;

#[derive(Parser)]
#[command(name = "ppt-agent")]
#[command(about = "AI-powered PowerPoint generation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a PowerPoint presentation
    Generate {
        /// Text description of the presentation
        #[arg(short = 't', long)]
        text: String,

        /// Optional context (audience, purpose, etc.)
        #[arg(short, long)]
        context: Option<String>,

        /// Template ID to use
        #[arg(short = 'T', long)]
        template: Option<String>,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Verbose output
        #[arg(short, long, default_value = "false")]
        verbose: bool,
    },

    /// List available templates
    Templates {
        #[command(subcommand)]
        action: TemplatesAction,
    },
}

#[derive(Subcommand)]
enum TemplatesAction {
    /// List all available templates
    List,

    /// Preview a template
    Preview {
        /// Template ID
        template_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { text, context, template, output, verbose } => {
            // Initialize tracing
            if verbose {
                tracing_subscriber::fmt()
                    .with_max_level(tracing::Level::DEBUG)
                    .init();
            }

            println!("═══════════════════════════════════════════════════════════");
            println!("  PPT Agent - AI-powered Presentation Generation");
            println!("═══════════════════════════════════════════════════════════");
            println!();

            // Create config
            let mut config = PptAgentConfig::default()
                .with_llm_provider("anthropic-main")
                .with_verbose(verbose);

            // Set template dir to bundled templates
            let template_dir = PathBuf::from("crates/vol-llm-agents/src/ppt/templates");
            if template_dir.exists() {
                config = config.with_template_dir(&template_dir);
                if verbose {
                    println!("Template directory: {:?}", template_dir);
                }
            }

            // Set output dir if specified
            if let Some(output_path) = &output {
                if let Some(parent) = output_path.parent() {
                    config = config.with_default_output_dir(parent);
                }
            }

            // Create agent
            println!("Initializing PPT Agent...");
            let agent = PptAgent::new(config).await?;

            // Build input
            let input = match &context {
                Some(ctx) => {
                    println!("Topic: {}", text);
                    println!("Context: {}", ctx);
                    PptInput::text_with_context(&text, ctx)
                },
                None => {
                    println!("Topic: {}", text);
                    PptInput::text(&text)
                }
            };

            // Override template in input if specified via CLI
            let input_with_template = match (&template, input) {
                (Some(tpl_id), PptInput::Text { description, context }) => {
                    PptInput::Text {
                        description,
                        context: Some(match context {
                            Some(ctx) => format!("{}. Use template: {}", ctx, tpl_id),
                            None => format!("Use template: {}", tpl_id),
                        }),
                    }
                },
                (None, input) => input,
            };

            if let Some(template_id) = &template {
                println!("Using template: {}", template_id);
            }

            println!();
            println!("Generating presentation...");
            println!();

            // Generate PPT
            let result = agent.generate(input_with_template).await?;

            // Show results
            println!("═══════════════════════════════════════════════════════════");
            println!("  Generation Complete");
            println!("═══════════════════════════════════════════════════════════");
            println!();
            println!("  Output file: {:?}", result.output_path);
            println!("  Slide count: {}", result.slide_count);
            println!("  Template:    {}", result.template_id);
            println!();
            println!("Slides:");
            for (i, slide) in result.slides.iter().enumerate() {
                println!("  {}. {} ({:?})", i + 1, slide.title, slide.layout);
            }
        }

        Commands::Templates { action } => {
            match action {
                TemplatesAction::List => {
                    println!("═══════════════════════════════════════════════════════════");
                    println!("  Available Templates");
                    println!("═══════════════════════════════════════════════════════════");
                    println!();

                    // Load templates from bundled location
                    let template_dir = PathBuf::from("crates/vol-llm-agents/src/ppt/templates");
                    let mut registry = TemplateRegistry::new();

                    if template_dir.exists() {
                        if let Err(e) = registry.load_from_dir(&template_dir) {
                            eprintln!("Failed to load templates: {}", e);
                        }
                    }

                    let templates = registry.list_templates();
                    if templates.is_empty() {
                        println!("No templates found in {:?}", template_dir);
                    } else {
                        for t in templates {
                            println!("  {} - {}", t.id, t.name);
                            println!("    {}", t.description);
                            println!("    Tags: occasion={:?}, style={:?}", t.tags.occasion, t.tags.style);
                            println!();
                        }
                    }
                }
                TemplatesAction::Preview { template_id } => {
                    println!("Preview for template: {}", template_id);
                    // TODO: Implement preview
                    println!("(Preview not yet implemented)");
                }
            }
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Complete");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
