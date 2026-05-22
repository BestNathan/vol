//! JSON-RPC serialization helpers and data types.
//!
//! Provides:
//! - `serialize_agent_event()` — maps `AgentStreamEvent` variants to (event_type, data) tuples
//! - `to_jsonrpc_event()` — wraps event in JSON-RPC subscription format
//! - `to_jsonrpc_response()` / `to_jsonrpc_error()` — response builders

use vol_llm_agent::react::AgentStreamEvent;

// ---------------------------------------------------------------------------
// serialize_agent_event  (mirrors handler.rs lines 203-269 exactly)
// ---------------------------------------------------------------------------

/// Map an `AgentStreamEvent` to a `(event_type, data)` tuple suitable for
/// wire serialization.  The event_type strings and JSON payloads must stay
/// in sync with the existing `AgentStreamEvent` variants.
pub fn serialize_agent_event(event: &AgentStreamEvent) -> (String, serde_json::Value) {
    match event {
        AgentStreamEvent::AgentStart { input, .. } => (
            "agent_start".into(),
            serde_json::json!({ "input": input }),
        ),
        AgentStreamEvent::AgentComplete { response, .. } => (
            "agent_complete".into(),
            serde_json::json!({ "response": response }),
        ),
        AgentStreamEvent::AgentAborted { reason, .. } => (
            "agent_aborted".into(),
            serde_json::json!({ "reason": reason }),
        ),
        AgentStreamEvent::ThinkingStart { .. } => (
            "thinking_start".into(),
            serde_json::json!({}),
        ),
        AgentStreamEvent::ThinkingDelta { delta, .. } => (
            "thinking_delta".into(),
            serde_json::json!({ "delta": delta }),
        ),
        AgentStreamEvent::ThinkingComplete { thinking, .. } => (
            "thinking_complete".into(),
            serde_json::json!({ "thinking": thinking }),
        ),
        AgentStreamEvent::ContentStart { .. } => (
            "content_start".into(),
            serde_json::json!({}),
        ),
        AgentStreamEvent::ContentDelta { delta, .. } => (
            "content_delta".into(),
            serde_json::json!({ "delta": delta }),
        ),
        AgentStreamEvent::ContentComplete { content, .. } => (
            "content_complete".into(),
            serde_json::json!({ "content": content }),
        ),
        AgentStreamEvent::ToolCallBegin {
            tool_call_id,
            tool_name,
            arguments,
            ..
        } => (
            "tool_call_begin".into(),
            serde_json::json!({
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "arguments": arguments,
            }),
        ),
        AgentStreamEvent::ToolCallArgumentDelta {
            tool_call_id,
            tool_name,
            delta,
            ..
        } => (
            "tool_call_argument_delta".into(),
            serde_json::json!({
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "delta": delta,
            }),
        ),
        AgentStreamEvent::ToolCallComplete {
            tool_call_id,
            tool_name,
            result,
            duration_ms,
            ..
        } => (
            "tool_call_complete".into(),
            serde_json::json!({
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "result": result,
                "duration_ms": duration_ms,
            }),
        ),
        AgentStreamEvent::ToolCallError {
            tool_call_id,
            tool_name,
            error,
            duration_ms,
            ..
        } => (
            "tool_call_error".into(),
            serde_json::json!({
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "error": error,
                "duration_ms": duration_ms,
            }),
        ),
        AgentStreamEvent::ToolCallSkipped {
            tool_call_id,
            tool_name,
            reason,
            duration_ms,
            ..
        } => (
            "tool_call_skipped".into(),
            serde_json::json!({
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "reason": reason,
                "duration_ms": duration_ms,
            }),
        ),
        AgentStreamEvent::MaxIterationsReached {
            current_iteration,
            max_iterations,
            ..
        } => (
            "max_iterations_reached".into(),
            serde_json::json!({
                "current": current_iteration,
                "max": max_iterations,
            }),
        ),
        AgentStreamEvent::IterationContinued {
            from_iteration, ..
        } => (
            "iteration_continued".into(),
            serde_json::json!({ "from_iteration": from_iteration }),
        ),
        AgentStreamEvent::IterationComplete {
            iteration,
            tool_calls: _,
            final_answer,
            ..
        } => (
            "iteration_complete".into(),
            serde_json::json!({
                "iteration": iteration,
                "final_answer": final_answer,
            }),
        ),
        AgentStreamEvent::LLMCallStart { iteration, .. } => (
            "llm_call_start".into(),
            serde_json::json!({ "iteration": iteration }),
        ),
        AgentStreamEvent::LLMCallComplete { model, usage, .. } => (
            "llm_call_complete".into(),
            serde_json::json!({ "model": model, "usage": usage }),
        ),
        AgentStreamEvent::LLMCallError { error, .. } => (
            "llm_call_error".into(),
            serde_json::json!({ "error": error }),
        ),
        AgentStreamEvent::PluginEvent { name: _, data, .. } => (
            "plugin_event".into(),
            serde_json::Value::Object(data.clone()),
        ),
    }
}

// ---------------------------------------------------------------------------
// Response / event builders
// ---------------------------------------------------------------------------

/// Wrap an agent event in a JSON-RPC subscription notification.
///
/// Produces a string like:
/// ```json
/// {"jsonrpc":"2.0","method":"agent.event","params":{"subscription":1,"result":{"req_id":"...","event_type":"...","data":{...}}}}
/// ```
pub fn to_jsonrpc_event(
    event: &AgentStreamEvent,
    sub_id: u64,
    req_id: &str,
) -> String {
    let (event_type, data) = serialize_agent_event(event);
    let envelope = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "agent.event",
        "params": {
            "subscription": sub_id,
            "result": {
                "req_id": req_id,
                "event_type": event_type,
                "data": data,
            },
        },
    });
    match serde_json::to_string(&envelope) {
        Ok(text) => text,
        Err(e) => {
            tracing::error!(%e, "failed to serialize JSON-RPC event envelope");
            "{}".into()
        }
    }
}

/// Build a JSON-RPC success response.
///
/// ```json
/// {"jsonrpc":"2.0","id":1,"result":{...}}
/// ```
pub fn to_jsonrpc_response(id: u64, result: serde_json::Value) -> String {
    let envelope = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".into())
}

/// Build a JSON-RPC error response.
///
/// ```json
/// {"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"..."}}
/// ```
pub fn to_jsonrpc_error(id: Option<u64>, code: i32, message: String) -> String {
    let envelope = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        },
    });
    serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".into())
}
