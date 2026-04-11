# PPT Agent MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现独立的 PPT Agent CLI 工具，支持文字描述生成 PPTX 报告

**Architecture:** 独立 binary `ppt-agent`，基于 `ppt-rs` 库生成 PPTX，YAML 模板存储，智能模板匹配

**Tech Stack:** Rust, ppt-rs (PPTX 生成), clap (CLI), serde_yaml (模板加载), vol-llm-provider (LLM 接入)

---

## Phase 1: 项目设置与依赖

### Task 1: 添加依赖和目录结构

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/mod.rs`
- Create: `crates/vol-llm-agents/src/ppt/templates/` (directory)
- Modify: `crates/vol-llm-agents/Cargo.toml`
- Modify: `crates/vol-llm-agents/src/lib.rs`

- [ ] **Step 1: 添加 ppt-rs 和 serde_yaml 依赖到 Cargo.toml**

在 `crates/vol-llm-agents/Cargo.toml` 的 `[dependencies]` 部分添加：

```toml
ppt-rs = "0.1"
serde_yaml = "0.9"
clap = { version = "4.4", features = ["derive"] }
```

- [ ] **Step 2: 创建 ppt 模块目录结构**

```bash
mkdir -p crates/vol-llm-agents/src/ppt/templates
mkdir -p crates/vol-llm-agents/src/ppt/tools
```

- [ ] **Step 3: 创建 mod.rs 导出公共类型**

创建 `crates/vol-llm-agents/src/ppt/mod.rs`：

```rust
//! PPT Agent: AI-powered PowerPoint generation.

pub mod agent;
pub mod config;
pub mod types;
pub mod template;
pub mod renderer;
pub mod tools;

pub use agent::PptAgent;
pub use config::PptAgentConfig;
pub use types::{PptInput, PptOutput, Slide, SlideLayout};
pub use template::{TemplateRegistry, PptTemplate};
```

- [ ] **Step 4: 更新 lib.rs 导出 ppt 模块**

在 `crates/vol-llm-agents/src/lib.rs` 添加：

```rust
pub mod ppt;
pub use ppt::{PptAgent, PptAgentConfig, PptInput, PptOutput};
```

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/Cargo.toml crates/vol-llm-agents/src/ppt/
git commit -m "feat(ppt): add ppt module structure and dependencies"
```

---

## Phase 2: 类型定义

### Task 2: 定义核心类型 (types.rs)

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/types.rs`

- [ ] **Step 1: 创建 PptInput 类型**

```rust
use std::path::PathBuf;

/// PPT 生成请求的输入类型
#[derive(Clone, Debug)]
pub enum PptInput {
    /// 纯文字描述
    Text {
        description: String,
        context: Option<String>,
    },
}

impl PptInput {
    pub fn text(description: impl Into<String>) -> Self {
        Self::Text {
            description: description.into(),
            context: None,
        }
    }

    pub fn text_with_context(description: impl Into<String>, context: impl Into<String>) -> Self {
        Self::Text {
            description: description.into(),
            context: Some(context.into()),
        }
    }
}
```

- [ ] **Step 2: 创建 PptOutput 类型**

```rust
/// PPT 生成结果
#[derive(Clone, Debug)]
pub struct PptOutput {
    pub output_path: PathBuf,
    pub slide_count: usize,
    pub template_id: String,
    pub slides: Vec<Slide>,
}
```

- [ ] **Step 3: 创建 Slide 和 SlideLayout 类型**

```rust
use serde::{Deserialize, Serialize};

/// 幻灯片类型
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideType {
    Title,
    TableOfContents,
    Content,
    SectionHeader,
}

/// 幻灯片布局
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideLayout {
    TitleOnly,
    TitleAndContent,
    TwoColumn,
    BulletList,
}

/// 单张幻灯片
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slide {
    pub slide_type: SlideType,
    pub layout: SlideLayout,
    pub title: String,
    pub content: SlideContent,
}

