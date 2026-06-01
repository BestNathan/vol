//! RgCliBackend — delegates to the `rg` (ripgrep) binary.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::OnceLock;

use crate::backend::GrepBackend;
use crate::GrepParams;
use crate::SearchResult;
use crate::MODE_COUNT;
use crate::MODE_CONTENT;
use crate::MODE_FILES;

static RG_AVAILABLE: OnceLock<bool> = OnceLock::new();

pub struct RgCliBackend;

impl RgCliBackend {
    fn detect() -> bool {
        *RG_AVAILABLE.get_or_init(|| {
            Command::new("rg")
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
    }
}

#[async_trait::async_trait]
impl GrepBackend for RgCliBackend {
    fn is_available() -> bool {
        Self::detect()
    }

    async fn search(params: &GrepParams, root: &Path) -> Result<Vec<SearchResult>, String> {
        let mut cmd = Command::new("rg");
        cmd.args([
            "--no-heading",
            "--with-filename",
            "--color",
            "never",
            "--max-depth",
            "50",
            "--max-filesize",
            "10M",
        ]);

        // Output mode flag
        match params.output_mode.as_str() {
            MODE_FILES => {
                cmd.arg("-l");
            }
            MODE_COUNT => {
                cmd.arg("-c");
            }
            MODE_CONTENT => {
                cmd.arg("-n");
            }
            _ => unreachable!(),
        };

        // Case sensitivity
        if params.case_sensitive {
            cmd.arg("-s");
        } else {
            cmd.arg("-i");
        }

        // Glob filter
        if let Some(ref glob) = params.glob {
            cmd.arg("-g").arg(glob);
        }

        // Pattern and path
        cmd.arg("--")
            .arg(&params.pattern)
            .arg(root);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn rg process: {}", e))?;

        let output = tokio::task::spawn_blocking(move || child.wait_with_output())
            .await
            .map_err(|e| format!("rg process join error: {}", e))?
            .map_err(|e| format!("rg process error: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Exit code 1 means no matches — not an error for grep
            if output.status.code() == Some(1) {
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
