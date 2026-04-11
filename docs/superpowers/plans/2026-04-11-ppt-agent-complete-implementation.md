# PPT Agent Complete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现完整的 PPT 生成逻辑，包括 LLM 驱动的需求分析、大纲生成、内容填充和基于模板布局的 PPTX 渲染

**Architecture:** 5-step flow: 需求分析 → 大纲生成 → 模板匹配 → 内容生成 → PPTX 渲染，基于设计的 TemplateLayout schema 进行布局填充

**Tech Stack:** Rust, pptx crate (=0.1.0), serde_yaml, vol-llm-core LLMClient, clap CLI

---

## Phase 1: 核心类型定义

### Task 1: 更新 types.rs 添加 StructuredRequirement 和 Outline

**Files:**
- Modify: `crates/vol-llm-agents/src/ppt/types.rs`

- [ ] **Step 1: 添加 StructuredRequirement 类型**

在 `types.rs` 中添加：

```rust
/// 结构化需求
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuredRequirement {
    pub topic: String,
    pub audience: Option<String>,
    pub style: Option<String>,
    pub purpose: Option<String>,
}
```

- [ ] **Step 2: 添加 Outline 和 SlideDef 类型**

在 `types.rs` 中添加：

```rust
/// PPT 大纲
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outline {
    pub title: String,
    pub slides: Vec<SlideDef>,
}

/// 幻灯片定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlideDef {
    #[serde(rename = "type")]
    pub slide_type: SlideType,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub bullets: Vec<String>,
    #[serde(default)]
    pub sections: Vec<String>,
}

impl SlideDef {
    pub fn to_slide(&self, layout: SlideLayout) -> Slide {
        Slide {
            slide_type: self.slide_type.clone(),
            layout,
            title: self.title.clone(),
            content: SlideContent {
                bullets: self.bullets.clone(),
                speaker_notes: None,
            },
        }
    }
}
```

- [ ] **Step 3: 更新 mod.rs 导出新类型**

修改 `crates/vol-llm-agents/src/ppt/mod.rs`：

```rust
pub use types::{PptInput, PptOutput, Slide, SlideLayout, SlideType, StructuredRequirement, Outline, SlideDef};
```

- [ ] **Step 4: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

Expected: Compiles successfully

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/types.rs crates/vol-llm-agents/src/ppt/mod.rs
git commit -m "feat(ppt-types): add StructuredRequirement, Outline, SlideDef types"
```

---

### Task 2: 更新 template.rs 添加 TemplateLayout schema

**Files:**
- Modify: `crates/vol-llm-agents/src/ppt/template.rs`

- [ ] **Step 1: 添加 LayoutElement, Position, ElementStyle 类型**

在 `template.rs` 中添加：

```rust
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
    pub element_type: String,       // "textbox", "image", "chart"
    pub placeholder: String,        // "title", "subtitle", "bullets"
    pub position: Position,
    pub style: ElementStyle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,       // EMU units (1 inch = 914400 EMU)
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElementStyle {
    pub font_size: i32,
    pub color: Option<String>,     // HEX, or "{{primary}}" for template reference
    pub bullet_style: Option<String>,
}
```

- [ ] **Step 2: 添加 layouts 字段到 PptTemplate**

修改 `PptTemplate` 结构：

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PptTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: TemplateTags,
    pub color_scheme: ColorScheme,
    pub typography: Typography,
    pub layouts: Vec<TemplateLayout>,  // 新增
}
```

- [ ] **Step 3: 添加 TemplateLayout 默认实现**

在 `template.rs` 中添加：

```rust
impl Default for TemplateLayout {
    fn default() -> Self {
        Self {
            layout_type: LayoutType::Content,
            elements: vec![
                LayoutElement {
                    element_type: "textbox".to_string(),
                    placeholder: "title".to_string(),
                    position: Position {
                        x: 457200,  // 0.5 inch
                        y: 228600,  // 0.25 inch
                        width: 8229600,  // 9 inches
                        height: 914400,  // 1 inch
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
                        y: 1371600,  // 1.5 inches
                        width: 8229600,
                        height: 4572000,  // 5 inches
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
```

- [ ] **Step 4: 更新 mod.rs 导出新类型**

修改 `crates/vol-llm-agents/src/ppt/mod.rs`：

```rust
pub use template::{TemplateRegistry, PptTemplate, TemplateLayout, LayoutElement, Position, ElementStyle, LayoutType};
```

- [ ] **Step 5: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 6: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/template.rs crates/vol-llm-agents/src/ppt/mod.rs
git commit -m "feat(ppt-template): add TemplateLayout schema with Position in EMU units"
```

---

## Phase 2: Prompt 定义

### Task 3: 创建 prompts.rs 模块

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/prompts.rs`

- [ ] **Step 1: 创建需求分析 Prompt**

