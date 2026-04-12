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
    pub layouts: Vec<TemplateLayout>,
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

/// 模板布局定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateLayout {
    pub layout_type: LayoutType,
    pub elements: Vec<LayoutElement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutType {
    Title,
    Content,
    TableOfContents,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutElement {
    pub element_type: String, // "textbox", "image", "chart"
    pub placeholder: String,  // "title", "subtitle", "bullets"
    pub position: Position,
    pub style: ElementStyle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub x: i32, // EMU units (1 inch = 914400 EMU)
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElementStyle {
    pub font_size: i32,
    pub color: Option<String>, // HEX, or "{{primary}}" for template reference
    pub bullet_style: Option<String>,
}

impl Default for TemplateLayout {
    fn default() -> Self {
        Self {
            layout_type: LayoutType::Content,
            elements: vec![
                LayoutElement {
                    element_type: "textbox".to_string(),
                    placeholder: "title".to_string(),
                    position: Position {
                        x: 457200,      // 0.5 inch
                        y: 228600,      // 0.25 inch
                        width: 8229600, // 9 inches
                        height: 914400, // 1 inch
                    },
                    style: ElementStyle {
                        font_size: 24,
                        color: Some("{{primary}}".to_string()),
                        bullet_style: None,
                    },
                },
                LayoutElement {
                    element_type: "textbox".to_string(),
                    placeholder: "content".to_string(),
                    position: Position {
                        x: 457200,
                        y: 1371600, // 1.5 inches
                        width: 8229600,
                        height: 4572000, // 5 inches
                    },
                    style: ElementStyle {
                        font_size: 16,
                        color: Some("{{text_primary}}".to_string()),
                        bullet_style: Some("bullet".to_string()),
                    },
                },
            ],
        }
    }
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
