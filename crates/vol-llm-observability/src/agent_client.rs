//! HTTP client + batch sender for agent-side event pushing.

use reqwest::Client;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use chrono::Utc;

use vol_llm_core::AgentStreamEvent;

/// Command for the batch sender task.
pub enum BatchCommand {
    Event(AgentStreamEvent),
}

/// Spawn a background task that batches events and POSTs them to the ingest service.
pub fn spawn_batch_sender(
    ingest_url: String,
    channel_capacity: usize,
    batch_size: usize,
    flush_interval_ms: u64,
    run_id: String,
    session_id: String,
    agent_id: String,
    agent_type: String,
) -> mpsc::Sender<BatchCommand> {
    let (tx, mut rx) = mpsc::channel::<BatchCommand>(channel_capacity);

    tokio::spawn(async move {
        let client = Client::new();
        let mut buffer: Vec<serde_json::Value> = Vec::with_capacity(batch_size);
        let mut flush_interval = interval(Duration::from_millis(flush_interval_ms));
        flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(BatchCommand::Event(event)) => {
                            if !should_log(&event) {
                                continue;
                            }
                            if let Some(value) = serialize_event(&event, &run_id, &session_id, &agent_id, &agent_type) {
                                buffer.push(value);
                                if buffer.len() >= batch_size {
                                    send_batch(&client, &ingest_url, std::mem::take(&mut buffer)).await;
                                }
                            }
                        }
                        None => {
                            if !buffer.is_empty() {
                                send_batch(&client, &ingest_url, std::mem::take(&mut buffer)).await;
                            }
                            break;
                        }
                    }
                }
                _ = flush_interval.tick() => {
                    if !buffer.is_empty() {
                        send_batch(&client, &ingest_url, std::mem::take(&mut buffer)).await;
                    }
                }
            }
        }
    });

    tx
}

/// Filter out delta events.
fn should_log(event: &AgentStreamEvent) -> bool {
    !matches!(
        event,
        AgentStreamEvent::ThinkingDelta { .. }
            | AgentStreamEvent::ContentDelta { .. }
            | AgentStreamEvent::ToolCallArgumentDelta { .. }
    )
}

/// Serialize an AgentStreamEvent into the ingest format.
fn serialize_event(
    event: &AgentStreamEvent,
    run_id: &str,
    session_id: &str,
    agent_id: &str,
    agent_type: &str,
) -> Option<serde_json::Value> {
    let (event_name, data) = match event {
        AgentStreamEvent::AgentStart { input, .. } => {
            ("AgentStart", serde_json::json!({ "input": input }))
        }
        AgentStreamEvent::AgentComplete { response, .. } => {
            ("AgentComplete", serde_json::json!({ "response": response }))
        }
        AgentStreamEvent::AgentAborted { reason, .. } => {
            ("AgentAborted", serde_json::json!({ "reason": reason }))
        }
        AgentStreamEvent::LLMCallStart { iteration, messages, .. } => {
            let last_n: Vec<_> = messages.iter().rev().take(5).rev().collect();
            let msgs: Vec<serde_json::Value> = last_n.iter().map(|m| {
                serde_json::json!({ "role": m.role, "content": m.content.as_ref().map(|c| c.as_str()).unwrap_or("") })
            }).collect();
            ("LLMCallStart", serde_json::json!({ "iteration": iteration, "message_count": messages.len(), "messages": msgs }))
        }
        AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
            ("LLMCallComplete", serde_json::json!({ "model": model, "usage": usage }))
        }
        AgentStreamEvent::LLMCallError { error, .. } => {
            ("LLMCallError", serde_json::json!({ "error": error }))
        }
        AgentStreamEvent::ThinkingStart { .. } => {
            ("ThinkingStart", serde_json::json!({}))
        }
        AgentStreamEvent::ThinkingComplete { thinking, .. } => {
            ("ThinkingComplete", serde_json::json!({ "thinking": thinking }))
        }
        AgentStreamEvent::ContentStart { .. } => {
            ("ContentStart", serde_json::json!({}))
        }
        AgentStreamEvent::ContentComplete { content, .. } => {
            ("ContentComplete", serde_json::json!({ "content": content }))
        }
        AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, arguments, .. } => {
            ("ToolCallBegin", serde_json::json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "arguments": arguments }))
        }
        AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } => {
            ("ToolCallComplete", serde_json::json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "result": result, "duration_ms": duration_ms }))
        }
        AgentStreamEvent::ToolCallError { tool_call_id, tool_name, error, duration_ms, .. } => {
            ("ToolCallError", serde_json::json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "error": error, "duration_ms": duration_ms }))
        }
        AgentStreamEvent::ToolCallSkipped { tool_call_id, tool_name, reason, duration_ms, .. } => {
            ("ToolCallSkipped", serde_json::json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "reason": reason, "duration_ms": duration_ms }))
        }
        AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer, .. } => {
            let tc: Vec<serde_json::Value> = tool_calls.iter().map(|tc| {
                serde_json::json!({ "id": &tc.id, "name": &tc.name, "arguments": &tc.arguments, "type": &tc.r#type })
            }).collect();
            ("IterationComplete", serde_json::json!({ "iteration": iteration, "tool_calls": tc, "final_answer": final_answer }))
        }
        AgentStreamEvent::PluginEvent { name, data, .. } => {
            let mut map = serde_json::Map::new();
            map.insert("name".to_string(), serde_json::Value::String(name.clone()));
            for (k, v) in data {
                map.insert(k.clone(), v.clone());
            }
            ("PluginEvent", serde_json::Value::Object(map))
        }
        AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
            ("MaxIterationsReached", serde_json::json!({ "current_iteration": current_iteration, "max_iterations": max_iterations }))
        }
        AgentStreamEvent::IterationContinued { from_iteration, .. } => {
            ("IterationContinued", serde_json::json!({ "from_iteration": from_iteration }))
        }
        AgentStreamEvent::ThinkingDelta { .. }
        | AgentStreamEvent::ContentDelta { .. }
        | AgentStreamEvent::ToolCallArgumentDelta { .. } => {
            return None;
        }
    };

    Some(serde_json::json!({
        "run_id": run_id,
        "session_id": session_id,
        "agent_id": agent_id,
        "agent_type": agent_type,
        "timestamp": Utc::now().to_rfc3339(),
        "event": event_name,
        "data": data,
    }))
}

/// Send a batch of events to the ingest service.
async fn send_batch(client: &Client, url: &str, events: Vec<serde_json::Value>) {
    let body = serde_json::json!({ "events": events });

    match client.post(url).json(&body).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                tracing::error!(
                    status = %status,
                    body = %body_text,
                    "Observability push failed"
                );
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to send events to observability service");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_log_filters_delta() {
        assert!(!should_log(&AgentStreamEvent::ThinkingDelta {
            timestamp: Utc::now(),
            delta: "chunk".to_string(),
        }));
        assert!(!should_log(&AgentStreamEvent::ContentDelta {
            timestamp: Utc::now(),
            delta: "partial".to_string(),
        }));
        assert!(!should_log(&AgentStreamEvent::ToolCallArgumentDelta {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            delta: "arg".to_string(),
        }));
        assert!(should_log(&AgentStreamEvent::ThinkingStart {
            timestamp: Utc::now(),
        }));
        assert!(should_log(&AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        }));
    }
}
