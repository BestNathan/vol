//! PPT Agent 类型定义。

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

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

/// PPT 生成结果
#[derive(Clone, Debug)]
pub struct PptOutput {
    pub output_path: PathBuf,
    pub slide_count: usize,
    pub template_id: String,
    pub slides: Vec<Slide>,
}

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
    pub subtitle: Option<String>,
    pub content: SlideContent,
}

/// 幻灯片内容
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlideContent {
    pub bullets: Vec<String>,
    pub speaker_notes: Option<String>,
}

/// 结构化需求
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuredRequirement {
    pub topic: String,
    pub audience: Option<String>,
    pub style: Option<String>,
    pub purpose: Option<String>,
}

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
            subtitle: self.subtitle.clone(),
            content: SlideContent {
                bullets: if !self.bullets.is_empty() {
                    self.bullets.clone()
                } else {
                    self.sections.clone()
                },
                speaker_notes: None,
            },
        }
    }
}
