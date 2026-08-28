//! Tests for WasmSandbox — config deserialization and runtime behaviour.
//!
//! Tests gated behind `#[cfg(feature = "wasm")]` exercise the full sandbox
//! trait (execute, read_file, write_file, resolve_path, etc.) using a
//! minimal WASI module compiled from WAT at test time.

use vol_llm_sandbox::SandboxSpec;
// WasmConfig / WasmModuleConfig are only used inside #[cfg(feature = "wasm")]

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Config deserialization — no wasm feature needed
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_wasm_config_defaults() {
    let toml_str = r#"
name = "wasm"
provider = "wasm"
work_dir = "/tmp/wasm"

[wasm]
[[wasm.modules]]
name = "test"
path = "/opt/test.wasm"
"#;
    let spec: SandboxSpec = toml::from_str(toml_str).unwrap();
    let wasm = spec.config.as_wasm().unwrap();
    assert_eq!(wasm.max_memory_bytes, 134_217_728); // 128 MB default
    assert_eq!(wasm.max_execution_ms, 30_000); // 30s default
    assert_eq!(wasm.modules.len(), 1);
    let m0 = wasm.modules.first().expect("one module");
    assert_eq!(m0.name, "test");
    assert_eq!(m0.path, "/opt/test.wasm");
    assert!(!m0.expose_as_tool); // default false
}

#[test]
fn test_wasm_config_with_expose_as_tool() {
    let toml_str = r#"
name = "wasm"
provider = "wasm"

[wasm]
max_memory_bytes = 268435456
max_execution_ms = 60000

[[wasm.modules]]
name = "linter"
path = "/opt/linter.wasm"
expose_as_tool = true

[[wasm.modules]]
name = "runner"
path = "/opt/runner.wasm"
"#;
    let spec: SandboxSpec = toml::from_str(toml_str).unwrap();
    let wasm = spec.config.as_wasm().unwrap();
    assert_eq!(wasm.max_memory_bytes, 268_435_456);
    assert_eq!(wasm.max_execution_ms, 60_000);
    assert_eq!(wasm.modules.len(), 2);
    assert!(wasm.modules.first().expect("module 0").expose_as_tool);
    assert!(!wasm.modules.get(1).expect("module 1").expose_as_tool);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Runtime tests — require `wasm` feature (wasmtime + wasmtime-wasi)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(feature = "wasm")]
mod runtime {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;
    use vol_llm_sandbox::wasm::WasmSandbox;
    use vol_llm_sandbox::{CommandRequest, Sandbox};
    use vol_llm_sandbox::{WasmConfig, WasmModuleConfig};

    /// Helper: create a fresh work_dir and return it (caller cleans up).
    fn setup_work_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wasm_test_{label}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Helper: compile a WAT snippet to binary wasm and write it to `work_dir/name.wasm`.
    fn write_wat_module(work_dir: &std::path::Path, name: &str, wat: &str) -> std::path::PathBuf {
        let wasm_bytes = wat::parse_str(wat).expect("wat parse failed");
        let path = work_dir.join(format!("{name}.wasm"));
        std::fs::write(&path, &wasm_bytes).unwrap();
        path
    }

    /// Helper: build a WasmSandbox with a single named module at `path`.
    fn build_sandbox(
        name: &str,
        work_dir: &std::path::Path,
        module_name: &str,
        module_path: &std::path::Path,
    ) -> WasmSandbox {
        let config = WasmConfig {
            max_memory_bytes: 134_217_728,
            max_execution_ms: 30_000,
            modules: vec![WasmModuleConfig {
                name: module_name.to_string(),
                path: module_path.to_string_lossy().to_string(),
                expose_as_tool: false,
            }],
        };
        WasmSandbox::new(name.to_string(), work_dir.to_path_buf(), config)
            .expect("should create sandbox with valid wasm module")
    }

    /// Minimal WASI module: empty _start that returns (equivalent to exit 0).
    const EXIT0_WAT: &str = r#"
        (module
            (func (export "_start"))
        )
    "#;