/// 幻灯片内容
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlideContent {
    pub bullets: Vec<String>,
    pub speaker_notes: Option<String>,
}
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/types.rs
git commit -m "feat(ppt-types): define core types for PPT generation"
```

---

## Phase 3: 配置与 Prompt

### Task 3: 实现 PptAgentConfig (config.rs)

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/config.rs`

- [ ] **Step 1: 创建配置结构**

```rust
use std::path::PathBuf;

/// PPT Agent 配置
#[derive(Clone, Debug)]
pub struct PptAgentConfig {
    /// LLM Provider ID
    pub llm_provider_id: String,
    /// 模板目录路径
    pub template_dir: Option<PathBuf>,
    /// 默认输出目录
    pub default_output_dir: Option<PathBuf>,
    /// 详细日志
    pub verbose: bool,
}

impl Default for PptAgentConfig {
    fn default() -> Self {
        Self {
            llm_provider_id: "anthropic-main".to_string(),
            template_dir: None,
            default_output_dir: None,
            verbose: false,
        }
    }
}

impl PptAgentConfig {
    pub fn with_llm_provider(mut self, provider_id: impl Into<String>) -> Self {
        self.llm_provider_id = provider_id.into();
        self
    }

    pub fn with_template_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.template_dir = Some(path.into());
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/config.rs
git commit -m "feat(ppt-config): add PptAgentConfig"
```

---

### Task 4: 定义 Prompts (prompt.rs)

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/prompt.rs`

- [ ] **Step 1: 创建 System Prompt**

```rust
/// 大纲生成 System Prompt
pub const OUTLINE_SYSTEM_PROMPT: &str = r#"You are a professional presentation designer. 
Your task is to create a structured outline for a PowerPoint presentation based on the user's request.

Output format (JSON):
{
    "title": "Presentation Title",
    "slides": [
        {"type": "title", "title": "Main Title", "subtitle": "Subtitle"},
        {"type": "toc", "title": "Table of Contents", "sections": ["Section 1", "Section 2"]},
        {"type": "content", "title": "Slide Title", "bullets": ["Point 1", "Point 2"]},
        ...
    ]
}

Guidelines:
- Start with a title slide
- Include a table of contents slide
- Create 5-10 content slides
- End with a summary or Q&A slide
- Use clear, concise language
- Each content slide should have 3-5 bullet points

Return ONLY valid JSON, no explanation."#;

/// 内容生成 System Prompt
pub const CONTENT_SYSTEM_PROMPT: &str = r#"You are a professional content writer for business presentations.
Expand the outline into detailed slide content.

Guidelines:
- Each bullet point should be 1-2 lines
- Use action verbs and specific data
- Avoid full sentences - use fragments
- Maintain consistent tone and style

Return ONLY valid JSON with expanded content."#;
```

- [ ] **Step 2: 创建 User Prompt 构建函数**

```rust
pub fn build_outline_user_prompt(topic: &str, context: Option<&str>) -> String {
    let context_part = context.map(|c| format!("\n\nContext: {}", c)).unwrap_or_default();
    format!(
        r#"Create a PowerPoint outline for the following topic:

Topic: {}
{}

Remember to output valid JSON with title and slides array."#,
        topic, context_part
    )
}

pub fn build_content_user_prompt(outline_json: &str) -> String {
    format!(
        r#"Expand the following outline into detailed slide content:

{}

Return valid JSON with expanded bullet points for each slide."#,
        outline_json
    )
}
```

- [ ] **Step 3: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/prompt.rs
git commit -m "feat(ppt-prompts): add system and user prompts for outline and content generation"
```

---

## Phase 4: 模板系统

### Task 5: 定义模板 YAML 格式 (template.rs)

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/template.rs`

- [ ] **Step 1: 创建模板结构**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
```

- [ ] **Step 2: 创建 TemplateRegistry**

