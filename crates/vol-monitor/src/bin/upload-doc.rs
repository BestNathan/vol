//! Upload design document to Feishu Drive.
//!
//! Usage: cargo run --bin upload-doc

use vol_feishu::FeishuClient;
use std::fs;
use tracing_subscriber::{self, EnvFilter};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    // Feishu credentials from config.toml
    let app_id = "cli_a936b13197385bde";
    let app_secret = "JnWnFrrOvzHi4deDmFY9kd1NMGbiWuNz";

    // Read the design document
    let doc_path = "docs/superpowers/plans/2026-04-01-channel-monitor-architecture.md";
    let content = fs::read_to_string(doc_path)
        .map_err(|e| format!("Failed to read {}: {}", doc_path, e))?;

    info!("Read design document: {} bytes", content.len());

    // Create Feishu client
    let client = FeishuClient::new(app_id.to_string(), app_secret.to_string());

    // Get or create deribit folder
    let folder_name = "deribit";
    info!("Getting or creating folder: {}", folder_name);
    let folder_token = client.get_or_create_folder(folder_name).await?;
    info!("Folder token: {}", folder_token);

    // Upload document
    let doc_title = "Channel-Based Monitoring Architecture";
    info!("Uploading document: {}", doc_title);
    let doc_token = client.upload_markdown_doc(doc_title, &content, &folder_token).await?;
    info!("Document uploaded successfully!");

    // Generate document URL
    let doc_url = format!("https://feishu.cn/docx/{}", doc_token);
    println!("\n===========================================");
    println!("Document uploaded successfully!");
    println!("Folder: {}", folder_name);
    println!("Document: {}", doc_title);
    println!("URL: {}", doc_url);
    println!("===========================================");

    Ok(())
}
