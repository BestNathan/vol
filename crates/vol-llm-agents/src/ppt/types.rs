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
    pub content: SlideContent,
}

/// 幻灯片内容
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlideContent {
    pub bullets: Vec<String>,
    pub speaker_notes: Option<String>,
}