```rust
use std::fs;
use std::sync::Arc;

/// 模板注册表
#[derive(Clone)]
pub struct TemplateRegistry {
    templates: Vec<Arc<PptTemplate>>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    /// 从目录加载 YAML 模板
    pub fn load_from_dir(&mut self, dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
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

    /// 获取所有模板
    pub fn list_templates(&self) -> &[Arc<PptTemplate>] {
        &self.templates
    }

    /// 按 ID 获取模板
    pub fn get_template(&self, id: &str) -> Option<&Arc<PptTemplate>> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// 简单匹配：基于关键词
    pub fn match_template(&self, keywords: &[String]) -> Option<&Arc<PptTemplate>> {
        // MVP: 简单关键词匹配
        // 后续迭代：LLM 分析 + 向量相似度
        for template in &self.templates {
            for keyword in keywords {
                if template.tags.occasion.iter().any(|t| t.contains(keyword))
                    || template.tags.style.iter().any(|t| t.contains(keyword))
                {
                    return Some(template);
                }
            }
        }
        // 默认返回第一个模板
        self.templates.first()
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 3: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/template.rs
git commit -m "feat(ppt-template): implement TemplateRegistry with YAML loading"
```

---

### Task 6: 创建预定义模板 YAML 文件

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/templates/business_formal.yaml`
- Create: `crates/vol-llm-agents/src/ppt/templates/tech_minimal.yaml`
- Create: `crates/vol-llm-agents/src/ppt/templates/academic_report.yaml`

- [ ] **Step 1: 商务正式模板**

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
```

- [ ] **Step 2: 简洁科技模板**

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
```

- [ ] **Step 3: 学术报告模板**

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
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/templates/
git commit -m "feat(ppt-templates): add 3 predefined templates (business, tech, academic)"
```

---

## Phase 5: PPTX 渲染器

### Task 7: 实现 PptxRenderer (renderer.rs)

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/renderer.rs`

- [ ] **Step 1: 导入 ppt-rs 并创建渲染器结构**

```rust
use ppt_rs::{Presentation, Slide as PptSlide, Shape, TextBox};
use crate::ppt::{Slide, SlideType, PptTemplate};
use std::path::PathBuf;

/// PPTX 渲染器
pub struct PptxRenderer {
    presentation: Presentation,
}

impl PptxRenderer {
    pub fn new() -> Self {
        Self {
            presentation: Presentation::new(),
        }
    }

    /// 创建封面页
    pub fn add_title_slide(&mut self, title: &str, subtitle: &str, template: &PptTemplate) {
        let slide = self.presentation.add_slide();
        
        // 添加标题
        let title_box = TextBox::new()
            .with_text(title)
            .with_font_size(44)
            .with_color(&template.color_scheme.primary);
        slide.add_shape(Shape::TextBox(title_box));
        
        // 添加副标题
        let subtitle_box = TextBox::new()
            .with_text(subtitle)
            .with_font_size(24)
            .with_color(&template.color_scheme.text_secondary);
        slide.add_shape(Shape::TextBox(subtitle_box));
    }

    /// 创建目录页
    pub fn add_toc_slide(&mut self, title: &str, sections: &[String], template: &PptTemplate) {
        let slide = self.presentation.add_slide();
        
        let title_box = TextBox::new()
            .with_text(title)
            .with_font_size(32)
            .with_color(&template.color_scheme.primary);
        slide.add_shape(Shape::TextBox(title_box));
        
        for (i, section) in sections.iter().enumerate() {
            let bullet = TextBox::new()
                .with_text(&format!("{} . {}", i + 1, section))
                .with_font_size(18)
                .with_color(&template.color_scheme.text_primary);
            slide.add_shape(Shape::TextBox(bullet));
        }
    }

    /// 创建内容页
    pub fn add_content_slide(&mut self, title: &str, bullets: &[String], template: &PptTemplate) {
        let slide = self.presentation.add_slide();
        
        let title_box = TextBox::new()
            .with_text(title)
            .with_font_size(28)
            .with_color(&template.color_scheme.primary);
        slide.add_shape(Shape::TextBox(title_box));
        
        for bullet in bullets {
            let bullet_box = TextBox::new()
                .with_text(&format!("• {}", bullet))
                .with_font_size(16)
                .with_color(&template.color_scheme.text_primary);
            slide.add_shape(Shape::TextBox(bullet_box));
        }
    }

