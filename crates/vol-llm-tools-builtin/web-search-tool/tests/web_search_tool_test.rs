//! Integration tests for WebSearchTool using a mock SearchFn.

use async_trait::async_trait;
use serde_json::json;
use vol_llm_tool::web::search::{SearchError, SearchFn, SearchItem, SearchOptions, SearchResult};
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_web_search::WebSearchTool;

/// A mock search provider that returns predefined results.
struct MockSearchProvider {
    results: Vec<SearchItem>,
    should_fail: bool,
}

impl MockSearchProvider {
    fn with_results(results: Vec<SearchItem>) -> Self {
        Self {
            results,
            should_fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            results: vec![],
            should_fail: true,
        }
    }
}

#[async_trait]
impl SearchFn for MockSearchProvider {
    async fn search(&self, query: &str, _opts: SearchOptions) -> Result<SearchResult, SearchError> {
        if self.should_fail {
            return Err(SearchError::RequestFailed("mock failure".to_string()));
        }
        Ok(SearchResult {
            query: query.to_string(),
            results: self.results.clone(),
        })
    }
}

#[tokio::test]
async fn test_web_search_formats_results() {
    let provider = MockSearchProvider::with_results(vec![
        SearchItem {
            title: "Rust Programming Language".into(),
            url: "https://rust-lang.org".into(),
            snippet: Some("A language empowering everyone".into()),
        },
        SearchItem {
            title: "Rust GitHub".into(),
            url: "https://github.com/rust-lang".into(),
            snippet: None,
        },
    ]);
    let tool = WebSearchTool::new(provider);
    let args = json!({"query": "rust"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("Search results for: rust"));
    assert!(result.content.contains("[1] Rust Programming Language"));
    assert!(result.content.contains("https://rust-lang.org"));
    assert!(result.content.contains("[2] Rust GitHub"));
}

#[tokio::test]
async fn test_web_search_empty_results() {
    let provider = MockSearchProvider::with_results(vec![]);
    let tool = WebSearchTool::new(provider);
    let args = json!({"query": "nonexistent_xyz"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result
        .content
        .contains("Search results for: nonexistent_xyz"));
}

#[tokio::test]
async fn test_web_search_request_failure() {
    let provider = MockSearchProvider::failing();
    let tool = WebSearchTool::new(provider);
    let args = json!({"query": "anything"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("mock failure"));
}

#[tokio::test]
async fn test_web_search_default_num_results() {
    let mut items = Vec::new();
    for i in 1..=10 {
        items.push(SearchItem {
            title: format!("Result {i}"),
            url: format!("https://example.com/{i}"),
            snippet: Some(format!("Snippet {i}")),
        });
    }
    let provider = MockSearchProvider::with_results(items);
    let tool = WebSearchTool::new(provider);
    let args = json!({"query": "test"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    // All 10 results should be in output
    assert!(result.content.contains("[10] Result 10"));
}

#[tokio::test]
async fn test_web_search_tool_name_and_description() {
    let provider = MockSearchProvider::with_results(vec![]);
    let tool = WebSearchTool::new(provider);
    assert_eq!(tool.name(), "web_search");
    assert!(!tool.description().is_empty());
    assert!(tool.parameters().is_object());
}
