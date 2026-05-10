use serde::Deserialize;

use super::SearchCratesParams;

#[derive(Debug, Deserialize)]
struct CratesIoResponse {
    crates: Vec<CrateInfo>,
    meta: Meta,
}

#[derive(Debug, Deserialize)]
struct CrateInfo {
    name: String,
    description: Option<String>,
    downloads: u64,
    documentation: Option<String>,
    #[serde(rename = "newest_version")]
    newest_version: String,
}

#[derive(Debug, Deserialize)]
struct Meta {
    total: u64,
}

pub async fn search_crates(
    client: &reqwest::Client,
    params: &SearchCratesParams,
) -> Result<String, String> {
    let per_page = params.per_page.unwrap_or(10).min(100).max(1);
    let sort = params.sort.as_deref().unwrap_or("relevance");

    let resp = client
        .get("https://crates.io/api/v1/crates")
        .header("User-Agent", "vol-mcp-servers/0.1.0")
        .header("Accept", "application/json")
        .query(&[
            ("q", &params.query),
            ("per_page", &per_page.to_string()),
            ("sort", &sort.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("Failed to send request to crates.io: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "crates.io API returned error {}: {}",
            status, body
        ));
    }

    let body: CratesIoResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse crates.io response: {e}"))?;

    if body.crates.is_empty() {
        return Ok(format!(
            "No crates found for query \"{}\". Try broadening your search terms.",
            params.query
        ));
    }

    let mut md = format!("# Crate Search Results for \"{}\"\n\n", params.query);
    md.push_str(&format!("**Total results:** {}\n\n", body.meta.total));

    for c in &body.crates {
        md.push_str(&format!("## {} ({})\n\n", c.name, c.newest_version));

        if let Some(ref desc) = c.description {
            md.push_str(&format!("**Description:** {desc}\n\n"));
        } else {
            md.push_str("**Description:** _No description available._\n\n");
        }

        md.push_str(&format!("**Downloads:** {}\n\n", c.downloads));

        if let Some(ref doc_url) = c.documentation {
            md.push_str(&format!("**Documentation:** {doc_url}\n\n"));
        } else {
            md.push_str(&format!(
                "**Documentation:** https://docs.rs/{}\n\n",
                c.name
            ));
        }

        md.push_str("---\n\n");
    }

    Ok(md)
}
