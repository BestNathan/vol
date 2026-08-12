//! Centralized web tool tests — cross-tool scenarios involving web tools.

// Tests intentionally unwrap after asserting is_err()/is_ok(); the crate
// inherits the workspace's deny-level unwrap/expect lints.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod fixtures;

use async_trait::async_trait;
use serde_json::json;
use vol_llm_tool::web::fetch::{FetchError, FetchFn, FetchOptions, FetchResult, FetchStatus};
use vol_llm_tool::web::search::{SearchError, SearchFn, SearchItem, SearchOptions, SearchResult};
use vol_llm_tool::ExecutableTool;
use vol_llm_tools_builtin::{WebFetchTool, WebSearchTool};

// Re-use mock providers (simplified versions for centralized tests)

struct MockSearch {
    items: Vec<SearchItem>,
}

#[async_trait]
impl SearchFn for MockSearch {
    async fn search(&self, query: &str, _opts: SearchOptions) -> Result<SearchResult, SearchError> {
        Ok(SearchResult {
            query: query.to_string(),
            results: self.items.clone(),
        })
    }
}

struct MockFetch {
    html: String,
}

#[async_trait]
impl FetchFn for MockFetch {
    async fn fetch(&self, url: &str, _opts: FetchOptions) -> Result<FetchResult, FetchError> {
        Ok(FetchResult {
            url: url.to_string(),
            status: FetchStatus::Success,
            content: self.html.clone(),
            title: Some("Mock Page".to_string()),
            from_cache: false,
        })
    }
}

#[tokio::test]
async fn test_search_then_fetch_result_url() {
    // Search returns a URL, fetch retrieves its content
    let search = WebSearchTool::new(MockSearch {
        items: vec![SearchItem {
            title: "Docs".into(),
            url: "https://docs.example.com".into(),
            snippet: Some("Documentation".into()),
        }],
    });
    let fetch = WebFetchTool::new(MockFetch {
        html: "This is the documentation page.".into(),
    });

    // Search
    let result = search
        .execute(&json!({"query": "docs"}), &fixtures::sandbox_in_tempdir().0)
        .await
        .unwrap();
    assert!(result.content.contains("https://docs.example.com"));

    // Fetch the URL
    let result = fetch
        .execute(
            &json!({"url": "https://docs.example.com"}),
            &fixtures::sandbox_in_tempdir().0,
        )
        .await
        .unwrap();
    assert!(result.content.contains("This is the documentation page."));
}
