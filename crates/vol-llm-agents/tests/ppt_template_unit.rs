//! PPT 模板单元测试。

use vol_llm_agents::ppt::template::{PptTemplate, TemplateLayout, LayoutType, Position};

#[test]
fn test_template_layout_loading() {
    // Test loading a template with layouts
    let yaml_content = r##"
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
"##;

    let template: PptTemplate = serde_yaml::from_str(yaml_content).unwrap();

    assert_eq!(template.id, "test_template");
    assert_eq!(template.layouts.len(), 1);
    assert!(matches!(template.layouts[0].layout_type, LayoutType::Title));
}

#[test]
fn test_position_resolution() {
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

#[test]
fn test_color_scheme_parsing() {
    let yaml_content = r##"
id: color_test
name: Color Test
description: Test color scheme
tags:
  occasion: ["business"]
  style: ["professional"]
  audience: ["executives"]
color_scheme:
  primary: "#1E88E5"
  secondary: "#43A047"
  accent: "#FDD835"
  background: "#FAFAFA"
  text_primary: "#212121"
  text_secondary: "#757575"
typography:
  title_font: "Helvetica"
  body_font: "Georgia"
layouts: []
"##;

    let template: PptTemplate = serde_yaml::from_str(yaml_content).unwrap();

    assert_eq!(template.color_scheme.primary, "#1E88E5");
    assert_eq!(template.color_scheme.secondary, "#43A047");
    assert_eq!(template.color_scheme.accent, "#FDD835");
    assert_eq!(template.color_scheme.background, "#FAFAFA");
    assert_eq!(template.color_scheme.text_primary, "#212121");
    assert_eq!(template.color_scheme.text_secondary, "#757575");
}

#[test]
fn test_typography_parsing() {
    let yaml_content = r##"
id: typo_test
name: Typography Test
description: Test typography
tags:
  occasion: ["creative"]
  style: ["modern"]
  audience: ["designers"]
color_scheme:
  primary: "#000000"
  secondary: "#FFFFFF"
  accent: "#FF5722"
  background: "#EEEEEE"
  text_primary: "#212121"
  text_secondary: "#616161"
typography:
  title_font: "Montserrat"
  body_font: "Open Sans"
layouts: []
"##;

    let template: PptTemplate = serde_yaml::from_str(yaml_content).unwrap();

    assert_eq!(template.typography.title_font, "Montserrat");
    assert_eq!(template.typography.body_font, "Open Sans");
}

#[test]
fn test_multiple_layouts() {
    let yaml_content = r##"
id: multi_layout
name: Multi Layout Template
description: Multiple layouts
tags:
  occasion: ["conference"]
  style: ["formal"]
  audience: ["industry"]
color_scheme:
  primary: "#0D47A1"
  secondary: "#1976D2"
  accent: "#64B5F6"
  background: "#FFFFFF"
  text_primary: "#212121"
  text_secondary: "#616161"
typography:
  title_font: "Roboto"
  body_font: "Roboto"
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
          font_size: 32
          color: "{{primary}}"
  - layout_type: table_of_contents
    elements:
      - element_type: textbox
        placeholder: content
        position:
          x: 457200
          y: 457200
          width: 8229600
          height: 4572000
        style:
          font_size: 18
          color: "{{text_primary}}"
"##;

    let template: PptTemplate = serde_yaml::from_str(yaml_content).unwrap();

    assert_eq!(template.id, "multi_layout");
    assert_eq!(template.layouts.len(), 3);
    assert!(matches!(template.layouts[0].layout_type, LayoutType::Title));
    assert!(matches!(template.layouts[1].layout_type, LayoutType::Content));
    assert!(matches!(template.layouts[2].layout_type, LayoutType::TableOfContents));
}

#[test]
fn test_template_default_layout() {
    let default_layout = TemplateLayout::default();

    assert!(matches!(default_layout.layout_type, LayoutType::Content));
    assert_eq!(default_layout.elements.len(), 2);

    // Check title element
    assert_eq!(default_layout.elements[0].placeholder, "title");
    assert_eq!(default_layout.elements[0].style.font_size, 24);

    // Check content element
    assert_eq!(default_layout.elements[1].placeholder, "content");
    assert_eq!(default_layout.elements[1].style.font_size, 16);
    assert_eq!(default_layout.elements[1].style.bullet_style, Some("bullet".to_string()));
}
