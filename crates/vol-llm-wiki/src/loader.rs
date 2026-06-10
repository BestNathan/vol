//! Wiki page discovery and loading.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::RwLock;

/// A wiki page discovered from the filesystem.
#[derive(Debug, Clone)]
pub struct WikiPage {
    /// Relative path from the wiki root.
    pub path: String,
    /// File title from frontmatter (falls back to filename).
    pub title: String,
    /// Tags from frontmatter.
    pub tags: Vec<String>,
    /// Absolute path to the file.
    pub absolute_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
struct WikiFrontmatter {
    title: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Discovers wiki pages from `.agents/wikis/` directories.
pub struct WikiLoader {
    roots: Vec<PathBuf>,
    pages: Arc<RwLock<Vec<WikiPage>>>,
}

impl WikiLoader {
    /// Create a WikiLoader with no roots (for testing).
    pub fn new_empty() -> Self {
        Self {
            roots: Vec::new(),
            pages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Replace the internal pages list (for testing).
    pub async fn set_pages(&self, pages: Vec<WikiPage>) {
        *self.pages.write().await = pages;
    }

    pub fn new(working_dir: Option<&std::path::Path>) -> Self {
        let mut roots = Vec::new();

        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".agents").join("wikis"));
        }

        if let Some(wd) = working_dir {
            roots.push(wd.join(".agents").join("wikis"));
        }

        Self {
            roots,
            pages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a custom wiki root.
    pub fn add_root(&mut self, path: PathBuf) {
        self.roots.push(path);
    }

    /// Discover all wiki pages from registered roots.
    pub async fn discover_all(&self) -> Result<(), String> {
        let mut pages = Vec::new();

        for root in &self.roots {
            if !root.exists() || !root.is_dir() {
                continue;
            }

            let pattern = root.join("**/*.md");
            let pattern_str = pattern.to_string_lossy();

            let entries =
                glob::glob(&pattern_str).map_err(|e| format!("Invalid glob pattern: {e}"))?;

            for entry in entries.flatten() {
                match md_frontmatter::from_path::<WikiFrontmatter>(&entry).await {
                    Ok(doc) => {
                        let title = doc.frontmatter.title.clone();
                        let tags = doc.frontmatter.tags.clone();
                        pages.push(WikiPage {
                            path: entry.to_string_lossy().to_string(),
                            title,
                            tags,
                            absolute_path: entry,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(path = %entry.display(), error = %e, "Failed to parse wiki page, skipping");
                    }
                }
            }
        }

        *self.pages.write().await = pages;
        Ok(())
    }

    /// List discovered wiki pages.
    pub async fn list_pages(&self) -> Vec<WikiPage> {
        self.pages.read().await.clone()
    }

    /// List page paths (relative paths only).
    pub async fn list_paths(&self) -> Vec<String> {
        self.pages
            .read()
            .await
            .iter()
            .map(|p| p.path.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_loader() {
        let loader = WikiLoader::new(None);
        loader.discover_all().await.unwrap();
        assert!(loader.list_pages().await.is_empty());
    }

    #[tokio::test]
    async fn test_discover_from_temp_dir() {
        let temp = tempfile::tempdir().unwrap();
        let wiki_dir = temp.path().join(".agent").join("wikis");
        std::fs::create_dir_all(&wiki_dir).unwrap();

        std::fs::write(
            wiki_dir.join("INDEX.md"),
            "---\ntitle: Index\ntags: [index]\n---\n# Wiki Index\n",
        )
        .unwrap();

        std::fs::write(
            wiki_dir.join("entities.md"),
            "---\ntitle: Entities\ntags: [entities]\n---\n# Entities\n",
        )
        .unwrap();

        let mut loader = WikiLoader::new(None);
        // Override roots to only use our test dir
        loader.roots.clear();
        loader.add_root(wiki_dir);

        loader.discover_all().await.unwrap();

        let pages = loader.list_pages().await;
        assert_eq!(pages.len(), 2);
        assert!(pages.iter().any(|p| p.title == "Index"));
        assert!(pages.iter().any(|p| p.title == "Entities"));
    }

    #[tokio::test]
    async fn test_discover_no_directory() {
        let loader = WikiLoader::new(None);
        let result = loader.discover_all().await;
        assert!(result.is_ok());
        assert!(loader.list_pages().await.is_empty());
    }
}
