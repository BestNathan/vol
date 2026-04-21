use std::io::Write;
use std::sync::Arc;

use vol_llm_skill::{SkillDef, SkillLoader, SkillScope, SkillInjector, SkillTool};
use vol_llm_tool::{ExecutableTool, ToolContext};

#[tokio::test]
async fn test_full_skill_lifecycle() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let rust_dir = skills_dir.join("rust-conventions");
    std::fs::create_dir_all(&rust_dir).unwrap();

    let mut f = std::fs::File::create(rust_dir.join("SKILL.md")).unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "name: rust-conventions").unwrap();
    writeln!(f, "version: 1.0.0").unwrap();
    writeln!(f, "description: Rust coding conventions").unwrap();
    writeln!(f, "triggers: [rust, conventions]").unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "# Rust Conventions").unwrap();
    writeln!(f, "").unwrap();
    writeln!(f, "When writing Rust code:").unwrap();
    writeln!(f, "- Use snake_case for functions.").unwrap();

    let refs_dir = rust_dir.join("references");
    std::fs::create_dir_all(&refs_dir).unwrap();
    std::fs::write(refs_dir.join("style.md"), "# Style Guide").unwrap();

    let mut loader = SkillLoader::new_empty();
    loader.add_root(SkillScope::User, skills_dir.clone());
    loader.discover_all().await.unwrap();

    let metadata = loader.list_metadata().await;
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].name, "rust-conventions");

    let skill = loader.get("rust-conventions").await.unwrap();
    assert!(skill.file_listing.contains(&"SKILL.md".to_string()));
    assert!(skill.file_listing.contains(&"references/style.md".to_string()));
    assert!(skill.content.contains("# Rust Conventions"));
    assert!(skill.content.contains("snake_case"));

    let matched = loader.get_by_trigger("rust coding").await;
    assert_eq!(matched.len(), 1);

    let no_match = loader.get_by_trigger("python").await;
    assert_eq!(no_match.len(), 0);
}

#[tokio::test]
async fn test_skill_tool_loads_skill() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let test_dir = skills_dir.join("test-skill");
    std::fs::create_dir_all(&test_dir).unwrap();

    let mut f = std::fs::File::create(test_dir.join("SKILL.md")).unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "name: test-skill").unwrap();
    writeln!(f, "version: 2.0.0").unwrap();
    writeln!(f, "description: A test skill").unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "# Test Skill Body").unwrap();

    let mut loader = SkillLoader::new(None);
    loader.add_root(SkillScope::User, skills_dir.clone());
    loader.discover_all().await.unwrap();

    let tool = SkillTool::new(Arc::new(loader));
    let args = serde_json::json!({ "name": "test-skill" });
    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();

    assert!(result.content.contains("=== SKILL: test-skill (v2.0.0)"));
    assert!(result.content.contains("# Test Skill Body"));
}

#[tokio::test]
async fn test_injector_formats_prompt() {
    let loader = SkillLoader::new(None);

    let mut skill1 = SkillDef::new("rust", "# Rust")
        .with_description("Rust conventions");
    skill1.id = "user:rust".to_string();
    loader.register(skill1).await;

    let mut skill2 = SkillDef::new("python", "# Python")
        .with_description("Python conventions");
    skill2.id = "user:python".to_string();
    loader.register(skill2).await;

    let injector = SkillInjector::new(Arc::new(loader));
    let output = injector.format_metadata().await;

    assert!(output.contains("Available skills:"));
    assert!(output.contains("rust"));
    assert!(output.contains("Python conventions"));
    assert!(output.contains("skill"));
}

#[tokio::test]
async fn test_mixed_file_and_code_skills() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let file_dir = skills_dir.join("file-skill");
    std::fs::create_dir_all(&file_dir).unwrap();
    let mut f = std::fs::File::create(file_dir.join("SKILL.md")).unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "name: file-skill").unwrap();
    writeln!(f, "version: 1.0.0").unwrap();
    writeln!(f, "description: From file").unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "# File Skill").unwrap();

    let mut loader = SkillLoader::new_empty();
    loader.add_root(SkillScope::User, skills_dir.clone());
    loader.discover_all().await.unwrap();

    let mut code_skill = SkillDef::new("code-skill", "# Code Skill")
        .with_description("From code registration");
    code_skill.id = "code:code-skill".to_string();
    loader.register(code_skill).await;

    let metadata = loader.list_metadata().await;
    assert_eq!(metadata.len(), 2);

    assert!(loader.get("file-skill").await.is_some());
    assert!(loader.get("code-skill").await.is_some());
}

#[tokio::test]
async fn test_discover_non_utf8_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let bad_dir = skills_dir.join("bad-skill");
    std::fs::create_dir_all(&bad_dir).unwrap();
    std::fs::write(bad_dir.join("SKILL.md"), &[0xff, 0xfe, 0x00, 0x01]).unwrap();

    let mut loader = SkillLoader::new_empty();
    loader.add_root(SkillScope::User, skills_dir.clone());
    let result = loader.discover_all().await;
    assert!(result.is_ok());
    assert!(loader.list_metadata().await.is_empty());
}

#[test]
fn test_skill_scope_prefix() {
    assert_eq!(SkillScope::User.prefix(), "user");
    assert_eq!(SkillScope::Repo.prefix(), "repo");
    let custom = SkillScope::Custom(std::path::PathBuf::from("/opt/skills"));
    assert!(custom.prefix().starts_with("custom:"));
}

#[test]
fn test_skill_def_builder() {
    let skill = SkillDef::new("my-skill", "# Content")
        .with_description("My skill")
        .with_version("2.0.0")
        .with_triggers(vec!["test".to_string()])
        .with_file_listing(vec!["SKILL.md".to_string()]);

    assert_eq!(skill.name, "my-skill");
    assert_eq!(skill.content, "# Content");
    assert_eq!(skill.description, "My skill");
    assert_eq!(skill.version, "2.0.0");
    assert_eq!(skill.triggers, vec!["test"]);
    assert_eq!(skill.file_listing, vec!["SKILL.md"]);
}
