//! PPT 类型单元测试。

use serde_json::json;
use vol_llm_agents::ppt::{Outline, SlideDef, SlideType};

#[test]
fn test_outline_json_parsing() {
    let json_str = r#"{
        "title": "Test Presentation",
        "slides": [
            {"type": "title", "title": "Main Title", "subtitle": "Subtitle"},
            {"type": "table_of_contents", "title": "Table of Contents", "sections": ["Section 1", "Section 2"]},
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
    assert!(matches!(
        outline.slides[1].slide_type,
        SlideType::TableOfContents
    ));
    assert_eq!(outline.slides[1].sections.len(), 2);

    // Check content slide
    assert!(matches!(outline.slides[2].slide_type, SlideType::Content));
    assert_eq!(outline.slides[2].bullets.len(), 2);
}

#[test]
fn test_slide_type_serialization() {
    // Test Title
    let title_json = json!({"type": "title", "title": "Title"});
    let title_def: SlideDef = serde_json::from_value(title_json).unwrap();
    assert!(matches!(title_def.slide_type, SlideType::Title));

    // Test TableOfContents
    let toc_json = json!({"type": "table_of_contents", "title": "TOC"});
    let toc_def: SlideDef = serde_json::from_value(toc_json).unwrap();
    assert!(matches!(toc_def.slide_type, SlideType::TableOfContents));

    // Test Content
    let content_json = json!({"type": "content", "title": "Content"});
    let content_def: SlideDef = serde_json::from_value(content_json).unwrap();
    assert!(matches!(content_def.slide_type, SlideType::Content));

    // Test SectionHeader
    let section_json = json!({"type": "section_header", "title": "Section"});
    let section_def: SlideDef = serde_json::from_value(section_json).unwrap();
    assert!(matches!(section_def.slide_type, SlideType::SectionHeader));
}

#[test]
fn test_slide_def_default_values() {
    let json_str = r#"{"type": "content", "title": "Test"}"#;

    let slide_def: SlideDef = serde_json::from_str(json_str).unwrap();

    assert_eq!(slide_def.title, "Test");
    assert_eq!(slide_def.subtitle, None);
    assert!(slide_def.bullets.is_empty());
    assert!(slide_def.sections.is_empty());
}

#[test]
fn test_outline_with_section_header() {
    let json_str = r#"{
        "title": "Quarterly Review",
        "slides": [
            {"type": "title", "title": "Q4 2025"},
            {"type": "section_header", "title": "Financial Performance"},
            {"type": "content", "title": "Revenue", "bullets": ["$10M", "+15% YoY"]}
        ]
    }"#;

    let outline: Outline = serde_json::from_str(json_str).unwrap();

    assert_eq!(outline.title, "Quarterly Review");
    assert_eq!(outline.slides.len(), 3);

    assert!(matches!(
        outline.slides[1].slide_type,
        SlideType::SectionHeader
    ));
}
