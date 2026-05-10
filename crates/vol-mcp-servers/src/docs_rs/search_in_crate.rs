use super::{fetch_docs_rs_page, SearchInCrateParams};

fn classify_item_type(href: &str) -> &'static str {
    if href.contains("struct.") {
        "struct"
    } else if href.contains("trait.") {
        "trait"
    } else if href.contains("fn.") {
        "function"
    } else if href.contains("enum.") {
        "enum"
    } else if href.contains("type.") {
        "type"
    } else if href.contains("const.") {
        "constant"
    } else if href.contains("static.") {
        "static"
    } else if href.contains("macro.") {
        "macro"
    } else if href.contains("union.") {
        "union"
    } else {
        "unknown"
    }
}

pub async fn search_in_crate(
    client: &reqwest::Client,
    params: &SearchInCrateParams,
) -> Result<String, String> {
    let version = params.version.as_deref().unwrap_or("latest");
    let url = format!(
        "https://docs.rs/{}/{}/{}/all.html",
        params.crate_name, version, params.crate_name
    );

    let html = fetch_docs_rs_page(&url, client)
        .await
        .map_err(|e| format!("Failed to fetch docs.rs page: {e}"))?;

    let document = scraper::Html::parse_document(&html);
    let main_selector =
        scraper::Selector::parse("#main-content").map_err(|e| format!("Invalid selector: {e}"))?;
    let link_selector =
        scraper::Selector::parse("a").map_err(|e| format!("Invalid selector: {e}"))?;

    let mut results: Vec<(String, String, String)> = Vec::new();

    if let Some(main) = document.select(&main_selector).next() {
        for link in main.select(&link_selector) {
            let name = link.text().collect::<Vec<_>>().concat().trim().to_string();
            if name.is_empty() {
                continue;
            }

            let href = match link.value().attr("href") {
                Some(h) => h.to_string(),
                None => continue,
            };

            let item_type = classify_item_type(&href);

            // Filter by item_type if specified
            if let Some(ref filter_type) = params.item_type {
                if !item_type.eq_ignore_ascii_case(filter_type) {
                    continue;
                }
            }

            // Filter by query string (case-insensitive match on name)
            if !name.to_lowercase().contains(&params.query.to_lowercase()) {
                continue;
            }

            // Build full documentation link
            let full_link = if href.starts_with("http") {
                href
            } else {
                format!("https://docs.rs{href}")
            };

            results.push((name, item_type.to_string(), full_link));
        }
    }

    // Deduplicate by (name, type)
    results.sort_by(|a, b| {
        a.0.to_lowercase()
            .cmp(&b.0.to_lowercase())
            .then(a.1.cmp(&b.1))
    });
    results.dedup_by(|a, b| a.0.to_lowercase() == b.0.to_lowercase() && a.1 == b.1);

    if results.is_empty() {
        return Ok(format!(
            "# Search Results for \"{}\" in {}\n\nNo matching items found.",
            params.query, params.crate_name
        ));
    }

    let mut md = format!(
        "# Search Results for \"{}\" in {}\n\nFound {} item{}\n\n",
        params.query,
        params.crate_name,
        results.len(),
        if results.len() == 1 { "" } else { "s" }
    );

    for (name, item_type, link) in &results {
        md.push_str(&format!("## {name} ({item_type})\n\n"));
        md.push_str(&format!("**Link:** [View Documentation]({link})\n\n"));
        md.push_str("---\n\n");
    }

    Ok(md)
}
