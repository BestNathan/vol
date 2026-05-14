//! JSON-RPC serialization helpers and data types.
//!
//! Provides:
//! - `serialize_agent_event()` — maps `AgentStreamEvent` variants to (event_type, data) tuples
//! - `to_jsonrpc_event()` — wraps event in JSON-RPC subscription format
//! - `to_jsonrpc_response()` / `to_jsonrpc_error()` — response builders
//! - `parse_jsonrpc_request()` — parses incoming JSON-RPC text into `JsonRpcRequest`

use serde::Deserialize;
use vol_llm_agent::react::AgentStreamEvent;

// ---------------------------------------------------------------------------
// JSON-RPC request envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    jsonrpc: Option<String>,
    id: Option<u64>,
    method: Option<String>,
    params: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Parsed request enum
// ---------------------------------------------------------------------------

/// Parsed JSON-RPC request for the agent channel.
#[derive(Debug)]
pub enum JsonRpcRequest {
    AgentSubmit {
        id: u64,
        input: String,
        target: Option<String>,
    },
    AgentCancel {
        id: u64,
        req_id: String,
    },
    AgentSubscribe {
        id: u64,
    },
    AgentUnsubscribe {
        id: u64,
    },
    AgentApprove {
        id: u64,
        req_id: String,
        approved: bool,
        reason: Option<String>,
    },
    FileList {
        id: u64,
        path: String,
    },
    FileRead {
        id: u64,
        path: String,
    },
    LogList {
        id: u64,
    },
    LogRead {
        id: u64,
        run_id: String,
    },
    SessionList {
        id: u64,
    },
    SessionResume {
        id: u64,
        session_id: String,
    },
    AgentList {
        id: u64,
    },
    SessionEntries {
        id: u64,
        session_id: String,
    },
    McpListServers {
        id: u64,
    },
    McpListTools {
        id: u64,
        server: Option<String>,
    },
    McpCallTool {
        id: u64,
        server: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    McpListResources {
        id: u64,
        server: Option<String>,
    },
    McpListResourceTemplates {
        id: u64,
        server: Option<String>,
    },
    McpReadResource {
        id: u64,
        uri: String,
    },
    McpListPrompts {
        id: u64,
        server: Option<String>,
    },
    McpGetPrompt {
        id: u64,
        name: String,
        arguments: Option<std::collections::HashMap<String, serde_json::Value>>,
    },
    McpReconnect {
        id: u64,
        server: String,
    },
    McpServerStatus {
        id: u64,
    },
    /// Fallback for unknown/unrecognized methods.
    Unknown {
        id: Option<u64>,
        method: String,
    },
}

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

// ---------------------------------------------------------------------------
// parse_jsonrpc_request
// ---------------------------------------------------------------------------

/// Parse incoming JSON-RPC text into a `JsonRpcRequest`.
///
/// Returns `Err` for malformed JSON. Returns `Ok(JsonRpcRequest::Unknown)` for
/// unrecognized methods.
pub fn parse_jsonrpc_request(text: &str) -> Result<JsonRpcRequest, String> {
    let envelope: JsonRpcEnvelope =
        serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;

    if envelope.jsonrpc.as_deref() != Some("2.0") {
        return Err("missing or invalid jsonrpc field".into());
    }

    let id = envelope.id.ok_or_else(|| "missing id".to_string())?;
    let method = envelope
        .method
        .ok_or_else(|| "missing method".to_string())?;

    let params = envelope.params.unwrap_or(serde_json::Value::Null);

    match method.as_str() {
        "agent.submit" => {
            let input = params
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "agent.submit: missing 'input'".to_string())?
                .to_string();
            let target = params.get("target").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(JsonRpcRequest::AgentSubmit { id, input, target })
        }
        "agent.cancel" => {
            let req_id = params
                .get("req_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "agent.cancel: missing 'req_id'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::AgentCancel { id, req_id })
        }
        "agent.subscribe" => Ok(JsonRpcRequest::AgentSubscribe { id }),
        "agent.unsubscribe" => Ok(JsonRpcRequest::AgentUnsubscribe { id }),
        "agent.approve" => {
            let req_id = params
                .get("req_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "agent.approve: missing 'req_id'".to_string())?
                .to_string();
            let approved = params
                .get("approved")
                .and_then(|v| v.as_bool())
                .ok_or_else(|| "agent.approve: missing 'approved'".to_string())?;
            let reason = params
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(JsonRpcRequest::AgentApprove {
                id,
                req_id,
                approved,
                reason,
            })
        }
        "file.list" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "file.list: missing 'path'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::FileList { id, path })
        }
        "file.read" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "file.read: missing 'path'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::FileRead { id, path })
        }
        "log.list" => Ok(JsonRpcRequest::LogList { id }),
        "log.read" => {
            let run_id = params
                .get("run_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "log.read: missing 'run_id'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::LogRead { id, run_id })
        }
        "session.list" => Ok(JsonRpcRequest::SessionList { id }),
        "session.resume" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "session.resume: missing 'session_id'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::SessionResume { id, session_id })
        }
        "agent.list" => Ok(JsonRpcRequest::AgentList { id }),
        "session.entries" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "session.entries: missing 'session_id'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::SessionEntries { id, session_id })
        }
        "mcp.list_servers" => Ok(JsonRpcRequest::McpListServers { id }),
        "mcp.list_tools" => {
            let server = params
                .get("server")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(JsonRpcRequest::McpListTools { id, server })
        }
        "mcp.call_tool" => {
            let server = params
                .get("server")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "mcp.call_tool: missing 'server'".to_string())?
                .to_string();
            let tool_name = params
                .get("tool_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "mcp.call_tool: missing 'tool_name'".to_string())?
                .to_string();
            let arguments =
                params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
            Ok(JsonRpcRequest::McpCallTool {
                id,
                server,
                tool_name,
                arguments,
            })
        }
        "mcp.list_resources" => {
            let server = params
                .get("server")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(JsonRpcRequest::McpListResources { id, server })
        }
        "mcp.list_resource_templates" => {
            let server = params
                .get("server")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(JsonRpcRequest::McpListResourceTemplates { id, server })
        }
        "mcp.read_resource" => {
            let uri = params
                .get("uri")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "mcp.read_resource: missing 'uri'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::McpReadResource { id, uri })
        }
        "mcp.list_prompts" => {
            let server = params
                .get("server")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(JsonRpcRequest::McpListPrompts { id, server })
        }
        "mcp.get_prompt" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "mcp.get_prompt: missing 'name'".to_string())?
                .to_string();
            let arguments = params
                .get("arguments")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            Ok(JsonRpcRequest::McpGetPrompt {
                id,
                name,
                arguments,
            })
        }
        "mcp.reconnect" => {
            let server = params
                .get("server")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "mcp.reconnect: missing 'server'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::McpReconnect { id, server })
        }
        "mcp.server_status" => Ok(JsonRpcRequest::McpServerStatus { id }),
        _ => Ok(JsonRpcRequest::Unknown {
            id: Some(id),
            method,
        }),
    }
}

