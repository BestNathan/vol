use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars;
use rmcp::tool;
use rmcp::tool_router;

mod get_item;
mod readme;
mod search_crates;
mod search_in_crate;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchCratesParams {
    pub query: String,
    pub per_page: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadMeParams {
    pub crate_name: String,
    pub version: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetItemParams {
    pub crate_name: String,
    pub item_type: String,
    pub item_path: String,
    pub version: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchInCrateParams {
    pub crate_name: String,
    pub query: String,
    pub version: Option<String>,
    pub item_type: Option<String>,
}

#[derive(Clone)]
pub struct DocsRsClient {
    client: reqwest::Client,
}

impl DocsRsClient {
    pub fn new() -> Self {
        Self { client: reqwest::Client::new() }
    }
}

impl Default for DocsRsClient {
    fn default() -> Self { Self::new() }
}

pub(crate) async fn fetch_docs_rs_page(url: &str, client: &reqwest::Client) -> anyhow::Result<String> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Failed to fetch {}: HTTP {}", url, resp.status());
    }
    Ok(resp.text().await?)
}

#[allow(dead_code)]
fn extract_doc_content(html: &str) -> String {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("#main-content").unwrap();
    if let Some(el) = document.select(&selector).next() {
        return html2md::parse_html(&el.html());
    }
    let selector = scraper::Selector::parse(".docblock").unwrap();
    if let Some(el) = document.select(&selector).next() {
        return html2md::parse_html(&el.html());
    }
    "No documentation content found.".to_string()
}

#[derive(Clone)]
pub struct DocsRsServer {
    client: DocsRsClient,
}

impl DocsRsServer {
    pub fn new() -> Self {
        Self { client: DocsRsClient::new() }
    }
}

impl Default for DocsRsServer {
    fn default() -> Self { Self::new() }
}

#[tool_router(server_handler)]
impl DocsRsServer {
    #[tool(description = "Search for Rust crates by keywords on crates.io.")]
    async fn docs_rs_search_crates(&self, Parameters(params): Parameters<SearchCratesParams>) -> Result<String, String> {
        search_crates::search_crates(&self.client.client, &params).await
    }

    #[tool(description = "Get README/overview content of the specified crate")]
    async fn docs_rs_readme(&self, Parameters(params): Parameters<ReadMeParams>) -> Result<String, String> {
        readme::get_readme(&self.client.client, &params).await
    }

    #[tool(description = "Get documentation content of a specific item within a crate")]
    async fn docs_rs_get_item(&self, Parameters(params): Parameters<GetItemParams>) -> Result<String, String> {
        get_item::get_item(&self.client.client, &params).await
    }

    #[tool(description = "Search for traits, structs, methods, etc. from the crate's all.html page")]
    async fn docs_rs_search_in_crate(&self, Parameters(params): Parameters<SearchInCrateParams>) -> Result<String, String> {
        search_in_crate::search_in_crate(&self.client.client, &params).await
    }
}
