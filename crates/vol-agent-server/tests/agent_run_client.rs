//! Integration test: connect to control-plane as a client, list agents,
//! submit an agent run, and verify the run completes.
//!
//! Requires a running control-plane (e.g. port-forward or NodePort).
//! Set `CONTROL_PLANE_URL` env var to override the default.

use serde_json::{json, Value};
use std::env;
use std::time::Duration;
use tokio::time;
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};

const DEFAULT_URL: &str = "ws://127.0.0.1:3001/ws";

#[tokio::test]
async fn agent_list_and_submit_run() {
    let url = env::var("CONTROL_PLANE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());

    let (ws, _) = connect_async(&url)
        .await
        .expect("failed to connect to control-plane");

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
    println!("selected agent: {}", agent_name);

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

    // agent.submit returns an ack with run_id
    let run_id = submit_resp["result"]["run_id"]
        .as_str()
        .expect("run_id missing from submit response");
    let accepted = submit_resp["result"]["accepted"].as_bool().unwrap_or(false);
    assert!(accepted, "run not accepted");
    println!("run submitted: {}", run_id);

    // ── 3. Wait for agent events (streaming output) ─────────────────────────

    println!("=== agent events ===");
    let mut event_count = 0;
    let mut found_completion = false;

    loop {
        let evt = time::timeout(Duration::from_secs(120), read.next())
            .await
            .expect("timeout waiting for agent events");

        match evt {
            Some(Ok(msg)) => {
                let text = msg.into_text().expect("not text");
                let evt_val: Value = serde_json::from_str(&text).expect("invalid JSON");
                event_count += 1;

                // agent.event contains streaming messages
                if let Some(method) = evt_val.get("method").and_then(|m| m.as_str()) {
                    println!("[{event_count}] method={method}");
                    if method == "agent.event" {
                        if let Some(params) = evt_val.get("params") {
                            if let Some(payload) = params.get("payload") {
                                println!("  {}", serde_json::to_string_pretty(payload).unwrap());
                                // Check for completion
                                if let Some(status) = payload.get("status").and_then(|s| s.as_str()) {
                                    if status == "completed" || status == "failed" {
                                        found_completion = true;
                                    }
                                }
                            }
                        }
                    }
                }

                // Also check for error responses
                if let Some(error) = evt_val.get("error") {
                    println!("  ERROR: {}", error);
                }

                if found_completion {
                    println!("agent run completed after {event_count} events");
                    break;
                }
            }
            Some(Err(e)) => {
                panic!("websocket error: {}", e);
            }
            None => {
                break;
            }
        }
    }

    assert!(found_completion, "agent run did not complete");

    // ── 4. Check agent status ──────────────────────────────────────────────

    let status_msg = json!({
        "jsonrpc": "2.0",
        "id": "status-1",
        "method": "agent.status",
        "params": { "run_id": run_id }
    })
    .to_string();

    write
        .send(tokio_tungstenite::tungstenite::Message::Text(status_msg))
        .await
        .expect("send agent.status");

    let resp = time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("timeout")
        .expect("closed")
        .expect("error")
        .into_text()
        .expect("not text");

    let status: Value = serde_json::from_str(&resp).expect("invalid JSON");
    println!("=== agent.status ===");
    println!("{}", serde_json::to_string_pretty(&status).unwrap());

    println!("\nAll checks passed ✅");
}