    /// 保存到文件
    pub fn save(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        self.presentation.save(path)?;
        Ok(())
    }
}

impl Default for PptxRenderer {
    fn default() -> Self {
        Self::new()
    }
}
```

> **注意**: 上述代码假设 `ppt-rs` API。实际实现时需要根据 `ppt-rs` 文档调整。如果 API 不同，需要查阅 `https://docs.rs/ppt-rs` 进行调整。

- [ ] **Step 2: 测试 ppt-rs 基本用法**

运行以下命令验证 `ppt-rs` 是否可编译：

```bash
cd crates/vol-llm-agents && cargo check
```

如果 `ppt-rs` API 与假设不同，需要：
1. 查阅 https://docs.rs/ppt-rs 获取正确的 API
2. 调整 `PptxRenderer` 实现

- [ ] **Step 3: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/renderer.rs
git commit -m "feat(ppt-renderer): implement PptxRenderer with ppt-rs"
```

---

## Phase 6: 工具集

### Task 8: 实现 OutlineGeneratorTool

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/tools/outline.rs`

- [ ] **Step 1: 创建工具结构**

```rust
use vol_llm_tool::{Tool, ExecutableTool, ToolContext, ToolResult, ToolError, Result};
use vol_llm_core::LLMClient;
use vol_llm_core::{ConversationRequest, Message};
use serde_json::{json, Value};
use async_trait::async_trait;
use crate::ppt::prompt::{OUTLINE_SYSTEM_PROMPT, build_outline_user_prompt};

/// 大纲生成工具
pub struct OutlineGeneratorTool {
    llm: std::sync::Arc<dyn LLMClient>,
}

impl OutlineGeneratorTool {
    pub fn new(llm: std::sync::Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl ExecutableTool for OutlineGeneratorTool {
    fn name(&self) -> &'static str {
        "generate_outline"
    }

    fn description(&self) -> &'static str {
        "Generate a structured outline for a PowerPoint presentation based on user input."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "The presentation topic"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context (audience, purpose, etc.)"
                }
            },
            "required": ["topic"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let topic = args["topic"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'topic' argument".to_string())
        })?;
        
        let context = args["context"].as_str();
        
        // Build messages
        let system_message = Message::system(OUTLINE_SYSTEM_PROMPT.to_string());
        let user_message = Message::user(build_outline_user_prompt(topic, context));
        
        // Call LLM
        let request = ConversationRequest::with_messages(vec![system_message, user_message]);
        let response = self.llm.converse(request).await
            .map_err(|e| ToolError::ExecutionFailed(format!("LLM call failed: {}", e)))?;
        
        // Parse JSON response
        let outline: Value = serde_json::from_str(&response.content)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse outline JSON: {}", e)))?;
        
        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content: serde_json::to_string_pretty(&outline).unwrap_or_else(|_| outline.to_string()),
            error: None,
            data: Some(outline),
        })
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/tools/outline.rs
git commit -m "feat(ppt-tool-outline): implement OutlineGeneratorTool"
```

---

### Task 9: 实现 ContentGeneratorTool

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/tools/content.rs`

- [ ] **Step 1: 创建工具结构**

```rust
use vol_llm_tool::{Tool, ExecutableTool, ToolContext, ToolResult, ToolError, Result};
use vol_llm_core::LLMClient;
use vol_llm_core::{ConversationRequest, Message};
use serde_json::{json, Value};
use async_trait::async_trait;
use crate::ppt::prompt::{CONTENT_SYSTEM_PROMPT, build_content_user_prompt};

/// 内容生成工具
pub struct ContentGeneratorTool {
    llm: std::sync::Arc<dyn LLMClient>,
}

