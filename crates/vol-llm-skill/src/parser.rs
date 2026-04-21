use std::path::Path;

use serde::Deserialize;

use crate::Result;

/// Parsed frontmatter from SKILL.md.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Result of parsing a SKILL.md file.
#[derive(Debug, Clone)]
pub struct ParsedSkill {
    pub name: String,
    pub version: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub body: String,
}

/// Parse SKILL.md content into frontmatter + body.
///
/// If frontmatter is missing or invalid, treats entire content as body
/// with default name "default".
pub fn parse_skill_content(content: &str) -> Result<ParsedSkill> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok(ParsedSkill {
            name: "default".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            triggers: Vec::new(),
            body: content.to_string(),
        });
    }

    let rest = &trimmed[3..];
    if let Some(end_idx) = rest.find("\n---") {
        let frontmatter_str = &rest[..end_idx];
        let body = &rest[end_idx + 4..];

        match serde_yaml::from_str::<SkillFrontmatter>(frontmatter_str) {
            Ok(fm) => Ok(ParsedSkill {
                name: fm.name,
                version: fm.version,
                description: fm.description,
                triggers: fm.triggers,
                body: body.trim_start().to_string(),
            }),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse SKILL.md frontmatter, treating as plain body");
                Ok(ParsedSkill {
                    name: "default".to_string(),
                    version: "1.0.0".to_string(),
                    description: String::new(),
                    triggers: Vec::new(),
                    body: content.to_string(),
                })
            }
        }
    } else {
        Ok(ParsedSkill {
            name: "default".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            triggers: Vec::new(),
            body: content.to_string(),
        })
    }
}

/// Scan a skill directory for files, returning relative paths.
pub fn scan_skill_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            collect_files_recursive(&path, root, &mut files);
        }
    }
    files.sort();
    files
}

/// Keep all files since the LLM may want to read any of them.
pub fn filter_skill_files(files: &[String]) -> Vec<String> {
    files.to_vec()
}

fn collect_files_recursive(path: &Path, root: &Path, files: &mut Vec<String>) {
    if path.is_file() {
        if let Ok(rel) = path.strip_prefix(root) {
            files.push(rel.to_string_lossy().to_string());
        }
    } else if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_files_recursive(&entry.path(), root, files);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_with_frontmatter() {
        let content = "---
name: rust-conventions
version: 1.0.0
description: Rust coding conventions
triggers: [rust, conventions]
---

# Rust Conventions

When writing code:
- Use snake_case for functions.
";
        let result = parse_skill_content(content).unwrap();
        assert_eq!(result.name, "rust-conventions");
        assert_eq!(result.version, "1.0.0");
        assert_eq!(result.description, "Rust coding conventions");
        assert_eq!(result.triggers, vec!["rust", "conventions"]);
        assert!(result.body.contains("# Rust Conventions"));
    }

    #[test]
    fn test_parse_skill_without_frontmatter() {
        let content = "# Plain Skill\n\nJust markdown, no frontmatter.";
        let result = parse_skill_content(content).unwrap();
        assert_eq!(result.name, "default");
        assert_eq!(result.version, "1.0.0");
        assert!(result.body.contains("# Plain Skill"));
    }

    #[test]
    fn test_parse_invalid_frontmatter() {
        let content = "---\ninvalid: yaml: : :\n---\nbody";
        let result = parse_skill_content(content).unwrap();
        // Should fail to parse frontmatter, treated as no frontmatter
        assert_eq!(result.name, "default");
        assert!(result.body.contains("invalid: yaml:"));
    }

    #[test]
    fn test_scan_files() {
        let files = vec![
            "SKILL.md".to_string(),
            "scripts/format.sh".to_string(),
            "references/style.md".to_string(),
        ];
        let filtered = filter_skill_files(&files);
        assert!(filtered.contains(&"SKILL.md".to_string()));
        assert!(filtered.contains(&"scripts/format.sh".to_string()));
    }
}
