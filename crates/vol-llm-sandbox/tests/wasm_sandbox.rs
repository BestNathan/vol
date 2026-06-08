//! Unit tests for WasmSandbox.

use vol_llm_sandbox::registry::{SandboxConfig, WasmConfig, WasmModuleConfig};
use vol_llm_sandbox::Sandbox;

#[test]
fn test_wasm_config_defaults() {
    let toml_str = r#"
name = "wasm"
type = "wasm"
work_dir = "/tmp/wasm"

[wasm]
[[wasm.modules]]
name = "test"
path = "/opt/test.wasm"
"#;
    let config: SandboxConfig = toml::from_str(toml_str).unwrap();
    let wasm = config.wasm.unwrap();
    assert_eq!(wasm.max_memory_bytes, 134_217_728); // 128 MB default
    assert_eq!(wasm.max_execution_ms, 30_000);      // 30s default
    assert_eq!(wasm.modules.len(), 1);
    assert_eq!(wasm.modules[0].name, "test");
    assert_eq!(wasm.modules[0].path, "/opt/test.wasm");
    assert!(!wasm.modules[0].expose_as_tool);         // default false
}

#[test]
fn test_wasm_config_with_expose_as_tool() {
    let toml_str = r#"
name = "wasm"
type = "wasm"

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
    let config: SandboxConfig = toml::from_str(toml_str).unwrap();
    let wasm = config.wasm.unwrap();
    assert_eq!(wasm.max_memory_bytes, 268_435_456);
    assert_eq!(wasm.max_execution_ms, 60_000);
    assert_eq!(wasm.modules.len(), 2);
    assert!(wasm.modules[0].expose_as_tool);
    assert!(!wasm.modules[1].expose_as_tool);
}

#[cfg(feature = "wasm")]
#[test]
fn test_wasm_sandbox_rejects_nonexistent_module() {
    let work_dir = std::env::temp_dir().join("wasm_test_nonexistent");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();

    let config = WasmConfig {
        max_memory_bytes: 134_217_728,
        max_execution_ms: 30_000,
        modules: vec![WasmModuleConfig {
            name: "ghost".to_string(),
            path: "/nonexistent/path/module.wasm".to_string(),
            expose_as_tool: false,
        }],
    };

    let result = vol_llm_sandbox::wasm::WasmSandbox::new(
        "test".to_string(),
        work_dir.clone(),
        config,
    );
    assert!(result.is_err(), "Should fail: module file does not exist");

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[cfg(feature = "wasm")]
#[test]
fn test_wasm_sandbox_smoke() {
    let work_dir = std::env::temp_dir().join("wasm_test_smoke");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();

    // Minimal WASI module: just exit 0
    let wasm_bytes = wat::parse_str(r#"
        (module
            (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
            (func $main (export "_start")
                i32.const 0
                call $exit
            )
        )
    "#).expect("wat parse failed");

    let wasm_path = work_dir.join("smoke.wasm");
    std::fs::write(&wasm_path, &wasm_bytes).unwrap();

    let config = WasmConfig {
        max_memory_bytes: 134_217_728,
        max_execution_ms: 30_000,
        modules: vec![WasmModuleConfig {
            name: "smoke".to_string(),
            path: wasm_path.to_string_lossy().to_string(),
            expose_as_tool: false,
        }],
    };

    let sandbox = vol_llm_sandbox::wasm::WasmSandbox::new(
        "test".to_string(),
        work_dir.clone(),
        config,
    ).expect("Should create sandbox with valid wasm module");

    assert_eq!(sandbox.kind(), "wasm");
    assert_eq!(sandbox.name(), "test");

    let _ = std::fs::remove_dir_all(&work_dir);
}
