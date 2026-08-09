//! Cross-tool chain integration tests.
//!
//! Each test runs multiple tools in sequence within the same restricted sandbox,
//! verifying that tools can interoperate correctly.

// Tests intentionally unwrap after asserting is_err()/is_ok(); the crate
// inherits the workspace's deny-level unwrap/expect lints.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod fixtures;

use serde_json::json;
use vol_llm_tool::ExecutableTool;
use vol_llm_tools_builtin::{BashTool, EditTool, GlobTool, GrepTool, ReadTool, WriteTool};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Chain: write → read → edit → read
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_chain_write_read_edit_read() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    let file_path = tmp.path().join("doc.txt").to_str().unwrap().to_string();

    // 1. Write
    let write = WriteTool::new();
    let result = write
        .execute(
            &json!({"file_path": file_path, "content": "alpha beta gamma"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.success);

    // 2. Read — verify content
    let read = ReadTool::new();
    let result = read
        .execute(&json!({"file_path": file_path}), &ctx)
        .await
        .unwrap();
    assert!(result.content.contains("alpha beta gamma"));

    // 3. Edit — replace "beta" with "delta"
    let edit = EditTool::new();
    let result = edit
        .execute(
            &json!({"file_path": file_path, "old_string": "beta", "new_string": "delta"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.success);

    // 4. Read — verify the edit
    let result = read
        .execute(&json!({"file_path": file_path}), &ctx)
        .await
        .unwrap();
    assert!(result.content.contains("alpha delta gamma"));
    assert!(!result.content.contains("alpha beta gamma"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Chain: glob → grep → read
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_chain_glob_grep_read() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    fixtures::populate_files(
        &tmp,
        &[
            ("src/main.rs", "fn main() {\n    println!(\"hello\");\n}"),
            ("src/lib.rs", "pub fn greet() -> &'static str { \"hello\" }"),
            ("tests/test.rs", "#[test]\nfn test_greet() {}"),
            ("README.md", "# My Project\n\nA hello world project."),
        ],
    );

    // 1. Glob — find all .rs files
    let glob = GlobTool::new();
    let result = glob
        .execute(&json!({"pattern": "**/*.rs"}), &ctx)
        .await
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    let paths: Vec<&str> = output
        .get("matches")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"src/main.rs"));
    assert!(paths.contains(&"src/lib.rs"));
    assert!(paths.contains(&"tests/test.rs"));

    // 2. Grep — find files containing "hello" (only .rs files)
    let grep = GrepTool::new();
    let result = grep
        .execute(
            &json!({
                "pattern": "hello",
                "path": tmp.path().to_str().unwrap(),
                "glob": "*.rs",
                "output_mode": "files_with_matches"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.content.contains("main.rs"));
    assert!(result.content.contains("lib.rs"));
    assert!(!result.content.contains("test.rs"));

    // 3. Read — read the matched file and verify content
    let read = ReadTool::new();
    let result = read
        .execute(
            &json!({"file_path": tmp.path().join("src/main.rs").to_str().unwrap()}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.content.contains("println!(\"hello\")"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Chain: bash → write → bash → read
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_chain_bash_write_bash_read() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    let bash = BashTool::new();
    let write = WriteTool::new();
    let read = ReadTool::new();

    // 1. Bash: generate data
    let result = bash
        .execute(
            &json!({"command": "echo 'generated content from bash'"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.success);
    assert!(result.content.contains("generated content from bash"));

    // 2. Write: save to file
    let file_path = tmp.path().join("output.txt").to_str().unwrap().to_string();
    let result = write
        .execute(
            &json!({"file_path": file_path, "content": "data from write tool"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.success);

    // 3. Bash: verify file exists and has content
    let result = bash
        .execute(&json!({"command": format!("cat {}", file_path)}), &ctx)
        .await
        .unwrap();
    assert!(result.success);
    assert!(result.content.contains("data from write tool"));

    // 4. Read: verify through read tool
    let result = read
        .execute(&json!({"file_path": file_path}), &ctx)
        .await
        .unwrap();
    assert!(result.content.contains("data from write tool"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Chain: glob → edit (batch) → grep
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_chain_glob_edit_grep() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    fixtures::populate_files(
        &tmp,
        &[
            ("a.txt", "TODO: fix bug\nTODO: add test\ndone"),
            ("b.txt", "TODO: refactor\nall good"),
            ("c.txt", "nothing to do"),
        ],
    );

    // 1. Glob — find all .txt files
    let glob = GlobTool::new();
    let result = glob
        .execute(&json!({"pattern": "*.txt"}), &ctx)
        .await
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    let paths: Vec<String> = output
        .get("matches")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap().to_string())
        .collect();

    // 2. Edit — replace "TODO" with "DONE" in each file.
    //    Files that don't contain "TODO" (c.txt) error gracefully with
    //    "not found in file" — expected, and must not fail the chain.
    let edit = EditTool::new();
    for path in &paths {
        let full_path = tmp.path().join(path).to_str().unwrap().to_string();
        let result = edit
            .execute(
                &json!({
                    "file_path": full_path,
                    "old_string": "TODO",
                    "new_string": "DONE",
                    "replace_all": true
                }),
                &ctx,
            )
            .await;
        match result {
            Ok(r) => assert!(r.success, "Edit reported failure for {path}"),
            Err(e) => assert!(
                e.to_string().contains("not found in file"),
                "Unexpected edit failure for {path}: {e:?}"
            ),
        }
    }

    // 3. Grep — verify "TODO" is gone and "DONE" is present
    let grep = GrepTool::new();
    let result = grep
        .execute(
            &json!({
                "pattern": "TODO",
                "path": tmp.path().to_str().unwrap(),
                "output_mode": "files_with_matches"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result.content.contains("No matches"),
        "Expected no TODO matches, got: {}",
        result.content
    );

    let result = grep
        .execute(
            &json!({
                "pattern": "DONE",
                "path": tmp.path().to_str().unwrap(),
                "output_mode": "files_with_matches"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.content.contains("a.txt"));
    assert!(result.content.contains("b.txt"));
    assert!(!result.content.contains("c.txt"));
}
