//! Error types for the cli-tool crate.
#[derive(Debug, thiserror::Error)]
pub enum CliToolError {
    #[error("placeholder")]
    Placeholder,
}
