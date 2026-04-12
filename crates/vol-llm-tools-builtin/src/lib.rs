//! vol-llm-tools-builtin: Built-in tools for LLM Agent.
//!
//! Each tool is a separate sub-crate for optional dependencies.
//! Use `register_all()` to register all tools at once.

pub mod read_tool {
    pub use vol_llm_tools_builtin_read::*;
}

pub mod write_tool {
    pub use vol_llm_tools_builtin_write::*;
}

pub mod edit_tool {
    pub use vol_llm_tools_builtin_edit::*;
}

pub mod glob_tool {
    pub use vol_llm_tools_builtin_glob::*;
}

pub mod grep_tool {
    pub use vol_llm_tools_builtin_grep::*;
}

pub mod bash_tool {
    pub use vol_llm_tools_builtin_bash::*;
}

// Re-export all tools for convenience
pub use read_tool::ReadTool;
pub use write_tool::WriteTool;
pub use edit_tool::EditTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use bash_tool::BashTool;

// Re-export error type
pub use read_tool::BuiltinToolError;

/// Register all built-in tools to a ToolRegistry
pub fn register_all(registry: &mut vol_llm_tool::ToolRegistry) {
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GlobTool::new());
    registry.register(GrepTool::new());
    registry.register(BashTool::new());
}
