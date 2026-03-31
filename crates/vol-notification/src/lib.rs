//! vol-notification: Notification handler implementations.
//!
//! Includes:
//! - Feishu/Lark webhook notification
//! - Stdout notification (for testing)
//! - Portfolio JSONL output

mod feishu;
mod portfolio_output;
mod stdout;

pub use feishu::FeishuNotification;
pub use portfolio_output::PortfolioOutput;
pub use stdout::StdoutNotification;
