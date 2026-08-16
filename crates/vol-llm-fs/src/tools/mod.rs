//! LLM tools for filesystem operations.

mod fs_cli;

pub use fs_cli::FsCliTool;

/// Register the CLI-style `fs` tool to a ToolRegistry.
pub fn register_cli(registry: &mut vol_llm_tool::ToolRegistry) {
    registry.register(FsCliTool::new());
}
