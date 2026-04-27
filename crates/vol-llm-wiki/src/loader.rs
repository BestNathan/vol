//! Wiki page discovery and loading.

use std::path::PathBuf;
use std::sync::Arc;

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

/// Discovers wiki pages from `.agent/wikis/` directories.
pub struct WikiLoader {
    roots: Vec<PathBuf>,
    pages: Arc<RwLock<Vec<WikiPage>>>,
}

impl WikiLoader {
    pub fn new(working_dir: Option<&std::path::Path>) -> Self {
        let mut roots = Vec::new();

        // User root
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".agents").join("wikis"));
        }

        // Repo root
        if let Some(wd) = working_dir {
            roots.push(wd.join(".agent").join("wikis"));
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
            Self::walk_dir(root, &mut pages);
        }

        *self.pages.write().await = pages;
        Ok(())
    }

    /// List discovered wiki pages.
    pub async fn list_pages(&self) -> Vec<WikiPage> {
        self.pages.read().await.clone()
    }

    /// List page paths (relative paths only, lighter than full metadata).
    pub async fn list_paths(&self) -> Vec<String> {
        self.pages.read().await.iter().map(|p| p.path.clone()).collect()
    }

    fn walk_dir(dir: &std::path::Path, pages: &mut Vec<WikiPage>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();

        for path in paths {
            if path.is_dir() {
                Self::walk_dir(&path, pages);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let title = Self::extract_title(&path);
                let tags = Self::extract_tags(&path);
                pages.push(WikiPage {
                    path: path.to_string_lossy().to_string(),
                    title,
                    tags,
                    absolute_path: path.clone(),
                });
            }
        }
    }

    fn extract_title(path: &std::path::Path) -> String {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(title) = Self::parse_frontmatter_value(&content, "title") {
                return title;
            }
        }
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn extract_tags(path: &std::path::Path) -> Vec<String> {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(raw) = Self::parse_frontmatter_value(&content, "tags") {
                let trimmed = raw.trim().trim_start_matches('[').trim_end_matches(']');
                return trimmed
                    .split(',')
                    .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
            }
        }
        Vec::new()
    }

    fn parse_frontmatter_value(content: &str, key: &str) -> Option<String> {
        let Some(start) = content.find("---\n") else { return None };
        let rest = &content[start + 4..];
        let Some(end) = rest.find("\n---") else { return None };
        let frontmatter = &rest[..end];

        for line in frontmatter.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(&format!("{}:", key)) {
                let value = trimmed[key.len() + 1..].trim();
                return Some(value.to_string());
            }
        }
        None
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

        let loader = WikiLoader::new(Some(temp.path()));
        // Only keep the repo root (the .agent/wikis we just created)
        // The home dir root won't exist so it will be skipped during discovery
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
