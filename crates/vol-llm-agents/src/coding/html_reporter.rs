//! HTMLReporter - generates HTML timeline reports.

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Mutex;
use vol_llm_core::AgentStreamEvent;

use crate::coding::observer::EventObserver;
use crate::coding::error::ObserverError;

/// HTML Reporter - records events and generates HTML report on complete
pub struct HTMLReporter {
    output_path: PathBuf,
    task_description: String,
    events: Mutex<Vec<AgentStreamEvent>>,
    start_time: Mutex<Option<std::time::Instant>>,
}

impl HTMLReporter {
    /// Create a new HTMLReporter
    pub fn new(output_path: PathBuf, task_description: String) -> Self {
        Self {
            output_path,
            task_description,
            events: Mutex::new(Vec::new()),
            start_time: Mutex::new(None),
        }
    }

    /// Generate HTML report from recorded events
    async fn generate_html_report(&self, events: Vec<AgentStreamEvent>) -> Result<(), ObserverError> {
        let start_time = self.start_time.lock().unwrap()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);

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
        html.push_str(&format!("<p><strong>Duration:</strong> {}s | <strong>Iterations:</strong> {} | <strong>Tool Calls:</strong> {}</p>\n", start_time, iteration_count, tool_call_count));
        html.push_str("</div>\n");
        html.push_str("<h2>Timeline</h2>\n<ul class=\"timeline\">\n");

        for event in &events {
            let (class, detail) = match event {
                AgentStreamEvent::AgentStart { input } => {
                    ("", format!("Agent started: {}", input))
                }
                AgentStreamEvent::ThinkingComplete { thinking } => {
                    ("thinking", format!("Thinking:\n{}", thinking))
                }
                AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
                    ("tool", format!("→ {}({})\n", tool_name, arguments))
                }
                AgentStreamEvent::ToolCallComplete { tool_name, result, tool_call_id: _ } => {
                    ("tool", format!("← {} result:\n{}", tool_name, result))
                }
                AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer } => {
                    ("", format!("Iteration {} complete{}{}",
                        iteration,
                        if !tool_calls.is_empty() { format!(" ({} tools)", tool_calls.len()) } else { "".to_string() },
                        if let Some(answer) = final_answer { format!("\nAnswer: {}", answer) } else { "".to_string() }
                    ))
                }
                AgentStreamEvent::AgentComplete => {
                    ("complete", "Agent completed".to_string())
                }
                AgentStreamEvent::AgentAborted { reason } => {
                    ("complete", format!("Agent aborted: {}", reason))
                }
                AgentStreamEvent::PluginEvent { name, data } => {
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
            AgentStreamEvent::ThinkingComplete { .. } => "Thinking",
            AgentStreamEvent::ToolCallBegin { .. } => "Tool Call",
            AgentStreamEvent::ToolCallComplete { .. } => "Tool Result",
            AgentStreamEvent::IterationComplete { .. } => "Iteration",
            AgentStreamEvent::AgentComplete => "Complete",
            AgentStreamEvent::AgentAborted { .. } => "Aborted",
            AgentStreamEvent::PluginEvent { .. } => "Plugin",
        }
    }
}

#[async_trait]
impl EventObserver for HTMLReporter {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let event_type = Self::event_name(event);
        tracing::debug!(event_type = %event_type, "Observer received event");

        // Record start time on first event
        {
            let mut start_time = self.start_time.lock().unwrap();
            if start_time.is_none() {
                *start_time = Some(std::time::Instant::now());
            }
        }

        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        let events: Vec<AgentStreamEvent> = self.events.lock().unwrap().drain(..).collect();
        tracing::info!("Generating HTML report with {} events", events.len());
        self.generate_html_report(events).await
    }
}
