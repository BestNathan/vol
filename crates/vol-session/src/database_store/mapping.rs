use sea_orm::ActiveValue;

use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};
use crate::manager::SessionInfo;
use crate::store::{Result, StoreError};

use super::entity::{session_entries, sessions};

pub fn entry_type_to_db(entry_type: &SessionEntryType) -> &'static str {
    match entry_type {
        SessionEntryType::Message => "message",
        SessionEntryType::Checkpoint => "checkpoint",
        SessionEntryType::Summary => "summary",
    }
}

pub fn entry_type_from_db(value: &str) -> Result<SessionEntryType> {
    match value {
        "message" => Ok(SessionEntryType::Message),
        "checkpoint" => Ok(SessionEntryType::Checkpoint),
        "summary" => Ok(SessionEntryType::Summary),
        other => Err(StoreError::Serialization(format!(
            "unknown session entry type from database: {other}"
        ))),
    }
}

pub fn entry_to_active_model(entry: SessionEntry) -> Result<session_entries::ActiveModel> {
    let data = serde_json::to_string(&entry.data).map_err(|e| {
        StoreError::Serialization(format!("failed to serialize session entry data: {e}"))
    })?;

    Ok(session_entries::ActiveModel {
        id: ActiveValue::Set(entry.id),
        session_id: ActiveValue::Set(entry.session_id),
        created_at: ActiveValue::Set(entry.created_at),
        parent_id: ActiveValue::Set(entry.parent_id),
        entry_type: ActiveValue::Set(entry_type_to_db(&entry.r#type).to_string()),
        data: ActiveValue::Set(data),
    })
}

pub fn model_to_entry(model: session_entries::Model) -> Result<SessionEntry> {
    let data: SessionEntryData = serde_json::from_str(&model.data).map_err(|e| {
        StoreError::Serialization(format!("failed to deserialize session entry data: {e}"))
    })?;
    let entry_type = entry_type_from_db(&model.entry_type)?;

    Ok(SessionEntry {
        id: model.id,
        session_id: model.session_id,
        created_at: model.created_at,
        parent_id: model.parent_id,
        r#type: entry_type,
        data,
    })
}

pub fn session_model_to_info(model: sessions::Model) -> SessionInfo {
    SessionInfo {
        id: model.id.clone(),
        agent_id: model.agent_id,
        session_id: model.id,
        entry_count: model.entry_count.max(0) as usize,
        created_at: model.created_at,
        updated_at: Some(model.updated_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_summary_entry_round_trip() {
        let entry = SessionEntry {
            id: "entry-1".to_string(),
            session_id: "session-1".to_string(),
            created_at: 42,
            parent_id: Some("parent-1".to_string()),
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary {
                summary: "hello".to_string(),
            },
        };

        let active = entry_to_active_model(entry.clone()).unwrap();
        let model = session_entries::Model {
            id: active.id.unwrap(),
            session_id: active.session_id.unwrap(),
            created_at: active.created_at.unwrap(),
            parent_id: active.parent_id.unwrap(),
            entry_type: active.entry_type.unwrap(),
            data: active.data.unwrap(),
        };

        let mapped = model_to_entry(model).unwrap();
        assert_eq!(mapped.id, entry.id);
        assert_eq!(mapped.session_id, entry.session_id);
        assert_eq!(mapped.created_at, entry.created_at);
        assert_eq!(mapped.parent_id, entry.parent_id);
        assert_eq!(mapped.r#type, entry.r#type);
        match mapped.data {
            SessionEntryData::Summary { summary } => assert_eq!(summary, "hello"),
            _ => panic!("expected summary data"),
        }
    }

    #[test]
    fn rejects_unknown_entry_type() {
        let err = entry_type_from_db("bogus").unwrap_err();
        assert!(err.to_string().contains("unknown session entry type"));
    }
}
