use async_trait::async_trait;
use vol_llm_core::Message;

use crate::{AttentionAnchor, ContextBlock, ContextContributor, ContextError, estimate_tokens};

/// A file specification for FileContributor.
#[derive(Clone)]
pub struct FileSpec {
    pub path: String,
    pub anchor: AttentionAnchor,
}

impl FileSpec {
    pub fn new(path: impl Into<String>, anchor: AttentionAnchor) -> Self {
        Self {
            path: path.into(),
            anchor,
        }
    }
}

/// File-based context contributor — reads markdown files from disk.
pub struct FileContributor {
    specs: Vec<FileSpec>,
}

impl FileContributor {
    pub fn new(specs: Vec<FileSpec>) -> Self {
        Self {
            specs,
        }
    }
}

#[async_trait]
impl ContextContributor for FileContributor {
    fn name(&self) -> &str {
        "file"
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        let mut blocks = Vec::new();
        for spec in &self.specs {
            match std::fs::read_to_string(&spec.path) {
                Ok(content) => {
                    let msg = Message::system(content);
                    blocks.push(ContextBlock::new(vec![msg], spec.anchor.clone()));
                }
                Err(e) => {
                    tracing::warn!(path = %spec.path, error = %e, "Failed to read file, skipping");
                }
            }
        }

        Ok(blocks)
    }

    async fn compress(&mut self) {
        // File content is non-compressible
    }

    fn estimate_size(&self) -> usize {
        self.specs
            .iter()
            .filter_map(|spec| std::fs::read_to_string(&spec.path).ok())
            .map(|content| estimate_tokens(&Message::system(content)))
            .sum()
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(FileContributor {
            specs: self.specs.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_file_contributor_single_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "# Role\nYou are helpful").unwrap();
        let contributor = FileContributor::new(vec![FileSpec::new(
            file.path().to_str().unwrap(),
            AttentionAnchor::Head(0),
        )]);
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Head(0)));
        assert!(blocks[0].messages[0].content.as_ref().unwrap().as_str().contains("# Role"));
    }

    #[tokio::test]
    async fn test_file_contributor_multiple_files() {
        let mut f1 = tempfile::NamedTempFile::new().unwrap();
        writeln!(f1, "# Role").unwrap();
        let mut f2 = tempfile::NamedTempFile::new().unwrap();
        writeln!(f2, "# Task").unwrap();

        let contributor = FileContributor::new(vec![
            FileSpec::new(f1.path().to_str().unwrap(), AttentionAnchor::Head(0)),
            FileSpec::new(f2.path().to_str().unwrap(), AttentionAnchor::Tail(0)),
        ]);
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Head(0)));
        assert!(matches!(blocks[1].anchor, AttentionAnchor::Tail(0)));
    }

    #[tokio::test]
    async fn test_file_contributor_missing_file() {
        let contributor = FileContributor::new(vec![FileSpec::new(
            "/nonexistent/path.md",
            AttentionAnchor::Head(0),
        )]);
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks.len(), 0);
    }

    #[tokio::test]
    async fn test_file_contributor_mixed_exists() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "# Content").unwrap();
        let contributor = FileContributor::new(vec![
            FileSpec::new(f.path().to_str().unwrap(), AttentionAnchor::Head(0)),
            FileSpec::new("/nonexistent/path.md", AttentionAnchor::Tail(0)),
        ]);
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Head(0)));
    }

    #[tokio::test]
    async fn test_file_contributor_compress_noop() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "# Content").unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let mut contributor = FileContributor::new(vec![FileSpec::new(
            &path,
            AttentionAnchor::Head(0),
        )]);
        contributor.compress().await;
        // compress is no-op, content unchanged
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
    }
}
