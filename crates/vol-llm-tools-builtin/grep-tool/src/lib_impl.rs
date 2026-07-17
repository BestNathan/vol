//! RustLibBackend — uses `ignore` crate for .gitignore-aware walking
//! and `grep-searcher` for parallel regex search.

use std::path::Path;
use std::sync::Arc;

use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use ignore::WalkBuilder;
use vol_llm_sandbox::Sandbox;

use crate::backend::GrepBackend;
use crate::{GrepParams, SearchResult};

pub struct RustLibBackend;

impl RustLibBackend {
    /// Convert a simple glob pattern to a regex for path matching.
    /// Supports: `*.ext`, `**/*.rs`, `**/*.ext`, `prefix*`, `*suffix`
    fn glob_to_regex(glob: &str) -> regex::Regex {
        let escaped = regex::escape(glob);
        // Replace **/ first (before single *), then *, then ?
        let regex_str = escaped
            .replace(r"\*\*/", "<<<DS>>>/")
            .replace(r"\*\*", ".*")
            .replace("<<<DS>>>/", "(.*/)?")
            .replace(r"\*", "[^/]*")
            .replace(r"\?", "[^/]");
        regex::Regex::new(&format!("^{regex_str}$"))
            .unwrap_or_else(|_| regex::Regex::new(".*").unwrap())
    }
}

#[async_trait::async_trait]
impl GrepBackend for RustLibBackend {
    async fn search(
        params: &GrepParams,
        root: &Path,
        _sandbox: &dyn Sandbox,
    ) -> Result<Vec<SearchResult>, String> {
        let pattern = params.pattern.clone();
        let case_sensitive = params.case_sensitive;
        let glob = params.glob.clone();
        let output_mode = params.output_mode.clone();
        let root_path = root.to_path_buf();

        tokio::task::spawn_blocking(move || {
            search_blocking(&pattern, case_sensitive, &glob, &output_mode, &root_path)
        })
        .await
        .map_err(|e| format!("Search task panicked: {e}"))?
    }
}

#[allow(unused_variables)]
fn search_blocking(
    pattern: &str,
    case_sensitive: bool,
    glob: &Option<String>,
    output_mode: &str,
    root: &Path,
) -> Result<Vec<SearchResult>, String> {
    // Build regex matcher
    let regex_pattern = if case_sensitive {
        pattern.to_string()
    } else {
        format!("(?i){pattern}")
    };
    let matcher =
        RegexMatcher::new(&regex_pattern).map_err(|e| format!("Invalid regex pattern: {e}"))?;

    // Build walker that auto-respects .gitignore and skips hidden dirs
    let mut walker = WalkBuilder::new(root);
    walker.max_depth(Some(50)).hidden(false).git_ignore(true);

    // Apply glob type filter if provided
    if let Some(ref g) = glob {
        let mut type_builder = ignore::types::TypesBuilder::new();
        type_builder
            .add("custom", g)
            .map_err(|e| format!("Invalid glob: {e}"))?;
        type_builder.select("custom");
        walker
            .types(
                type_builder
                    .build()
                    .map_err(|e| format!("Glob error: {e}"))?,
            )
            .git_ignore(true);
    }

    let mut searcher = Searcher::new();
    let glob_regex = glob.as_ref().map(|g| RustLibBackend::glob_to_regex(g));
    let results: Arc<std::sync::Mutex<Vec<SearchResult>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    for entry in walker.build().filter_map(|e| e.ok()) {
        let path = entry.path();
        let file_type = entry.file_type();

        if !file_type.is_some_and(|ft| ft.is_file()) {
            continue;
        }

        // Apply glob filter on file name or relative path
        if let Some(ref re) = glob_regex {
            let target = if glob.as_ref().is_some_and(|g| g.contains("**")) {
                match path.strip_prefix(root) {
                    Ok(rel) => rel.to_string_lossy().to_string(),
                    Err(_) => path.to_string_lossy().to_string(),
                }
            } else {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            };
            if !re.is_match(&target) {
                continue;
            }
        }

        let mut collector = MatchCollector {
            line_numbers: Vec::new(),
            count: 0,
        };

        match searcher.search_path(&matcher, path, &mut collector) {
            Ok(()) => {
                if collector.count > 0 {
                    results.lock().unwrap().push(SearchResult {
                        path: path.to_path_buf(),
                        match_count: collector.count,
                        line_numbers: collector.line_numbers,
                    });
                }
            }
            Err(_) => continue, // Skip files we can't read
        }
    }

    let inner = Arc::try_unwrap(results)
        .unwrap_or_else(|_| panic!("Arc still has references"))
        .into_inner()
        .unwrap();
    Ok(inner)
}

/// Sink for collecting line numbers during grep-searcher search.
struct MatchCollector {
    line_numbers: Vec<usize>,
    count: usize,
}

impl grep_searcher::Sink for &mut MatchCollector {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &grep_searcher::SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        let line_num = mat.line_number().unwrap_or(0) as usize;
        self.line_numbers.push(line_num);
        self.count += 1;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrepParams;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;
    use vol_llm_sandbox::local::LocalSandbox;

    fn make_params(pattern: &str, glob: Option<&str>, output_mode: &str) -> GrepParams {
        GrepParams {
            pattern: pattern.to_string(),
            path: None,
            glob: glob.map(|g| g.to_string()),
            output_mode: output_mode.to_string(),
            case_sensitive: false,
        }
    }

    #[tokio::test]
    async fn test_rustlib_basic_search() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();
        writeln!(f1, "foo bar").unwrap();
        writeln!(f1, "hello again").unwrap();

        let sb = LocalSandbox::new(Some(dir.path().to_path_buf()));
        let params = make_params("hello", None, "files_with_matches");
        let results = RustLibBackend::search(&params, dir.path(), &sb)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].path.ends_with("test.txt"));
    }

    #[tokio::test]
    async fn test_rustlib_no_matches() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();

        let sb = LocalSandbox::new(Some(dir.path().to_path_buf()));
        let params = make_params("nonexistent", None, "files_with_matches");
        let results = RustLibBackend::search(&params, dir.path(), &sb)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_rustlib_glob_filter() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.rs")).unwrap();
        writeln!(f1, "fn hello() {{}}").unwrap();
        let mut f2 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f2, "hello world").unwrap();

        let sb = LocalSandbox::new(Some(dir.path().to_path_buf()));
        let params = make_params("hello", Some("*.rs"), "files_with_matches");
        let results = RustLibBackend::search(&params, dir.path(), &sb)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].path.to_string_lossy().ends_with(".rs"),
            "Expected .rs file, got: {}",
            results[0].path.display()
        );
    }

    #[tokio::test]
    async fn test_rustlib_count_mode() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello").unwrap();
        writeln!(f1, "hello").unwrap();
        writeln!(f1, "world").unwrap();

        let sb = LocalSandbox::new(Some(dir.path().to_path_buf()));
        let params = make_params("hello", None, "count");
        let results = RustLibBackend::search(&params, dir.path(), &sb)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_count, 2);
    }

    #[tokio::test]
    async fn test_rustlib_content_mode() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "line 1").unwrap();
        writeln!(f1, "hello world").unwrap();
        writeln!(f1, "line 3").unwrap();

        let sb = LocalSandbox::new(Some(dir.path().to_path_buf()));
        let params = make_params("hello", None, "content");
        let results = RustLibBackend::search(&params, dir.path(), &sb)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line_numbers, vec![2]);
    }
}
