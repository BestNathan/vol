//! PPT Agent 集成测试。
//!
//! 此测试验证 PPT Agent 的完整工作流程：
//! 1. 使用真实 LLM API 分析需求
//! 2. 生成大纲
//! 3. 扩展内容
//! 4. 匹配模板
//! 5. 渲染 PPTX 文件
//!
//! Requirements:
//! - ANTHROPIC_AUTH_TOKEN environment variable
//!
//! Run with:
//! ```bash
//! cargo test -p vol-llm-agents --test ppt_agent_integration -- --nocapture --ignored
//! ```

use std::path::PathBuf;
use vol_llm_agents::ppt::{PptAgent, PptAgentConfig, PptInput};

/// 获取项目根目录的 ppt/templates 路径
fn get_template_dir() -> PathBuf {
    // Try multiple possible locations
    let candidates = vec![
        PathBuf::from("src/ppt/templates"),
        PathBuf::from("crates/vol-llm-agents/src/ppt/templates"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ppt/templates"),
    ];

    for path in candidates {
        if path.exists() {
            return path;
        }
    }

    // Default to the first option (will fail gracefully if templates don't exist)
    PathBuf::from("src/ppt/templates")
}

#[tokio::test]
#[ignore] // Requires LLM API key (ANTHROPIC_AUTH_TOKEN)
async fn test_full_ppt_generation() {
    // Skip if API key not set
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
        eprintln!("Skipping test: ANTHROPIC_AUTH_TOKEN not set");
        return;
    }

    let template_dir = get_template_dir();

    println!("Using template directory: {template_dir:?}");

    let config = PptAgentConfig::default()
        .with_llm_provider("anthropic-main")
        .with_template_dir(&template_dir)
        .with_default_output_dir(PathBuf::from("test_output"));

    println!("Creating PPT Agent...");
    let agent = PptAgent::new(config)
        .await
        .expect("Failed to create PPT Agent");

    println!("Generating PPT for: 做一个期权周报，包含 IV 分析、RV 分析、交易建议");
    let input = PptInput::text("做一个期权周报，包含 IV 分析、RV 分析、交易建议");
    let result = agent.generate(input).await.expect("PPT generation failed");

    println!("PPT generated successfully!");
    println!("  Output path: {:?}", result.output_path);
    println!("  Slide count: {}", result.slide_count);
    println!("  Template ID: {}", result.template_id);

    // Verify output file exists
    assert!(result.output_path.exists(), "Output PPTX file should exist");
    println!("✓ Output file exists");

    // Verify minimum slide count (title + TOC + at least 1 content slide)
    assert!(
        result.slide_count >= 3,
        "Should have at least 3 slides (title, TOC, 1 content), got {}",
        result.slide_count
    );
    println!("✓ Slide count is valid: {}", result.slide_count);

    // Verify template was matched (business_formal is the default fallback)
    assert!(
        !result.template_id.is_empty(),
        "Template ID should not be empty"
    );
    println!("✓ Template matched: {}", result.template_id);

    // Verify file size is reasonable (> 1KB for a valid PPTX)
    if let Ok(metadata) = std::fs::metadata(&result.output_path) {
        assert!(
            metadata.len() > 1024,
            "PPTX file should be larger than 1KB, got {} bytes",
            metadata.len()
        );
        println!("✓ File size is valid: {} bytes", metadata.len());
    }

    // Cleanup
    let _ = std::fs::remove_file(&result.output_path);
    println!("✓ Cleanup completed");

    println!("\n=== Test passed successfully ===");
}
