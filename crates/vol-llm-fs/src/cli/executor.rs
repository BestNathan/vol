//! FS CLI executor — execute ParsedCommand by delegating to builtin file op tools.

use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};
use vol_llm_tools_builtin_edit::EditTool;
use vol_llm_tools_builtin_glob::GlobTool;
use vol_llm_tools_builtin_grep::GrepTool;
use vol_llm_tools_builtin_read::ReadTool;
use vol_llm_tools_builtin_write::WriteTool;

use super::commands::ParsedCommand;

/// Execute a parsed command by delegating to the underlying file op tool.
pub(crate) async fn execute(
    cmd: ParsedCommand,
    context: &ToolContext,
) -> Result<ToolResult, String> {
    match cmd {
        ParsedCommand::Read {
            file_path,
            offset,
            limit,
            json,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "offset": offset,
                "limit": limit,
            });
            let result = ReadTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Write {
            file_path,
            content,
            json,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "content": content,
            });
            let result = WriteTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Edit {
            file_path,
            old_string,
            new_string,
            replace_all,
            json,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "old_string": old_string,
                "new_string": new_string,
                "replace_all": replace_all,
            });
            let result = EditTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Grep {
            pattern,
            path,
            glob,
            output_mode,
            case_sensitive,
            json,
        } => {
            let params = serde_json::json!({
                "pattern": pattern,
                "path": path,
                "glob": glob,
                "output_mode": output_mode,
                "case_sensitive": case_sensitive,
            });
            let result = GrepTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Glob {
            pattern,
            path,
            exclude,
            kind,
            max_results,
            include_hidden,
            follow_symlinks,
            sort,
            with_metadata,
            json,
        } => {
            let params = serde_json::json!({
                "pattern": pattern,
                "path": path,
                "exclude": exclude,
                "kind": kind,
                "max_results": max_results,
                "include_hidden": include_hidden,
                "follow_symlinks": follow_symlinks,
                "sort": sort,
                "with_metadata": with_metadata,
            });
            let result = GlobTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Scheme { subcommand } => Ok(ToolResult {
            success: true,
            content: scheme_for(subcommand.as_deref()),
            error: None,
            data: None,
            call_id: String::new(),
        }),
    }
}

/// Wrap the content in a JSON envelope when `--json` is requested.
fn finalize(result: ToolResult, json: bool) -> ToolResult {
    if json {
        let content = super::format::envelope(result.success, &result.content);
        ToolResult { content, ..result }
    } else {
        result
    }
}

