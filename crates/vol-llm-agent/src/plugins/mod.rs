//! Built-in plugins for ReAct Agent.

pub mod hitl_cli;
pub mod hitl_http;

pub use hitl_cli::CliApprovalChannel;
pub use hitl_http::{HttpApprovalChannel, SimpleHttpApprovalChannel};
