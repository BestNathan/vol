//! Output formatting for the fs CLI — text passthrough and JSON envelope.

/// Format a scheme (parameter list) for a specific subcommand.
pub(crate) fn fmt_scheme(subcommand: &str, params: &[(&str, bool, &str)]) -> String {
    let mut out = format!("{subcommand} parameters:\n");
    for (name, required, desc) in params {
        let req = if *required {
            "(required)"
        } else {
            "(optional)"
        };
        out.push_str(&format!("  --{name:<14} {req:<10} {desc}\n"));
    }
    out.trim_end().to_string()
}

/// Wrap a tool result in a JSON envelope.
pub(crate) fn envelope(success: bool, content: &str) -> String {
    serde_json::json!({ "success": success, "content": content }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_scheme_lists_flags_and_required_marker() {
        let out = fmt_scheme("read", &[("file_path", true, "Path to the file to read")]);
        assert!(out.contains("--file_path"));
        assert!(out.contains("(required)"));
    }

    #[test]
    fn envelope_serializes_success_and_content() {
        let out = envelope(true, "hi");
        // serde_json preserve_order → insertion order (success, content)
        assert_eq!(out, "{\"success\":true,\"content\":\"hi\"}");
    }

    #[test]
    fn envelope_serializes_failure() {
        let out = envelope(false, "boom");
        assert!(out.contains("\"success\":false"));
    }
}
