//! LLM tools for task management.

mod task_claim;
mod task_cli;
mod task_create;
mod task_get;
mod task_list;
mod task_output;
mod task_stop;
mod task_update;

pub use task_claim::TaskClaim;
pub use task_cli::TaskCliTool;
pub use task_create::TaskCreate;
pub use task_get::TaskGet;
pub use task_list::TaskList;
pub use task_output::TaskOutput;
pub use task_stop::TaskStop;
pub use task_update::TaskUpdate;

use std::sync::Arc;

use crate::store::TaskStore;

/// Register all task management tools to a ToolRegistry.
pub fn register_all(registry: &mut vol_llm_tool::ToolRegistry, store: Arc<dyn TaskStore>) {
    registry.register(TaskCreate::new(store.clone()));
    registry.register(TaskGet::new(store.clone()));
    registry.register(TaskList::new(store.clone()));
    registry.register(TaskOutput::new(store.clone()));
    registry.register(TaskStop::new(store.clone()));
    registry.register(TaskUpdate::new(store.clone()));
    registry.register(TaskClaim::new(store));
}

/// Register the CLI-style task tool (mutually exclusive with register_all).
pub fn register_cli(registry: &mut vol_llm_tool::ToolRegistry, store: Arc<dyn TaskStore>) {
    registry.register(TaskCliTool::new(store));
}
