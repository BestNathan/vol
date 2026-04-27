//! Wiki page injection into system prompt.

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor, ContextError};
use vol_llm_core::Message;

use crate::loader::WikiLoader;

/// Formats wiki pages for system prompt injection.
pub struct WikiInjector {
    loader: Arc<WikiLoader>,
}

impl WikiInjector {
    pub fn new(loader: Arc<WikiLoader>) -> Self {
        Self { loader }
    }

    /// Create a WikiInjector that loads wiki pages from `{working_dir}/.agent/wikis`.
    pub async fn from_workdir(working_dir: &std::path::Path) -> Self {
        let loader = Arc::new(WikiLoader::new(Some(working_dir)));
        Self::new(loader)
    }

    /// Discover wiki pages from the configured roots.
    /// Must be called before `contribute()` returns any content.
    pub async fn discover_all(&self) -> Result<(), String> {
        self.loader.discover_all().await
    }

    /// Format wiki metadata as prompt string.
    /// Returns empty string if no wiki pages are available.
    pub async fn format_metadata(&self) -> String {
        let pages = self.loader.list_pages().await;
        if pages.is_empty() {
            return String::new();
        }

        let mut output = String::from("# Wiki\n\nAvailable pages:\n");
        for page in &pages {
            if page.tags.is_empty() {
                output.push_str(&format!("- {} ({})\n", page.title, page.path));
            } else {
                let tags = page.tags.join(", ");
                output.push_str(&format!("- {} ({}) [{}]\n", page.title, page.path, tags));
            }
        }
        output.push_str("\nUse the `read` tool to load any page. Use `write`/`edit` to update.\n");
        output
    }
}

#[async_trait]
impl ContextContributor for WikiInjector {
    fn name(&self) -> &str {
        "wiki"
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        let metadata_text = self.format_metadata().await;
        if metadata_text.is_empty() {
            return Ok(vec![]);
        }
        let msg = Message::user(metadata_text);
        Ok(vec![ContextBlock::new(vec![msg], AttentionAnchor::Head(0))])
    }

    async fn compress(&mut self) {
        // Wiki pages are static prompt content — nothing to compress.
    }

    fn estimate_size(&self) -> usize {
        0
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(WikiInjector {
            loader: self.loader.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::WikiPage;

    #[tokio::test]
    async fn test_format_metadata_empty() {
        let loader = WikiLoader::new(None);
        let injector = WikiInjector::new(Arc::new(loader));
        let output = injector.format_metadata().await;
        assert!(output.is_empty());
    }

    #[tokio::test]
    async fn test_format_metadata_with_pages() {
        let loader = WikiLoader::new(None);
        let pages = vec![
            WikiPage {
                path: "INDEX.md".to_string(),
                title: "Index".to_string(),
                tags: vec!["index".to_string()],
                absolute_path: std::path::PathBuf::from("/test/INDEX.md"),
            },
            WikiPage {
                path: "entities.md".to_string(),
                title: "Entities".to_string(),
                tags: vec!["entities".to_string()],
                absolute_path: std::path::PathBuf::from("/test/entities.md"),
            },
        ];
        loader.set_pages(pages).await;

        let injector = WikiInjector::new(Arc::new(loader));
        let output = injector.format_metadata().await;

        assert!(output.contains("# Wiki"));
        assert!(output.contains("Available pages:"));
        assert!(output.contains("Index"));
        assert!(output.contains("Entities"));
        assert!(output.contains("read"));
    }

    #[tokio::test]
    async fn test_wiki_injector_contribute_empty() {
        let loader = WikiLoader::new(None);
        let injector = WikiInjector::new(Arc::new(loader));
        let blocks = injector.contribute().await.unwrap();
        assert!(blocks.is_empty());
    }

    #[tokio::test]
    async fn test_wiki_injector_clone_box() {
        let loader = WikiLoader::new(None);
        let injector = WikiInjector::new(Arc::new(loader));
        let cloned = injector.clone_box();
        assert_eq!(cloned.name(), "wiki");
    }
}
