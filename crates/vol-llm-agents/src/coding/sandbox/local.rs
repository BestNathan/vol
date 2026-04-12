use std::path::PathBuf;
use vol_llm_core::{Sandbox, SandboxError, SandboxResult};

/// A sandbox using a local directory as its root.
///
/// If created with `Some(path)`, the directory is caller-owned and NOT deleted on cleanup.
/// If created with `None`, a temp directory is created and IS deleted on cleanup.
pub struct LocalSandbox {
    root_path: PathBuf,
    is_temp: bool,
}

impl LocalSandbox {
    pub fn new(path: Option<PathBuf>) -> Self {
        let (root_path, is_temp) = match path {
            Some(p) => (p, false),
            None => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let temp = std::env::temp_dir().join(format!("sandbox_{:x}", timestamp % 0xFFFFFF));
                (temp, true)
            }
        };
        Self { root_path, is_temp }
    }
}

impl Sandbox for LocalSandbox {
    fn kind(&self) -> &str {
        "local"
    }

    fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(&self.root_path).map_err(SandboxError::Io)
    }

    fn cleanup(&self) -> SandboxResult<()> {
        if self.is_temp {
            std::fs::remove_dir_all(&self.root_path).map_err(SandboxError::Io)?;
        }
        Ok(())
    }

    fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        // Reject absolute paths — they escape the sandbox
        if rel.starts_with('/') {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }

        let resolved = self.root_path.join(rel);
        let normalized = normalize_path(&resolved);
        let canonical_root = self
            .root_path
            .canonicalize()
            .map(|p| normalize_path(&p))
            .unwrap_or_else(|_| normalize_path(&self.root_path));

        if !normalized.starts_with(&canonical_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }

        Ok(normalized)
    }
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &std::path::Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            _ => result.push(component),
        }
    }
    result
}
