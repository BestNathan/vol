//! Upload PPT file to Feishu
//!
//! Usage: cargo run --bin upload-ppt -- <pptx-path>

use std::env;
use std::path::Path;
use std::fs::File;
use std::io::Read;
use reqwest::multipart::{Form, Part};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin upload-ppt -- <pptx-path>");
        std::process::exit(1);
    }

    let pptx_path = &args[1];

    // Check file exists
    if !Path::new(pptx_path).exists() {
        eprintln!("Error: File not found: {}", pptx_path);
        std::process::exit(1);
    }

    // Get credentials from environment
    let app_id = env::var("FEISHU_APP_ID")
        .expect("FEISHU_APP_ID must be set");
    let app_secret = env::var("FEISHU_APP_SECRET")
        .expect("FEISHU_APP_SECRET must be set");
    let receive_id = env::var("FEISHU_RECEIVE_ID")
        .expect("FEISHU_RECEIVE_ID must be set");

    println!("═══════════════════════════════════════════════════════════");
    println!("  Uploading PPT to Feishu");
    println!("═══════════════════════════════════════════════════════════");
    println!("  File: {}", pptx_path);
    println!("  To:   {}", receive_id);

    // Read the PPTX file
    let mut file = File::open(pptx_path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let file_size = buffer.len();
    let file_name = Path::new(pptx_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    println!("  Size: {} bytes ({:.1} KB)", file_size, file_size as f64 / 1024.0);
    println!();

    // Create HTTP client
    let client = reqwest::Client::new();

    // Step 1: Get tenant_access_token
    println!("Getting access token...");
    let token_url = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";
    let token_resp = client.post(token_url)
        .json(&json!({
            "app_id": app_id,
            "app_secret": app_secret
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to get token: {}", e))?;

    let token_json: serde_json::Value = token_resp.json().await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    if token_json.get("code").and_then(|v| v.as_i64()) != Some(0) {
        eprintln!("  ✗ Failed to get token: {:?}", token_json);
        return Err(format!("Token request failed: {:?}", token_json).into());
    }

    let tenant_token = token_json.get("tenant_access_token")
        .and_then(|v| v.as_str())
        .ok_or("No tenant_access_token in response")?;

    println!("  ✓ Token obtained");
    println!();

    // Step 2: Upload file to Feishu
    println!("Uploading file...");
    let upload_url = "https://open.feishu.cn/open-apis/im/v1/files";

    let file_part = Part::bytes(buffer)
        .file_name(file_name.clone())
        .mime_str("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        .unwrap();

    let form = Form::new()
        .part("file", file_part)
        .text("file_type", "stream");

    let upload_resp = client.post(upload_url)
        .header("Authorization", format!("Bearer {}", tenant_token))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to upload file: {}", e))?;

    let upload_json: serde_json::Value = upload_resp.json().await
        .map_err(|e| format!("Failed to parse upload response: {}", e))?;

    if upload_json.get("code").and_then(|v| v.as_i64()) != Some(0) {
        eprintln!("  ✗ Upload failed: {:?}", upload_json);
        return Err(format!("Upload failed: {:?}", upload_json).into());
    }

    let file_key = upload_json.get("data")
        .and_then(|d| d.get("file_key"))
        .and_then(|fk| fk.as_str())
        .ok_or("No file_key in upload response")?;

    println!("  ✓ File uploaded, file_key: {}", file_key);
    println!();

    // Step 3: Send file message
    println!("Sending file message...");

    // Determine receive_id_type based on prefix
    let receive_id_type = if receive_id.starts_with("oc_") {
        "chat_id"
    } else if receive_id.starts_with("ou_") {
        "open_id"
    } else {
        "chat_id"
    };

    // Build file message content
    let file_content = json!({
        "file_key": file_key,
        "file_type": "application/vnd.openxmlformats-officedocument.presentationml.presentation"
    });

    // Send message with receive_id_type as query parameter
    let msg_url = format!("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type={}", receive_id_type);

    let msg_resp = client.post(msg_url)
        .header("Authorization", format!("Bearer {}", tenant_token))
        .json(&json!({
            "receive_id": receive_id,
            "msg_type": "file",
            "content": file_content.to_string()
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    let msg_json: serde_json::Value = msg_resp.json().await
        .map_err(|e| format!("Failed to parse message response: {}", e))?;

    if msg_json.get("code").and_then(|v| v.as_i64()) != Some(0) {
        eprintln!("  ✗ Send message failed: {:?}", msg_json);
        return Err(format!("Send message failed: {:?}", msg_json).into());
    }

    let msg_id = msg_json.get("data")
        .and_then(|d| d.get("message_id"))
        .and_then(|mid| mid.as_str())
        .unwrap_or("unknown");

    println!("  ✓ File message sent successfully!");
    println!("    Message ID: {}", msg_id);

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Complete");
    println!("═══════════════════════════════════════════════════════════");
    Ok(())
}
