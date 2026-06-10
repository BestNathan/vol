//! Integration test: parse YAML and build agent with a mock LLM.

use vol_llm_provider::LLMProviderRegistry;

#[tokio::test]
async fn test_parse_yaml_file() {
    let yaml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_agent.yaml");
    assert!(yaml_path.exists(), "Test YAML file should exist");

    let builder = vol_llm_yaml_agent::YamlAgentBuilder::from_file(&yaml_path).unwrap();
    assert_eq!(builder.config().name, "test-agent");
    assert_eq!(builder.config().tools.len(), 6);
    assert_eq!(builder.config().plugins.as_ref().unwrap().len(), 1);
    assert_eq!(builder.config().max_iterations, 5);
}

#[tokio::test]
async fn test_build_fails_without_llm_registry() {
    let yaml = r#"
name: test
llm: non-existent-provider
tools: [read]
"#;
    let builder = vol_llm_yaml_agent::YamlAgentBuilder::from_yaml(yaml).unwrap();
    let empty_registry = LLMProviderRegistry::new();
    let builder = builder.with_llm_registry(empty_registry);
    let result = builder.build();
    assert!(result.is_err());
    let err = match result {
        Ok(_) => panic!("Expected error"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("non-existent-provider"));
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN and real LLM"]
async fn test_full_agent_run() {
    // In a real test, set up an LLM provider registry with a real provider
    // and call build() + run(). Left as ignored for manual verification.
}
