# PPT 完整生成逻辑设计

**日期**: 2026-04-11
**状态**: Draft
**作者**: AI Assistant

---

## 1. 概述

### 1.1 背景

当前 PPT Agent MVP 已实现：
- CLI 工具（`ppt-agent generate`、`ppt-agent templates list`）
- 模板 YAML 加载和关键词匹配
- 占位符生成逻辑

缺失能力：实际的 PPT 生成逻辑（LLM 大纲生成、内容生成、PPTX 渲染）。

### 1.2 目标

实现完整的 PPT 生成流程：
1. LLM 驱动的大纲生成
2. LLM 驱动的内容填充
3. 基于模板布局的 PPTX 渲染

---

## 2. 架构设计

### 2.1 组件概览

```
┌────────────────────────────────────────────────────────────┐
│                    PptAgent                                 │
├────────────────────────────────────────────────────────────┤
│  - llm: Arc<dyn LLMClient>                                │
│  - template_registry: TemplateRegistry                    │
│  - renderer: PptxRenderer                                 │
└────────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│  Analysis     │ │  Outline      │ │  Content      │
│  Module       │ │  Generator    │ │  Generator    │
└───────────────┘ └───────────────┘ └───────────────┘
                          │
                          ▼
                  ┌───────────────┐
                  │  PptxRenderer │
                  │  (布局填充)    │
                  └───────────────┘
```

### 2.2 生成流程

```
用户输入 (文字描述)
     │
     ▼
┌─────────────────┐
│ 1. 需求分析      │  LLM 提取：主题、受众、风格、目的
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 2. 大纲生成      │  LLM 生成：标题 + 幻灯片数组
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 3. 模板匹配      │  关键词匹配 (已实现)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 4. 内容生成      │  LLM 填充：每页标题 + bullets
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 5. PPTX 渲染     │  基于模板布局填充元素
└─────────────────┘
```

---

## 3. 核心类型

### 3.1 新增类型

```rust
// crates/vol-llm-agents/src/ppt/types.rs

/// 结构化需求
pub struct StructuredRequirement {
    pub topic: String,
    pub audience: Option<String>,
    pub style: Option<String>,
    pub purpose: Option<String>,
}

/// PPT 大纲
pub struct Outline {
    pub title: String,
    pub slides: Vec<SlideDef>,
}

/// 幻灯片定义
pub struct SlideDef {
    pub slide_type: SlideType,
    pub title: String,
    pub bullets: Vec<String>,
}
```

### 3.2 模板布局 Schema

```rust
// crates/vol-llm-agents/src/ppt/template.rs

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
    pub x: i32,       // EMU units (English Metric Units: 1 inch = 914400 EMU)
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

### 3.3 更新 PptTemplate

```rust
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

---

## 4. Prompt 设计

### 4.1 需求分析 Prompt

```rust
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
```

### 4.2 大纲生成 Prompt

```rust
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
        {"type": "title", "title": "Main Title"},
        {"type": "toc", "title": "Table of Contents", "bullets": ["Section 1", "Section 2"]},
        {"type": "content", "title": "Slide Title", "bullets": ["Point 1", "Point 2", "Point 3"]}
    ]
}"#;
```

### 4.3 内容生成 Prompt

```rust
pub const CONTENT_SYSTEM_PROMPT: &str = r#"You are a professional content writer for business presentations.
Expand outline bullets into detailed, presentation-ready content.

Guidelines:
- Each bullet should be 1-2 lines, concise
- Use action verbs and specific data
- Avoid full sentences - use fragments
- Maintain consistent tone

Return ONLY valid JSON with expanded bullets for each slide."#;
```

---

## 5. PPTX 渲染设计

### 5.1 PptxRenderer 接口

```rust
pub struct PptxRenderer {
    presentation: Presentation,  // pptx::Presentation
    template: PptTemplate,
}

impl PptxRenderer {
    pub fn new(template: PptTemplate) -> Result<Self>;
    
    pub fn add_title_slide(&mut self, title: &str, subtitle: &str) -> Result<()>;
    pub fn add_toc_slide(&mut self, title: &str, sections: &[String]) -> Result<()>;
    pub fn add_content_slide(&mut self, title: &str, bullets: &[String]) -> Result<()>;
    
    pub fn save(&self, path: &Path) -> Result<()>;
}
```

