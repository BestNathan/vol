use crate::state::{WorkspaceTree, WorkspaceEntry};
use std::fs;
use std::path::Path;

/// Scan a directory and build a WorkspaceTree.
///
/// Ignores hidden files/directories (starting with '.') and common
/// non-source directories (.git, node_modules, target, .cargo).
pub fn scan_workspace(root: &str) -> WorkspaceTree {
    let path = Path::new(root);
    let mut entries = Vec::new();

    if !path.is_dir() {
        return WorkspaceTree {
            root: root.to_string(),
            entries,
        };
    }

    scan_dir(path, path, 0, &mut entries);

    WorkspaceTree {
        root: root.to_string(),
        entries,
    }
}

fn scan_dir(base: &Path, dir: &Path, indent: usize, entries: &mut Vec<WorkspaceEntry>) {
    let ignored = [".git", "node_modules", "target", ".cargo", "__pycache__", ".venv"];

    let Ok(read_dir) = fs::read_dir(dir) else { return };

    let mut dir_entries: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            !name_str.starts_with('.')
        })
        .collect();

    // Sort: directories first, then by name
    dir_entries.sort_by_key(|e| {
        let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let name = e.file_name();
        (!is_dir, name)
    });

    for entry in dir_entries {
        let Ok(file_type) = entry.file_type() else { continue };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if file_type.is_dir() {
            if ignored.contains(&name_str.as_ref()) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(base)
                .unwrap_or(entry.path().as_path())
                .to_string_lossy()
                .to_string();
            entries.push(WorkspaceEntry {
                path: rel.clone(),
                is_dir: true,
                modified: false,
                indent,
            });
            scan_dir(base, &entry.path(), indent + 1, entries);
        } else {
            let rel = entry
                .path()
                .strip_prefix(base)
                .unwrap_or(entry.path().as_path())
                .to_string_lossy()
                .to_string();
            entries.push(WorkspaceEntry {
                path: rel,
                is_dir: false,
                modified: false,
                indent,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_nonexistent_directory() {
        let tree = scan_workspace("/tmp/vol-llm-ui-test-nonexistent-12345");
        assert_eq!(tree.root, "/tmp/vol-llm-ui-test-nonexistent-12345");
        assert!(tree.entries.is_empty());
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = std::env::temp_dir().join("vol-llm-ui-scan-test");
        let _ = fs::create_dir_all(&dir);
        let tree = scan_workspace(dir.to_str().unwrap());
        assert!(tree.entries.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_with_files_and_dirs() {
        let dir = std::env::temp_dir().join("vol-llm-ui-scan-test-2");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let _ = fs::create_dir(dir.join("src"));
        let _ = fs::write(dir.join("README.md"), "hello");
        let _ = fs::write(dir.join("src").join("main.rs"), "fn main() {}");
        // Hidden file should be ignored
        let _ = fs::write(dir.join(".hidden"), "secret");
        // .git directory should be ignored
        let _ = fs::create_dir(dir.join(".git"));
        // target directory should be ignored
        let _ = fs::create_dir(dir.join("target"));

        let tree = scan_workspace(dir.to_str().unwrap());

        // Should have README.md and src (and src/main.rs)
        let names: Vec<_> = tree.entries.iter().map(|e| e.path.as_str()).collect();
        assert!(names.contains(&"README.md"));
        assert!(names.contains(&"src"));
        assert!(names.contains(&"src/main.rs"));

        // Verify hidden/target entries are NOT present
        for entry in &tree.entries {
            assert!(!entry.path.contains(".git"));
            assert!(!entry.path.contains("target"));
            assert!(!entry.path.contains(".hidden"));
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
