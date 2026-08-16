//! vol-llm-fs: unified CLI-style `fs` tool for file operations.
//!
//! Provides the `fs` tool — a single entry point (CLI-style command string)
//! over the builtin file op tools (read/write/edit/grep/glob), modeled on
//! the `task` CLI in `vol-llm-task`. Delegates to the existing tools'
//! implementations; contains no file op logic of its own.

pub mod cli;
pub mod tools;
