//! Session entry types for multi-type session persistence.

use serde::{Deserialize, Serialize};

/// Entry type discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEntryType {
    Message,
    Checkpoint,
    Summary,
}

/// Reason a checkpoint was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointReason {
    Compression,
    Manual,
}

/// Polymorphic entry data — serialized with inline `type` discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEntryData {
    #[serde(rename = "message")]
    Message {
        message: vol_llm_core::Message,
    },
    #[serde(rename = "checkpoint")]
    Checkpoint {
        reason: CheckpointReason,
        note: Option<String>,
    },
    #[serde(rename = "summary")]
    Summary {
        summary: String,
    },
}

impl SessionEntryData {
    /// Returns the type discriminator for this data variant.
    pub fn entry_type(&self) -> SessionEntryType {
        match self {
            SessionEntryData::Message { .. } => SessionEntryType::Message,
            SessionEntryData::Checkpoint { .. } => SessionEntryType::Checkpoint,
            SessionEntryData::Summary { .. } => SessionEntryType::Summary,
        }
    }
}

/// Unified session entry — all content types stored in a single JSONL file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub session_id: String,
    pub created_at: i64,
    pub parent_id: Option<String>,
    pub r#type: SessionEntryType,
    pub data: SessionEntryData,
}

impl SessionEntry {
    /// Create a new message entry.
    pub fn new_message(session_id: String, message: vol_llm_core::Message) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            parent_id: None,
            r#type: SessionEntryType::Message,
            data: SessionEntryData::Message { message },
        }
    }

    /// Create a new checkpoint entry.
    pub fn new_checkpoint(
        session_id: String,
        reason: CheckpointReason,
        note: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            parent_id: None,
            r#type: SessionEntryType::Checkpoint,
            data: SessionEntryData::Checkpoint { reason, note },
        }
    }

    /// Create a new summary entry.
    pub fn new_summary(session_id: String, summary: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            parent_id: None,
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary { summary },
        }
    }

    /// Extract the Message from this entry, if it is a Message type.
    pub fn into_message(self) -> Option<vol_llm_core::Message> {
        match self.data {
            SessionEntryData::Message { message } => Some(message),
            _ => None,
        }
    }
}
