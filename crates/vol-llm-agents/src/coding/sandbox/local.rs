use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use vol_llm_core::{Sandbox, SandboxError, SandboxResult};

/// A sandbox using a local directory as its root.
///
/// If created with `Some(path)` and the directory already exists, it is caller-owned
/// and NOT deleted on cleanup. If the directory is created by `start()`, it WILL be
/// deleted on cleanup. If created with `None`, a temp directory is created and IS
/// deleted on cleanup.
pub struct LocalSandbox {
    root_path: PathBuf,
    created_by_start: AtomicBool,
}

impl LocalSandbox {
    pub fn new(path: Option<PathBuf>) -> Self {
        let root_path = match path {
            Some(p) => p,
            None => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                std::env::temp_dir().join(format!("sandbox_{:x}", timestamp % 0xFFFFFF))
            }
        };
        Self {
            root_path,
            created_by_start: AtomicBool::new(false),
        }
    }
}

impl Sandbox for LocalSandbox {
    fn kind(&self) -> &str {
        "local"
    }

    fn start(&self) -> SandboxResult<()> {
        let existed = self.root_path.exists();
        std::fs::create_dir_all(&self.root_path).map_err(SandboxError::Io)?;
        if !existed {
            self.created_by_start.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    fn cleanup(&self) -> SandboxResult<()> {
        if self.created_by_start.load(Ordering::Relaxed) {
            std::fs::remove_dir_all(&self.root_path).map_err(SandboxError::Io)?;
        }
        Ok(())
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') {
            return Ok(PathBuf::from(rel));
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

        Ok(resolved)
    }
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
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
