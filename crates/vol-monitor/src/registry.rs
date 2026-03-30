//! Plugin registry for managing handlers.

use vol_core::{AlertHandler, NotificationHandler};

/// Registry for alert handlers
#[allow(dead_code)]
pub struct AlertRegistry {
    handlers: Vec<Box<dyn AlertHandler>>,
}

#[allow(dead_code)]
impl AlertRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn register(&mut self, handler: Box<dyn AlertHandler>) {
        self.handlers.push(handler);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn AlertHandler>> {
        self.handlers.iter()
    }
}

impl Default for AlertRegistry {
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
