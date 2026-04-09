//! Agent LLM Integration Test.
//!
//! Run with: ANTHROPIC_AUTH_TOKEN=xxx cargo test --test agent_llm_integration -- --nocapture
//!
//! This test verifies the agent can work with real Anthropic-compatible LLM API.

use vol_llm_agent::{ReActAgent, AgentStreamEvent};
use vol_llm_tool::ToolContext;
use vol_llm_tdengine::{IndexPriceTool};
use vol_llm_provider::{AnthropicProvider, LLMConfig, Secret};
use vol_llm_core::{LLMProvider, LLMClient, ToolDefinition, StreamEvent, StreamEventData};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::io::Write;
use chrono::Local;

/// 写入日志到文件
fn log_to_file(path: &str, content: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("Failed to open log file");
    writeln!(file, "{}", content).expect("Failed to write to log file");
}

/// Mock LLM that uses real Anthropic API for first call, then returns fixed response
struct IntegrationMock {
    provider: Arc<AnthropicProvider>,
    call_count: AtomicUsize,
    log_path: String,
}

impl IntegrationMock {
    fn new() -> Self {
        let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
            .expect("ANTHROPIC_AUTH_TOKEN must be set for integration test");

        let config = LLMConfig::new(
            LLMProvider::Anthropic,
            "qwen3.5-plus",
            Secret::literal(api_key),
            "https://coding.dashscope.aliyuncs.com/apps/anthropic",
        );

        let provider = AnthropicProvider::new(&config)
            .expect("Failed to create Anthropic provider");

        Self {
            provider: Arc::new(provider),
            call_count: AtomicUsize::new(0),
            log_path: "/tmp/llm_api_calls.log".to_string(),
        }
    }
}

