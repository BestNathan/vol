//! Message compression trait.

use crate::message::SessionMessage;

/// Abstract message compression: input messages, output fewer messages.
#[async_trait::async_trait]
pub trait MessageCompressor: Send + Sync {
    /// Compress a set of messages into a smaller set.
    /// Input: the messages that SessionContributor just contributed
    ///        (i.e., what get_messages(limit) returned).
    /// Output: a smaller set of "精华" messages to keep in context.
    async fn compress(&self, messages: Vec<SessionMessage>) -> Vec<SessionMessage>;
}
