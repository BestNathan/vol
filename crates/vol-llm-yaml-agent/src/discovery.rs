//! Discover YAML agent files from a directory.

use std::path::{Path, PathBuf};
use crate::error::YamlAgentError;

/// Find all .yaml files in the given directory.
pub fn discover_agents(dir: &Path) -> Result<Vec<PathBuf>, YamlAgentError> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let entries = std::fs::read_dir(dir)
        .map_err(YamlAgentError::Io)?;

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("yaml") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Discover agents from the standard `.agent/agents/` directory.
pub fn discover_from_workdir(working_dir: &Path) -> Result<Vec<PathBuf>, YamlAgentError> {
    let agents_dir = working_dir.join(".agent").join("agents");
    discover_agents(&agents_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_empty_dir() {
        let temp = tempfile::tempdir().unwrap();
        let files = discover_agents(temp.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_yaml_files() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("agent1.yaml"), "name: a\nllm: x\n").unwrap();
        std::fs::write(temp.path().join("agent2.yaml"), "name: b\nllm: y\n").unwrap();
        std::fs::write(temp.path().join("readme.md"), "not yaml").unwrap();

        let files = discover_agents(temp.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("agent1.yaml"));
        assert!(files[1].ends_with("agent2.yaml"));
    }

    #[test]
    fn test_discover_nonexistent_dir() {
        let files = discover_agents(Path::new("/nonexistent")).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_from_workdir() {
        let temp = tempfile::tempdir().unwrap();
        let agents_dir = temp.path().join(".agent").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("coding.yaml"), "name: c\nllm: p\n").unwrap();

        let files = discover_from_workdir(temp.path()).unwrap();
        assert_eq!(files.len(), 1);
    }
}