### 5.2 布局渲染逻辑

```rust
fn render_element(&mut self, element: &LayoutElement, content: &str) -> Result<()> {
    match element.element_type.as_str() {
        "textbox" => {
            // Get position from element.position
            // Get style from element.style (resolve {{primary}} to actual color)
            // Create textbox with content
        }
        "image" => { /* TODO: future iteration */ }
        "chart" => { /* TODO: future iteration */ }
        _ => Err(Error::UnknownElement(element.element_type.clone()))
    }
}
```

### 5.3 颜色模板变量解析

```rust
fn resolve_color(&self, color: &str) -> String {
    // "{{primary}}" → actual color from template.color_scheme.primary
    if let Some(stripped) = color.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        match stripped {
            "primary" => self.template.color_scheme.primary.clone(),
            "secondary" => self.template.color_scheme.secondary.clone(),
            "text_primary" => self.template.color_scheme.text_primary.clone(),
            "text_secondary" => self.template.color_scheme.text_secondary.clone(),
            _ => "#000000".to_string(), // default to black
        }
    } else {
        color.to_string() // already a HEX color
    }
}
```

---

## 6. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum PptAgentError {
    #[error("LLM call failed: {0}")]
    LlmError(#[from] vol_llm_core::Error),
    
    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("PPTX rendering failed: {0}")]
    RenderError(#[from] pptx::error::PptxError),
    
    #[error("Template '{0}' not found")]
    TemplateNotFound(String),
    
    #[error("Invalid outline structure: {0}")]
    InvalidOutline(String),
}
```

---

## 7. 文件变更清单

### 7.1 修改文件

| 文件 | 变更内容 |
|------|----------|
| `crates/vol-llm-agents/src/ppt/types.rs` | 新增 `StructuredRequirement`, `Outline`, `SlideDef` |
| `crates/vol-llm-agents/src/ppt/template.rs` | 新增 `TemplateLayout`, `LayoutElement`, `Position`, `ElementStyle` |
| `crates/vol-llm-agents/src/ppt/agent.rs` | 实现完整生成流程 |
| `crates/vol-llm-agents/src/ppt/renderer.rs` | 实现 PPTX 渲染逻辑 |
| `crates/vol-llm-agents/src/ppt/prompt.rs` | 新增 Prompts |

### 7.2 新增文件

| 文件 | 内容 |
|------|------|
| `crates/vol-llm-agents/src/ppt/analysis.rs` | 需求分析模块 |
| `crates/vol-llm-agents/src/ppt/outline.rs` | 大纲生成模块 |
| `crates/vol-llm-agents/src/ppt/content.rs` | 内容生成模块 |

### 7.3 模板更新

更新现有 YAML 模板，添加 `layouts` 定义：
- `business_formal.yaml`
- `tech_minimal.yaml`
- `academic_report.yaml`

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[test]
fn test_outline_json_parsing() { /* ... */ }

#[test]
fn test_template_layout_loading() { /* ... */ }

#[test]
fn test_position_resolution() { /* ... */ }
```

### 8.2 集成测试

```rust
#[tokio::test]
#[ignore] // Requires LLM API key
async fn test_full_ppt_generation() {
    let agent = PptAgent::new(config).await.unwrap();
    let input = PptInput::text("期权周报");
    let output = agent.generate(input).await.unwrap();
    
    assert!(output.output_path.exists());
    assert!(output.slide_count >= 3);
}
```

---

## 9. 验收标准

- [ ] `ppt-agent generate --text "期权周报"` 生成有效的 .pptx 文件
- [ ] 生成的 PPTX 可用 PowerPoint 或 LibreOffice 打开
- [ ] 包含封面页、目录页、至少 3 个内容页
- [ ] 模板颜色和字体正确应用
- [ ] LLM API 调用错误有适当处理

---

## 10. 开放问题

1. **图片支持**: 是否在当前 iteration 支持图片插入？
   - 决策：MVP 不支持，后续 iteration 添加

2. **图表支持**: 是否需要生成数据图表？
   - 决策：MVP 不支持，后续 iteration 添加

3. **演讲者备注**: 是否生成 speaker notes？
   - 决策：MVP 不支持，后续 iteration 添加