impl ContentGeneratorTool {
    pub fn new(llm: std::sync::Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl ExecutableTool for ContentGeneratorTool {
    fn name(&self) -> &'static str {
        "generate_content"
    }

    fn description(&self) -> &'static str {
        "Expand outline into detailed slide content with bullet points."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "outline": {
                    "type": "string",
                    "description": "JSON outline to expand"
                }
            },
            "required": ["outline"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let outline = args["outline"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'outline' argument".to_string())
        })?;
        
        // Build messages
        let system_message = Message::system(CONTENT_SYSTEM_PROMPT.to_string());
        let user_message = Message::user(build_content_user_prompt(outline));
        
        // Call LLM
        let request = ConversationRequest::with_messages(vec![system_message, user_message]);
        let response = self.llm.converse(request).await
            .map_err(|e| ToolError::ExecutionFailed(format!("LLM call failed: {}", e)))?;
        
        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content: response.content.clone(),
            error: None,
            data: Some(serde_json::from_str(&response.content).unwrap_or(Value::Null)),
        })
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/tools/content.rs
git commit -m "feat(ppt-tool-content): implement ContentGeneratorTool"
```

---

### Task 10: 实现 TemplateMatcherTool

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/tools/template.rs`

- [ ] **Step 1: 创建工具结构**

```rust
use vol_llm_tool::{Tool, ExecutableTool, ToolContext, ToolResult, ToolError, Result};
use serde_json::{json, Value};
use async_trait::async_trait;
use crate::ppt::template::TemplateRegistry;

/// 模板匹配工具
pub struct TemplateMatcherTool {
    registry: TemplateRegistry,
}

impl TemplateMatcherTool {
    pub fn new(registry: TemplateRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ExecutableTool for TemplateMatcherTool {
    fn name(&self) -> &'static str {
        "match_template"
    }

    fn description(&self) -> &'static str {
        "Match the best template based on presentation topic and context."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "Presentation topic"
                },
                "style_preference": {
                    "type": "string",
                    "description": "Preferred style (e.g., 'formal', 'minimal')"
                }
            },
            "required": ["topic"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let topic = args["topic"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'topic' argument".to_string())
        })?;
        
        let style = args["style_preference"].as_str();
        
        // Extract keywords from topic
        let keywords: Vec<String> = topic.split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
        
        // Match template
        let template = self.registry.match_template(&keywords);
        
        let result = match template {
            Some(t) => json!({
                "template_id": t.id,
                "template_name": t.name,
                "description": t.description,
            }),
            None => json!({
                "error": "No matching template found"
            }),
        };
        
        Ok(ToolResult {
            call_id: String::new(),
            success: template.is_some(),
            content: serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()),
            error: None,
            data: Some(result),
        })
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/tools/template.rs
git commit -m "feat(ppt-tool-template): implement TemplateMatcherTool"
```

---

### Task 11: 实现 PptxRendererTool

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/tools/renderer.rs`

- [ ] **Step 1: 创建工具结构**

```rust
use vol_llm_tool::{Tool, ExecutableTool, ToolContext, ToolResult, ToolError, Result};
use serde_json::{json, Value};
use async_trait::async_trait;
use std::path::PathBuf;
use crate::ppt::renderer::PptxRenderer;
use crate::ppt::template::PptTemplate;

/// PPTX 渲染工具
pub struct PptxRendererTool;