    /// WASI module that exits with code 42 via proc_exit.
    /// NOTE: exercises the I32Exit downcast path in wasm.rs:269.
    /// The module must export its memory — the wiggle shim requires a
    /// "memory" export before it will call any host function.
    const EXIT42_WAT: &str = r#"
        (module
            (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
            (memory (export "memory") 1)
            (func (export "_start")
                i32.const 42
                call $exit
            )
        )
    "#;

    // ── Construction & metadata ───────────────────────────────────────

    #[test]
    fn test_new_rejects_nonexistent_module() {
        let work_dir = setup_work_dir("nonexistent");
        let config = WasmConfig {
            max_memory_bytes: 134_217_728,
            max_execution_ms: 30_000,
            modules: vec![WasmModuleConfig {
                name: "ghost".to_string(),
                path: "/nonexistent/path/module.wasm".to_string(),
                expose_as_tool: false,
            }],
        };
        let result = WasmSandbox::new("test".to_string(), work_dir.clone(), config);
        assert!(result.is_err(), "Should fail: module file does not exist");
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[test]
    fn test_kind_and_name() {
        let work_dir = setup_work_dir("smoke");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("my-wasm-sb", &work_dir, "smoke", &wasm_path);
        assert_eq!(sandbox.kind(), "wasm");
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[test]
    fn test_root_path() {
        let work_dir = setup_work_dir("root");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);
        assert_eq!(sandbox.root_path(), Some(work_dir.as_path()));
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_new_sandbox_has_work_dir() {
        let work_dir = setup_work_dir("start");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);
        // In the new lifecycle, work_dir is set on construction (no start() needed).
        assert!(work_dir.exists());
        assert_eq!(sandbox.root_path(), Some(work_dir.as_path()));
        // Cleanup: recreate work_dir so the Drop guard doesn't fail
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    // ── Path resolution ───────────────────────────────────────────────

    #[test]
    fn test_resolve_path_relative() {
        let work_dir = setup_work_dir("resolve");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);
        let resolved = sandbox.resolve_path("subdir/file.txt").unwrap();
        assert!(resolved.starts_with(&work_dir));
        assert!(resolved.ends_with("subdir/file.txt"));
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[test]
    fn test_resolve_path_rejects_absolute() {
        let work_dir = setup_work_dir("resolve_abs");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);
        assert!(sandbox.resolve_path("/etc/passwd").is_err());
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[test]
    fn test_resolve_path_rejects_traversal() {
        let work_dir = setup_work_dir("resolve_trav");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);
        assert!(sandbox.resolve_path("../../../etc/passwd").is_err());
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    // ── File I/O ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_write_and_read_file() {
        let work_dir = setup_work_dir("fileio");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);

        let path = work_dir.join("hello.txt");
        sandbox
            .write_file(&path, b"wasm test content")
            .await
            .unwrap();
        let content = sandbox.read_file(&path, None, None).await.unwrap();
        assert_eq!(content, b"wasm test content");
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_read_file_with_offset_and_limit() {
        let work_dir = setup_work_dir("fileio2");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);

        let path = work_dir.join("data.bin");
        sandbox.write_file(&path, b"0123456789").await.unwrap();
        // offset=3, limit=4 → "3456"
        let content = sandbox.read_file(&path, Some(3), Some(4)).await.unwrap();
        assert_eq!(content, b"3456");
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_create_dir_all_and_read_dir() {
        let work_dir = setup_work_dir("dirio");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);

        let deep = work_dir.join("a").join("b").join("c");
        sandbox.create_dir_all(&deep).await.unwrap();
        assert!(deep.is_dir());

        sandbox
            .write_file(&work_dir.join("a").join("b").join("file.txt"), b"x")
            .await
            .unwrap();
        let entries = sandbox
            .read_dir(&work_dir.join("a").join("b"))
            .await
            .unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"c"));
        assert!(names.contains(&"file.txt"));
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_metadata() {
        let work_dir = setup_work_dir("meta");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);

        let path = work_dir.join("meta.txt");
        sandbox.write_file(&path, b"hello").await.unwrap();
        let meta = sandbox.metadata(&path).await.unwrap();
        assert_eq!(meta.size, 5);
        assert!(meta.mtime > 0);
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    // ── Execute ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_smoke_exit_zero() {
        let work_dir = setup_work_dir("exec_smoke");
        let wasm_path = write_wat_module(&work_dir, "exit0", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "exit0", &wasm_path);

        let req = CommandRequest {
            program: "exit0".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sandbox.execute(req).await.unwrap();
        assert_eq!(output.exit_code, 0);
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_execute_stdout() {
        let work_dir = setup_work_dir("exec_stdout");
        // WASI module that writes "hello from wasm\n" to stdout via fd_write
        let wasm_path = write_wat_module(
            &work_dir,
            "hello",
            r#"
            (module
                (import "wasi_snapshot_preview1" "fd_write"
                    (func $fd_write (param i32 i32 i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "proc_exit"
                    (func $exit (param i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "hello from wasm\n")
                (func (export "_start")
                    ;; ciovec: iov_base=0, iov_len=16
                    (i32.store (i32.const 100) (i32.const 0))
                    (i32.store (i32.const 104) (i32.const 16))
                    ;; fd_write(1, ciovec_ptr=100, iovcnt=1, nwritten_ptr=200)
                    (call $fd_write
                        (i32.const 1) (i32.const 100) (i32.const 1) (i32.const 200))
                    drop
                    i32.const 0
                    call $exit
                )
            )
        "#,
        );
        let sandbox = build_sandbox("sb", &work_dir, "hello", &wasm_path);

        let req = CommandRequest {
            program: "hello".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sandbox.execute(req).await.unwrap();
        assert_eq!(output.exit_code, 0);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello from wasm"));
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_execute_nonzero_exit() {
        let work_dir = setup_work_dir("exec_exit42");
        let wasm_path = write_wat_module(&work_dir, "exit42", EXIT42_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "exit42", &wasm_path);

        let req = CommandRequest {
            program: "exit42".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sandbox.execute(req).await.unwrap();
        assert_eq!(output.exit_code, 42);
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_execute_unknown_module_errors() {
        let work_dir = setup_work_dir("exec_unknown");
        let wasm_path = write_wat_module(&work_dir, "smoke", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "smoke", &wasm_path);

        let req = CommandRequest {
            program: "nonexistent_module".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let result = sandbox.execute(req).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown wasm module") || err.contains("nonexistent_module"));
        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[tokio::test]
    async fn test_execute_with_args_and_env() {
        let work_dir = setup_work_dir("exec_args");
        let wasm_path = write_wat_module(&work_dir, "args", EXIT0_WAT);
        let sandbox = build_sandbox("sb", &work_dir, "args", &wasm_path);

        let mut env = HashMap::new();
        env.insert("MY_VAR".to_string(), "my_value".to_string());
        let req = CommandRequest {
            program: "args".to_string(),
            args: vec!["--flag".to_string(), "value".to_string()],
            env,
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sandbox.execute(req).await.unwrap();
        assert_eq!(output.exit_code, 0);
        let _ = std::fs::remove_dir_all(&work_dir);
    }
}
