use std::path::PathBuf;

/// Error type for all frontmatter operations.
#[derive(Debug, thiserror::Error)]
pub enum MdFmError {
    #[error("frontmatter parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("file is not valid UTF-8: {path}")]
    InvalidUtf8 { path: PathBuf },

    #[error("no frontmatter found: {path}")]
    MissingFrontmatter { path: PathBuf },
}
