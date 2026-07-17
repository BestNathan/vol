use crate::state::WorkspaceTreeNode;
use std::fs;
use std::path::Path;

/// Scan a directory and build a WorkspaceTreeNode tree.
pub fn scan_workspace(root: &str) -> WorkspaceTreeNode {
    let path = Path::new(root);
    let name = path
        .file_name()
        .unwrap_or(std::ffi::OsStr::new(root))
        .to_string_lossy()
        .to_string();

    if !path.is_dir() {
        return WorkspaceTreeNode::root(name, root.to_string());
    }

    scan_dir(path, path, &name)
}

fn scan_dir(base: &Path, dir: &Path, dir_name: &str) -> WorkspaceTreeNode {
    let ignored = [
        ".git",
        "node_modules",
        "target",
        ".cargo",
        "__pycache__",
        ".venv",
    ];
    let rel = dir.strip_prefix(base).unwrap_or(dir);
    let path_str = rel.to_string_lossy().to_string();

    let mut children = Vec::new();

    let Ok(read_dir) = fs::read_dir(dir) else {
        return WorkspaceTreeNode {
            name: dir_name.to_string(),
            path: path_str,
            is_dir: true,
            loaded: true,
            load_error: false,
            children,
        };
    };

    let mut entries: Vec<_> = read_dir
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            !name_str.starts_with('.')
        })
        .collect();

    entries.sort_by_key(|e| {
        let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let name = e.file_name();
        (!is_dir, name)
    });

    for entry in entries {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();

        if file_type.is_dir() {
            if ignored.contains(&name.as_str()) {
                continue;
            }
            let child = scan_dir(base, &entry.path(), &name);
            children.push(child);
        } else {
            let full_path = entry.path();
            let child_rel = full_path.strip_prefix(base).unwrap_or(&full_path);
            children.push(WorkspaceTreeNode {
                name,
                path: child_rel.to_string_lossy().to_string(),
                is_dir: false,
                loaded: false,
                load_error: false,
                children: vec![],
            });
        }
    }

    WorkspaceTreeNode {
        name: dir_name.to_string(),
        path: path_str,
        is_dir: true,
        loaded: true,
        load_error: false,
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_nonexistent_directory() {
        let tree = scan_workspace("/tmp/vol-llm-ui-test-nonexistent-12345");
        assert!(tree.is_dir);
        assert!(tree.children.is_empty());
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = std::env::temp_dir().join("vol-llm-ui-scan-test");
        let _ = fs::create_dir_all(&dir);
        let tree = scan_workspace(dir.to_str().unwrap());
        assert!(tree.children.is_empty());
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
        let _ = fs::write(dir.join(".hidden"), "secret");
        let _ = fs::create_dir(dir.join(".git"));
        let _ = fs::create_dir(dir.join("target"));

        let tree = scan_workspace(dir.to_str().unwrap());

        let names: Vec<_> = tree.children.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"README.md"));
        assert!(names.contains(&"src"));

        let src = tree.children.iter().find(|c| c.name == "src").unwrap();
        assert!(src.children.iter().any(|c| c.name == "main.rs"));

        for child in &tree.children {
            assert!(!child.path.contains(".git"));
            assert!(!child.path.contains("target"));
            assert!(!child.path.contains(".hidden"));
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
