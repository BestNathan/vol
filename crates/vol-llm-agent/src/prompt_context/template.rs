//! Prompt template definitions.

use once_cell::sync::Lazy;

/// Compiled regex pattern for parsing injection points (static, initialized once).
static INJECTION_RE: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\{(\w+)\}").unwrap());

/// A prompt template - a user-defined template with named injection points.
///
/// Templates support `{name}` placeholders for dynamic content injection.
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    /// Unique identifier for the template (used for caching)
    pub id: String,
    /// Template content with optional `{name}` placeholders
    pub content: String,
    /// List of injection point names parsed from the content
    pub injections: Vec<String>,
}

impl PromptTemplate {
    /// Create a new template from a string, automatically parsing injection points.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the template
    /// * `content` - Template content with optional `{name}` placeholders
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::PromptTemplate;
    ///
    /// let template = PromptTemplate::new(
    ///     "market-analyst",
    ///     r#"You are a {role}.
    ///
    /// ## Tools
    /// {tools}
    ///
    /// ## Rules
    /// {rules}"#
    /// );
    ///
    /// assert_eq!(template.id, "market-analyst");
    /// assert_eq!(template.injections.len(), 3);
    /// ```
    pub fn new(id: &str, content: &str) -> Self {
        let injections = Self::parse_injection_points(content);
        Self {
            id: id.to_string(),
            content: content.to_string(),
            injections,
        }
    }

    /// Parse injection points from template content.
    ///
    /// Finds all `{name}` placeholders in the content and returns their names.
    ///
    /// # Arguments
    /// * `content` - Template content to parse
    ///
    /// # Returns
    /// A vector of injection point names (without braces)
    pub(crate) fn parse_injection_points(content: &str) -> Vec<String> {
        INJECTION_RE
            .captures_iter(content)
            .map(|cap| cap[1].to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_template_new_single_injection() {
        let template = PromptTemplate::new("test-template", "You are a {role}.");

        assert_eq!(template.id, "test-template");
        assert_eq!(template.content, "You are a {role}.");
        assert_eq!(template.injections, vec!["role"]);
    }

    #[test]
    fn test_prompt_template_new_multiple_injections() {
        let content = r#"You are a {role}.

## Tools
{tools}

## Rules
{rules}

## Format
{format}"#;

        let template = PromptTemplate::new("market-analyst", content);

        assert_eq!(template.id, "market-analyst");
        assert_eq!(template.injections.len(), 4);
        assert!(template.injections.contains(&"role".to_string()));
        assert!(template.injections.contains(&"tools".to_string()));
        assert!(template.injections.contains(&"rules".to_string()));
        assert!(template.injections.contains(&"format".to_string()));
    }

    #[test]
    fn test_prompt_template_new_no_injections() {
        let template = PromptTemplate::new("simple", "You are a helpful assistant.");

        assert_eq!(template.id, "simple");
        assert_eq!(template.content, "You are a helpful assistant.");
        assert!(template.injections.is_empty());
    }

    #[test]
    fn test_parse_injection_points_single() {
        let content = "You are a {role}.";
        let injections = PromptTemplate::parse_injection_points(content);

        assert_eq!(injections, vec!["role"]);
    }

    #[test]
    fn test_parse_injection_points_multiple() {
        let content = "Use {tool1} and {tool2} to {action}.";
        let injections = PromptTemplate::parse_injection_points(content);

        assert_eq!(injections.len(), 3);
        assert!(injections.contains(&"tool1".to_string()));
        assert!(injections.contains(&"tool2".to_string()));
        assert!(injections.contains(&"action".to_string()));
    }

    #[test]
    fn test_parse_injection_points_duplicates() {
        let content = "Use {tool} to {action}, then use {tool} again.";
        let injections = PromptTemplate::parse_injection_points(content);

        // Duplicates should be preserved (we collect all matches)
        assert_eq!(injections.len(), 3);
        assert_eq!(injections.iter().filter(|&x| x == "tool").count(), 2);
    }

    #[test]
    fn test_parse_injection_points_empty() {
        let content = "No placeholders here.";
        let injections = PromptTemplate::parse_injection_points(content);

        assert!(injections.is_empty());
    }

    #[test]
    fn test_parse_injection_points_underscores() {
        let content = "Use {my_tool_name} and {another_one}.";
        let injections = PromptTemplate::parse_injection_points(content);

        assert_eq!(injections, vec!["my_tool_name", "another_one"]);
    }

    #[test]
    fn test_parse_injection_points_numbers() {
        let content = "Values: {var1}, {var2}, {test123}.";
        let injections = PromptTemplate::parse_injection_points(content);

        assert_eq!(injections, vec!["var1", "var2", "test123"]);
    }

    #[test]
    fn test_prompt_template_clone() {
        let template = PromptTemplate::new("test", "content with {injection}");
        let cloned = template.clone();

        assert_eq!(cloned.id, template.id);
        assert_eq!(cloned.content, template.content);
        assert_eq!(cloned.injections, template.injections);
    }

    #[test]
    fn test_prompt_template_debug() {
        let template = PromptTemplate::new("test", "content {x}");
        let debug = format!("{:?}", template);

        assert!(debug.contains("PromptTemplate"));
        assert!(debug.contains("test"));
    }
}
