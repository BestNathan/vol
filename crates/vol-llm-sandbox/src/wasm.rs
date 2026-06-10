//! Wasm sandbox — execute WebAssembly modules in a WASI environment.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use wasmtime::*;
use wasmtime_wasi::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::{DirPerms, FilePerms};

use crate::registry::{WasmConfig, WasmModuleConfig};
use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxResult,
};

/// Wasm sandbox — executes `.wasm` modules in an isolated WASI environment.
pub struct WasmSandbox {
    name: String,
    work_dir: PathBuf,
    root_path: PathBuf,
    engine: Engine,
    modules: HashMap<String, Module>,
    #[allow(dead_code)]
    module_configs: Vec<WasmModuleConfig>,
    max_execution: Duration,
}

impl WasmSandbox {
    /// Create a new WasmSandbox, precompiling all configured modules.
    pub fn new(name: String, work_dir: PathBuf, config: WasmConfig) -> SandboxResult<Self> {
        let mut engine_config = Config::new();
        engine_config.wasm_multi_memory(true);
        let engine = Engine::new(&engine_config)
            .map_err(|e| SandboxError::Io(std::io::Error::other(e.to_string())))?;

        let mut modules = HashMap::new();
        for mc in &config.modules {
            let wasm_bytes = std::fs::read(&mc.path).map_err(|e| SandboxError::Io(e))?;
            let module = Module::from_binary(&engine, &wasm_bytes).map_err(|e| {
                SandboxError::Io(std::io::Error::other(format!(
                    "failed to compile {}: {}",
                    mc.path, e
                )))
            })?;
            modules.insert(mc.name.clone(), module);
        }

        let root_path = work_dir.clone();
        std::fs::create_dir_all(&work_dir).map_err(SandboxError::Io)?;

        Ok(Self {
            name,
            work_dir,
            root_path,
            engine,
            modules,
            module_configs: config.modules,
            max_execution: Duration::from_millis(config.max_execution_ms),
        })
    }

    /// Return the subset of module configs that should be exposed as agent tools.
    pub fn tool_modules(&self) -> &[WasmModuleConfig] {
        &self.module_configs
    }
}

#[async_trait]
impl Sandbox for WasmSandbox {
    fn kind(&self) -> &str {
        "wasm"
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(&self.work_dir).map_err(SandboxError::Io)
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        Ok(())
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        let resolved = self.root_path.join(rel);
        let normalized = crate::normalize_path(&resolved);
        let normalized_root = crate::normalize_path(&self.root_path);
        if !normalized.starts_with(&normalized_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        Ok(normalized)
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> SandboxResult<Vec<u8>> {
        let content = std::fs::read(path).map_err(SandboxError::Io)?;
        let start = offset.unwrap_or(0) as usize;
        let end = limit.map(|l| start + l as usize).unwrap_or(content.len());
        Ok(content[start..end.min(content.len())].to_vec())
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(SandboxError::Io)?;
        }
        std::fs::write(path, content).map_err(SandboxError::Io)
    }

    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()> {
        std::fs::create_dir_all(path).map_err(SandboxError::Io)
    }

    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>> {
        let entries: Vec<DirEntry> = std::fs::read_dir(path)
            .map_err(SandboxError::Io)?
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let file_type = e
                    .file_type()
                    .map(|ft| {
                        if ft.is_dir() {
                            FileType::Directory
                        } else if ft.is_file() {
                            FileType::File
                        } else if ft.is_symlink() {
                            FileType::Symlink
                        } else {
                            FileType::Other
                        }
                    })
                    .unwrap_or(FileType::Other);
                DirEntry { name, file_type }
            })
            .collect();
        Ok(entries)
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        let meta = std::fs::metadata(path).map_err(SandboxError::Io)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let file_type = if meta.is_dir() {
            FileType::Directory
        } else if meta.is_file() {
            FileType::File
        } else if meta.is_symlink() {
            FileType::Symlink
        } else {
            FileType::Other
        };
        Ok(FileMetadata {
            size: meta.len(),
            mtime,
            file_type,
        })
    }

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        let module = self.modules.get(&req.program).ok_or_else(|| {
            SandboxError::Wasm(format!(
                "unknown wasm module: {} (available: {:?})",
                req.program,
                self.modules.keys().collect::<Vec<_>>()
            ))
        })?;

        let work_dir = self.work_dir.clone();
        let engine = self.engine.clone();
        let wasm_module = module.clone();
        let max_execution = self.max_execution;

        tokio::task::spawn_blocking(move || {
            execute_wasm_module(&engine, &wasm_module, &work_dir, &req, max_execution)
        })
        .await
        .map_err(|e| SandboxError::Io(std::io::Error::other(e.to_string())))?
    }
}

/// Execute a single Wasm module in a blocking context (called from spawn_blocking).
fn execute_wasm_module(
    engine: &Engine,
    module: &Module,
    work_dir: &Path,
    req: &crate::CommandRequest,
    _max_execution: Duration,
) -> SandboxResult<CommandOutput> {
    // Set up in-memory pipes for stdout and stderr
    let stdout_pipe = MemoryOutputPipe::new(usize::MAX / 2);
    let stderr_pipe = MemoryOutputPipe::new(usize::MAX / 2);

    let mut builder = WasiCtxBuilder::new();

    // Redirect stdout/stderr so we can capture the output
    builder.stdout(stdout_pipe.clone());
    builder.stderr(stderr_pipe.clone());

    // Provide stdin data if present
    if let Some(ref stdin_data) = req.stdin {
        let stdin_pipe = MemoryInputPipe::new(stdin_data.clone());
        builder.stdin(stdin_pipe);
    }

    // Environment variables
    for (k, v) in &req.env {
        builder.env(k, v);
    }

    // Arguments: program name first, then additional args
    builder.arg(&req.program);
    for arg in &req.args {
        builder.arg(arg);
    }

    // Preopen the work directory at root ("/")
    builder
        .preopened_dir(work_dir, "/", DirPerms::all(), FilePerms::all())
        .map_err(|e| {
            SandboxError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to preopen dir: {}", e),
            ))
        })?;

    // Build the WASIp1 context
    let wasi_ctx = builder.build_p1();

    // Set up the linker with WASIp1 support
    let mut linker: Linker<WasiP1Ctx> = Linker::new(engine);
    preview1::add_to_linker_sync(&mut linker, |cx| cx).map_err(|e| {
        SandboxError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("failed to add WASI to linker: {}", e),
        ))
    })?;

    // Create the store and instantiate the module
    let mut store = Store::new(engine, wasi_ctx);
    let instance = linker
        .instantiate(&mut store, module)
        .map_err(|e| SandboxError::Wasm(format!("instantiation failed: {}", e)))?;

    // Call the WASI _start entry point
    let exit_code = match instance.get_typed_func::<(), ()>(&mut store, "_start") {
        Ok(start) => match start.call(&mut store, ()) {
            Ok(_) => 0,
            Err(trap) => {
                // Check for the special I32Exit that WASI uses for exit codes
                if let Some(exit) = trap.downcast_ref::<wasmtime_wasi::I32Exit>() {
                    exit.0
                } else {
                    return Err(SandboxError::Wasm(format!("wasm trap: {}", trap)));
                }
            }
        },
        Err(_) => {
            return Err(SandboxError::Wasm(
                "no _start export found — WASI command modules must export _start".to_string(),
            ));
        }
    };

    // Capture the output from the memory pipes
    let stdout = stdout_pipe.contents().to_vec();
    let stderr = stderr_pipe.contents().to_vec();

    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code,
        killed_by_signal: None,
    })
}