impl PptxRendererTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PptxRendererTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for PptxRendererTool {
    fn name(&self) -> &'static str {
        "render_pptx"
    }

    fn description(&self) -> &'static str {
        "Render PowerPoint presentation to .pptx file."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "slides": {
                    "type": "array",
                    "items": {"type": "object"},
                    "description": "Array of slide objects"
                },
                "template": {
                    "type": "object",
                    "description": "Template configuration"
                },
                "output_path": {
                    "type": "string",
                    "description": "Output file path"
                }
            },
            "required": ["slides", "template", "output_path"]
        })
    }

    async fn execute(&self, args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let slides_json = args["slides"].as_object().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'slides' argument".to_string())
        })?;
        
        let output_path = args["output_path"].as_str().ok_or_else(|| {
            ToolError::InvalidArguments("Missing required 'output_path' argument".to_string())
        })?;
        
        // TODO: 实现实际渲染逻辑
        // 需要解析 slides_json 并调用 PptxRenderer
        
        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content: format!("PPTX rendered to: {}", output_path),
            error: None,
            data: Some(json!({"output_path": output_path})),
        })
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/tools/renderer.rs
git commit -m "feat(ppt-tool-renderer): implement PptxRendererTool (stub)"
```

---

### Task 12: 创建 tools/mod.rs 并注册工具

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/tools/mod.rs`

- [ ] **Step 1: 创建模块导出**

```rust
//! PPT Agent tools.

pub mod outline;
pub mod content;
pub mod template;
pub mod renderer;

pub use outline::OutlineGeneratorTool;
pub use content::ContentGeneratorTool;
pub use template::TemplateMatcherTool;
pub use renderer::PptxRendererTool;
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/tools/mod.rs
git commit -m "feat(ppt-tools): export all PPT tools"
```

---

## Phase 7: PptAgent 核心

### Task 13: 实现 PptAgent (agent.rs)

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/agent.rs`

- [ ] **Step 1: 创建 PptAgent 结构**

```rust
use std::sync::Arc;
use vol_llm_core::LLMClient;
use vol_llm_provider::LLMProviderRegistry;
use crate::ppt::{PptAgentConfig, PptInput, PptOutput};
use crate::ppt::template::TemplateRegistry;
use crate::ppt::renderer::PptxRenderer;
use crate::ppt::tools::{OutlineGeneratorTool, ContentGeneratorTool, TemplateMatcherTool, PptxRendererTool};
use vol_llm_tool::ToolRegistry;

/// PPT Agent
pub struct PptAgent {
    config: PptAgentConfig,
    llm: Arc<dyn LLMClient>,
    template_registry: TemplateRegistry,
    tool_registry: ToolRegistry,
}

impl PptAgent {
    /// 创建新的 PPT Agent
    pub async fn new(config: PptAgentConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // 初始化 LLM
        let registry = LLMProviderRegistry::new();
        let llm = registry.get(&config.llm_provider_id)
            .ok_or_else(|| format!("LLM provider '{}' not found", config.llm_provider_id))?;
        
        // 初始化模板注册表
        let mut template_registry = TemplateRegistry::new();
        if let Some(template_dir) = &config.template_dir {
            template_registry.load_from_dir(template_dir)?;
        }
        
        // 初始化工具注册表
        let mut tools = ToolRegistry::new();
        tools.register(OutlineGeneratorTool::new(llm.clone()));
        tools.register(ContentGeneratorTool::new(llm.clone()));
        tools.register(TemplateMatcherTool::new(template_registry.clone()));
        tools.register(PptxRendererTool::new());
        
        Ok(Self {
            config,
            llm,
            template_registry,
            tool_registry: tools,
        })
    }

