use vol_llm_core::Message;

/// Attention zone with position value for sorting.
/// Lower position = closer to the zone boundary.
/// Zone priority: Head(0) > Middle(1) > Tail(2) — used for drop ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttentionAnchor {
    Head(u32),
    Middle(u32),
    Tail(u32),
}

impl AttentionAnchor {
    /// Zone priority for drop ordering. Head is most important (0), Tail least (2).
    pub fn zone_priority(&self) -> u8 {
        match self {
            AttentionAnchor::Head(_) => 0,
            AttentionAnchor::Middle(_) => 1,
            AttentionAnchor::Tail(_) => 2,
        }
    }

    /// Position within the zone.
    pub fn position(&self) -> u32 {
        match self {
            AttentionAnchor::Head(p) | AttentionAnchor::Middle(p) | AttentionAnchor::Tail(p) => *p,
        }
    }
}

impl PartialOrd for AttentionAnchor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AttentionAnchor {
    /// Higher priority = should be kept (not dropped).
    /// Head > Middle > Tail; within same zone, lower position = higher priority.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let self_priority = (self.zone_priority(), self.position());
        let other_priority = (other.zone_priority(), other.position());
        // Lower zone_priority = more important; lower position = more important
        self_priority.cmp(&other_priority).reverse()
    }
}

/// A unit of context — contributor-formatted messages with an attention anchor.
#[derive(Debug, Clone)]
pub struct ContextBlock {
    pub messages: Vec<Message>,
    pub anchor: AttentionAnchor,
}

impl ContextBlock {
    pub fn new(messages: Vec<Message>, anchor: AttentionAnchor) -> Self {
        Self { messages, anchor }
    }
}

/// Token budget tracker for compression decisions.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub total: usize,
    pub head_size: usize,
    pub tail_size: usize,
    pub used: usize,
}

impl TokenBudget {
    pub fn new(total: usize, head_size: usize, tail_size: usize) -> Self {
        Self {
            total,
            head_size,
            tail_size,
            used: 0,
        }
    }

    pub fn with_used(mut self, used: usize) -> Self {
        self.used = used;
        self
    }

    /// Available tokens for middle-zone blocks.
    pub fn middle_budget(&self) -> usize {
        self.total
            .saturating_sub(self.head_size)
            .saturating_sub(self.tail_size)
    }

    /// Remaining tokens in the middle budget.
    pub fn remaining(&self) -> usize {
        self.middle_budget().saturating_sub(self.used)
    }

    /// Whether the middle budget is exceeded.
    pub fn is_exceeded(&self) -> bool {
        self.used > self.middle_budget()
    }
}

/// Estimate token count for a message.
/// Uses JSON length / 4 as a rough approximation.
pub fn estimate_tokens(msg: &Message) -> usize {
    let json = serde_json::to_string(msg).unwrap_or_default();
    json.len() / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_anchor_ordering() {
        // Head(0) > Head(1) > Middle(0) > Middle(10) > Tail(0)
        let anchors = vec![
            AttentionAnchor::Tail(0),
            AttentionAnchor::Middle(10),
            AttentionAnchor::Head(1),
            AttentionAnchor::Head(0),
            AttentionAnchor::Middle(0),
        ];
        let mut sorted = anchors.clone();
        sorted.sort();
        sorted.reverse(); // highest priority first
        assert_eq!(sorted[0], AttentionAnchor::Head(0));
        assert_eq!(sorted[1], AttentionAnchor::Head(1));
        assert_eq!(sorted[2], AttentionAnchor::Middle(0));
        assert_eq!(sorted[3], AttentionAnchor::Middle(10));
        assert_eq!(sorted[4], AttentionAnchor::Tail(0));
    }

    #[test]
    fn test_token_budget() {
        let budget = TokenBudget::new(1000, 200, 100);
        assert_eq!(budget.middle_budget(), 700);
        assert!(!budget.clone().with_used(500).is_exceeded());
        assert!(budget.clone().with_used(800).is_exceeded());
        assert_eq!(budget.clone().with_used(600).remaining(), 100);
    }

    #[test]
    fn test_estimate_tokens() {
        let msg = Message::user("Hello world");
        let tokens = estimate_tokens(&msg);
        assert!(tokens > 0);
    }
}
