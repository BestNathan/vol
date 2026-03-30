//! State persistence for alert cooldown tracking.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use vol_core::VolError;

/// Load state from JSON file
pub fn load_state(path: &str) -> Result<HashMap<String, u64>, VolError> {
    let expanded = expand_path(path);

    if !Path::new(&expanded).exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(&expanded)
        .map_err(|e| VolError::Internal(format!("Failed to read state file: {}", e)))?;

    serde_json::from_str(&content)
        .map_err(|e| VolError::Internal(format!("Failed to parse state file: {}", e)))
}

/// Save state to JSON file
pub fn save_state(path: &str, state: &HashMap<String, u64>) -> Result<(), VolError> {
    let expanded = expand_path(path);

    // Create parent directory if it doesn't exist
    if let Some(parent) = Path::new(&expanded).parent() {
        fs::create_dir_all(parent)
            .map_err(|e| VolError::Internal(format!("Failed to create state directory: {}", e)))?;
    }

    let content = serde_json::to_string_pretty(state)
        .map_err(|e| VolError::Internal(format!("Failed to serialize state: {}", e)))?;

    fs::write(&expanded, content)
        .map_err(|e| VolError::Internal(format!("Failed to write state file: {}", e)))?;

    Ok(())
}

/// Expand ~ to home directory
fn expand_path(path: &str) -> String {
    if path.starts_with("~") {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen("~", &home, 1);
        }
    }
    path.to_string()
}
