//! Prompt fragment definitions.

use vol_llm_core::ToolDefinition;

/// The type of a prompt fragment, used for categorization.
#[derive(Debug, Clone, PartialEq)]
pub enum FragmentType {
    /// Role definition
    Role,
    /// Tool list
    Tools,
    /// Behavior rules
    Rules,
    /// Output format
    Format,
    /// Custom content
    Custom,
}

/// A prompt fragment - a reusable content block.
#[derive(Debug, Clone)]
pub struct PromptFragment {
    /// Unique identifier for the fragment
    pub id: String,
    /// The content of the fragment
    pub content: String,
    /// The type of the fragment
    pub fragment_type: FragmentType,
}

impl PromptFragment {
    /// Create a new prompt fragment.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the fragment
    /// * `content` - The content of the fragment
    /// * `fragment_type` - The type of the fragment
    ///
    /// # Example
    /// ```text
    /// use vol_llm_agent::prompt_context::{PromptFragment, FragmentType};
    ///
    /// let fragment = PromptFragment::new(
    ///     "role",
    ///     "You are a helpful assistant.",
    ///     FragmentType::Role,
    /// );
    /// ```
    pub fn new(id: &str, content: &str, fragment_type: FragmentType) -> Self {
        Self {
            id: id.to_string(),
            content: content.to_string(),
            fragment_type,
        }
    }

    /// Generate a tools fragment from a list of tool definitions.
    ///
    /// # Arguments
    /// * `tools` - Slice of tool definitions to convert into a fragment
    ///
    /// # Returns
    /// A PromptFragment with id "tools" and content formatted as a markdown list
    ///
    /// # Example
    /// ```text
    /// use vol_llm_agent::prompt_context::PromptFragment;
    /// use vol_llm_core::ToolDefinition;
    ///
    /// let tools = vec![
    ///     ToolDefinition {
    ///         name: "get_weather".to_string(),
    ///         description: Some("Get the weather for a location".to_string()),
    ///         parameters: None,
    ///     },
    /// ];
    ///
    /// let fragment = PromptFragment::from_tools(&tools);
    /// assert_eq!(fragment.id, "tools");
    /// assert!(fragment.content.contains("get_weather"));
    /// ```
    pub fn from_tools(tools: &[ToolDefinition]) -> Self {
        let content = tools
            .iter()
            .map(|t| {
                let desc = t.description.as_deref().unwrap_or("No description");
                format!("- `{}`: {}", t.name, desc)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Self::new("tools", &content, FragmentType::Tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fragment_type_variants() {
        // Test all FragmentType variants exist and are comparable
        assert_eq!(FragmentType::Role, FragmentType::Role);
        assert_eq!(FragmentType::Tools, FragmentType::Tools);
        assert_eq!(FragmentType::Rules, FragmentType::Rules);
        assert_eq!(FragmentType::Format, FragmentType::Format);
        assert_eq!(FragmentType::Custom, FragmentType::Custom);

        // Test different variants are not equal
        assert_ne!(FragmentType::Role, FragmentType::Tools);
        assert_ne!(FragmentType::Rules, FragmentType::Format);
    }

    #[test]
    fn test_fragment_type_debug() {
        // Test Debug trait works
        let _debug = format!("{:?}", FragmentType::Role);
        let _debug = format!("{:?}", FragmentType::Tools);
    }

    #[test]
    fn test_prompt_fragment_new() {
        let fragment = PromptFragment::new("test_id", "test content", FragmentType::Role);

        assert_eq!(fragment.id, "test_id");
        assert_eq!(fragment.content, "test content");
        assert_eq!(fragment.fragment_type, FragmentType::Role);
    }

    #[test]
    fn test_prompt_fragment_from_tools_single_tool() {
        let tools = vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: Some("Get the weather for a location".to_string()),
            parameters: None,
        }];

        let fragment = PromptFragment::from_tools(&tools);

        assert_eq!(fragment.id, "tools");
        assert_eq!(fragment.fragment_type, FragmentType::Tools);
        assert!(fragment.content.contains("`get_weather`"));
        assert!(fragment.content.contains("Get the weather for a location"));
    }

    #[test]
    fn test_prompt_fragment_from_tools_multiple_tools() {
        let tools = vec![
            ToolDefinition {
                name: "get_weather".to_string(),
                description: Some("Get the weather for a location".to_string()),
                parameters: None,
            },
            ToolDefinition {
                name: "search".to_string(),
                description: Some("Search the web".to_string()),
                parameters: None,
            },
            ToolDefinition {
                name: "calculator".to_string(),
                description: None,
                parameters: None,
            },
        ];

        let fragment = PromptFragment::from_tools(&tools);

        assert_eq!(fragment.id, "tools");
        assert_eq!(fragment.fragment_type, FragmentType::Tools);
        assert!(fragment.content.contains("`get_weather`"));
        assert!(fragment.content.contains("`search`"));
        assert!(fragment.content.contains("`calculator`"));
        // Test that tools without description show "No description"
        assert!(fragment.content.contains("No description"));
    }

    #[test]
    fn test_prompt_fragment_from_tools_empty() {
        let tools: Vec<ToolDefinition> = vec![];

        let fragment = PromptFragment::from_tools(&tools);

        assert_eq!(fragment.id, "tools");
        assert_eq!(fragment.fragment_type, FragmentType::Tools);
        assert_eq!(fragment.content, "");
    }

    #[test]
    fn test_prompt_fragment_clone() {
        let fragment = PromptFragment::new("test", "content", FragmentType::Rules);

        let cloned = fragment.clone();

        assert_eq!(cloned.id, fragment.id);
        assert_eq!(cloned.content, fragment.content);
        assert_eq!(cloned.fragment_type, fragment.fragment_type);
    }
}
