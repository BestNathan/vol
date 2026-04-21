use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::loader::SkillLoader;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity};

/// Parameters for the Skill tool.
#[derive(Debug, Deserialize, Serialize)]
pub struct SkillToolParams {
    /// Skill name to load
    pub name: String,
}

/// Tool that loads skill instructions by name.
pub struct SkillTool {
    loader: Arc<SkillLoader>,
}

impl SkillTool {
    pub fn new(loader: Arc<SkillLoader>) -> Self {
        Self { loader }
    }

    /// Format a skill as tool output.
    fn format_skill_output(&self, def: &crate::def::SkillDef) -> String {
        let mut output = String::new();
        output.push_str(&format!("=== SKILL: {} (v{}) ===\n", def.name, def.version));

        let root_path = match &def.scope {
            crate::def::SkillScope::User => {
                dirs::home_dir().map(|p| p.join(".agents").join("skills").join(&def.name))
            }
            crate::def::SkillScope::Repo => None,
            crate::def::SkillScope::Custom(path) => Some(path.join(&def.name)),
        };

        if let Some(ref root) = root_path {
            output.push_str(&format!("Skill root: {}\n", root.display()));
        }

        if !def.file_listing.is_empty() {
            output.push_str("\nContents:\n");
            for file in &def.file_listing {
                output.push_str(&format!("  {}\n", file));
            }
            output.push_str("\nUse the `read` tool with absolute paths to access these files.\n");
        }

        output.push_str("\n---\n");
        output.push_str(&def.content);
        output.push_str("\n---\n");
        output.push_str("=== END SKILL ===");
        output
    }

    /// Format error with available skills list.
    async fn format_not_found(&self, name: &str) -> String {
        let metadata = self.loader.list_metadata().await;
        let mut output = format!("Skill '{}' not found.\n\n", name);
        if metadata.is_empty() {
            output.push_str("No skills available.");
        } else {
            output.push_str("Available skills:\n");
            for m in &metadata {
                output.push_str(&format!("- {}: {}\n", m.name, m.description));
            }
        }
        output.push_str("\nUse the `read` tool with absolute paths to access files relative to the skill root.");
        output
    }
}

#[async_trait]
impl ExecutableTool for SkillTool {
    fn name(&self) -> &'static str {
        "skill"
    }

    fn description(&self) -> &'static str {
        "Load a skill's full instructions by name. \
         Use the 'read' tool with absolute paths to access files relative to the skill root. \
         Available skills are listed in the system prompt."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to load"
                }
            },
            "required": ["name"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: SkillToolParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        match self.loader.get(&params.name).await {
            Some(def) => {
                let content = self.format_skill_output(&def);
                Ok(ToolResult::success(content))
            }
            None => {
                let error_msg = self.format_not_found(&params.name).await;
                Err(ToolError::ExecutionFailed(error_msg))
            }
        }
    }
}
