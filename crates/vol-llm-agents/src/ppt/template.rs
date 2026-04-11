//! PPT Agent 模板系统。

use serde::{Deserialize, Serialize};
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

    pub fn load_from_dir(&mut self, _dir: &std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Implement YAML loading
        Ok(())
    }

    pub fn list_templates(&self) -> &[Arc<PptTemplate>] {
        &self.templates
    }

    pub fn get_template(&self, id: &str) -> Option<&Arc<PptTemplate>> {
        self.templates.iter().find(|t| t.id == id)
    }

    pub fn match_template(&self, _keywords: &[String]) -> Option<&Arc<PptTemplate>> {
        // TODO: Implement template matching
        self.templates.first()
    }
}