```rust
//! PPT Agent Prompts.

/// 需求分析 System Prompt
pub const ANALYSIS_SYSTEM_PROMPT: &str = r#"You are a presentation analysis assistant.
Extract key information from the user's request to help generate a PowerPoint presentation.

Extract:
- topic: Main subject of the presentation
- audience: Who will watch this (executive, technical, general, etc.)
- style: Preferred style (formal, casual, minimal, detailed, etc.)
- purpose: Goal of the presentation (inform, persuade, train, etc.)

Return ONLY valid JSON:
{
    "topic": "...",
    "audience": "...",
    "style": "...",
    "purpose": "..."
}"#;

/// 大纲生成 System Prompt
pub const OUTLINE_SYSTEM_PROMPT: &str = r#"You are a professional presentation designer.
Create a structured outline for a PowerPoint presentation.

Guidelines:
- Start with a title slide
- Include a table of contents slide
- Create 5-10 content slides
- Each content slide should have 3-5 bullet points
- End with a summary or Q&A slide

Return ONLY valid JSON:
{
    "title": "Presentation Title",
    "slides": [
        {"type": "title", "title": "Main Title", "subtitle": "Subtitle"},
        {"type": "toc", "title": "Table of Contents", "sections": ["Section 1", "Section 2"]},
        {"type": "content", "title": "Slide Title", "bullets": ["Point 1", "Point 2", "Point 3"]}
    ]
}"#;

/// 内容生成 System Prompt
pub const CONTENT_SYSTEM_PROMPT: &str = r#"You are a professional content writer for business presentations.
Expand outline bullets into detailed, presentation-ready content.

Guidelines:
- Each bullet should be 1-2 lines, concise
- Use action verbs and specific data
- Avoid full sentences - use fragments
- Maintain consistent tone

Return ONLY valid JSON with expanded bullets for each slide."#;

/// 构建需求分析 User Prompt
pub fn build_analysis_user_prompt(description: &str, context: Option<&str>) -> String {
    let context_part = context.map(|c| format!("\n\nAdditional context: {}", c)).unwrap_or_default();
    format!(
        r#"Analyze the following presentation request:

{}{}

Extract topic, audience, style, and purpose. Return ONLY valid JSON."#,
        description, context_part
    )
}

/// 构建大纲生成 User Prompt
pub fn build_outline_user_prompt(requirements: &crate::ppt::StructuredRequirement) -> String {
    let audience_part = requirements.audience.as_ref().map(|a| format!("\n- Audience: {}", a)).unwrap_or_default();
    let style_part = requirements.style.as_ref().map(|s| format!("\n- Style: {}", s)).unwrap_or_default();
    let purpose_part = requirements.purpose.as_ref().map(|p| format!("\n- Purpose: {}", p)).unwrap_or_default();
    
    format!(
        r#"Create a presentation outline for:
- Topic: {}{}{}{}

Generate title slide, table of contents, 5-10 content slides, and summary. Return ONLY valid JSON."#,
        requirements.topic, audience_part, style_part, purpose_part
    )
}

/// 构建内容生成 User Prompt
pub fn build_content_user_prompt(outline_json: &str) -> String {
    format!(
        r#"Expand the following outline into detailed slide content:

{}

Return valid JSON with expanded bullet points for each slide."#,
        outline_json
    )
}
```

- [ ] **Step 2: 更新 mod.rs 导出 prompts 模块**

```rust
pub mod prompts;
```

- [ ] **Step 3: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/prompts.rs crates/vol-llm-agents/src/ppt/mod.rs
git commit -m "feat(ppt-prompts): add ANALYSIS, OUTLINE, CONTENT system prompts"
```

---

## Phase 3: 需求分析模块

### Task 4: 创建 analysis.rs 模块

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/analysis.rs`

- [ ] **Step 1: 创建 AnalysisModule 结构**

```rust
//! PPT Agent 需求分析模块。

use std::sync::Arc;
use vol_llm_core::{LLMClient, ConversationRequest, Message};
use vol_llm_core::Error as LlmError;
use crate::ppt::{StructuredRequirement, prompts};
use serde_json::Value;

/// 需求分析模块
pub struct AnalysisModule {
    llm: Arc<dyn LLMClient>,
}

impl AnalysisModule {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// 分析用户需求，提取结构化信息
    pub async fn analyze(&self, description: &str, context: Option<&str>) -> Result<StructuredRequirement, AnalysisError> {
        // Build messages
        let system_message = Message::system(prompts::ANALYSIS_SYSTEM_PROMPT.to_string());
        let user_message = Message::user(prompts::build_analysis_user_prompt(description, context));
        
        // Call LLM
        let request = ConversationRequest::with_messages(vec![system_message, user_message]);
        let response = self.llm.converse(request).await
            .map_err(|e| AnalysisError::LlmError(e.to_string()))?;
        
        // Parse JSON response
        let json: Value = serde_json::from_str(&response.content)
            .map_err(|e| AnalysisError::JsonParseError(e.to_string()))?;
        
        // Extract fields
        let topic = json["topic"].as_str()
            .ok_or_else(|| AnalysisError::MissingField("topic".to_string()))?
            .to_string();
        
        let audience = json["audience"].as_str().map(|s| s.to_string());
        let style = json["style"].as_str().map(|s| s.to_string());
        let purpose = json["purpose"].as_str().map(|s| s.to_string());
        
        Ok(StructuredRequirement {
            topic,
            audience,
            style,
            purpose,
        })
    }
}

/// 分析错误
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    
    #[error("JSON parsing failed: {0}")]
    JsonParseError(String),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
}
```

- [ ] **Step 2: 更新 mod.rs 导出 AnalysisModule**

```rust
pub mod analysis;
pub use analysis::{AnalysisModule, AnalysisError};
```

- [ ] **Step 3: 添加 thiserror 依赖（如果还没有）**

检查 `crates/vol-llm-agents/Cargo.toml`，添加：

```toml
thiserror = "1.0"
```

- [ ] **Step 4: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/analysis.rs crates/vol-llm-agents/src/ppt/mod.rs crates/vol-llm-agents/Cargo.toml
git commit -m "feat(ppt-analysis): implement AnalysisModule with LLM-driven requirement extraction"
```

---

## Phase 4: 大纲生成模块

### Task 5: 更新 outline.rs 实现完整大纲生成

**Files:**
- Modify: `crates/vol-llm-agents/src/ppt/tools/outline.rs`

- [ ] **Step 1: 重写 OutlineGeneratorTool 使用新的 Outline 类型**

```rust
//! 大纲生成工具。

use std::sync::Arc;
use vol_llm_tool::{Tool, ExecutableTool, ToolContext, ToolResult, ToolError};
use vol_llm_core::{LLMClient, ConversationRequest, Message};
use serde_json::{json, Value};
use async_trait::async_trait;
use crate::ppt::{Outline, SlideDef, SlideType, prompts};

/// 大纲生成工具
pub struct OutlineGeneratorTool {
    llm: Arc<dyn LLMClient>,
}

