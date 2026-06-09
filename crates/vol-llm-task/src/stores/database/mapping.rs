//! Mapping between task models and SeaORM rows.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sea_orm::ActiveValue::{NotSet, Set};

use crate::model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
use crate::store::{Result, StoreError};

use super::entity;

pub(super) fn task_id_to_db(id: TaskId) -> Result<i64> {
    i64::try_from(id.0).map_err(|_| {
        StoreError::Serialization(format!("task id exceeds database i64 range: {}", id.0))
    })
}

pub(super) fn task_id_from_db(id: i64) -> Result<TaskId> {
    let id = u64::try_from(id)
        .map_err(|_| StoreError::Serialization(format!("negative task id: {id}")))?;
    Ok(TaskId(id))
}

pub(super) fn status_to_db(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Killed => "killed",
    }
}

fn status_from_db(value: &str) -> Result<TaskStatus> {
    match value {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "killed" => Ok(TaskStatus::Killed),
        other => Err(StoreError::Serialization(format!(
            "unknown task status: {other}"
        ))),
    }
}

fn kind_to_db(kind: &TaskKind) -> &'static str {
    match kind {
        TaskKind::Agent => "agent",
        TaskKind::Manual => "manual",
    }
}

fn kind_from_db(value: &str) -> Result<TaskKind> {
    match value {
        "agent" => Ok(TaskKind::Agent),
        "manual" => Ok(TaskKind::Manual),
        other => Err(StoreError::Serialization(format!(
            "unknown task kind: {other}"
        ))),
    }
}

fn system_time_to_secs(value: SystemTime) -> Result<i64> {
    let duration = value.duration_since(UNIX_EPOCH).map_err(|e| {
        StoreError::Serialization(format!("task time is before unix epoch: {e}"))
    })?;
    i64::try_from(duration.as_secs()).map_err(|_| {
        StoreError::Serialization(format!(
            "task timestamp exceeds database i64 range: {}",
            duration.as_secs()
        ))
    })
}

fn secs_to_system_time(value: i64) -> Result<SystemTime> {
    let secs = u64::try_from(value)
        .map_err(|_| StoreError::Serialization(format!("negative task timestamp: {value}")))?;
    Ok(UNIX_EPOCH + Duration::from_secs(secs))
}

fn option_time_to_secs(value: Option<SystemTime>) -> Result<Option<i64>> {
    value.map(system_time_to_secs).transpose()
}

fn option_secs_to_time(value: Option<i64>) -> Result<Option<SystemTime>> {
    value.map(secs_to_system_time).transpose()
}

fn task_ids_to_json(ids: &[TaskId]) -> Result<String> {
    serde_json::to_string(ids).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn task_ids_from_json(value: &str) -> Result<Vec<TaskId>> {
    serde_json::from_str(value).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn result_to_json(result: &Option<TaskResult>) -> Result<Option<String>> {
    result
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| StoreError::Serialization(e.to_string()))
}

fn result_from_json(value: Option<String>) -> Result<Option<TaskResult>> {
    value
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|e| StoreError::Serialization(e.to_string()))
}

fn path_to_db(path: &Option<PathBuf>) -> Option<String> {
    path.as_ref().map(|p| p.to_string_lossy().to_string())
}

fn path_from_db(value: Option<String>) -> Option<PathBuf> {
    value.map(PathBuf::from)
}

pub(super) fn task_to_active_model(task: Task) -> Result<entity::ActiveModel> {
    Ok(entity::ActiveModel {
        id: if task.id.0 == 0 {
            NotSet
        } else {
            Set(task_id_to_db(task.id)?)
        },
        status: Set(status_to_db(task.status).to_string()),
        kind: Set(kind_to_db(&task.kind).to_string()),
        publisher: Set(task.publisher),
        assignee: Set(task.assignee),
        subject: Set(task.subject),
        description: Set(task.description),
        active_form: Set(task.active_form),
        dependencies_json: Set(task_ids_to_json(&task.dependencies)?),
        blocks_json: Set(task_ids_to_json(&task.blocks)?),
        result_json: Set(result_to_json(&task.result)?),
        summary: Set(task.summary),
        output_file: Set(path_to_db(&task.output_file)),
        created_at_secs: Set(system_time_to_secs(task.created_at)?),
        started_at_secs: Set(option_time_to_secs(task.started_at)?),
        completed_at_secs: Set(option_time_to_secs(task.completed_at)?),
    })
}

pub(super) fn model_to_task(model: entity::Model) -> Result<Task> {
    Ok(Task {
        id: task_id_from_db(model.id)?,
        status: status_from_db(&model.status)?,
        kind: kind_from_db(&model.kind)?,
        publisher: model.publisher,
        assignee: model.assignee,
        subject: model.subject,
        description: model.description,
        active_form: model.active_form,
        dependencies: task_ids_from_json(&model.dependencies_json)?,
        blocks: task_ids_from_json(&model.blocks_json)?,
        result: result_from_json(model.result_json)?,
        summary: model.summary,
        output_file: path_from_db(model.output_file),
        created_at: secs_to_system_time(model.created_at_secs)?,
        started_at: option_secs_to_time(model.started_at_secs)?,
        completed_at: option_secs_to_time(model.completed_at_secs)?,
    })
}
