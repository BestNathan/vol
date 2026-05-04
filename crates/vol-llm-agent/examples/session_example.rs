//! Session and SessionEntryStore example.
//!
//! Demonstrates how to use Session with ReActAgent.

use std::sync::Arc;
use vol_session::{InMemoryEntryStore, Session, SessionMessage};
use vol_llm_core::Message;

#[tokio::main]
async fn main() {
    println!("=== Session and SessionEntryStore Example ===\n");

    // 1. Create entry store
    let entry_store = Arc::new(InMemoryEntryStore::new());
    println!("1. Created InMemoryEntryStore");

    // 2. Create session (auto-generates UUID)
    let session = Arc::new(Session::new(entry_store.clone()));
    println!("2. Created Session: {}", session.id);

    // 3. Add messages to session
    let user_msg = SessionMessage::new(session.id.clone(), Message::user("What is the BTC price?"));
    session.add_message(user_msg).await.unwrap();
    println!("3. Added user message to session");

    let assistant_msg = SessionMessage::new(
        session.id.clone(),
        Message::assistant("The BTC price is $69,000."),
    );
    session.add_message(assistant_msg).await.unwrap();
    println!("4. Added assistant message to session");

    // 5. Retrieve messages from session
    let messages = session.get_messages().await.unwrap();
    println!("5. Retrieved {} messages from session", messages.len());

    for (i, msg) in messages.iter().enumerate() {
        let content_str = match &msg.message.content {
            Some(vol_llm_core::MessageContent::Text(s)) => s.as_str(),
            _ => "",
        };
        println!(
            "   Message {}: role={:?}, content={:?}",
            i + 1,
            msg.message.role,
            content_str
        );
    }

    // 6. Clone session for branching
    let child_session = session.clone();
    println!("6. Cloned session for branching: {}", child_session.id);

    // 7. Create new session from builder pattern (simulated)
    println!("\n7. AgentConfig.builder() can be used to create agent with session:");
    println!("   let config = AgentConfig::builder()");
    println!("       .with_llm(llm)");
    println!("       .with_tool(tool)");
    println!("       .with_session(session)");
    println!("       .build()?;");
    println!("   let agent = ReActAgent::new(config);");

    // 8. Demonstrate max_history_messages configuration
    println!("\n8. Configure conversation history limit:");
    println!("   Default history limit: 20 messages");
    println!("\n9. Custom history limit via builder:");
    println!("   let config = AgentConfig::builder()");
    println!("       .with_llm(llm)");
    println!("       .with_max_history_messages(50)  // Load up to 50 history messages");
    println!("       .build()?;");
    println!("   let agent = ReActAgent::new(config);");

    println!("\n=== Example Complete ===");
    println!("Session and SessionEntryStore are ready for use with ReActAgent!");
}
