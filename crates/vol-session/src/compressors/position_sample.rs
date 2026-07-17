//! Position-based sampling compressor.
//!
//! Keeps first N messages (always preserve session start),
//! then samples every M-th message from the rest.

use crate::compressor::MessageCompressor;
use crate::message::SessionMessage;

/// Compressor that samples messages by position.
pub struct PositionSampleCompressor {
    /// Number of messages to keep from the start.
    keep_first: usize,
    /// Sample interval for remaining messages.
    sample_every: usize,
}

impl PositionSampleCompressor {
    pub fn new(keep_first: usize, sample_every: usize) -> Self {
        Self {
            keep_first: keep_first.max(1),
            sample_every: sample_every.max(1),
        }
    }
}

impl Default for PositionSampleCompressor {
    fn default() -> Self {
        Self::new(3, 5)
    }
}

#[async_trait::async_trait]
impl MessageCompressor for PositionSampleCompressor {
    async fn compress(&self, messages: Vec<SessionMessage>) -> Vec<SessionMessage> {
        if messages.is_empty() {
            return vec![];
        }

        let mut result = Vec::new();

        // Keep first N messages
        let keep = self.keep_first.min(messages.len());
        result.extend(messages.get(..keep).unwrap_or(&[]).iter().cloned());

        // Sample every M-th from the rest
        for (i, msg) in messages.get(keep..).unwrap_or(&[]).iter().enumerate() {
            if i % self.sample_every == 0 {
                result.push(msg.clone());
            }
        }

        // Always include the last message if not already included
        if let Some(last) = messages.last() {
            if result.last().map(|m| m.id != last.id).unwrap_or(true) {
                result.push(last.clone());
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    fn make_msg(id: &str) -> SessionMessage {
        SessionMessage::new("test".to_string(), Message::user(id))
    }

    #[tokio::test]
    async fn test_empty_input() {
        let compressor = PositionSampleCompressor::default();
        let result = compressor.compress(vec![]).await;
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_small_input() {
        let compressor = PositionSampleCompressor::new(3, 5);
        let messages = vec![make_msg("1"), make_msg("2")];
        let result = compressor.compress(messages).await;
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_sampling() {
        let compressor = PositionSampleCompressor::new(2, 3);
        let messages: Vec<_> = (1..=10).map(|i| make_msg(&i.to_string())).collect();
        let result = compressor.compress(messages).await;
        // Keep first 2: [1, 2]
        // Sample every 3rd from rest [3..10]: indices 0,3,6 → [3, 6, 9]
        // Last message 10 already included if not sampled
        // Expected: [1, 2, 3, 6, 9, 10]
        assert_eq!(result.len(), 6);
        assert_eq!(result[0].message.content.as_ref().unwrap().as_str(), "1");
        assert_eq!(
            result
                .last()
                .unwrap()
                .message
                .content
                .as_ref()
                .unwrap()
                .as_str(),
            "10"
        );
    }
}
