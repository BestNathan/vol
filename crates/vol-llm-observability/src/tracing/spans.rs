use tracing::Span;

pub fn llm_call_span(run_id: &str, agent_id: &str, iteration: u32) -> Span {
    tracing::info_span!("llm_call", run_id = %run_id, agent_id = %agent_id, iteration = iteration)
}

pub fn tool_call_span(run_id: &str, agent_id: &str, tool_name: &str, tool_call_id: &str) -> Span {
    tracing::info_span!("tool_call", run_id = %run_id, agent_id = %agent_id, tool_name = %tool_name, tool_call_id = %tool_call_id)
}

pub fn tool_call_span_with_result(run_id: &str, agent_id: &str, tool_name: &str, tool_call_id: &str, success: bool) -> Span {
    tracing::info_span!("tool_call_result", run_id = %run_id, agent_id = %agent_id, tool_name = %tool_name, tool_call_id = %tool_call_id, success = success)
}
