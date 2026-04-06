//! vol-llm-tool: Tool framework for LLM Agent.

pub mod tool;
pub mod registry;
pub mod tools;
pub mod tdengine;

pub use tool::{Tool, ExecutableTool, ToolContext, ToolResult, ToolError, ToolResultType, Result};
pub use registry::ToolRegistry;
pub use tools::*;
pub use tdengine::{TdengineClient, TdengineConfig};
