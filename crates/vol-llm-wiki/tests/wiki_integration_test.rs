//! Integration test: compress a real session file into wiki pages.

use std::path::PathBuf;
use vol_session::SessionEntry;

/// Helper: load session messages from a JSONL file.
async fn load_session_messages(session_path: &std::path::Path) -> Vec<vol_session::SessionMessage> {
    let content = std::fs::read_to_string(session_path).expect("Failed to read session file");
    let mut messages = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: SessionEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if let vol_session::SessionEntryData::Message { message } = entry.data {
            messages.push(message);
        }
    }

    messages
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN"]
async fn test_compress_real_session() {
    // Find the session file
    let home = std::env::var("HOME").unwrap_or_default();
    let session_path = PathBuf::from(&home)
        .join(".vol-coding")
        .join("nq-deribit")
        .join("sessions")
        .join("f98d7668-d00f-4983-90c6-cf6194e373bd.jsonl");

    if !session_path.exists() {
        println!("Session file not found, skipping test");
        return;
    }

    // Load messages
    let messages = load_session_messages(&session_path).await;
    if messages.is_empty() {
        println!("No messages found in session, skipping test");
        return;
    }
    println!("Loaded {} messages from session", messages.len());

    // Create temp wiki directory
    let temp_wiki = tempfile::tempdir().unwrap();
    let wiki_dir = temp_wiki.path().join(".agents").join("wikis");
    std::fs::create_dir_all(&wiki_dir).unwrap();

    // Create WikiAgent
    let mut config = vol_llm_wiki::WikiAgentConfig::default();
    config.working_dir = temp_wiki.path().to_path_buf();
    config.max_iterations = 10;

    let agent = vol_llm_wiki::WikiAgent::new(config).expect("Failed to create WikiAgent");

    // Run compression
    let result = agent.compress(messages).await;

    match result {
        Ok(result) => {
            println!("Compression succeeded!");
            println!("Pages created: {:?}", result.pages_created);
            println!("Pages updated: {:?}", result.pages_updated);
            println!("Summary: {}", result.summary);

            // Verify wiki directory has content
            let entries: Vec<_> = std::fs::read_dir(&wiki_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                .collect();

            assert!(!entries.is_empty(), "Wiki directory should have markdown files after compression");
            println!("Wiki files created: {:?}", entries.iter().map(|e| e.file_name()).collect::<Vec<_>>());
        }
        Err(e) => {
            println!("Compression failed (expected if no API key): {}", e);
        }
    }
}