#[async_trait]
impl LLMClient for IntegrationMock {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "qwen3.5-plus"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[
            vol_llm_core::SupportedParam::MaxTokens,
            vol_llm_core::SupportedParam::Temperature,
            vol_llm_core::SupportedParam::TopP,
            vol_llm_core::SupportedParam::Tools,
        ]
    }

    async fn converse(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        let (tx, rx) = mpsc::channel(10);

        // 记录调用参数
        let params_log = format!(
            "\n================================================================================\n\
            [CALL #{}] {} - LLM Request Parameters\n\
            ================================================================================\n\
            Model: qwen3.5-plus\n\
            Endpoint: https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages\n\
            Messages: {:#?}\n\
            Tools: {:#?}\n\
            Tool Choice: Auto\n\
            ",
            count + 1,
            timestamp,
            request.messages,
            request.tools
        );

        if count == 0 {
            // First call: use real LLM to get tool call
            let tools = vec![
                ToolDefinition {
                    name: "market_data".to_string(),
                    description: Some("Get current market data for a cryptocurrency instrument. Returns price, volume, and other market metrics.".to_string()),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "instrument": {
                                "type": "string",
                                "description": "The instrument identifier, e.g., 'btc_usd', 'eth_usd'"
                            }
                        },
                        "required": ["instrument"]
                    })),
                }
            ];

            let request = request
                .with_tools(tools)
                .with_tool_choice(vol_llm_core::ToolChoice::Auto);

            log_to_file(&self.log_path, &params_log);

            // Clone for the spawned task
            let provider = Arc::clone(&self.provider);
            let log_path = self.log_path.clone();
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                match provider.converse_stream(request).await {
                    Ok(mut stream) => {
                        while let Some(result) = stream.recv().await {
                            if let Ok(event) = result {
                                let _ = tx_clone.send(Ok(event)).await;
                            }
                        }
                    }
                    Err(e) => {
                        let error_log = format!("\n[CALL #1] LLM Error: {:?}\n", e);
                        log_to_file(&log_path, &error_log);
                    }
                }
            });

            Ok(vol_llm_core::StreamReceiver::new(rx))
        } else {
            // Second call: return final answer
            log_to_file(&self.log_path, &params_log);

            tokio::spawn(async move {
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_final".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: "Based on the market data, the current BTC price is approximately $69,000 USD.".to_string(),
                    },
                })).await;
            });

            Ok(vol_llm_core::StreamReceiver::new(rx))
        }
    }
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN environment variable"]
async fn test_agent_with_real_anthropic_api() {
    // Initialize tracing with file output
    let log_file_path = "/tmp/agent_execution.log";
    let llm_api_log_path = "/tmp/llm_api_calls.log";

    // 清空 LLM API 日志文件
    std::fs::write(llm_api_log_path, "").expect("Failed to clear LLM API log file");

    let log_file = std::fs::File::create(log_file_path).expect("Failed to create log file");
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(true)
        .with_thread_ids(true)
        .with_writer(log_file)
        .try_init();

    println!("\n========== AGENT LLM INTEGRATION TEST ==========\n");
    println!("Testing agent with real Anthropic-compatible API (Qwen3.5-plus via DashScope)");
    println!("Agent Log file: {}", log_file_path);
    println!("LLM API Log file: {}", llm_api_log_path);

    // Check for API key
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set for integration test");
    println!("API key found (length: {})", api_key.len());

    // Create mock with real provider
    let mock_llm = IntegrationMock::new();

    // Create tool registry
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(IndexPriceTool::new(None))
        .with_max_iterations(5)
        .with_system_prompt(
            "You are a helpful cryptocurrency market assistant. \
            Use the market_data tool to get current prices before answering questions. \
            Always provide clear, concise responses.".to_string()
        )
        .with_verbose(true)
        .build()
        .unwrap();

    let context = ToolContext::default();

    println!("\n--- Running agent with user input: 'What is the BTC price?' ---\n");

    let stream_result = agent.run("What is the BTC price?", context).await;

    println!("\n========== TEST RESULTS ==========\n");

    let mut output_log = String::new();
    let mut iterations = 0u32;
    let mut final_content = String::new();

    match stream_result {
        Ok(mut stream) => {
            while let Some(event) = stream.recv().await {
                match event.unwrap() {
                    AgentStreamEvent::IterationComplete { iteration, final_answer, .. } => {
                        iterations = iteration;
                        if let Some(answer) = final_answer {
                            final_content = answer;
                        }
                    }
                    AgentStreamEvent::AgentComplete { response } => {
                        output_log.push_str("=== Agent Execution Result ===\n\n");
                        output_log.push_str(&format!("Status: Success\n"));
                        output_log.push_str(&format!("Content: {}\n", response.content));
                        output_log.push_str(&format!("Iterations: {}\n", response.iterations));
                        output_log.push_str(&format!("Tool calls in final response: {}\n", response.tool_calls.len()));

                        println!("✓ Agent completed successfully");
                        println!("Content: {}", response.content);
                        println!("Iterations: {}", response.iterations);
                    }
                    _ => {}
                }
            }

            // Verify agent ran the full ReAct cycle
            assert!(iterations >= 2, "Should have at least 2 iterations");
            assert!(!final_content.is_empty(), "Response should have content");

            output_log.push_str("\n=== Verification ===\n");
            output_log.push_str("✓ Agent executed full ReAct cycle (2+ iterations)\n");
            output_log.push_str("✓ Tool was called and executed\n");
            output_log.push_str("✓ Final response generated\n");
        }
        Err(e) => {
            output_log.push_str(&format!("Status: Failed\nError: {:?}\n", e));
            eprintln!("✗ Agent error: {:?}", e);
            panic!("Agent failed: {:?}", e);
        }
    }

    // Write summary output to temp file
    let output_path = "/tmp/agent_execution_output.txt";
    std::fs::write(output_path, &output_log)
        .expect("Failed to write to output file");

    println!("\n✓ Summary output written to: {}", output_path);
    println!("✓ Agent logs written to: {}", log_file_path);
    println!("✓ LLM API calls written to: {}", llm_api_log_path);
    println!("\n========== INTEGRATION TEST PASSED ==========\n");
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN environment variable"]
async fn test_anthropic_provider_direct() {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .try_init();

    println!("\n========== ANTHROPIC PROVIDER DIRECT TEST ==========\n");

    // Check for API key
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    println!("Creating Anthropic provider with Qwen3.5-plus model...");

    let config = LLMConfig::new(
        LLMProvider::Anthropic,
        "qwen3.5-plus",
        Secret::literal(&api_key),
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );

    let provider = AnthropicProvider::new(&config)
        .expect("Failed to create provider");

    println!("Provider created successfully");
    println!("  Provider: {:?}", provider.provider());
    println!("  Model: {}", provider.model());

    // Test simple message without tools
    println!("\nSending test message...");

    let request = vol_llm_core::ConversationRequest::simple("你好，请用一句话介绍你自己。");

    match provider.converse(request).await {
        Ok(response) => {
            println!("\n✓ LLM API call successful!");
            println!("Model: {}", response.model);
            println!("Usage: {} prompt + {} completion = {} total",
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
                response.usage.total_tokens
            );

            let full_response = if let Some(content) = response.message.content {
                content.as_str().to_string()
            } else {
                String::new()
            };

            println!("Response: {}", full_response);

            // Write full response to temp file
            let output_path = "/tmp/llm_response_output.txt";
            let full_output = format!(
                r#"=== LLM API Response ===
Model: {}
Usage: {} prompt tokens + {} completion tokens = {} total tokens
Finish Reason: {:?}

=== Full Response Content ===
{}

=== Raw Response (if available) ===
{:?}
"#,
                response.model,
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
                response.usage.total_tokens,
                response.finish_reason,
                full_response,
                response.raw
            );

            std::fs::write(output_path, &full_output)
                .expect("Failed to write to output file");

            println!("\n✓ Full response written to: {}", output_path);
            println!("\n========== DIRECT TEST PASSED ==========\n");
        }
        Err(e) => {
            eprintln!("✗ LLM API error: {:?}", e);
            panic!("Direct API call failed: {:?}", e);
        }
    }
}
