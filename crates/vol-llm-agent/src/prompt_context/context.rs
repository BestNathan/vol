//! Prompt context manager.
//!
//! Manages fixed fragments and dynamic injections to build cache-friendly prompts.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use vol_llm_core::ToolDefinition;

use super::{PromptFragment, PromptTemplate};

/// Prompt context manager.
///
/// Manages fixed fragments and dynamic injections, generating cache-friendly prompts.
#[derive(Debug, Clone)]
pub struct PromptContext {
    /// Main template
    template: PromptTemplate,

    /// Registered fragments (fixed content)
    fragments: HashMap<String, PromptFragment>,

    /// Dynamic content (varies per turn)
    dynamic_vars: HashMap<String, String>,

    /// Cache key (computed from fixed content)
    cache_key: String,
}

impl PromptContext {
    /// Create a new prompt context from a template.
    ///
    /// # Arguments
    /// * `template` - The prompt template to use
    ///
    /// # Returns
    /// A new PromptContext with empty fragments and dynamic vars
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate};
    ///
    /// let template = PromptTemplate::new("simple", "You are a helpful assistant.");
    /// let context = PromptContext::new(template);
    ///
    /// assert!(context.cache_key().starts_with("prompt_"));
    /// ```
    pub fn new(template: PromptTemplate) -> Self {
        let cache_key = Self::compute_cache_key(&template, &HashMap::new());
        Self {
            template,
            fragments: HashMap::new(),
            dynamic_vars: HashMap::new(),
            cache_key,
        }
    }

    /// Add a fixed fragment to the context.
    ///
    /// # Arguments
    /// * `fragment` - The fragment to add
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate, PromptFragment, FragmentType};
    ///
    /// let template = PromptTemplate::new("test", "Role: {role}");
    /// let role_fragment = PromptFragment::new("role", "You are a helpful assistant.", FragmentType::Role);
    ///
    /// let context = PromptContext::new(template)
    ///     .with_fragment(role_fragment);
    /// ```
    pub fn with_fragment(mut self, fragment: PromptFragment) -> Self {
        let id = fragment.id.clone();
        self.fragments.insert(id, fragment);
        self.recompute_cache_key();
        self
    }

    /// Add a tools fragment from a list of tool definitions.
    ///
    /// Convenience method that creates a fragment from tools and adds it.
    ///
    /// # Arguments
    /// * `tools` - Slice of tool definitions
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate};
    /// use vol_llm_core::ToolDefinition;
    ///
    /// let template = PromptTemplate::new("test", "Tools: {tools}");
    /// let tools = vec![
    ///     ToolDefinition {
    ///         name: "get_weather".to_string(),
    ///         description: Some("Get weather info".to_string()),
    ///         parameters: None,
    ///     },
    /// ];
    ///
    /// let context = PromptContext::new(template)
    ///     .with_tools(&tools);
    /// ```
    pub fn with_tools(mut self, tools: &[ToolDefinition]) -> Self {
        let fragment = PromptFragment::from_tools(tools);
        let id = fragment.id.clone();
        self.fragments.insert(id, fragment);
        self.recompute_cache_key();
        self
    }

    /// Set a dynamic variable (does not affect cache key).
    ///
    /// Dynamic variables are used for per-turn content that shouldn't affect caching.
    ///
    /// # Arguments
    /// * `name` - Variable name (must match an injection point in the template)
    /// * `value` - Variable value
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate};
    ///
    /// let template = PromptTemplate::new("test", "Role: {role}");
    /// let context = PromptContext::new(template)
    ///     .with_dynamic("role", "You are a financial analyst.");
    /// ```
    pub fn with_dynamic(mut self, name: &str, value: &str) -> Self {
        self.dynamic_vars
            .insert(name.to_string(), value.to_string());
        self
    }

    /// Build the System message content (fixed content only, cache-friendly).
    ///
    /// Replaces all injection points with either fragment content or dynamic vars.
    ///
    /// # Returns
    /// The fully rendered system message content as a String
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate, PromptFragment, FragmentType};
    ///
    /// let template = PromptTemplate::new("test", "Role: {role}\nRules: {rules}");
    /// let context = PromptContext::new(template)
    ///     .with_fragment(PromptFragment::new("role", "Assistant", FragmentType::Role))
    ///     .with_fragment(PromptFragment::new("rules", "Be helpful", FragmentType::Rules));
    ///
    /// let system = context.build_system();
    /// assert!(system.contains("Role: Assistant"));
    /// assert!(system.contains("Rules: Be helpful"));
    /// ```
    pub fn build_system(&self) -> String {
        let mut content = self.template.content.clone();

        // Replace fixed fragments first
        for (id, fragment) in &self.fragments {
            let placeholder = format!("{{{id}}}");
            if content.contains(&placeholder) {
                content = content.replace(&placeholder, &fragment.content);
            }
        }

        // Replace dynamic variables (for injection points without fragments)
        for injection in &self.template.injections {
            if !self.fragments.contains_key(injection) {
                let placeholder = format!("{{{injection}}}");
                if content.contains(&placeholder) {
                    let dynamic_value = self
                        .dynamic_vars
                        .get(injection)
                        .cloned()
                        .unwrap_or_default();
                    content = content.replace(&placeholder, &dynamic_value);
                }
            }
        }

        content
    }

    /// Build the User message content (dynamic content).
    ///
    /// Formats the user query with optional RAG context.
    ///
    /// # Arguments
    /// * `query` - The user's query/question
    /// * `rag_context` - Optional RAG context to include
    ///
    /// # Returns
    /// The formatted user message content as a String
    ///
    /// # Example
    /// ```
    /// use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate};
    ///
    /// let template = PromptTemplate::new("test", "System prompt");
    /// let context = PromptContext::new(template);
    ///
    /// let user = context.build_user("What is the weather?", None);
    /// assert!(user.contains("问题：What is the weather?"));
    /// ```
    pub fn build_user(&self, query: &str, rag_context: Option<&str>) -> String {
        let mut parts = Vec::new();

        // RAG context (if provided)
        if let Some(ctx) = rag_context {
            parts.push(format!("参考资料:\n{ctx}\n"));
        }

        // User question
        parts.push(format!("问题：{query}"));

        parts.join("\n\n")
    }

    /// Get the cache key for this context.
    ///
    /// The cache key is computed from the template ID, content, and registered fragments.
    /// Dynamic variables do not affect the cache key.
    ///
    /// # Returns
    /// A reference to the cache key string
    pub fn cache_key(&self) -> &str {
        &self.cache_key
    }

    /// Recompute the cache key from current template and fragments.
    ///
    /// Called internally when fragments are modified.
    fn recompute_cache_key(&mut self) {
        self.cache_key = Self::compute_cache_key(&self.template, &self.fragments);
    }

    /// Compute a cache key from a template and fragments.
    ///
    /// Uses a hash of the template ID, content, and fragment IDs/content to generate
    /// a unique key. Fragments are sorted by ID before hashing to ensure consistency.
    ///
    /// # Arguments
    /// * `template` - The prompt template
    /// * `fragments` - Map of registered fragments
    ///
    /// # Returns
    /// A cache key string in the format "prompt_{hash}"
    fn compute_cache_key(
        template: &PromptTemplate,
        fragments: &HashMap<String, PromptFragment>,
    ) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash template identity
        template.id.hash(&mut hasher);
        template.content.hash(&mut hasher);

        // Hash fragments in sorted order for consistency
        let mut ids: Vec<_> = fragments.keys().collect();
        ids.sort();
        for id in ids {
            id.hash(&mut hasher);
            fragments[id].content.hash(&mut hasher);
        }

        format!("prompt_{}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt_context::FragmentType;

    #[test]
    fn test_prompt_context_new() {
        let template = PromptTemplate::new("test-template", "You are a helpful assistant.");
        let context = PromptContext::new(template);

        assert!(context.cache_key().starts_with("prompt_"));
    }

    #[test]
    fn test_prompt_context_with_fragment() {
        let template = PromptTemplate::new("test", "Role: {role}");
        let fragment = PromptFragment::new("role", "You are an assistant.", FragmentType::Role);

        let context = PromptContext::new(template).with_fragment(fragment);

        assert!(context.cache_key().starts_with("prompt_"));
    }

    #[test]
    fn test_prompt_context_with_tools() {
        let template = PromptTemplate::new("test", "Tools: {tools}");
        let tools = vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: Some("Get weather info".to_string()),
            parameters: None,
        }];

        let context = PromptContext::new(template).with_tools(&tools);

        assert!(context.cache_key().starts_with("prompt_"));
    }

    #[test]
    fn test_prompt_context_with_dynamic() {
        let template = PromptTemplate::new("test", "Role: {role}");

        let context =
            PromptContext::new(template).with_dynamic("role", "You are a financial analyst.");

        // Dynamic vars don't affect cache key
        assert!(context.cache_key().starts_with("prompt_"));
    }

    #[test]
    fn test_build_system_with_fragments() {
        let template = PromptTemplate::new("test", "Role: {role}\n\nRules: {rules}");

        let context = PromptContext::new(template)
            .with_fragment(PromptFragment::new(
                "role",
                "You are a helpful assistant.",
                FragmentType::Role,
            ))
            .with_fragment(PromptFragment::new(
                "rules",
                "Be concise and accurate.",
                FragmentType::Rules,
            ));

        let system = context.build_system();

        assert!(system.contains("Role: You are a helpful assistant."));
        assert!(system.contains("Rules: Be concise and accurate."));
    }

    #[test]
    fn test_build_system_with_dynamic_vars() {
        let template = PromptTemplate::new("test", "Role: {role}");

        let context =
            PromptContext::new(template).with_dynamic("role", "You are a financial analyst.");

        let system = context.build_system();

        assert!(system.contains("Role: You are a financial analyst."));
    }

    #[test]
    fn test_build_system_fragment_takes_precedence_over_dynamic() {
        let template = PromptTemplate::new("test", "Role: {role}");

        // Add both fragment and dynamic var with same name
        // Fragment should take precedence
        let context = PromptContext::new(template)
            .with_fragment(PromptFragment::new(
                "role",
                "Fragment role.",
                FragmentType::Role,
            ))
            .with_dynamic("role", "Dynamic role.");

        let system = context.build_system();

        // Fragment content should be used, not dynamic
        assert!(system.contains("Fragment role."));
        assert!(!system.contains("Dynamic role."));
    }

    #[test]
    fn test_build_user_without_rag_context() {
        let template = PromptTemplate::new("test", "System");
        let context = PromptContext::new(template);

        let user = context.build_user("What is the weather?", None);

        assert_eq!(user, "问题：What is the weather?");
    }

    #[test]
    fn test_build_user_with_rag_context() {
        let template = PromptTemplate::new("test", "System");
        let context = PromptContext::new(template);

        let user = context.build_user("What is the weather?", Some("Weather data: sunny, 25C"));

        assert!(user.contains("参考资料:"));
        assert!(user.contains("Weather data: sunny, 25C"));
        assert!(user.contains("问题：What is the weather?"));
    }

    #[test]
    fn test_cache_key_stability() {
        let template = PromptTemplate::new("test", "Role: {role}");

        // Same template and fragments should produce same cache key
        let context1 = PromptContext::new(template.clone()).with_fragment(PromptFragment::new(
            "role",
            "Assistant",
            FragmentType::Role,
        ));

        let context2 = PromptContext::new(template).with_fragment(PromptFragment::new(
            "role",
            "Assistant",
            FragmentType::Role,
        ));

        assert_eq!(context1.cache_key(), context2.cache_key());
    }

    #[test]
    fn test_cache_key_changes_with_different_fragments() {
        let template = PromptTemplate::new("test", "Role: {role}");

        let context1 = PromptContext::new(template.clone()).with_fragment(PromptFragment::new(
            "role",
            "Assistant",
            FragmentType::Role,
        ));

        let context2 = PromptContext::new(template).with_fragment(PromptFragment::new(
            "role",
            "Analyst",
            FragmentType::Role,
        ));

        // Different fragment content should produce different cache key
        assert_ne!(context1.cache_key(), context2.cache_key());
    }

    #[test]
    fn test_cache_key_unchanged_by_dynamic_vars() {
        let template = PromptTemplate::new("test", "Role: {role}");
        let fragment = PromptFragment::new("role", "Assistant", FragmentType::Role);

        let context1 = PromptContext::new(template.clone()).with_fragment(fragment.clone());

        let context2 = PromptContext::new(template)
            .with_fragment(fragment)
            .with_dynamic("role", "Different dynamic value");

        // Dynamic vars should not affect cache key
        assert_eq!(context1.cache_key(), context2.cache_key());
    }

    #[test]
    fn test_cache_key_unchanged_by_multiple_dynamic_vars() {
        let template = PromptTemplate::new("test", "A: {a}\nB: {b}");

        let context1 = PromptContext::new(template.clone());
        let context2 = PromptContext::new(template)
            .with_dynamic("a", "value_a")
            .with_dynamic("b", "value_b");

        // Multiple dynamic vars should not affect cache key
        assert_eq!(context1.cache_key(), context2.cache_key());
    }

    #[test]
    fn test_build_system_missing_fragment_uses_empty() {
        let template = PromptTemplate::new("test", "Role: {role}\nTools: {tools}");

        // Only add role fragment, not tools
        let context = PromptContext::new(template).with_fragment(PromptFragment::new(
            "role",
            "Assistant",
            FragmentType::Role,
        ));

        let system = context.build_system();

        assert!(system.contains("Role: Assistant"));
        // Missing fragment should be replaced with empty string
        assert!(system.contains("Tools: "));
    }

    #[test]
    fn test_prompt_context_chaining() {
        let template = PromptTemplate::new("test", "Role: {role}\nTools: {tools}\nRules: {rules}");
        let tools = vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            parameters: None,
        }];

        // Test method chaining works
        let context = PromptContext::new(template)
            .with_fragment(PromptFragment::new("role", "Assistant", FragmentType::Role))
            .with_tools(&tools)
            .with_fragment(PromptFragment::new(
                "rules",
                "Be helpful",
                FragmentType::Rules,
            ))
            .with_dynamic("extra", "unused");

        let system = context.build_system();

        assert!(system.contains("Role: Assistant"));
        assert!(system.contains("Tools:"));
        assert!(system.contains("test_tool"));
        assert!(system.contains("Rules: Be helpful"));
    }
}
