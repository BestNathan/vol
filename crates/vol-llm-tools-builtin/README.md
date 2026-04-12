# vol-llm-tools-builtin

Built-in tools for LLM Agent, providing file operations, search, and shell execution capabilities.

## Tools

| Tool | Crate | Description |
|------|-------|-------------|
| `read_file` | `vol-llm-tools-builtin-read` | Read file content with line numbers (cat -n format) |
| `write_file` | `vol-llm-tools-builtin-write` | Create or overwrite files |
| `edit_file` | `vol-llm-tools-builtin-edit` | Precise string replacement with uniqueness validation |
| `glob` | `vol-llm-tools-builtin-glob` | File path pattern matching (e.g., `**/*.rs`) |
| `grep` | `vol-llm-tools-builtin-grep` | File content regex search with multiple output modes |
| `bash` | `vol-llm-tools-builtin-bash` | Shell command execution with security blacklist |

## Usage

### Register all tools

```rust
use vol_llm_tools_builtin::register_all;

let mut registry = ToolRegistry::new();
register_all(&mut registry);
```

### Register individual tools

```rust
use vol_llm_tools_builtin::read_tool::ReadTool;

let mut registry = ToolRegistry::new();
registry.register(ReadTool::new());
```

## Tool Parameters

### read_file
- `file_path` (required): Absolute path to the file
- `offset` (optional, default 0): Line offset to skip (0-based)
- `limit` (optional, default 2000): Maximum lines to read

### write_file
- `file_path` (required): Absolute path to the file
- `content` (required): Content to write

### edit_file
- `file_path` (required): Absolute path to the file
- `old_string` (required): Exact string to find and replace
- `new_string` (required): String to replace with
- `replace_all` (optional, default false): Replace all occurrences

### glob
- `pattern` (required): Glob pattern (e.g., `**/*.rs`)
- `path` (optional, default "."): Root directory to search

### grep
- `pattern` (required): Regex pattern to search
- `path` (optional, default "."): Root directory to search
- `glob` (optional): File pattern filter (e.g., `*.rs`)
- `output_mode` (optional, default "files_with_matches"): `files_with_matches`, `count`, or `content`
- `case_sensitive` (optional, default false): Case-sensitive search

### bash
- `command` (required): Shell command to execute
- `timeout` (optional, default 120000): Timeout in milliseconds
- `working_dir` (optional): Working directory for the command

## Security

The `bash` tool includes a security blacklist for dangerous commands:
- `rm -rf /` and similar destructive deletes
- Fork bombs (`:(){:|:&}:`)
- Disk formatting commands (`mkfs`)
- Device writes (`dd of=/dev/...`)
- Curl/wget pipe to bash
- Reverse shells (netcat, bash /dev/tcp)

**Note**: The blacklist is not exhaustive. For production use, consider additional safeguards like HITL approval.

## License

MIT
