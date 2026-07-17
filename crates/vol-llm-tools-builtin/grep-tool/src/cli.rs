//! RgCliBackend — delegates to the `rg` (ripgrep) binary via sandbox.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use vol_llm_sandbox::{CommandRequest, Sandbox};

use crate::backend::GrepBackend;
use crate::GrepParams;
use crate::SearchResult;
use crate::MODE_CONTENT;
use crate::MODE_COUNT;
use crate::MODE_FILES;

pub struct RgCliBackend;

#[async_trait::async_trait]
impl GrepBackend for RgCliBackend {
    async fn search(
        params: &GrepParams,
        root: &Path,
        sandbox: &dyn Sandbox,
    ) -> Result<Vec<SearchResult>, String> {
        let mut args: Vec<String> = vec![
            "--no-heading".to_string(),
            "--with-filename".to_string(),
            "--color".to_string(),
            "never".to_string(),
            "--max-depth".to_string(),
            "50".to_string(),
            "--max-filesize".to_string(),
            "10M".to_string(),
        ];

        // Output mode flag
        match params.output_mode.as_str() {
            MODE_FILES => args.push("-l".to_string()),
            MODE_COUNT => args.push("-c".to_string()),
            MODE_CONTENT => args.push("-n".to_string()),
            _ => unreachable!(),
        };

        // Case sensitivity
        if params.case_sensitive {
            args.push("-s".to_string());
        } else {
            args.push("-i".to_string());
        }

        // Glob filter
        if let Some(ref glob) = params.glob {
            args.push("-g".to_string());
            args.push(glob.clone());
        }

        // Pattern and path
        args.push("--".to_string());
        args.push(params.pattern.clone());
        args.push(root.to_string_lossy().to_string());

        let req = CommandRequest {
            program: "rg".to_string(),
            args,
            env: HashMap::new(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(30),
        };

        let output = sandbox
            .execute(req)
            .await
            .map_err(|e| format!("rg execution failed: {e}"))?;

        if output.exit_code != 0 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Exit code 1 means no matches — not an error for grep
            if output.exit_code == 1 {
                return Ok(vec![]);
            }
            return Err(format!("rg failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_rg_output(&stdout, params.output_mode.as_str())
    }
}

fn parse_rg_output(stdout: &str, mode: &str) -> Result<Vec<SearchResult>, String> {
    let mut results: Vec<SearchResult> = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match mode {
            MODE_FILES => {
                results.push(SearchResult {
                    path: PathBuf::from(line),
                    match_count: 1,
                    line_numbers: Vec::new(),
                });
            }
            MODE_COUNT => {
                if let Some(colon_idx) = line.rfind(':') {
                    let path = PathBuf::from(&line[..colon_idx]);
                    let count: usize = line[colon_idx + 1..].parse().unwrap_or(0);
                    results.push(SearchResult {
                        path,
                        match_count: count,
                        line_numbers: Vec::new(),
                    });
                }
            }
            MODE_CONTENT => {
                // Format is path:line_number:content or path:line_number:
                let mut parts = line.splitn(3, ':');
                if let (Some(path_str), Some(line_str)) = (parts.next(), parts.next()) {
                    let line_num: usize = line_str.parse().unwrap_or(0);
                    let path = PathBuf::from(path_str);
                    match results.last_mut() {
                        Some(ref mut last) if last.path == path => {
                            last.line_numbers.push(line_num);
                            last.match_count += 1;
                        }
                        _ => {
                            results.push(SearchResult {
                                path,
                                match_count: 1,
                                line_numbers: vec![line_num],
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_files_mode() {
        let output = "src/main.rs\nsrc/lib.rs\n";
        let results = parse_rg_output(output, MODE_FILES).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path.to_string_lossy(), "src/main.rs");
        assert_eq!(results[1].path.to_string_lossy(), "src/lib.rs");
    }

    #[test]
    fn test_parse_count_mode() {
        let output = "src/main.rs:5\nsrc/lib.rs:12\n";
        let results = parse_rg_output(output, MODE_COUNT).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].match_count, 5);
        assert_eq!(results[1].match_count, 12);
    }

    #[test]
    fn test_parse_content_mode() {
        let output = "src/main.rs:10:hello\nsrc/main.rs:15:world\nsrc/lib.rs:3:test\n";
        let results = parse_rg_output(output, MODE_CONTENT).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path.to_string_lossy(), "src/main.rs");
        assert_eq!(results[0].line_numbers, vec![10, 15]);
        assert_eq!(results[1].path.to_string_lossy(), "src/lib.rs");
        assert_eq!(results[1].line_numbers, vec![3]);
    }
}
