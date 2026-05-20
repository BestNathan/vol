//! Integration tests for JSON-RPC serialization.
//!
//! Tests cover:
//! 1. JSON-RPC event format structure (frontend-expected format)
//! 2. All AgentStreamEvent variants serialize correctly
//! 3. Error responses

use serde_json::Value;
use vol_llm_agent::react::AgentStreamEvent;
use vol_llm_agent_channel::jsonrpc::serde_helpers::*;
use vol_llm_core::conversation::TokenUsage;

// ---------------------------------------------------------------------------
// Test 1: JSON-RPC event format structure
// ---------------------------------------------------------------------------

#[test]
fn test_jsonrpc_event_format_structure() {
    let event = AgentStreamEvent::agent_start("hello".to_string());
    let json = to_jsonrpc_event(&event, 1, "req-abc-123");

    let parsed: Value = serde_json::from_str(&json).expect("should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0", "jsonrpc field must be \"2.0\"");
    assert_eq!(
        parsed["method"], "agent.event",
        "method field must be \"agent.event\""
    );

    let params = &parsed["params"];
    assert!(
        params.get("subscription").is_some(),
        "params must have \"subscription\" (not \"sub_id\")"
    );
    assert_eq!(
        params["subscription"], 1,
        "subscription must equal the sub_id"
    );

    let result = &params["result"];
    assert_eq!(
        result["req_id"], "req-abc-123",
        "result must have req_id"
    );
    assert_eq!(
        result["event_type"], "agent_start",
        "result must have event_type"
    );
    assert!(result.get("data").is_some(), "result must have data");
    assert_eq!(result["data"]["input"], "hello", "data.input must match");
}

// ---------------------------------------------------------------------------
// Test 2: All AgentStreamEvent variants serialize correctly
// ---------------------------------------------------------------------------

#[test]
fn test_serialize_agent_start() {
    let event = AgentStreamEvent::agent_start("hello".to_string());
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "agent_start");
    assert_eq!(data["input"], "hello");
}

#[test]
fn test_serialize_agent_complete() {
    let event = AgentStreamEvent::agent_complete();
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "agent_complete");
    assert!(data["response"].is_null(), "response should be null when None");
}

#[test]
fn test_serialize_agent_complete_with_response() {
    let event = AgentStreamEvent::agent_complete_with_response(
        serde_json::json!({ "answer": "42" }),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "agent_complete");
    assert_eq!(data["response"]["answer"], "42");
}

#[test]
fn test_serialize_agent_aborted() {
    let event = AgentStreamEvent::agent_aborted("user cancelled".to_string());
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "agent_aborted");
    assert_eq!(data["reason"], "user cancelled");
}

#[test]
fn test_serialize_thinking_start() {
    let event = AgentStreamEvent::thinking_start();
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "thinking_start");
    assert_eq!(data, serde_json::json!({}));
}

#[test]
fn test_serialize_thinking_delta() {
    let event = AgentStreamEvent::thinking_delta("let me think...".to_string());
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "thinking_delta");
    assert_eq!(data["delta"], "let me think...");
}

#[test]
fn test_serialize_thinking_complete() {
    let event = AgentStreamEvent::thinking_complete("reasoning steps".to_string());
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "thinking_complete");
    assert_eq!(data["thinking"], "reasoning steps");
}

#[test]
fn test_serialize_content_start() {
    let event = AgentStreamEvent::content_start();
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "content_start");
    assert_eq!(data, serde_json::json!({}));
}

#[test]
fn test_serialize_content_delta() {
    let event = AgentStreamEvent::content_delta("Hello".to_string());
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "content_delta");
    assert_eq!(data["delta"], "Hello");
}

#[test]
fn test_serialize_content_complete() {
    let event = AgentStreamEvent::content_complete("Final answer.".to_string());
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "content_complete");
    assert_eq!(data["content"], "Final answer.");
}

