//! PPT Agent Prompts.

use super::types::StructuredRequirement;

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
pub fn build_outline_user_prompt(requirements: &StructuredRequirement) -> String {
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
