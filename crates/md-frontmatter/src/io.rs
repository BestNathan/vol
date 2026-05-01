use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::error::MdFmError;
use crate::parser::{ParsedDoc, parse, to_string};
use crate::Result;

/// Read a file and parse its frontmatter.
pub async fn from_path<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<ParsedDoc<T>> {
    let path = path.as_ref().to_path_buf();
    let content = tokio::fs::read_to_string(&path).await?;

    if content.contains('\u{FFFD}') {
        return Err(MdFmError::InvalidUtf8 { path: path.clone() });
    }

    let mut doc = parse::<T>(&content)?;
    doc.path = Some(path);
    Ok(doc)
}

/// Write a ParsedDoc back to its file.
pub async fn write<T: Serialize>(doc: &ParsedDoc<T>) -> Result<()> {
    let path = doc.path.as_ref().ok_or_else(|| MdFmError::Io(
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "no path set on ParsedDoc")
    ))?;
    let content = to_string(doc)?;
    tokio::fs::write(path, content).await.map_err(MdFmError::Io)
}

/// Recursively scan a directory for .md files and parse each one.
///
/// Returns `Ok(docs)` if all files parsed successfully, or `Err(errors)` if any
/// file had parse errors. A file failing to parse does not abort the entire scan.
pub async fn scan_dir<T: DeserializeOwned + Clone>(
    root: impl AsRef<Path>,
) -> std::result::Result<Vec<ParsedDoc<T>>, Vec<(PathBuf, MdFmError)>> {
    let root = root.as_ref().to_path_buf();
    let mut docs = Vec::new();
    let mut errors = Vec::new();

    let pattern = root.join("**/*.md");
    let pattern_str = pattern.to_string_lossy().to_string();

    for entry in glob::glob(&pattern_str).map_err(|e| {
        vec![(root.clone(), std::io::Error::other(e.to_string()).into())]
    })? {
        let path = match entry {
            Ok(p) => p,
            Err(e) => {
                errors.push((root.clone(), std::io::Error::other(e.to_string()).into()));
                continue;
            }
        };

        match from_path::<T>(&path).await {
            Ok(doc) => docs.push(doc),
            Err(e) => errors.push((path, e)),
        }
    }

    if errors.is_empty() {
        Ok(docs)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_frontmatter;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    struct TestFm {
        title: String,
        #[serde(default)]
        tags: Vec<String>,
    }

    #[tokio::test]
    async fn test_from_path_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        let content = "---\ntitle: File Test\ntags: [x, y]\n---\n\n# Body";
        tokio::fs::write(&file_path, content).await.unwrap();

        let doc = from_path::<TestFm>(&file_path).await.unwrap();
        assert_eq!(doc.frontmatter.title, "File Test");
        assert_eq!(doc.frontmatter.tags, vec!["x", "y"]);
        assert_eq!(doc.body, "\n\n# Body");
        assert_eq!(doc.path, Some(file_path.clone()));
    }

    #[tokio::test]
    async fn test_from_path_no_file() {
        let result = from_path::<TestFm>("/nonexistent/path.md").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::Io(_) => {}
            e => panic!("Expected Io error, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_from_path_non_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("binary.md");
        tokio::fs::write(&file_path, &[0x80, 0x81, 0x82]).await.unwrap();

        let result = from_path::<TestFm>(&file_path).await;
        // tokio::fs::read_to_string returns Io error for non-UTF8
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::Io(_) => {}
            e => panic!("Expected Io error, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_write_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("roundtrip.md");

        let doc = ParsedDoc {
            frontmatter: TestFm {
                title: "Roundtrip".to_string(),
                tags: vec!["a".to_string()],
            },
            body: "\nContent body".to_string(),
            path: Some(file_path.clone()),
        };

        write(&doc).await.unwrap();

        let raw = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("title: Roundtrip"));

        let reparsed = from_path::<TestFm>(&file_path).await.unwrap();
        assert_eq!(reparsed.frontmatter.title, "Roundtrip");
        // serde_yaml adds trailing newline, so body has extra leading \n
        assert!(reparsed.body.contains("Content body"));
    }

    #[tokio::test]
    async fn test_write_preserves_body_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("preserve.md");

        let original_body = "\n\n# Heading\n\n---\n\nHorizontal rule above\n";
        let original = format!("---\ntitle: Original\n---{}", original_body);
        tokio::fs::write(&file_path, &original).await.unwrap();

        let mut doc = from_path::<TestFm>(&file_path).await.unwrap();
        update_frontmatter(&mut doc, &TestFm {
            title: "Updated".to_string(),
            tags: vec![],
        });
        write(&doc).await.unwrap();

        let final_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(final_content.contains(original_body));
        assert!(final_content.starts_with("---\ntitle: Updated\n"));
    }

    #[tokio::test]
    async fn test_scan_dir_empty() {
        let dir = tempfile::tempdir().unwrap();
        let docs = scan_dir::<TestFm>(dir.path()).await.unwrap();
        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn test_scan_dir_with_files() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("a.md"), "---\ntitle: A\n---\nbody a").await.unwrap();
        tokio::fs::write(dir.path().join("b.md"), "---\ntitle: B\n---\nbody b").await.unwrap();
        tokio::fs::write(dir.path().join("c.txt"), "not markdown").await.unwrap();

        let docs = scan_dir::<TestFm>(dir.path()).await.unwrap();
        assert_eq!(docs.len(), 2);

        let titles: Vec<_> = docs.iter().map(|d| d.frontmatter.title.clone()).collect();
        assert!(titles.contains(&"A".to_string()));
        assert!(titles.contains(&"B".to_string()));

        assert!(!docs.iter().any(|d| d.path.as_ref().unwrap().to_string_lossy().ends_with(".txt")));
    }

    #[tokio::test]
    async fn test_scan_dir_with_errors() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("good.md"), "---\ntitle: Good\n---\nbody").await.unwrap();
        tokio::fs::write(dir.path().join("bad.md"), "# No frontmatter").await.unwrap();

        let result = scan_dir::<TestFm>(dir.path()).await;
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].0.to_string_lossy().contains("bad.md"));
    }

    #[tokio::test]
    async fn test_scan_dir_nested() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(dir.path().join("sub")).await.unwrap();
        tokio::fs::write(dir.path().join("root.md"), "---\ntitle: Root\n---\nbody").await.unwrap();
        tokio::fs::write(dir.path().join("sub/nested.md"), "---\ntitle: Nested\n---\nbody").await.unwrap();

        let docs = scan_dir::<TestFm>(dir.path()).await.unwrap();
        assert_eq!(docs.len(), 2);
        let titles: Vec<_> = docs.iter().map(|d| d.frontmatter.title.clone()).collect();
        assert!(titles.contains(&"Root".to_string()));
        assert!(titles.contains(&"Nested".to_string()));
    }
}