/// Scheme tables — (flag, required, description) per subcommand.
fn scheme_for(subcommand: Option<&str>) -> String {
    match subcommand {
        Some("read") => super::format::fmt_scheme(
            "read",
            &[
                ("file_path", true, "Path to the file to read"),
                (
                    "offset",
                    false,
                    "Line offset to start reading from (default: 0)",
                ),
                (
                    "limit",
                    false,
                    "Maximum number of lines to read (default: 2000)",
                ),
            ],
        ),
        Some("write") => super::format::fmt_scheme(
            "write",
            &[
                ("file_path", true, "Path to the file to write"),
                ("content", true, "Content to write to the file"),
            ],
        ),
        Some("edit") => super::format::fmt_scheme(
            "edit",
            &[
                ("file_path", true, "Path to the file to edit"),
                ("old_string", true, "Exact string to find and replace"),
                ("new_string", true, "String to replace with"),
                (
                    "replace_all",
                    false,
                    "Replace all occurrences (default: false)",
                ),
            ],
        ),
        Some("grep") => super::format::fmt_scheme(
            "grep",
            &[
                ("pattern", true, "Regex pattern to search for"),
                ("path", false, "Root directory to search in"),
                ("glob", false, "File pattern filter (e.g. \"*.rs\")"),
                (
                    "output_mode",
                    false,
                    "Output format: files_with_matches|count|content (default: files_with_matches)",
                ),
                (
                    "case_sensitive",
                    false,
                    "Case-sensitive matching (default: false)",
                ),
            ],
        ),
        Some("glob") => super::format::fmt_scheme(
            "glob",
            &[
                ("pattern", true, "Glob pattern relative to the search path"),
                ("path", false, "Search root directory (default: \".\")"),
                (
                    "exclude",
                    false,
                    "Glob patterns to exclude (comma-separated)",
                ),
                ("kind", false, "Return file|directory|all (default: file)"),
                ("max_results", false, "Maximum results (default: 100)"),
                (
                    "include_hidden",
                    false,
                    "Include hidden files (default: false)",
                ),
                (
                    "follow_symlinks",
                    false,
                    "Follow symbolic links (default: false)",
                ),
                (
                    "sort",
                    false,
                    "Sort order: path_asc|path_desc|modified_desc|modified_asc",
                ),
                (
                    "with_metadata",
                    false,
                    "Include file size and modification time",
                ),
            ],
        ),
        Some(other) => format!("Unknown subcommand: {other}\nUse 'fs scheme' to list subcommands."),
        None => "fs subcommands:\n  \
                 read    Read file contents with line numbers\n  \
                 write   Create or overwrite a file\n  \
                 edit    Replace exact strings in a file\n  \
                 grep    Search file content using regex\n  \
                 glob    Find files and directories by glob patterns\n  \
                 scheme  Show parameter definitions [<subcommand>]"
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use vol_llm_tool::{ToolContext, ToolResult};

    fn ctx() -> ToolContext {
        ToolContext::for_test()
    }

    async fn run(command: &str) -> Result<ToolResult, String> {
        let cmd = crate::cli::parser::parse(command).unwrap();
        execute(cmd, &ctx()).await
    }

    #[tokio::test]
    async fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        let p = path.display().to_string();

        let w = run(&format!("write --file_path {p} --content 'hello world'"))
            .await
            .unwrap();
        assert!(w.success);

        let r = run(&format!("read --file_path {p}")).await.unwrap();
        assert!(r.success);
        assert!(r.content.contains("hello world"));
    }

    #[tokio::test]
    async fn edit_single_occurrence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "one two one").unwrap();
        let p = path.display().to_string();

        let r = run(&format!(
            "edit --file_path {p} --old_string two --new_string THREE"
        ))
        .await
        .unwrap();
        assert!(r.success);
        assert_eq!(fs::read_to_string(&path).unwrap(), "one THREE one");
    }

    #[tokio::test]
    async fn edit_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "dup dup dup").unwrap();
        let p = path.display().to_string();

        let r = run(&format!(
            "edit --file_path {p} --old_string dup --new_string x --replace_all"
        ))
        .await
        .unwrap();
        assert!(r.success);
        assert_eq!(fs::read_to_string(&path).unwrap(), "x x x");
    }

    #[tokio::test]
    async fn edit_ambiguous_without_replace_all_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "dup dup dup").unwrap();
        let p = path.display().to_string();

        let err = run(&format!(
            "edit --file_path {p} --old_string dup --new_string x"
        ))
        .await
        .unwrap_err();
        assert!(err.contains("replace_all"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn grep_content_mode_finds_matches() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("code.rs"), "fn main() {}\n// TODO: fix\n").unwrap();
        let d = dir.path().display().to_string();

        let r = run(&format!(
            "grep --pattern 'TODO' --path {d} --output_mode content"
        ))
        .await
        .unwrap();
        assert!(r.success);
        // GrepTool content mode reports match locations as `path:line`; the
        // matched line text itself is not echoed by the tool.
        assert!(r.content.contains("code.rs"), "unexpected: {}", r.content);
    }

    #[tokio::test]
    async fn glob_finds_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("x.rs"), "").unwrap();
        fs::write(dir.path().join("y.txt"), "").unwrap();
        // GlobTool requires search paths relative to the sandbox root (/);
        // absolute paths are rejected by design.
        let rel = dir
            .path()
            .display()
            .to_string()
            .trim_start_matches('/')
            .to_string();

        let r = run(&format!("glob --pattern '*.rs' --path {rel}"))
            .await
            .unwrap();
        assert!(r.success, "unexpected: {}", r.content);
        assert!(r.content.contains("x.rs"), "unexpected: {}", r.content);
        assert!(!r.content.contains("y.txt"), "unexpected: {}", r.content);
    }

    #[tokio::test]
    async fn scheme_lists_params() {
        let r = run("scheme read").await.unwrap();
        assert!(r.success);
        assert!(r.content.contains("--file_path"));
        assert!(r.content.contains("required"));
    }

    #[tokio::test]
    async fn scheme_without_arg_lists_subcommands() {
        let r = run("scheme").await.unwrap();
        assert!(r.success);
        assert!(r.content.contains("read"));
        assert!(r.content.contains("scheme"));
    }

    #[tokio::test]
    async fn json_envelope_wraps_result() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("j.txt");
        fs::write(&path, "json test").unwrap();
        let p = path.display().to_string();

        let r = run(&format!("read --file_path {p} --json")).await.unwrap();
        assert!(r.content.starts_with('{'), "unexpected: {}", r.content);
        assert!(
            r.content.contains("\"success\""),
            "unexpected: {}",
            r.content
        );
        assert!(r.content.contains("json test"), "unexpected: {}", r.content);
    }
}
