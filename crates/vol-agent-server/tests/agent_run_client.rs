//! Integration test: connect to control-plane as a client, list agents,
//! submit an agent run, and verify the run completes.
//!
//! Requires a running control-plane (e.g. port-forward or NodePort).
//! Set `CONTROL_PLANE_URL` env var to override the default.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::env;
use std::time::Duration;
use tokio::time;
use tokio_tungstenite::connect_async;

const DEFAULT_URL: &str = "ws://127.0.0.1:3001/ws";

#[tokio::test]
#[ignore = "e2e: requires running control-plane server"]
async fn agent_list_and_submit_run() {
    let url = env::var("CONTROL_PLANE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());

    let (ws, _) = match connect_async(&url).await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!(
                "SKIP (e2e): control-plane unreachable at {url} ({e}) — \
                 start a control-plane server first"
            );
            return;
        }
    };

    let (mut write, mut read) = ws.split();

    // ── 1. List agents ────────────────────────────────────────────────────

    let list_msg = json!({
        "jsonrpc": "2.0",
        "id": "list-1",
        "method": "agent.list",
        "params": {}
    })
    .to_string();

    write
        .send(tokio_tungstenite::tungstenite::Message::Text(list_msg))
        .await
        .expect("send agent.list");

    let resp = time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("timeout waiting for agent.list response")
        .expect("websocket closed")
        .expect("websocket error")
        .into_text()
        .expect("not text");

    let list: Value = serde_json::from_str(&resp).expect("invalid JSON");
    println!("=== agent.list ===");
    println!("{}", serde_json::to_string_pretty(&list).unwrap());

    let agents = list["result"]["agents"]
        .as_array()
        .expect("agents array missing");
    assert!(!agents.is_empty(), "no agents registered");

    // Pick the first agent
    let first_agent = &agents[0];
    let agent_name = first_agent["name"].as_str().unwrap_or("unknown");
    println!("selected agent: {agent_name}");

    // ── 2. Submit agent run ────────────────────────────────────────────────

    let submit_msg = json!({
        "jsonrpc": "2.0",
        "id": "submit-1",
        "method": "agent.submit",
        "params": {
            "input": {
                "parts": [
                    {"type": "text", "text": "Say hello and confirm you are working properly."}
                ]
            },
            "target": agent_name
        }
    })
    .to_string();

    write
        .send(tokio_tungstenite::tungstenite::Message::Text(submit_msg))
        .await
        .expect("send agent.submit");

    let resp = time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("timeout waiting for agent.submit response")
        .expect("websocket closed")
        .expect("websocket error")
        .into_text()
        .expect("not text");

    let submit_resp: Value = serde_json::from_str(&resp).expect("invalid JSON");
    println!("=== agent.submit ===");
    println!("{}", serde_json::to_string_pretty(&submit_resp).unwrap());

    // agent.submit now returns a single SubmitResult:
    // { run_id, accepted, provider: {name, model}, tools, mcps, skills }
    let result = &submit_resp["result"];
    let run_id = result["run_id"]
        .as_str()
        .expect("run_id missing from submit response");
    let accepted = result["accepted"].as_bool().unwrap_or(false);
    assert!(accepted, "run not accepted");

    let provider = result["provider"].as_object().expect("provider missing");
    assert!(provider["name"].as_str().is_some(), "provider.name missing");
    assert!(
        provider["model"].as_str().is_some(),
        "provider.model missing"
    );

    assert!(
        result["tools"].is_array(),
        "tools array missing from submit response"
    );
    assert!(
        result["mcps"].is_array(),
        "mcps array missing from submit response"
    );
    assert!(
        result["skills"].is_array(),
        "skills array missing from submit response"
    );
    println!(
        "run submitted: {run_id} (provider {} / {}, {} tools, {} mcps, {} skills)",
        provider["name"].as_str().unwrap_or("?"),
        provider["model"].as_str().unwrap_or("?"),
        result["tools"].as_array().map(|a| a.len()).unwrap_or(0),
        result["mcps"].as_array().map(|a| a.len()).unwrap_or(0),
        result["skills"].as_array().map(|a| a.len()).unwrap_or(0),
    );

    // ── 3. Wait for agent events, stop on AgentComplete ────────────────────

    println!("=== agent events ===");
    let mut event_count = 0;
    let mut completed = false;

    loop {
        let evt = time::timeout(Duration::from_secs(60), read.next()).await;

        match evt {
            Ok(Some(Ok(msg))) => {
                let text = msg.into_text().expect("not text");
                let evt_val: Value = serde_json::from_str(&text).expect("invalid JSON");
                event_count += 1;

                if let Some(method) = evt_val.get("method").and_then(|m| m.as_str()) {
                    println!("[{event_count}] method={method}");
                    // Print full message for debugging
                    println!("  {}", serde_json::to_string_pretty(&evt_val).unwrap());

                    if method == "agent.event" {
                        // Check for AgentComplete inside the params
                        let event_text = serde_json::to_string(&evt_val).unwrap();
                        if event_text.contains("AgentComplete") {
                            println!("  >>> AgentComplete — run finished!");
                            completed = true;
                            break;
                        }
                    }
                }
                if let Some(error) = evt_val.get("error") {
                    println!("  ERROR: {error}");
                }
            }
            _ => {
                println!("  (no more events after {event_count})");
                break;
            }
        }
    }

    assert!(
        completed,
        "agent run did not complete (got {event_count} events)"
    );
    assert!(
        event_count >= 3,
        "expected at least 3 events, got {event_count}"
    );

    // Graceful close
    let _ = write
        .send(tokio_tungstenite::tungstenite::Message::Close(None))
        .await;

    println!("All checks passed ✅");
}
