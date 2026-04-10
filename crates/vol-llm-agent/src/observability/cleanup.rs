//! Log cleanup utilities for retention policy enforcement.

use std::path::Path;
use std::fs;
use chrono::{Utc, Duration, NaiveDate};
use regex::Regex;
use tracing;

pub async fn cleanup_old_logs(agent_path: &Path) -> Result<(), LogError> {
    let sessions_path = agent_path.join("sessions");
    let runs_path = agent_path.join("runs");

    // Clean session logs older than 7 days
    match cleanup_session_logs(&sessions_path, 7).await {
        Ok(count) => tracing::debug!(path = %sessions_path.display(), count, "Cleaned up old session logs"),
        Err(e) => tracing::warn!(path = %sessions_path.display(), error = %e, "Failed to cleanup session logs"),
    }

    // Keep only last 10 run logs
    match cleanup_run_logs(&runs_path, 10).await {
        Ok(count) => tracing::debug!(path = %runs_path.display(), count, "Cleaned up excess run logs"),
        Err(e) => tracing::warn!(path = %runs_path.display(), error = %e, "Failed to cleanup run logs"),
    }

    Ok(())
}

pub async fn cleanup_session_logs(sessions_path: &Path, retention_days: u32) -> Result<usize, LogError> {
    if !sessions_path.exists() {
        return Ok(0);
    }

    let cutoff_date = Utc::now().date_naive() - Duration::days(retention_days as i64);
    let session_pattern = Regex::new(r"session_(.+)_([0-9]{8})\.jsonl")
        .map_err(|e| LogError::Parse(format!("Invalid regex: {}", e)))?;

    let mut deleted_count = 0;

    for entry in fs::read_dir(sessions_path)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();

        if let Some(captures) = session_pattern.captures(&filename) {
            if let Some(date_str) = captures.get(2) {
                if let Ok(file_date) = NaiveDate::parse_from_str(date_str.as_str(), "%Y%m%d") {
                    if file_date < cutoff_date {
                        fs::remove_file(entry.path())?;
                        deleted_count += 1;
                    }
                }
            }
        }
    }

    Ok(deleted_count)
}

pub async fn cleanup_run_logs(runs_path: &Path, max_runs: usize) -> Result<usize, LogError> {
    if !runs_path.exists() {
        return Ok(0);
    }

    let mut run_files: Vec<_> = fs::read_dir(runs_path)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("run_")
        })
        .collect();

    // Sort by filename (run_id includes timestamp, so alphabetical = chronological)
    run_files.sort_by(|a, b| {
        let a_name = a.file_name().to_string_lossy().to_string();
        let b_name = b.file_name().to_string_lossy().to_string();
        a_name.cmp(&b_name)
    });

    // Delete oldest files if over limit
    let mut deleted_count = 0;
    while run_files.len() - deleted_count > max_runs {
        let file_to_delete = run_files[deleted_count].path();
        fs::remove_file(&file_to_delete)?;
        deleted_count += 1;
    }

    Ok(deleted_count)
}

#[derive(Debug)]
pub enum LogError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogError::Io(e) => write!(f, "IO error: {}", e),
            LogError::Parse(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl From<std::io::Error> for LogError {
    fn from(err: std::io::Error) -> Self {
        LogError::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cleanup_session_logs_removes_old_files() {
        let temp_dir = TempDir::new().unwrap();
        let sessions_path = temp_dir.path().join("sessions");
        fs::create_dir_all(&sessions_path).unwrap();

        // Create old session log (10 days ago)
        let old_date = (Utc::now().date_naive() - Duration::days(10)).format("%Y%m%d");
        let old_file = sessions_path.join(format!("session_abc_{}.jsonl", old_date));
        fs::write(&old_file, "old log").unwrap();

        // Create recent session log (2 days ago)
        let recent_date = (Utc::now().date_naive() - Duration::days(2)).format("%Y%m%d");
        let recent_file = sessions_path.join(format!("session_xyz_{}.jsonl", recent_date));
        fs::write(&recent_file, "recent log").unwrap();

        // Cleanup should remove only old file
        let deleted = cleanup_session_logs(&sessions_path, 7).await.unwrap();
        assert_eq!(deleted, 1);
        assert!(!old_file.exists());
        assert!(recent_file.exists());
    }

    #[tokio::test]
    async fn test_cleanup_run_logs_keeps_last_n() {
        let temp_dir = TempDir::new().unwrap();
        let runs_path = temp_dir.path().join("runs");
        fs::create_dir_all(&runs_path).unwrap();

        // Create 15 run logs
        for i in 0..15 {
            let file = runs_path.join(format!("run_{:03}.jsonl", i));
            fs::write(&file, format!("log {}", i)).unwrap();
        }

        // Cleanup should keep only last 10
        let deleted = cleanup_run_logs(&runs_path, 10).await.unwrap();
        assert_eq!(deleted, 5);

        // First 5 should be deleted, last 10 should remain
        assert!(!runs_path.join("run_000.jsonl").exists());
        assert!(runs_path.join("run_005.jsonl").exists());
        assert!(runs_path.join("run_014.jsonl").exists());
    }
}
