//! vol-llm-cli-tool: core abstraction for "CLI-as-Tool".
//!
//! Loads TOML configs that declare a named CLI tool backed by a Sandbox,
//! validates the first command token against a binaries whitelist,
//! interpolates `{{env.VAR}}` placeholders, and formats sandbox output
//! into a tool result. Reused by both the direct-load path
//! (`vol-llm-tools-builtin-cli-tool`) and the MCP server path
//! (`vol-mcp-servers::cli_tools`).

pub mod config;
pub mod error;
pub mod exec;
pub mod interpolate;
pub mod validate;

pub use config::CliToolConfig;
pub use error::CliToolError;
pub use exec::{CliTool, ToolOutput};

use std::path::Path;
use std::sync::Arc;
use vol_llm_sandbox::registry::SandboxRegistry;

/// Load every `*.toml` in `dir` as a CliTool.
///
/// - `sandbox_ref` entries are resolved against `registry`.
/// - Inline `[sandbox]` entries are constructed via
///   `vol_llm_sandbox::registry::SandboxRegistry::build_sandbox`.
/// - Files that fail to parse are logged as warnings and skipped.
/// - Name collisions: if a config's `name` matches an already-loaded tool,
///   returns an error (fail-fast).
pub async fn load_dir(
    dir: &Path,
    registry: &SandboxRegistry,
) -> Result<Vec<CliTool>, CliToolError> {
    let mut tools = Vec::new();
    let mut seen_names: std::collections::HashMap<String, std::path::PathBuf> = std::collections::HashMap::new();

    if !dir.exists() {
        return Ok(tools);
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| CliToolError::Config(format!("read_dir {}: {e}", dir.display())))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "cli-tool: read failed, skipping");
                continue;
            }
        };
        let config = match CliToolConfig::from_toml(&content) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "cli-tool: parse failed, skipping");
                continue;
            }
        };

        if let Some(first_path) = seen_names.get(&config.name) {
            return Err(CliToolError::Config(format!(
                "duplicate cli-tool name `{}` (first in {}, also in {})",
                config.name,
                first_path.display(),
                path.display()
            )));
        }
        seen_names.insert(config.name.clone(), path.clone());

        let sandbox: Arc<dyn vol_llm_sandbox::Sandbox> = if let Some(ref name) = config.sandbox_ref {
            registry.get(name).ok_or_else(|| {
                CliToolError::Config(format!(
                    "cli-tool `{}` references unknown sandbox `{}`",
                    config.name, name
                ))
            })?
        } else if let Some(sb_cfg) = config.sandbox.clone() {
            SandboxRegistry::build_sandbox(sb_cfg)
                .await
                .map_err(|e| {
                    CliToolError::Config(format!(
                        "cli-tool `{}` inline sandbox build failed: {e}",
                        config.name
                    ))
                })?
        } else {
            unreachable!("validate() guarantees one of sandbox/sandbox_ref");
        };

        tools.push(CliTool::new(config, sandbox));
    }

    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn load_dir_empty_when_missing() {
        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();
        let tools = load_dir(Path::new("/nonexistent/path/abc123"), &registry)
            .await
            .unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn load_dir_skips_unparseable_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("bad.toml"), "this is not valid toml {{{").unwrap();
        fs::write(dir.path().join("also_bad.toml"), "name = 42").unwrap();
        fs::write(dir.path().join("ignore.txt"), "not a toml").unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();

        let tools = load_dir(dir.path(), &registry).await.unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn load_dir_fails_fast_on_unknown_sandbox_ref() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("t.toml"),
            r#"
                name = "foo"
                description = "x"
                binaries = ["x"]
                cwd = "/"
                sandbox_ref = "no-such-sandbox"
            "#,
        )
        .unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();

        let err = load_dir(dir.path(), &registry).await.err().unwrap().to_string();
        assert!(err.contains("no-such-sandbox"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn load_dir_fails_fast_on_duplicate_name() {
        let dir = tempdir().unwrap();
        let body = r#"
            name = "dup"
            description = "x"
            binaries = ["x"]
            cwd = "/"
            sandbox_ref = "local"
        "#;
        fs::write(dir.path().join("a.toml"), body).unwrap();
        fs::write(dir.path().join("b.toml"), body).unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();

        let err = load_dir(dir.path(), &registry).await.err().unwrap().to_string();
        assert!(err.contains("duplicate"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn load_dir_resolves_sandbox_ref() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("my-tool.toml"),
            r#"
                name = "my-tool"
                description = "test tool"
                binaries = ["echo"]
                cwd = "/tmp"
                sandbox_ref = "local"
            "#,
        )
        .unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();
        let tools = load_dir(dir.path(), &registry).await.unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].config.name, "my-tool");
        assert_eq!(tools[0].config.binaries, vec!["echo"]);
    }

    #[tokio::test]
    async fn load_dir_builds_inline_sandbox() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("inline-tool.toml"),
            r#"
                name = "inline-tool"
                description = "test tool with inline sandbox"
                binaries = ["ls"]
                cwd = "/tmp"

                [sandbox]
                name = "inline-sandbox"
                type = "local"
            "#,
        )
        .unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();
        let tools = load_dir(dir.path(), &registry).await.unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].config.name, "inline-tool");
    }
}
