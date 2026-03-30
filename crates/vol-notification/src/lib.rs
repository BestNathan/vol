//! vol-notification: Notification handler implementations.
//!
//! Includes:
//! - Feishu/Lark webhook notification
//! - Stdout notification (for testing)

mod feishu;
mod stdout;

pub use feishu::FeishuNotification;
pub use stdout::StdoutNotification;