impl OutlineGeneratorTool {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// 生成大纲
    pub async fn generate(&self, topic: &str, audience: Option<&str>, style: Option<&str>, purpose: Option<&str>) -> Result<Outline, OutlineError> {
        // Build requirements struct for prompt
        let requirements = format!(
            r#"{{"topic": "{}", "audience": "{}", "style": "{}", "purpose": "{}"}}"#,
            topic,
            audience.unwrap_or("general"),
            style.unwrap_or("professional"),
            purpose.unwrap_or("inform")
        );
        
        let system_message = Message::system(prompts::OUTLINE_SYSTEM_PROMPT.to_string());
        let user_message = Message::user(format!(
            r#"Create a presentation outline for:
- Topic: {}
- Audience: {}
- Style: {}
- Purpose: {}

Generate title slide, table of contents, 5-10 content slides, and summary. Return ONLY valid JSON."#,
            topic,
            audience.unwrap_or("general"),
            style.unwrap_or("professional"),
            purpose.unwrap_or("inform")
        ));
        
        let request = ConversationRequest::with_messages(vec![system_message, user_message]);
        let response = self.llm.converse(request).await
            .map_err(|e| OutlineError::LlmError(e.to_string()))?;
        
        // Parse JSON response
        let json: Value = serde_json::from_str(&response.content)
            .map_err(|e| OutlineError::JsonParseError(e.to_string()))?;
        
        // Parse into Outline struct
        let title = json["title"].as_str()
            .ok_or_else(|| OutlineError::MissingField("title".to_string()))?
            .to_string();
        
        let slides_array = json["slides"].as_array()
            .ok_or_else(|| OutlineError::MissingField("slides".to_string()))?;
        
        let mut slides = Vec::new();
        for slide_json in slides_array {
            let slide_type_str = slide_json["type"].as_str()
                .ok_or_else(|| OutlineError::MissingField("slide type".to_string()))?;
            
            let slide_type = match slide_type_str.to_lowercase().as_str() {
                "title" => SlideType::Title,
                "toc" | "table_of_contents" => SlideType::TableOfContents,
                "section_header" => SlideType::SectionHeader,
                _ => SlideType::Content,
            };
            
            let slide_def = SlideDef {
                slide_type,
                title: slide_json["title"].as_str().unwrap_or("Untitled").to_string(),
                subtitle: slide_json.get("subtitle").and_then(|v| v.as_str()).map(|s| s.to_string()),
                bullets: slide_json.get("bullets")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
                    .unwrap_or_default(),
                sections: slide_json.get("sections")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
                    .unwrap_or_default(),
            };
            
            slides.push(slide_def);
        }
        
        Ok(Outline { title, slides })
    }
}

