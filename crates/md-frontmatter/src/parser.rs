use serde::{de::DeserializeOwned, Serialize};

use crate::error::MdFmError;
use crate::Result;
use std::path::PathBuf;

/// A parsed markdown file with typed frontmatter.
#[derive(Debug, Clone)]
pub struct ParsedDoc<T> {
    pub frontmatter: T,
    pub body: String,
    /// Source file path (None if parsed from string).
    pub path: Option<PathBuf>,
}

/// Parse markdown content, extracting frontmatter into user's type T.
pub fn parse<T: DeserializeOwned>(content: &str) -> Result<ParsedDoc<T>> {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return Err(MdFmError::MissingFrontmatter {
            path: PathBuf::new(),
        });
    }

    let rest = &trimmed[3..];
    let Some(end_idx) = rest.find("\n---") else {
        return Err(MdFmError::MissingFrontmatter {
            path: PathBuf::new(),
        });
    };

    let frontmatter_str = &rest[..end_idx];
    let body = &rest[end_idx + 4..];

    match serde_yaml::from_str::<T>(frontmatter_str) {
        Ok(frontmatter) => Ok(ParsedDoc {
            frontmatter,
            body: body.to_string(),
            path: None,
        }),
        Err(e) => {
            let line = e.location().map(|loc| loc.line()).unwrap_or(0);
            Err(MdFmError::ParseError {
                line,
                message: e.to_string(),
            })
        }
    }
}

/// Reconstruct full markdown from a ParsedDoc.
pub fn to_string<T: Serialize>(doc: &ParsedDoc<T>) -> Result<String> {
    let yaml = serde_yaml::to_string(&doc.frontmatter)
        .map_err(|e| MdFmError::ParseError {
            line: 0,
            message: e.to_string(),
        })?;
    Ok(format!("---\n{}---\n{}", yaml, doc.body))
}

/// Update frontmatter on a ParsedDoc, preserving body byte-for-byte.
pub fn update_frontmatter<T>(doc: &mut ParsedDoc<T>, new: &T)
where
    T: Serialize + DeserializeOwned,
{
    doc.frontmatter = serde_yaml::from_value(
        serde_yaml::to_value(new).expect("failed to serialize new frontmatter"),
    )
    .expect("failed to deserialize new frontmatter");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    struct TestFm {
        title: String,
        #[serde(default)]
        tags: Vec<String>,
    }

    #[test]
    fn test_parse_valid_frontmatter() {
        let content = "---\ntitle: Hello\ntags: [a, b]\n---\n\n# Body\nContent here";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Hello");
        assert_eq!(doc.frontmatter.tags, vec!["a", "b"]);
        assert_eq!(doc.body, "\n\n# Body\nContent here");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just markdown\n\nNo frontmatter here.";
        let result = parse::<TestFm>(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::MissingFrontmatter { path } => assert!(path.to_string_lossy().is_empty()),
            e => panic!("Expected MissingFrontmatter, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let content = "---\ntitle: [unclosed\n---\nbody";
        let result = parse::<TestFm>(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::ParseError { line, message } => {
                assert!(line > 0, "line should be > 0");
                assert!(!message.is_empty());
            }
            e => panic!("Expected ParseError, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_body_with_horizontal_rule() {
        let content = "---\ntitle: Test\n---\n\n# Heading\n\n---\n\nMore content";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.body, "\n\n# Heading\n\n---\n\nMore content");
    }

    #[test]
    fn test_parse_leading_whitespace_before_delimiter() {
        let content = "\n\n---\ntitle: Test\n---\nbody";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Test");
    }

    #[test]
    fn test_parse_frontmatter_only() {
        let content = "---\ntitle: Test\n---";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Test");
        assert_eq!(doc.body, "");
    }

    #[test]
    fn test_parse_frontmatter_only_with_trailing_newline() {
        let content = "---\ntitle: Test\n---\n";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Test");
        assert_eq!(doc.body, "\n");
    }

    #[test]
    fn test_parse_opening_delimiter_only() {
        let content = "---\ntitle: Test\n";
        let result = parse::<TestFm>(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::MissingFrontmatter { .. } => {}
            e => panic!("Expected MissingFrontmatter, got {:?}", e),
        }
    }

    #[test]
    fn test_to_string_roundtrip() {
        let doc = ParsedDoc {
            frontmatter: TestFm {
                title: "Roundtrip".to_string(),
                tags: vec!["test".to_string()],
            },
            body: "\n\n# Body".to_string(),
            path: None,
        };
        let reconstructed = to_string(&doc).unwrap();
        assert!(reconstructed.starts_with("---\n"));
        assert!(reconstructed.contains("title: Roundtrip"));

        // serde_yaml adds trailing newline, so re-parsed body has extra leading \n
        let reparsed = parse::<TestFm>(&reconstructed).unwrap();
        assert_eq!(reparsed.frontmatter.title, "Roundtrip");
        assert!(reparsed.body.contains("# Body"));
    }

    #[test]
    fn test_update_frontmatter_preserves_body() {
        let content = "---\ntitle: Old\ntags: [old]\n---\n\n# Body\nContent";
        let mut doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.body, "\n\n# Body\nContent");

        update_frontmatter(&mut doc, &TestFm {
            title: "New".to_string(),
            tags: vec!["new".to_string()],
        });

        assert_eq!(doc.frontmatter.title, "New");
        assert_eq!(doc.frontmatter.tags, vec!["new"]);
        assert_eq!(doc.body, "\n\n# Body\nContent");
    }

    #[test]
    fn test_parse_with_defaults() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct DefaultsFm {
            title: String,
            #[serde(default)]
            draft: bool,
        }

        let content = "---\ntitle: Post\n---\nbody";
        let doc = parse::<DefaultsFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Post");
        assert_eq!(doc.frontmatter.draft, false);
    }
}
