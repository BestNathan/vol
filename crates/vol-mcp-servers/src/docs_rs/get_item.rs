pub async fn get_item(client: &reqwest::Client, params: &super::GetItemParams) -> Result<String, String> {
    let version = params.version.as_deref().unwrap_or("latest");

    let url = if params.item_type == "module" {
        // Module: replace :: with / and append /index.html
        let path = params.item_path.replace("::", "/");
        format!(
            "https://docs.rs/{}/{}/{}",
            params.crate_name, version, path
        )
    } else {
        // Other items: split path, last segment is item_name, rest is module_path
        let segments: Vec<&str> = params.item_path.split("::").collect();
        let item_name = if segments.len() == 1 {
            segments[0]
        } else {
            segments.last().unwrap()
        };
        let module_path_owned;
        let module_path: &str = if segments.len() == 1 {
            &params.crate_name
        } else {
            module_path_owned = segments[..segments.len() - 1].join("/");
            &module_path_owned
        };
        format!(
            "https://docs.rs/{}/{}/{}/{}.{}.html",
            params.crate_name, version, module_path, params.item_type, item_name
        )
    };

    let html = super::fetch_docs_rs_page(&url, client)
        .await
        .map_err(|e| e.to_string())?;

    let content = super::extract_doc_content(&html);

    Ok(format!(
        "# {}::{} ({})\n\n{}\n\n---\n*Source: {}*",
        params.crate_name, params.item_path, params.item_type, content, url
    ))
}