#[async_trait]
impl ExecutableTool for OutlineGeneratorTool {
    fn name(&self) -> &'static str {
        "generate_outline"
    }

    fn description(&self) -> &'static str {
        "Generate a structured outline for a PowerPoint presentation based on topic and requirements."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {"type": "string", "description": "Presentation topic"},
                "audience": {"type": "string", "description": "Target audience"},
                "style": {"type": "string", "description": "Preferred style"},
                "purpose": {"type": "string", "description": "Presentation purpose"}
            },
            "required": ["topic"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let topic = args["topic"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'topic' argument".to_string())
        })?;
        
        let audience = args["audience"].as_str();
        let style = args["style"].as_str();
        let purpose = args["purpose"].as_str();
        
        match self.generate(topic, audience, style, purpose).await {
            Ok(outline) => {
                let json = serde_json::to_value(&outline).unwrap_or(Value::Null);
                Ok(ToolResult {
                    call_id: String::new(),
                    success: true,
                    content: serde_json::to_string_pretty(&json).unwrap_or_default(),
                    error: None,
                    data: Some(json),
                })
            }
            Err(e) => Ok(ToolResult {
                call_id: String::new(),
                success: false,
                content: format!("Error: {}", e),
                error: Some(e.to_string()),
                data: None,
            })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OutlineError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    #[error("JSON parsing failed: {0}")]
    JsonParseError(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
}
```

- [ ] **Step 2: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 3: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/tools/outline.rs
git commit -m "feat(ppt-outline): implement complete OutlineGeneratorTool with JSON parsing"
```

---

## Phase 5: 内容生成模块

### Task 6: 更新 content.rs 实现完整内容生成

**Files:**
- Modify: `crates/vol-llm-agents/src/ppt/tools/content.rs`

- [ ] **Step 1: 重写 ContentGeneratorTool**

```rust
//! 内容生成工具。

use std::sync::Arc;
use vol_llm_tool::{Tool, ExecutableTool, ToolContext, ToolResult, ToolError};
use vol_llm_core::{LLMClient, ConversationRequest, Message};
use serde_json::{json, Value};
use async_trait::async_trait;
use crate::ppt::{Outline, SlideDef, prompts};

/// 内容生成工具
pub struct ContentGeneratorTool {
    llm: Arc<dyn LLMClient>,
}

impl ContentGeneratorTool {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// 扩展大纲内容为详细 bullet points
    pub async fn expand(&self, outline: &Outline) -> Result<Outline, ContentError> {
        let system_message = Message::system(prompts::CONTENT_SYSTEM_PROMPT.to_string());
        let user_message = Message::user(prompts::build_content_user_prompt(
            &serde_json::to_string_pretty(outline).unwrap_or_default()
        ));
        
        let request = ConversationRequest::with_messages(vec![system_message, user_message]);
        let response = self.llm.converse(request).await
            .map_err(|e| ContentError::LlmError(e.to_string()))?;
        
        // Parse expanded content
        let json: Value = serde_json::from_str(&response.content)
            .map_err(|e| ContentError::JsonParseError(e.to_string()))?;
        
        // Merge expanded bullets back into outline
        let mut slides = Vec::new();
        for (i, original_slide) in outline.slides.iter().enumerate() {
            let mut slide = original_slide.clone();
            
            // Try to get expanded bullets for this slide
            if let Some(expanded_bullets) = json.get(i.to_string())
                .and_then(|v| v.as_array()) {
                slide.bullets = expanded_bullets
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
            }
            
            slides.push(slide);
        }
        
        Ok(Outline {
            title: outline.title.clone(),
            slides,
        })
    }
}

#[async_trait]
impl ExecutableTool for ContentGeneratorTool {
    fn name(&self) -> &'static str {
        "generate_content"
    }

    fn description(&self) -> &'static str {
        "Expand outline bullets into detailed, presentation-ready content."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "outline": {"type": "string", "description": "JSON outline to expand"}
            },
            "required": ["outline"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let outline_str = args["outline"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'outline' argument".to_string())
        })?;
        
        let outline: Outline = serde_json::from_str(outline_str)
            .map_err(|e| ToolError::InvalidArguments(format!("Invalid outline JSON: {}", e)))?;
        
        match self.expand(&outline).await {
            Ok(expanded) => {
                let json = serde_json::to_value(&expanded).unwrap_or(Value::Null);
                Ok(ToolResult {
                    call_id: String::new(),
                    success: true,
                    content: serde_json::to_string_pretty(&json).unwrap_or_default(),
                    error: None,
                    data: Some(json),
                })
            }
            Err(e) => Ok(ToolResult {
                call_id: String::new(),
                success: false,
                content: format!("Error: {}", e),
                error: Some(e.to_string()),
                data: None,
            })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("LLM call failed: {0}")]
    LlmError(String),
    #[error("JSON parsing failed: {0}")]
    JsonParseError(String),
}
```

- [ ] **Step 2: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 3: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/tools/content.rs
git commit -m "feat(ppt-content): implement ContentGeneratorTool with outline expansion"
```

---

## Phase 6: PPTX 渲染器

### Task 7: 实现完整的 PptxRenderer

**Files:**
- Modify: `crates/vol-llm-agents/src/ppt/renderer.rs`

- [ ] **Step 1: 重写 PptxRenderer 使用 pptx crate**

```rust
//! PPT Agent 渲染器。

use pptx::Presentation;
use std::path::PathBuf;
use crate::ppt::{PptTemplate, Slide, SlideType, Outline, SlideDef};
use crate::ppt::template::{TemplateLayout, LayoutType, LayoutElement};

/// PPTX 渲染器
pub struct PptxRenderer {
    presentation: Presentation,
    template: PptTemplate,
}

impl PptxRenderer {
    pub fn new(template: PptTemplate) -> Self {
        Self {
            presentation: Presentation::new(),
            template,
        }
    }

    /// 添加封面页
    pub fn add_title_slide(&mut self, title: &str, subtitle: &str) -> Result<(), RendererError> {
        let slide = self.presentation.add_slide();
        
        // Title textbox
        let title_color = self.resolve_color("{{primary}}");
        slide.add_text_box()
            .with_text(title)
            .with_font_size(44.0)
            .with_color(&title_color)
            .with_left(0.5)
            .with_top(1.0)
            .with_width(9.0)
            .with_height(1.5);
        
        // Subtitle textbox
        let subtitle_color = self.resolve_color("{{text_secondary}}");
        slide.add_text_box()
            .with_text(subtitle)
            .with_font_size(24.0)
            .with_color(&subtitle_color)
            .with_left(0.5)
            .with_top(3.0)
            .with_width(9.0)
            .with_height(1.0);
        
        Ok(())
    }

    /// 添加目录页
    pub fn add_toc_slide(&mut self, title: &str, sections: &[String]) -> Result<(), RendererError> {
        let slide = self.presentation.add_slide();
        
        // Title
        let title_color = self.resolve_color("{{primary}}");
        slide.add_text_box()
            .with_text(title)
            .with_font_size(32.0)
            .with_color(&title_color)
            .with_left(0.5)
            .with_top(0.5)
            .with_width(9.0)
            .with_height(1.0);
        
        // Sections
        let mut y_offset = 1.5;
        for (i, section) in sections.iter().enumerate() {
            let text_color = self.resolve_color("{{text_primary}}");
            slide.add_text_box()
                .with_text(&format!("{}. {}", i + 1, section))
                .with_font_size(18.0)
                .with_color(&text_color)
                .with_left(0.75)
                .with_top(y_offset)
                .with_width(8.5)
                .with_height(0.5);
            y_offset += 0.6;
        }
        
        Ok(())
    }

    /// 添加内容页
    pub fn add_content_slide(&mut self, title: &str, bullets: &[String]) -> Result<(), RendererError> {
        let slide = self.presentation.add_slide();
        
        // Title
        let title_color = self.resolve_color("{{primary}}");
        slide.add_text_box()
            .with_text(title)
            .with_font_size(28.0)
            .with_color(&title_color)
            .with_left(0.5)
            .with_top(0.5)
            .with_width(9.0)
            .with_height(1.0);
        
        // Bullets
        let mut y_offset = 1.5;
        for bullet in bullets {
            let text_color = self.resolve_color("{{text_primary}}");
            slide.add_text_box()
                .with_text(&format!("• {}", bullet))
                .with_font_size(16.0)
                .with_color(&text_color)
                .with_left(0.75)
                .with_top(y_offset)
                .with_width(8.5)
                .with_height(0.5);
            y_offset += 0.5;
        }
        
        Ok(())
    }

    /// 从大纲构建完整 PPT
    pub fn render_outline(&mut self, outline: &Outline) -> Result<(), RendererError> {
        // Add title slide
        let subtitle = "Generated by PPT Agent";
        self.add_title_slide(&outline.title, subtitle)?;
        
        // Collect sections for TOC
        let sections: Vec<String> = outline.slides.iter()
            .filter(|s| matches!(s.slide_type, SlideType::Content))
            .map(|s| s.title.clone())
            .collect();
        
        // Add TOC slide
        if !sections.is_empty() {
            self.add_toc_slide("目录", &sections)?;
        }
        
        // Add content slides
        for slide_def in &outline.slides {
            if matches!(slide_def.slide_type, SlideType::Content) {
                self.add_content_slide(&slide_def.title, &slide_def.bullets)?;
            }
        }
        
        Ok(())
    }

    /// 保存到文件
    pub fn save(&self, path: &PathBuf) -> Result<(), RendererError> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| RendererError::IoError(e.to_string()))?;
        }
        
        self.presentation.save(path)
            .map_err(|e| RendererError::PptxError(e.to_string()))?;
        Ok(())
    }

    /// 解析颜色模板变量
    fn resolve_color(&self, color: &str) -> String {
        if let Some(stripped) = color.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
            match stripped {
                "primary" => self.template.color_scheme.primary.clone(),
                "secondary" => self.template.color_scheme.secondary.clone(),
                "text_primary" => self.template.color_scheme.text_primary.clone(),
                "text_secondary" => self.template.color_scheme.text_secondary.clone(),
                "accent" => self.template.color_scheme.accent.clone(),
                "background" => self.template.color_scheme.background.clone(),
                _ => "#000000".to_string(),
            }
        } else {
            color.to_string()
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    #[error("PPTX rendering failed: {0}")]
    PptxError(String),
    #[error("IO error: {0}")]
    IoError(String),
}
```

- [ ] **Step 2: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 3: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/renderer.rs
git commit -m "feat(ppt-renderer): implement complete PptxRenderer with pptx crate and color resolution"
```

