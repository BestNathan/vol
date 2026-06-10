//! PPT Agent 模板系统。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// PPT 模板定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PptTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: TemplateTags,
    pub color_scheme: ColorScheme,
    pub typography: Typography,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateTags {
    pub occasion: Vec<String>,
    pub style: Vec<String>,
    pub audience: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorScheme {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub background: String,
    pub text_primary: String,
    pub text_secondary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Typography {
    pub title_font: String,
    pub body_font: String,
}

/// 模板注册表
#[derive(Clone, Default)]
pub struct TemplateRegistry {
    templates: Vec<Arc<PptTemplate>>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    pub fn load_from_dir(&mut self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut templates = Vec::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let content = fs::read_to_string(&path)?;
                let template: PptTemplate = serde_yaml::from_str(&content)?;
                templates.push(Arc::new(template));
            }
        }

        self.templates = templates;
        Ok(())
    }

    pub fn list_templates(&self) -> &[Arc<PptTemplate>] {
        &self.templates
    }

    pub fn get_template(&self, id: &str) -> Option<&Arc<PptTemplate>> {
        self.templates.iter().find(|t| t.id == id)
    }

    pub fn match_template(&self, keywords: &[String]) -> Option<&Arc<PptTemplate>> {
        // Simple keyword matching for MVP
        // Future iteration: LLM analysis + vector similarity
        for template in &self.templates {
            for keyword in keywords {
                let keyword_lower = keyword.to_lowercase();

                // Check occasion tags
                if template
                    .tags
                    .occasion
                    .iter()
                    .any(|t| t.to_lowercase().contains(&keyword_lower))
                {
                    return Some(template);
                }

                // Check style tags
                if template
                    .tags
                    .style
                    .iter()
                    .any(|t| t.to_lowercase().contains(&keyword_lower))
                {
                    return Some(template);
                }
            }
        }

        // Default to first template if no match
        self.templates.first()
    }
}
