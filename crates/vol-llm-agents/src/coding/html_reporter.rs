//! HTMLReporter - generates HTML timeline reports with ordered events.

use async_trait::async_trait;
use std::path::PathBuf;
use vol_llm_core::AgentStreamEvent;

use crate::coding::observer::EventObserver;
use crate::coding::error::ObserverError;
use crate::coding::channelled_observer::ChannelledEventObserver;

/// HTML Reporter - records events via ordered channel and generates HTML report on complete
pub struct HTMLReporter {
    inner: ChannelledEventObserver,
    output_path: PathBuf,
    task_description: String,
}

impl HTMLReporter {
    /// Create a new HTMLReporter
    pub fn new(output_path: PathBuf, task_description: String) -> Self {
        Self {
            inner: ChannelledEventObserver::new(),
            output_path,
            task_description,
        }
    }

    /// Generate HTML report from recorded events
    async fn generate_html_report(&self, events: Vec<AgentStreamEvent>) -> Result<(), ObserverError> {
        let iteration_count = events.iter().filter(|e| matches!(e, AgentStreamEvent::IterationComplete { .. })).count();
        let tool_call_count = events.iter().filter(|e| matches!(e, AgentStreamEvent::ToolCallBegin { .. } | AgentStreamEvent::ToolCallComplete { .. })).count() / 2;

        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str(&format!("<title>Coding Agent Report - {}</title>\n", self.task_description));
        html.push_str("<style>
            body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 2rem; }
            .summary { background: #f5f5f5; padding: 1rem; border-radius: 8px; margin-bottom: 1rem; }
            .timeline { list-style: none; padding: 0; }
            .timeline-item { padding: 0.5rem 1rem; margin: 0.5rem 0; border-left: 3px solid #007bff; background: #f9f9f9; }
            .timeline-item.thinking { border-color: #28a745; }
            .timeline-item.tool { border-color: #dc3545; }
            .timeline-item.complete { border-color: #17a2b8; }
            .event-type { font-weight: bold; color: #666; }
            .event-detail { margin-top: 0.25rem; white-space: pre-wrap; font-family: monospace; font-size: 0.9em; }
        </style>\n");
        html.push_str("</head>\n<body>\n");
        html.push_str("<h1>Coding Agent Report</h1>\n");
        html.push_str("<div class=\"summary\">\n");
        html.push_str(&format!("<p><strong>Task:</strong> {}</p>\n", self.task_description));
        html.push_str(&format!("<p><strong>Iterations:</strong> {} | <strong>Tool Calls:</strong> {}</p>\n", iteration_count, tool_call_count));
        html.push_str("</div>\n");
        html.push_str("<h2>Timeline</h2>\n<ul class=\"timeline\">\n");

        for event in &events {
            let (class, detail) = match event {
                AgentStreamEvent::AgentStart { input, .. } => {
                    ("", format!("Agent started: {}", input))
                }
                AgentStreamEvent::LLMCallStart { iteration, .. } => {
                    ("", format!("LLM call start (iteration {})", iteration))
                }
                AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
                    ("", format!("LLM call complete: {} (usage: {:?})", model, usage))
                }
                AgentStreamEvent::LLMCallError { error, .. } => {
                    ("error", format!("LLM call error: {}", error))
                }
                AgentStreamEvent::ThinkingStart { .. } => {
                    ("thinking", "Thinking started".to_string())
                }
                AgentStreamEvent::ThinkingDelta { delta, .. } => {
                    ("thinking", delta.clone())
                }
                AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                    ("thinking", format!("Thinking:\n{}", thinking))
                }
                AgentStreamEvent::ContentStart { .. } => {
                    ("", "Content started".to_string())
                }
                AgentStreamEvent::ContentDelta { delta, .. } => {
                    ("", delta.clone())
                }
                AgentStreamEvent::ContentComplete { content, .. } => {
                    ("", format!("Content complete: {}", content))
                }
                AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
                    ("tool", format!("→ {}({})\n", tool_name, arguments))
                }
                AgentStreamEvent::ToolCallComplete { tool_name, result, .. } => {
                    ("tool", format!("← {} result:\n{}", tool_name, result))
                }
                AgentStreamEvent::ToolCallError { tool_name, error, .. } => {
                    ("error", format!("Tool {} error: {}", tool_name, error))
                }
                AgentStreamEvent::ToolCallSkipped { tool_name, reason, .. } => {
                    ("", format!("Tool {} skipped: {}", tool_name, reason))
                }
                AgentStreamEvent::ToolCallArgumentDelta { tool_name, delta, .. } => {
                    ("tool", format!("→ {} argument delta: {}", tool_name, delta))
                }
                AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer, .. } => {
                    ("", format!("Iteration {} complete{}{}",
                        iteration,
                        if !tool_calls.is_empty() { format!(" ({} tools)", tool_calls.len()) } else { "".to_string() },
                        if let Some(answer) = final_answer { format!("\nAnswer: {}", answer) } else { "".to_string() }
                    ))
                }
                AgentStreamEvent::AgentComplete { .. } => {
                    ("complete", "Agent completed".to_string())
                }
                AgentStreamEvent::AgentAborted { reason, .. } => {
                    ("complete", format!("Agent aborted: {}", reason))
                }
                AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                    ("error", format!("Max iterations reached ({}/{}) — waiting for user decision", current_iteration, max_iterations))
                }
                AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                    ("", format!("Continuing from iteration {} (counter reset to 0)", from_iteration))
                }
                AgentStreamEvent::PluginEvent { name, data, .. } => {
                    ("", format!("Plugin event: {} = {:?}", name, data))
                }
            };

            html.push_str(&format!("  <li class=\"timeline-item {}\">\n", class));
            html.push_str(&format!("    <span class=\"event-type\">{}</span>\n", Self::event_name(event)));
            html.push_str(&format!("    <div class=\"event-detail\">{}</div>\n", detail.replace("<", "&lt;").replace(">", "&gt;")));
            html.push_str("  </li>\n");
        }

        html.push_str("</ul>\n</body>\n</html>");

        // Ensure parent directory exists
        if let Some(parent) = self.output_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| ObserverError::ReportFailed(format!("Failed to create directory: {}", e)))?;
        }

        tokio::fs::write(&self.output_path, &html)
            .await
            .map_err(|e| ObserverError::ReportFailed(format!("Failed to write report: {}", e)))?;

        Ok(())
    }

    fn event_name(event: &AgentStreamEvent) -> &'static str {
        match event {
            AgentStreamEvent::AgentStart { .. } => "Start",
            AgentStreamEvent::ThinkingStart { .. } => "Thinking Start",
            AgentStreamEvent::ThinkingDelta { .. } => "Thinking Delta",
            AgentStreamEvent::ThinkingComplete { .. } => "Thinking",
            AgentStreamEvent::ContentStart { .. } => "Content Start",
            AgentStreamEvent::ContentDelta { .. } => "Content Delta",
            AgentStreamEvent::ContentComplete { .. } => "Content",
            AgentStreamEvent::LLMCallStart { .. } => "LLM Call Start",
            AgentStreamEvent::LLMCallComplete { .. } => "LLM Call Complete",
            AgentStreamEvent::LLMCallError { .. } => "LLM Call Error",
            AgentStreamEvent::ToolCallBegin { .. } => "Tool Call",
            AgentStreamEvent::ToolCallComplete { .. } => "Tool Result",
            AgentStreamEvent::ToolCallError { .. } => "Tool Error",
            AgentStreamEvent::ToolCallSkipped { .. } => "Tool Skipped",
            AgentStreamEvent::ToolCallArgumentDelta { .. } => "Tool Argument Delta",
            AgentStreamEvent::IterationComplete { .. } => "Iteration",
            AgentStreamEvent::AgentComplete { .. } => "Complete",
            AgentStreamEvent::AgentAborted { .. } => "Aborted",
            AgentStreamEvent::MaxIterationsReached { .. } => "Max Iterations",
            AgentStreamEvent::IterationContinued { .. } => "Iteration Continued",
            AgentStreamEvent::PluginEvent { .. } => "Plugin",
        }
    }
}

#[async_trait]
impl EventObserver for HTMLReporter {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let event_type = Self::event_name(event);
        tracing::debug!(event_type = %event_type, "Observer received event");
        self.inner.on_event(event).await
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        self.inner.on_complete().await?;
        let events = self.inner.events().await;
        tracing::info!("Generating HTML report with {} events", events.len());
        self.generate_html_report(events).await
    }
}
