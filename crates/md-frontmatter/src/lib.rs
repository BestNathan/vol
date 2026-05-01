//! md-frontmatter: Parse YAML frontmatter from markdown files.
//!
//! # Quick Start
//!
//! ```rust
//! use md_frontmatter::ParsedDoc;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct MyDoc {
//!     title: String,
//!     tags: Vec<String>,
//! }
//!
//! // Sync parse from string
//! let doc = md_frontmatter::parse::<MyDoc>("---\ntitle: Hello\ntags: [a, b]\n---\n\nBody here").unwrap();
//! assert_eq!(doc.frontmatter.title, "Hello");
//! assert_eq!(doc.body, "\n\nBody here");
//! ```

pub mod error;
pub mod io;
pub mod parser;

pub use error::MdFmError;
pub use parser::{ParsedDoc, parse, to_string, update_frontmatter};
pub use io::{from_path, write, scan_dir};

/// Result alias for crate operations.
pub type Result<T> = std::result::Result<T, MdFmError>;
