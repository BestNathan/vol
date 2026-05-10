pub async fn get_readme(
    client: &reqwest::Client,
    params: &super::ReadMeParams,
) -> Result<String, String> {
    let version = params.version.as_deref().unwrap_or("latest");
    let url = format!(
        "https://docs.rs/{}/{}/{}",
        params.crate_name, version, params.crate_name
    );

    let html = super::fetch_docs_rs_page(&url, client)
        .await
        .map_err(|e| e.to_string())?;

    let content = super::extract_doc_content(&html);

    Ok(format!(
        "# {} (v{})\n\n{}\n\n---\n*Source: {}*",
        params.crate_name, version, content, url
    ))
}