#[test]
fn test_serialize_tool_call_begin() {
    let event = AgentStreamEvent::tool_call_begin(
        "call_1".to_string(),
        "get_weather".to_string(),
        r#"{"city":"London"}"#.to_string(),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "tool_call_begin");
    assert_eq!(data["tool_call_id"], "call_1");
    assert_eq!(data["tool_name"], "get_weather");
    assert_eq!(data["arguments"], r#"{"city":"London"}"#);
}

#[test]
fn test_serialize_tool_call_argument_delta() {
    let event = AgentStreamEvent::tool_call_argument_delta(
        "call_1".to_string(),
        "get_weather".to_string(),
        r#"{"ci"#.to_string(),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "tool_call_argument_delta");
    assert_eq!(data["tool_call_id"], "call_1");
    assert_eq!(data["tool_name"], "get_weather");
    assert_eq!(data["delta"], r#"{"ci"#);
}

#[test]
fn test_serialize_tool_call_complete() {
    let event = AgentStreamEvent::tool_call_complete(
        "call_1".to_string(),
        "get_weather".to_string(),
        "sunny, 25C".to_string(),
        Some(42),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "tool_call_complete");
    assert_eq!(data["tool_call_id"], "call_1");
    assert_eq!(data["tool_name"], "get_weather");
    assert_eq!(data["result"], "sunny, 25C");
    assert_eq!(data["duration_ms"], 42);
}

#[test]
fn test_serialize_tool_call_error() {
    let event = AgentStreamEvent::tool_call_error(
        "call_2".to_string(),
        "fetch_url".to_string(),
        "timeout".to_string(),
        Some(5000),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "tool_call_error");
    assert_eq!(data["tool_call_id"], "call_2");
    assert_eq!(data["tool_name"], "fetch_url");
    assert_eq!(data["error"], "timeout");
    assert_eq!(data["duration_ms"], 5000);
}

#[test]
fn test_serialize_tool_call_skipped() {
    let event = AgentStreamEvent::tool_call_skipped(
        "call_3".to_string(),
        "search".to_string(),
        "rate limited".to_string(),
        Some(0),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "tool_call_skipped");
    assert_eq!(data["tool_call_id"], "call_3");
    assert_eq!(data["tool_name"], "search");
    assert_eq!(data["reason"], "rate limited");
    assert_eq!(data["duration_ms"], 0);
}

#[test]
fn test_serialize_max_iterations_reached() {
    let event = AgentStreamEvent::max_iterations_reached(5, 5);
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "max_iterations_reached");
    assert_eq!(data["current"], 5);
    assert_eq!(data["max"], 5);
}

#[test]
fn test_serialize_iteration_continued() {
    let event = AgentStreamEvent::iteration_continued(3);
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "iteration_continued");
    assert_eq!(data["from_iteration"], 3);
}

#[test]
fn test_serialize_iteration_complete() {
    let event = AgentStreamEvent::iteration_complete(
        2,
        vec![],
        Some("done".to_string()),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "iteration_complete");
    assert_eq!(data["iteration"], 2);
    assert_eq!(data["final_answer"], "done");
}

#[test]
fn test_serialize_llm_call_start() {
    let event = AgentStreamEvent::llm_call_start(1, vec![]);
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "llm_call_start");
    assert_eq!(data["iteration"], 1);
}

#[test]
fn test_serialize_llm_call_complete() {
    let usage = TokenUsage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        cached_tokens: None,
    };
    let event = AgentStreamEvent::llm_call_complete(
        "gpt-4".to_string(),
        Some(usage),
    );
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "llm_call_complete");
    assert_eq!(data["model"], "gpt-4");
    assert_eq!(data["usage"]["prompt_tokens"], 100);
    assert_eq!(data["usage"]["completion_tokens"], 50);
    assert_eq!(data["usage"]["total_tokens"], 150);
}

#[test]
fn test_serialize_llm_call_error() {
    let event = AgentStreamEvent::llm_call_error("rate limit".to_string());
    let (event_type, data) = serialize_agent_event(&event);
    assert_eq!(event_type, "llm_call_error");
    assert_eq!(data["error"], "rate limit");
}

#[test]
fn test_serialize_plugin_event() {
    let mut data = serde_json::Map::new();
    data.insert("key".to_string(), serde_json::json!("value"));
    data.insert("count".to_string(), serde_json::json!(42));

    let event = AgentStreamEvent::plugin_event("custom_plugin".to_string(), data.clone());
    let (event_type, data_out) = serialize_agent_event(&event);
    assert_eq!(event_type, "plugin_event");
    assert_eq!(data_out["key"], "value");
    assert_eq!(data_out["count"], 42);
}

// ---------------------------------------------------------------------------
// Test 3: Error response formatting
// ---------------------------------------------------------------------------

#[test]
fn test_to_jsonrpc_error_format() {
    let error_text = to_jsonrpc_error(Some(1), -32600, "Invalid Request".to_string());
    let parsed: Value = serde_json::from_str(&error_text).expect("should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["error"]["code"], -32600);
    assert_eq!(parsed["error"]["message"], "Invalid Request");
}

#[test]
fn test_to_jsonrpc_error_null_id() {
    let error_text = to_jsonrpc_error(None, -32700, "Parse error".to_string());
    let parsed: Value = serde_json::from_str(&error_text).expect("should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert!(parsed["id"].is_null(), "id should be null when None");
    assert_eq!(parsed["error"]["code"], -32700);
}
