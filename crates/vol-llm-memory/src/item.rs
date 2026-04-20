use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Kind of memory item, categorizing what type of experience it represents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryKind {
    /// User preferences, communication style, code style
    UserPreference,
    /// Project architecture, tech stack, constraints
    ProjectFact,
    /// Tool success/failure patterns, gotchas, tips
    Experience,
    /// Past session summaries and key decisions
    ConversationSummary,
}

impl std::fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryKind::UserPreference => write!(f, "UserPreference"),
            MemoryKind::ProjectFact => write!(f, "ProjectFact"),
            MemoryKind::Experience => write!(f, "Experience"),
            MemoryKind::ConversationSummary => write!(f, "ConversationSummary"),
        }
    }
}

/// Atomic unit of memory — a single piece of information the agent has accumulated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub kind: MemoryKind,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub importance: f32,
}

impl MemoryItem {
    pub fn new(kind: MemoryKind, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            content: content.into(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            importance: 0.5,
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }
}

/// Composable filter for list/remove operations.
#[derive(Debug, Clone, Default)]
pub struct MemoryFilter {
    pub kinds: Option<Vec<MemoryKind>>,
    pub tags: Option<Vec<String>>,
    pub created_before: Option<DateTime<Utc>>,
    pub created_after: Option<DateTime<Utc>>,
    pub min_importance: Option<f32>,
}

impl MemoryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kinds(mut self, kinds: Vec<MemoryKind>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn created_after(mut self, dt: DateTime<Utc>) -> Self {
        self.created_after = Some(dt);
        self
    }

    pub fn created_before(mut self, dt: DateTime<Utc>) -> Self {
        self.created_before = Some(dt);
        self
    }

    pub fn min_importance(mut self, min: f32) -> Self {
        self.min_importance = Some(min);
        self
    }

    pub fn matches(&self, item: &MemoryItem) -> bool {
        if let Some(ref kinds) = self.kinds {
            if !kinds.contains(&item.kind) {
                return false;
            }
        }
        if let Some(ref filter_tags) = self.tags {
            if filter_tags.is_empty() || item.tags.is_empty() {
                return false;
            }
            if !filter_tags.iter().any(|t| item.tags.contains(t)) {
                return false;
            }
        }
        if let Some(ref before) = self.created_before {
            if item.created_at >= *before {
                return false;
            }
        }
        if let Some(ref after) = self.created_after {
            if item.created_at <= *after {
                return false;
            }
        }
        if let Some(min_imp) = self.min_importance {
            if item.importance < min_imp {
                return false;
            }
        }
        true
    }
}