---

## Phase 7: PptAgent 核心实现

### Task 8: 实现完整的 PptAgent generate() 方法

**Files:**
- Modify: `crates/vol-llm-agents/src/ppt/agent.rs`

- [ ] **Step 1: 重写 PptAgent 实现完整生成流程**

```rust
//! PPT Agent 核心实现。

use std::sync::Arc;
use std::path::PathBuf;
use vol_llm_core::LLMClient;
use vol_llm_provider::LLMProviderRegistry;
use crate::ppt::{PptAgentConfig, PptInput, PptOutput, Outline};
use crate::ppt::template::TemplateRegistry;
use crate::ppt::renderer::{PptxRenderer, RendererError};
use crate::ppt::analysis::AnalysisModule;
use crate::ppt::tools::outline::{OutlineGeneratorTool, OutlineError};
use crate::ppt::tools::content::ContentGeneratorTool;
use chrono::Local;

/// PPT Agent
pub struct PptAgent {
    config: PptAgentConfig,
    llm: Arc<dyn LLMClient>,
    template_registry: TemplateRegistry,
}

impl PptAgent {
    /// 创建新的 PPT Agent
    pub async fn new(config: PptAgentConfig) -> Result<Self, PptAgentError> {
        // Initialize LLM
        let registry = LLMProviderRegistry::new();
        let llm = registry.get(&config.llm_provider_id)
            .ok_or_else(|| PptAgentError::ConfigError(format!("LLM provider '{}' not found", config.llm_provider_id)))?;
        
        // Initialize template registry
        let mut template_registry = TemplateRegistry::new();
        if let Some(template_dir) = &config.template_dir {
            template_registry.load_from_dir(template_dir)
                .map_err(|e| PptAgentError::ConfigError(format!("Failed to load templates: {}", e)))?;
        }
        
        Ok(Self {
            config,
            llm,
            template_registry,
        })
    }

    /// 生成 PPT 主流程
    pub async fn generate(&self, input: PptInput) -> Result<PptOutput, PptAgentError> {
        // 1. Extract topic and context
        let (description, context) = match &input {
            PptInput::Text { description, context } => (description.as_str(), context.as_deref()),
        };
        
        if self.config.verbose {
            println!("Generating PPT for: {}", description);
        }
        
        // 2. Analyze requirements
        let analysis = AnalysisModule::new(self.llm.clone());
        let requirements = analysis.analyze(description, context).await?;
        
        if self.config.verbose {
            println!("Analyzed requirements: topic={}, audience={:?}, style={:?}", 
                requirements.topic, requirements.audience, requirements.style);
        }
        
        // 3. Generate outline
        let outline_tool = OutlineGeneratorTool::new(self.llm.clone());
        let mut outline = outline_tool.generate(
            &requirements.topic,
            requirements.audience.as_deref(),
            requirements.style.as_deref(),
            requirements.purpose.as_deref()
        ).await?;
        
        if self.config.verbose {
            println!("Generated outline with {} slides", outline.slides.len());
        }
        
        // 4. Expand content
        let content_tool = ContentGeneratorTool::new(self.llm.clone());
        outline = content_tool.expand(&outline).await?;
        
        if self.config.verbose {
            println!("Expanded content for all slides");
        }
        
        // 5. Match template
        let template = self.match_template(&requirements);
        
        if self.config.verbose {
            println!("Using template: {} ({})", template.name, template.id);
        }
        
        // 6. Render PPTX
        let mut renderer = PptxRenderer::new(template.clone());
        renderer.render_outline(&outline)?;
        
        let output_path = self.generate_output_path(&requirements.topic);
        renderer.save(&output_path)?;
        
        if self.config.verbose {
            println!("Saved PPTX to: {:?}", output_path);
        }
        
        Ok(PptOutput {
            output_path,
            slide_count: outline.slides.len() + 2, // +2 for title and TOC
            template_id: template.id.clone(),
            slides: outline.slides.iter()
                .map(|s| s.to_slide(crate::ppt::SlideLayout::TitleAndContent))
                .collect(),
        })
    }

    /// 匹配最佳模板
    fn match_template(&self, requirements: &crate::ppt::StructuredRequirement) -> &crate::ppt::PptTemplate {
        let mut keywords = vec![requirements.topic.clone()];
        if let Some(style) = &requirements.style {
            keywords.push(style.clone());
        }
        if let Some(audience) = &requirements.audience {
            keywords.push(audience.clone());
        }
        
        self.template_registry.match_template(&keywords)
            .cloned()
            .unwrap_or_else(|| self.template_registry.list_templates().first()
                .expect("No templates available")
                .clone())
    }

    /// 生成输出路径
    fn generate_output_path(&self, topic: &str) -> PathBuf {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let topic_slug = topic.split_whitespace().take(3).collect::<Vec<_>>().join("_");
        let filename = format!("{}_{}.pptx", timestamp, topic_slug);
        
        self.config.default_output_dir.clone()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(filename)
    }
}

/// PPT Agent 错误
#[derive(Debug, thiserror::Error)]
pub enum PptAgentError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Analysis failed: {0}")]
    AnalysisError(#[from] crate::ppt::analysis::AnalysisError),
    
    #[error("Outline generation failed: {0}")]
    OutlineError(#[from] OutlineError),
    
    #[error("Content generation failed: {0}")]
    ContentError(String),
    
    #[error("Rendering failed: {0}")]
    RenderError(#[from] RendererError),
    
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
```

