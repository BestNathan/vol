//! Plugin registry for managing handlers.

use vol_core::{RuleProcessor, NotificationHandler};

/// Registry for rule processors
#[allow(dead_code)]
pub struct RuleRegistry {
    rules: Vec<Box<dyn RuleProcessor>>,
}

#[allow(dead_code)]
impl RuleRegistry {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
        }
    }

    pub fn register(&mut self, rule: Box<dyn RuleProcessor>) {
        self.rules.push(rule);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn RuleProcessor>> {
        self.rules.iter()
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry for notification handlers
#[allow(dead_code)]
pub struct NotificationRegistry {
    handlers: Vec<Box<dyn NotificationHandler>>,
}

#[allow(dead_code)]
impl NotificationRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn register(&mut self, handler: Box<dyn NotificationHandler>) {
        self.handlers.push(handler);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn NotificationHandler>> {
        self.handlers.iter()
    }
}

impl Default for NotificationRegistry {
    fn default() -> Self {
        Self::new()
    }
}