    /// 生成 PPT
    pub async fn generate(&self, input: PptInput) -> Result<PptOutput, Box<dyn std::error::Error>> {
        // TODO: 实现生成流程
        todo!()
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/agent.rs
git commit -m "feat(ppt-agent): create PptAgent skeleton"
```

---

### Task 14: 实现生成流程 (service.rs)

**Files:**
- Create: `crates/vol-llm-agents/src/ppt/service.rs`

- [ ] **Step 1: 创建服务函数**

```rust
use crate::ppt::{PptInput, PptOutput, Slide, SlideType, SlideLayout, SlideContent};
use crate::ppt::agent::PptAgent;
use std::path::PathBuf;
use chrono::Local;

impl PptAgent {
    /// 生成 PPT 主流程
    pub async fn generate(&self, input: PptInput) -> Result<PptOutput, Box<dyn std::error::Error>> {
        // 1. 提取主题和上下文
        let (topic, context) = match &input {
            PptInput::Text { description, context } => (description.as_str(), context.as_deref()),
        };
        
        if self.config.verbose {
            println!("Generating PPT for topic: {}", topic);
        }
        
        // 2. 生成大纲
        let outline_result = self.tool_registry.execute("generate_outline", &serde_json::json!({
            "topic": topic,
            "context": context.unwrap_or(""),
        })).await?;
        
        let outline: serde_json::Value = serde_json::from_str(&outline_result.content)?;
        
        if self.config.verbose {
            println!("Generated outline: {}", outline);
        }
        
        // 3. 匹配模板
        let template_result = self.tool_registry.execute("match_template", &serde_json::json!({
            "topic": topic,
        })).await?;
        
        let template_match: serde_json::Value = serde_json::from_str(&template_result.content)?;
        let template_id = template_match["template_id"].as_str().unwrap_or("business_formal");
        let template = self.template_registry.get_template(template_id)
            .ok_or_else(|| format!("Template '{}' not found", template_id))?;
        
        if self.config.verbose {
            println!("Using template: {} ({})", template.name, template.id);
        }
        
        // 4. 生成内容
        let content_result = self.tool_registry.execute("generate_content", &serde_json::json!({
            "outline": outline.to_string(),
        })).await?;
        
        let content: serde_json::Value = serde_json::from_str(&content_result.content)?;
        
        // 5. 构建 Slide 结构
        let slides = self.parse_slides(&content)?;
        
        // 6. 渲染 PPTX
        let output_path = self.generate_output_path(topic)?;
        let mut renderer = PptxRenderer::new();
        
        // 添加封面
        let title = outline["title"].as_str().unwrap_or("Presentation");
        let subtitle = context.unwrap_or("Generated by PPT Agent");
        renderer.add_title_slide(title, subtitle, template);
        
        // 添加目录
        let sections: Vec<String> = slides.iter()
            .filter(|s| matches!(s.slide_type, SlideType::Content))
            .map(|s| s.title.clone())
            .collect();
        renderer.add_toc_slide("目录", &sections, template);
        
        // 添加内容页
        for slide in &slides {
            renderer.add_content_slide(&slide.title, &slide.content.bullets, template);
        }
        
        renderer.save(&output_path)?;
        
        Ok(PptOutput {
            output_path,
            slide_count: slides.len() + 2, // +2 for title and TOC
            template_id: template.id.clone(),
            slides,
        })
    }
    
    fn parse_slides(&self, content: &serde_json::Value) -> Result<Vec<Slide>, Box<dyn std::error::Error>> {
        // TODO: 解析 JSON 内容为 Slide 结构
        Ok(Vec::new())
    }
    
    fn generate_output_path(&self, topic: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let topic_slug = topic.split_whitespace().take(3).collect::<Vec<_>>().join("_");
        let filename = format!("{}_{}.pptx", timestamp, topic_slug);
        
        let output_dir = self.config.default_output_dir.clone()
            .unwrap_or_else(|| PathBuf::from("."));
        
        Ok(output_dir.join(filename))
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/src/ppt/service.rs
git commit -m "feat(ppt-service): implement PPT generation flow"
```

---

## Phase 8: CLI 工具

### Task 15: 创建 CLI binary

**Files:**
- Create: `crates/ppt-agent/src/main.rs`
- Create: `crates/ppt-agent/Cargo.toml`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "ppt-agent"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-agents = { path = "../vol-llm-agents" }
vol-llm-provider = { path = "../vol-llm-provider" }
tokio = { workspace = true }
clap = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 2: 更新 workspace Cargo.toml**

在 `/root/nq-deribit/Cargo.toml` 的 `members` 中添加：

```toml
members = [
    # ... existing members ...
    "crates/ppt-agent",
]
```

- [ ] **Step 3: 创建 CLI main.rs**

```rust
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
        #[arg(short, long)]
        text: String,
        
        /// Optional context (audience, purpose, etc.)
        #[arg(short, long)]
        context: Option<String>,
        
        /// Template ID to use
        #[arg(short, long)]
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
            
            // Create config
            let mut config = PptAgentConfig::default()
                .with_verbose(verbose);
            
            if let Some(output_dir) = output.as_ref().and_then(|p| p.parent()) {
                config = config.with_default_output_dir(output_dir.to_path_buf());
            }
            
            // Create agent
            let agent = PptAgent::new(config).await?;
            
            // Generate PPT
            let input = match context {
                Some(ctx) => PptInput::text_with_context(&text, &ctx),
                None => PptInput::text(&text),
            };
            
            let result = agent.generate(input).await?;
            
            println!("✓ PPT generated successfully!");
            println!("  Output: {}", result.output_path.display());
            println!("  Slides: {}", result.slide_count);
            println!("  Template: {}", result.template_id);
        }
        
        Commands::Templates { action } => {
            match action {
                TemplatesAction::List => {
                    let registry = TemplateRegistry::new();
                    // TODO: Load templates from default path
                    println!("Available templates:");
                    for template in registry.list_templates() {
                        println!("  {} - {}", template.id, template.name);
                    }
                }
                TemplatesAction::Preview { template_id } => {
                    println!("Preview for template: {}", template_id);
                    // TODO: Implement preview
                }
            }
        }
    }
    
    Ok(())
}
```

- [ ] **Step 4: 提交**

```bash
git add crates/ppt-agent/ Cargo.toml
git commit -m "feat(ppt-cli): create ppt-agent CLI binary"
```

---

## Phase 9: 测试与验证

### Task 16: 编写集成测试

**Files:**
- Create: `crates/vol-llm-agents/tests/ppt_agent_integration.rs`

- [ ] **Step 1: 创建端到端测试**

```rust
use vol_llm_agents::ppt::{PptAgent, PptAgentConfig, PptInput};
use std::path::PathBuf;

#[tokio::test]
#[ignore] // Requires LLM API key
async fn test_ppt_generation_text_only() {
    let config = PptAgentConfig::default()
        .with_verbose(true)
        .with_template_dir(PathBuf::from("src/ppt/templates"));
    
    let agent = PptAgent::new(config).await.unwrap();
    
    let input = PptInput::text("做一个期权周报，包含 IV 分析、RV 分析、交易建议");
    let result = agent.generate(input).await.unwrap();
    
    assert!(result.output_path.exists());
    assert!(result.slide_count >= 3); // At least title, TOC, and 1 content slide
}
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-llm-agents/tests/ppt_agent_integration.rs
git commit -m "test(ppt-integration): add end-to-end PPT generation test"
```

---

### Task 17: 手动验证

- [ ] **Step 1: 构建并运行 CLI**

```bash
cargo build --bin ppt-agent

# Test generation
./target/debug/ppt-agent generate --text "期权周报" --verbose

# List templates
./target/debug/ppt-agent templates list
```

- [ ] **Step 2: 验证生成的 PPTX 文件**

```bash
# Check file exists
ls -la *.pptx

# Open with PowerPoint or LibreOffice to verify
```

- [ ] **Step 3: 提交**

```bash
git add .
git commit -m "docs(ppt): add manual verification notes"
```

---

## Verification

Run these commands to verify the implementation:

```bash
# Build all crates
cargo build --workspace

# Run integration test (requires API key)
cargo test -p vol-llm-agents ppt_generation -- --ignored

# Run CLI
./target/debug/ppt-agent generate --text "测试 PPT" --verbose
```

---

## Summary

- **Total Tasks**: 17
- **Total Steps**: ~40
- **Estimated Time**: 2-4 hours (depending on ppt-rs API familiarity)

**Key Risks:**
1. `ppt-rs` API may differ from assumptions - need to check docs.rs
2. LLM API integration requires valid credentials
3. JSON parsing from LLM responses may need error handling improvements