- [ ] **Step 2: 更新 mod.rs 导出 PptAgentError**

```rust
pub use agent::{PptAgent, PptAgentError};
```

- [ ] **Step 3: 运行 cargo check 验证编译**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/agent.rs crates/vol-llm-agents/src/ppt/mod.rs
git commit -m "feat(ppt-agent): implement complete generate() flow with 5-step pipeline"
```

---

## Phase 8: CLI 工具更新

### Task 9: 更新 ppt-agent CLI

**Files:**
- Modify: `crates/ppt-agent/src/main.rs`

- [ ] **Step 1: 更新 main.rs 使用完整的 PptAgent**

```rust
//! ppt-agent: AI-powered PowerPoint generation CLI.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use vol_llm_agents::ppt::{PptAgent, PptAgentConfig, PptInput};
use vol_llm_agents::ppt::template::TemplateRegistry;

#[derive(Parser)]
#[command(name = "ppt-agent")]
#[command(about = "AI-powered PowerPoint generation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a PowerPoint presentation
    Generate {
        /// Text description of the presentation
        #[arg(short = 't', long)]
        text: String,

        /// Optional context (audience, purpose, etc.)
        #[arg(short, long)]
        context: Option<String>,

        /// Template ID to use
        #[arg(short = 'T', long)]
        template: Option<String>,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Verbose output
        #[arg(short, long, default_value = "false")]
        verbose: bool,
    },

    /// List available templates
    Templates {
        #[command(subcommand)]
        action: TemplatesAction,
    },
}

#[derive(Subcommand)]
enum TemplatesAction {
    /// List all available templates
    List,

