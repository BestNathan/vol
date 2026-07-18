//! Rule-based tool call result compressor.

use vol_session::SessionMessage;

const TOOL_ARGS_MAX: usize = 200;
const TOOL_RESULT_MAX: usize = 500;

pub struct ToolCallCompressor;

impl ToolCallCompressor {
    pub fn compress(&self, messages: &[SessionMessage]) -> Option<SessionMessage> {
        if messages.is_empty() {
            return None;
        }

        let summary_lines: Vec<String> = messages
            .iter()
            .filter_map(|sm| self.compress_one(sm))
            .collect();

        if summary_lines.is_empty() {
            return None;
        }

        let summary = summary_lines.join("\n");
        let session_id = messages
            .first()
            .map(|m| m.session_id.clone())
            .unwrap_or_default();
        let system_msg = vol_llm_core::Message::system(summary);
        Some(SessionMessage::new(session_id, system_msg))
    }

    fn compress_one(&self, msg: &SessionMessage) -> Option<String> {
        let tool_name = msg.message.name.as_deref().unwrap_or("unknown");
        let args = msg
            .message
            .content
            .as_ref()
            .map(vol_llm_core::MessageContent::as_str)
            .unwrap_or("");
        let result = msg
            .message
            .content
            .as_ref()
            .map(vol_llm_core::MessageContent::as_str)
            .unwrap_or("");

        let args_truncated = truncate(args, TOOL_ARGS_MAX);
        let result_truncated = truncate(result, TOOL_RESULT_MAX);

        Some(format!(
            "[{tool_name}] {args_truncated} → {result_truncated}"
        ))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    fn make_tool_msg(session_id: &str, name: &str, _args: &str, result: &str) -> SessionMessage {
        let mut msg = Message::tool(result.to_string(), "call_1".to_string());
        msg.name = Some(name.to_string());
        SessionMessage::new(session_id.to_string(), msg)
    }

    #[test]
    fn test_compress_empty() {
        let compressor = ToolCallCompressor;
        assert!(compressor.compress(&[]).is_none());
    }

    #[test]
    fn test_compress_single_tool() {
        let compressor = ToolCallCompressor;
        let msgs = vec![make_tool_msg("s1", "bash", "ls -la", "total 42")];
        let result = compressor.compress(&msgs).unwrap();
        assert_eq!(
            result.message.role,
            vol_llm_core::message::MessageRole::System
        );
        let content = result.message.content.as_ref().unwrap().as_str();
        assert!(content.contains("[bash]"));
        assert!(content.contains("total 42"));
    }

    #[test]
    fn test_compress_multiple_tools() {
        let compressor = ToolCallCompressor;
        let msgs = vec![
            make_tool_msg("s1", "read_file", "{\"path\": \"test.rs\"}", "fn main() {}"),
            make_tool_msg("s1", "bash", "cargo check", "Finished dev"),
        ];
        let result = compressor.compress(&msgs).unwrap();
        let content = result.message.content.as_ref().unwrap().as_str();
        assert!(content.contains("[read_file]"));
        assert!(content.contains("[bash]"));
    }

    #[test]
    fn test_truncate_long_content() {
        let long_args = "a".repeat(300);
        let result = super::truncate(&long_args, TOOL_ARGS_MAX);
        assert_eq!(result.len(), TOOL_ARGS_MAX + 3);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_short_content() {
        let result = super::truncate("short", 200);
        assert_eq!(result, "short");
    }
}
