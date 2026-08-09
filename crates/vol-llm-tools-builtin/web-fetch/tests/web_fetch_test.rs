//! Integration tests for WebFetchTool using a mock FetchFn.

use async_trait::async_trait;
use serde_json::json;
use vol_llm_tool::web::fetch::{FetchError, FetchFn, FetchOptions, FetchResult};
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_web_search::WebFetchTool;

/// A mock fetch provider that returns predefined content.
struct MockFetchProvider {
    content: String,
    title: Option<String>,
    should_fail: bool,
    fail_with: Option<FetchError>,
}

impl MockFetchProvider {
    fn with_content(content: &str) -> Self {
        Self {
            content: content.to_string(),
            title: Some("Mock Page".to_string()),
            should_fail: false,
            fail_with: None,
        }
    }

    fn failing(error: FetchError) -> Self {
        Self {
            content: String::new(),
            title: None,
            should_fail: true,
            fail_with: Some(error),
        }
    }
}

#[async_trait]
impl FetchFn for MockFetchProvider {
    async fn fetch(&self, url: &str, _opts: FetchOptions) -> Result<FetchResult, FetchError> {
        if self.should_fail {
            return Err(self.fail_with.clone().unwrap());
        }
        Ok(FetchResult {
            url: url.to_string(),
            content: self.content.clone(),
            title: self.title.clone(),
        })
    }
}

#[tokio::test]
async fn test_web_fetch_returns_content() {
    let provider = MockFetchProvider::with_content("This is the extracted page content.");
    let tool = WebFetchTool::new(provider);
    let args = json!({"url": "https://example.com/article"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("Title: Mock Page"));
    assert!(result.content.contains("https://example.com/article"));
    assert!(result
        .content
        .contains("This is the extracted page content."));
}

#[tokio::test]
async fn test_web_fetch_no_title() {
    let provider = MockFetchProvider {
        content: "content without title".to_string(),
        title: None,
        should_fail: false,
        fail_with: None,
    };
    let tool = WebFetchTool::new(provider);
    let args = json!({"url": "https://example.com"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(!result.content.contains("Title:"));
    assert!(result.content.contains("URL: https://example.com"));
}

#[tokio::test]
async fn test_web_fetch_request_failed() {
    let provider =
        MockFetchProvider::failing(FetchError::RequestFailed("connection refused".to_string()));
    let tool = WebFetchTool::new(provider);
    let args = json!({"url": "https://down.example.com"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("connection refused"));
}

#[tokio::test]
async fn test_web_fetch_invalid_url() {
    let provider =
        MockFetchProvider::failing(FetchError::InvalidUrl("not a valid URL".to_string()));
    let tool = WebFetchTool::new(provider);
    let args = json!({"url": "not-a-url"});
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not a valid URL"));
}

#[tokio::test]
async fn test_web_fetch_with_prompt() {
    let provider = MockFetchProvider::with_content("Full page content here.");
    let tool = WebFetchTool::new(provider);
    let args = json!({
        "url": "https://example.com",
        "prompt": "extract the main heading"
    });
    let ctx = ToolContext::default();

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("Full page content here."));
}

#[tokio::test]
async fn test_web_fetch_tool_name_and_description() {
    let provider = MockFetchProvider::with_content("test");
    let tool = WebFetchTool::new(provider);
    assert_eq!(tool.name(), "web_fetch");
    assert!(!tool.description().is_empty());
    assert!(tool.parameters().is_object());
    // Verify "url" is a required parameter
    let params = tool.parameters();
    assert!(params["required"]
        .as_array()
        .unwrap()
        .contains(&json!("url")));
}