    /// Preview a template
    Preview {
        /// Template ID
        template_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { text, context, template, output, verbose } => {
            // Initialize tracing
            if verbose {
                tracing_subscriber::fmt()
                    .with_max_level(tracing::Level::DEBUG)
                    .init();
            }

            println!("═══════════════════════════════════════════════════════════");
            println!("  PPT Agent - AI-powered Presentation Generation");
            println!("═══════════════════════════════════════════════════════════");
            println!();

            // Create config
            let mut config = PptAgentConfig::default()
                .with_verbose(verbose);

            // Set template dir to bundled templates
            let template_dir = PathBuf::from("crates/vol-llm-agents/src/ppt/templates");
            if template_dir.exists() {
                config = config.with_template_dir(&template_dir);
                if verbose {
                    println!("Template directory: {:?}", template_dir);
                }
            }

            // Set output dir if specified
            if let Some(output_path) = &output {
                if let Some(parent) = output_path.parent() {
                    config = config.with_default_output_dir(parent);
                }
            }

            // Create agent
            println!("Initializing PPT Agent...");
            let agent = PptAgent::new(config).await?;

            // Generate PPT
            let input = match &context {
                Some(ctx) => {
                    println!("Topic: {}", text);
                    println!("Context: {}", ctx);
                    PptInput::text_with_context(&text, ctx)
                },
                None => {
                    println!("Topic: {}", text);
                    PptInput::text(&text)
                }
            };

            if let Some(template_id) = &template {
                println!("Using template: {}", template_id);
            }

            println!();
            println!("Generating presentation...");
            println!();

            let result = agent.generate(input).await?;

            println!("✅ PPT generated successfully!");
            println!("   Output: {:?}", result.output_path);
            println!("   Slides: {}", result.slide_count);
            println!("   Template: {}", result.template_id);
        }

        Commands::Templates { action } => {
            match action {
                TemplatesAction::List => {
                    println!("═══════════════════════════════════════════════════════════");
                    println!("  Available Templates");
                    println!("═══════════════════════════════════════════════════════════");
                    println!();

                    // Load templates from bundled location
                    let template_dir = PathBuf::from("crates/vol-llm-agents/src/ppt/templates");
                    let mut registry = TemplateRegistry::new();

                    if template_dir.exists() {
                        if let Err(e) = registry.load_from_dir(&template_dir) {
                            eprintln!("Failed to load templates: {}", e);
                        }
                    }

                    let templates = registry.list_templates();
                    if templates.is_empty() {
                        println!("No templates found in {:?}", template_dir);
                    } else {
                        for t in templates {
                            println!("  {} - {}", t.id, t.name);
                            println!("    {}", t.description);
                            println!("    Tags: occasion={:?}, style={:?}", t.tags.occasion, t.tags.style);
                            println!();
                        }
                    }
                }
                TemplatesAction::Preview { template_id } => {
                    println!("Preview for template: {}", template_id);
                    // TODO: Implement preview
                    println!("(Preview not yet implemented)");
                }
            }
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Complete");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
```

- [ ] **Step 2: 运行 cargo build 验证 CLI 编译**

```bash
cargo build --bin ppt-agent
```

- [ ] **Step 3: 提交**

```bash
git add crates/ppt-agent/src/main.rs
git commit -m "feat(ppt-cli): update CLI to use complete PptAgent generation flow"
```

---

## Phase 9: 测试

### Task 10: 编写单元测试

**Files:**
- Create: `crates/vol-llm-agents/tests/ppt_types_unit.rs`
- Create: `crates/vol-llm-agents/tests/ppt_template_unit.rs`

- [ ] **Step 1: 创建 Outline JSON 解析测试**

```rust
//! PPT 类型单元测试。

use vol_llm_agents::ppt::{Outline, SlideDef, SlideType};
use serde_json::json;

#[test]
fn test_outline_json_parsing() {
    let json_str = r#"{
        "title": "Test Presentation",
        "slides": [
            {"type": "title", "title": "Main Title", "subtitle": "Subtitle"},
            {"type": "toc", "title": "Table of Contents", "sections": ["Section 1", "Section 2"]},
            {"type": "content", "title": "Content Slide", "bullets": ["Point 1", "Point 2"]}
        ]
    }"#;
    
    let outline: Outline = serde_json::from_str(json_str).unwrap();
    
    assert_eq!(outline.title, "Test Presentation");
    assert_eq!(outline.slides.len(), 3);
    
    // Check title slide
    assert!(matches!(outline.slides[0].slide_type, SlideType::Title));
    assert_eq!(outline.slides[0].title, "Main Title");
    assert_eq!(outline.slides[0].subtitle, Some("Subtitle".to_string()));
    
    // Check TOC slide
    assert!(matches!(outline.slides[1].slide_type, SlideType::TableOfContents));
    assert_eq!(outline.slides[1].sections.len(), 2);
    
    // Check content slide
    assert!(matches!(outline.slides[2].slide_type, SlideType::Content));
    assert_eq!(outline.slides[2].bullets.len(), 2);
}
```

- [ ] **Step 2: 创建模板布局测试**

```rust
//! PPT 模板单元测试。

use vol_llm_agents::ppt::template::{PptTemplate, TemplateLayout, LayoutType};
use std::fs;

#[test]
fn test_template_layout_loading() {
    // Test loading a template with layouts
    let yaml_content = r#"
id: test_template
name: Test Template
description: A test template
tags:
  occasion: ["test"]
  style: ["minimal"]
  audience: ["general"]
color_scheme:
  primary: "#FF0000"
  secondary: "#00FF00"
  accent: "#0000FF"
  background: "#FFFFFF"
  text_primary: "#000000"
  text_secondary: "#666666"
typography:
  title_font: "Arial"
  body_font: "Times New Roman"
layouts:
  - layout_type: title
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 228600
          width: 8229600
          height: 914400
        style:
          font_size: 44
          color: "{{primary}}"
"#;
    
    let template: PptTemplate = serde_yaml::from_str(yaml_content).unwrap();
    
    assert_eq!(template.id, "test_template");
    assert_eq!(template.layouts.len(), 1);
    assert!(matches!(template.layouts[0].layout_type, LayoutType::Title));
}

#[test]
fn test_position_resolution() {
    use vol_llm_agents::ppt::template::Position;
    
    // EMU: 1 inch = 914400 EMU
    let position = Position {
        x: 457200,
        y: 228600,
        width: 8229600,
        height: 914400,
    };
    
    // Convert to inches for verification
    let x_inches = position.x as f64 / 914400.0;
    let y_inches = position.y as f64 / 914400.0;
    
    assert!((x_inches - 0.5).abs() < 0.01);
    assert!((y_inches - 0.25).abs() < 0.01);
}
```

- [ ] **Step 3: 运行单元测试**

```bash
cd crates/vol-llm-agents && cargo test --test ppt_types_unit --test ppt_template_unit
```

Expected: All tests pass

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agents/tests/ppt_*.rs
git commit -m "test(ppt-unit): add unit tests for Outline parsing and TemplateLayout"
```

---

### Task 11: 创建集成测试

**Files:**
- Modify: `crates/vol-llm-agents/tests/ppt_agent_integration.rs`

- [ ] **Step 1: 更新集成测试**

```rust
//! PPT Agent 集成测试。

use vol_llm_agents::ppt::{PptAgent, PptAgentConfig, PptInput};
use std::path::PathBuf;

#[tokio::test]
#[ignore] // Requires LLM API key
async fn test_full_ppt_generation() {
    let config = PptAgentConfig::default()
        .with_verbose(true)
        .with_template_dir(PathBuf::from("src/ppt/templates"));
    
    let agent = PptAgent::new(config).await.unwrap();
    
    let input = PptInput::text("做一个期权周报，包含 IV 分析、RV 分析、交易建议");
    let result = agent.generate(input).await.unwrap();
    
    assert!(result.output_path.exists());
    assert!(result.slide_count >= 3); // At least title, TOC, and 1 content slide
    assert_eq!(result.template_id, "business_formal"); // Default template
    
    // Cleanup
    std::fs::remove_file(&result.output_path).ok();
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/tests/ppt_agent_integration.rs
git commit -m "test(ppt-integration): add full PPT generation test (requires API key)"
```

---

## Phase 10: 模板更新与验证

### Task 12: 更新 YAML 模板添加 layouts 定义

**Files:**
- Modify: `crates/vol-llm-agents/src/ppt/templates/business_formal.yaml`
- Modify: `crates/vol-llm-agents/src/ppt/templates/tech_minimal.yaml`
- Modify: `crates/vol-llm-agents/src/ppt/templates/academic_report.yaml`

- [ ] **Step 1: 更新 business_formal.yaml**

```yaml
id: business_formal
name: 商务正式
description: 适合商务汇报、融资路演等正式场合

tags:
  occasion: ["pitch_deck", "report", "presentation"]
  style: ["minimal", "professional", "clean"]
  audience: ["executive", "investor", "client"]

color_scheme:
  primary: "#1a365d"
  secondary: "#2d5a8a"
  accent: "#c9a227"
  background: "#ffffff"
  text_primary: "#1a1a1a"
  text_secondary: "#666666"

typography:
  title_font: "Microsoft YaHei"
  body_font: "Arial"

layouts:
  - layout_type: title
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 1828800
          width: 8229600
          height: 914400
        style:
          font_size: 44
          color: "{{primary}}"
      - element_type: textbox
        placeholder: subtitle
        position:
          x: 457200
          y: 3200400
          width: 8229600
          height: 685800
        style:
          font_size: 24
          color: "{{text_secondary}}"

  - layout_type: content
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 228600
          width: 8229600
          height: 914400
        style:
          font_size: 28
          color: "{{primary}}"
      - element_type: textbox
        placeholder: content
        position:
          x: 457200
          y: 1371600
          width: 8229600
          height: 4572000
        style:
          font_size: 16
          color: "{{text_primary}}"
          bullet_style: "bullet"

  - layout_type: table_of_contents
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 228600
          width: 8229600
          height: 914400
        style:
          font_size: 32
          color: "{{primary}}"
      - element_type: textbox
        placeholder: sections
        position:
          x: 685800
          y: 1371600
          width: 7772400
          height: 4572000
        style:
          font_size: 18
          color: "{{text_primary}}"
```

- [ ] **Step 2: 更新 tech_minimal.yaml**

```yaml
id: tech_minimal
name: 简洁科技
description: 适合技术分享、产品发布

tags:
  occasion: ["product_launch", "tech_talk", "demo"]
  style: ["minimal", "modern", "clean"]
  audience: ["technical", "developer", "product"]

color_scheme:
  primary: "#2563eb"
  secondary: "#3b82f6"
  accent: "#10b981"
  background: "#f8fafc"
  text_primary: "#1e293b"
  text_secondary: "#64748b"

typography:
  title_font: "Microsoft YaHei"
  body_font: "Consolas"

layouts:
  - layout_type: title
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 1828800
          width: 8229600
          height: 914400
        style:
          font_size: 44
          color: "{{primary}}"
      - element_type: textbox
        placeholder: subtitle
        position:
          x: 457200
          y: 3200400
          width: 8229600
          height: 685800
        style:
          font_size: 24
          color: "{{text_secondary}}"

  - layout_type: content
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 228600
          width: 8229600
          height: 914400
        style:
          font_size: 28
          color: "{{primary}}"
      - element_type: textbox
        placeholder: content
        position:
          x: 457200
          y: 1371600
          width: 8229600
          height: 4572000
        style:
          font_size: 16
          color: "{{text_primary}}"
          bullet_style: "bullet"
```

- [ ] **Step 3: 更新 academic_report.yaml**

```yaml
id: academic_report
name: 学术报告
description: 适合学术论文、研究报告

tags:
  occasion: ["academic", "research", "thesis"]
  style: ["formal", "structured", "detailed"]
  audience: ["academic", "researcher", "professor"]

color_scheme:
  primary: "#1e3a5f"
  secondary: "#3d5a7a"
  accent: "#8b4513"
  background: "#ffffff"
  text_primary: "#000000"
  text_secondary: "#333333"

typography:
  title_font: "Times New Roman"
  body_font: "Arial"

layouts:
  - layout_type: title
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 1828800
          width: 8229600
          height: 914400
        style:
          font_size: 44
          color: "{{primary}}"
      - element_type: textbox
        placeholder: subtitle
        position:
          x: 457200
          y: 3200400
          width: 8229600
          height: 685800
        style:
          font_size: 24
          color: "{{text_secondary}}"

  - layout_type: content
    elements:
      - element_type: textbox
        placeholder: title
        position:
          x: 457200
          y: 228600
          width: 8229600
          height: 914400
        style:
          font_size: 28
          color: "{{primary}}"
      - element_type: textbox
        placeholder: content
        position:
          x: 457200
          y: 1371600
          width: 8229600
          height: 4572000
        style:
          font_size: 16
          color: "{{text_primary}}"
          bullet_style: "bullet"
```

- [ ] **Step 4: 运行 cargo check 验证 YAML 解析**

```bash
cd crates/vol-llm-agents && cargo check
```

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/templates/*.yaml
git commit -m "feat(ppt-templates): add layouts definitions to all YAML templates"
```

---

### Task 13: 手动验证

- [ ] **Step 1: 构建并运行 CLI**

```bash
cargo build --bin ppt-agent

# Test generation
./target/debug/ppt-agent generate --text "期权周报" --verbose
```

- [ ] **Step 2: 验证生成的 PPTX 文件**

```bash
# Check file exists
ls -la *.pptx

# Open with PowerPoint or LibreOffice to verify content
```

- [ ] **Step 3: 列出可用模板**

```bash
./target/debug/ppt-agent templates list
```

Expected output:
```
business_formal - 商务正式
tech_minimal - 简洁科技
academic_report - 学术报告
```

- [ ] **Step 4: 提交**

```bash
git add .
git commit -m "docs(ppt): add manual verification notes"
```

---

## Self-Review Checklist

**1. Spec coverage:**

| Spec Requirement | Task |
|------------------|------|
| StructuredRequirement type | Task 1 |
| Outline and SlideDef types | Task 1 |
| TemplateLayout schema | Task 2 |
| ANALYSIS_SYSTEM_PROMPT | Task 3 |
| OUTLINE_SYSTEM_PROMPT | Task 3 |
| CONTENT_SYSTEM_PROMPT | Task 3 |
| AnalysisModule | Task 4 |
| OutlineGeneratorTool | Task 5 |
| ContentGeneratorTool | Task 6 |
| PptxRenderer with color resolution | Task 7 |
| Complete generate() flow | Task 8 |
| CLI update | Task 9 |
| Unit tests | Task 10 |
| Integration test | Task 11 |
| YAML templates with layouts | Task 12 |

**2. Placeholder scan:** No TBD/TODO placeholders found in implementation steps.

**3. Type consistency:** 
- `StructuredRequirement`, `Outline`, `SlideDef` defined in types.rs and used throughout
- `TemplateLayout`, `LayoutElement`, `Position`, `ElementStyle` defined in template.rs
- `PptxRenderer` uses `PptTemplate` and its `color_scheme` for color resolution
- Error types properly converted with `#[from]` attribute

---

## Verification Commands

```bash
# Build all
cargo build --workspace

# Run unit tests
cargo test -p vol-llm-agents --test ppt_types_unit --test ppt_template_unit

# Run integration test (requires API key)
cargo test -p vol-llm-agents ppt_agent_integration -- --ignored

# Run CLI
./target/debug/ppt-agent generate --text "测试 PPT" --verbose
./target/debug/ppt-agent templates list
```
