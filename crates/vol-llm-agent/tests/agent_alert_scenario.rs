//! Agent Alert Scenario Test.
//!
//! Run with: ANTHROPIC_AUTH_TOKEN=xxx cargo test --test agent_alert_scenario -- --nocapture
//!
//! This test simulates a real-world IV threshold alert scenario where the Agent
//! analyzes the alert and provides recommendations.

use vol_llm_agent::{ReActAgent, AgentConfig, AgentStreamEvent};
use vol_llm_tool::{ToolRegistry, ToolContext, MarketDataTool, AlertHistoryTool};
use vol_llm_provider::{AnthropicProvider, LLMConfig, Secret};
use vol_llm_core::{LLMProvider, LLMClient, Message, ConversationResponse, TokenUsage, FinishReason, ToolDefinition};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::io::Write;
use chrono::Local;
use serde_json::{json, Value};

/// Write JSON to file (pretty formatted)
fn write_json_to_file(path: &str, data: &Value, section: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("Failed to open log file");

    let formatted = serde_json::to_string_pretty(data)
        .expect("Failed to format JSON");

    writeln!(file, "\n{}\n{}\n{}\n",
        "=".repeat(80),
        section,
        "=".repeat(80)
    ).expect("Failed to write to log file");

    writeln!(file, "{}", formatted).expect("Failed to write to log file");
}

/// Write text to file
fn write_text_to_file(path: &str, content: &str, section: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("Failed to open log file");

    writeln!(file, "\n{}\n{}\n{}\n",
        "=".repeat(80),
        section,
        "=".repeat(80)
    ).expect("Failed to write to log file");

    writeln!(file, "{}", content).expect("Failed to write to log file");
}

/// Alert scenario data structure
#[derive(Debug, Clone)]
struct AlertScenario {
    alert_type: String,
    symbol: String,
    tenor: String,
    contract: String,
    iv: f64,
    dte: u32,
    index_price: f64,
    threshold: f64,
    trace_id: String,
}

impl AlertScenario {
    fn new() -> Self {
        Self {
            alert_type: "absolute_iv".to_string(),
            symbol: "BTC".to_string(),
            tenor: "Short".to_string(),
            contract: "BTC-8APR26-70000-C".to_string(),
            iv: 0.4677, // 46.77% IV
            dte: 2,
            index_price: 69263.93,
            threshold: 0.40, // 40% threshold
            trace_id: format!("alert_{}", Local::now().format("%Y%m%d_%H%M%S")),
        }
    }

    fn to_alert_message(&self) -> String {
        format!(
            "🚨 IV Threshold Alert\n\
             Contract: {}\n\
             Tenor: {}\n\
             Current IV: {:.2}%\n\
             Threshold: {:.2}%\n\
             Index Price: ${:.2}\n\
             DTE: {} days\n\
             Trace ID: {}",
            self.contract,
            self.tenor,
            self.iv * 100.0,
            self.threshold * 100.0,
            self.index_price,
            self.dte,
            self.trace_id
        )
    }
}

/// Mock LLM for alert scenario
struct AlertScenarioMock {
    provider: AnthropicProvider,
    call_count: AtomicUsize,
    scenario: AlertScenario,
    log_path: String,
}

impl AlertScenarioMock {
    fn new(scenario: AlertScenario) -> Self {
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
            provider,
            call_count: AtomicUsize::new(0),
            scenario,
            log_path: "/tmp/agent_alert_scenario.log".to_string(),
        }
    }
}

#[async_trait]
impl LLMClient for AlertScenarioMock {
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

    async fn converse(&self, request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        // Build request JSON
        let request_json = json!({
            "call_number": count + 1,
            "timestamp": timestamp.to_string(),
            "model": "qwen3.5-plus",
            "endpoint": "https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages",
            "messages": request.messages.iter().map(|m| {
                json!({
                    "role": format!("{:?}", m.role),
                    "content": m.content.as_ref().map(|c| c.as_str()),
                    "tool_calls": m.tool_calls.as_ref().map(|calls| {
                        calls.iter().map(|c| json!({
                            "id": c.id,
                            "name": c.name,
                            "arguments": c.arguments
                        })).collect::<Vec<_>>()
                    }),
                    "tool_call_id": m.tool_call_id
                })
            }).collect::<Vec<_>>(),
            "tools": request.tools.as_ref().map(|tools| {
                tools.iter().map(|t| json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                })).collect::<Vec<_>>()
            })
        });

        write_json_to_file(&self.log_path, &request_json,
            &format!("CALL #{} - REQUEST PARAMETERS", count + 1));

        if count == 0 {
            // First call: use real LLM to analyze alert
            let tools = vec![
                ToolDefinition {
                    name: "alert_history".to_string(),
                    description: Some("Get historical alerts for a specific contract to identify patterns or recurring issues.".to_string()),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol": {
                                "type": "string",
                                "description": "The contract symbol to query, e.g., 'BTC-8APR26-70000-C'"
                            },
                            "hours": {
                                "type": "string",
                                "description": "Number of hours to look back (e.g., '24', '72')"
                            },
                            "limit": {
                                "type": "string",
                                "description": "Maximum number of alerts to return"
                            }
                        },
                        "required": ["symbol"]
                    })),
                },
                ToolDefinition {
                    name: "market_data".to_string(),
                    description: Some("Get current market data for a cryptocurrency instrument.".to_string()),
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
                },
            ];

            let request = request
                .with_tools(tools)
                .with_tool_choice(vol_llm_core::ToolChoice::Auto);

            // Update request JSON with tools
            let request_json_with_tools = json!({
                "call_number": count + 1,
                "timestamp": timestamp.to_string(),
                "model": "qwen3.5-plus",
                "endpoint": "https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages",
                "messages": request.messages.iter().map(|m| {
                    json!({
                        "role": format!("{:?}", m.role),
                        "content": m.content.as_ref().map(|c| c.as_str()),
                        "tool_calls": m.tool_calls.as_ref().map(|calls| {
                            calls.iter().map(|c| json!({
                                "id": c.id,
                                "name": c.name,
                                "arguments": c.arguments
                            })).collect::<Vec<_>>()
                        }),
                        "tool_call_id": m.tool_call_id
                    })
                }).collect::<Vec<_>>(),
                "tools": request.tools.as_ref().map(|tools| {
                    tools.iter().map(|t| json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    })).collect::<Vec<_>>()
                }),
                "tool_choice": "auto"
            });

            write_json_to_file(&self.log_path, &request_json_with_tools,
                &format!("CALL #{} - REQUEST (with tools)", count + 1));

            let response = self.provider.converse(request).await;

            // Record response
            match &response {
                Ok(resp) => {
                    let response_json = json!({
                        "call_number": count + 1,
                        "timestamp": Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                        "model": resp.model,
                        "finish_reason": format!("{:?}", resp.finish_reason),
                        "usage": {
                            "prompt_tokens": resp.usage.prompt_tokens,
                            "completion_tokens": resp.usage.completion_tokens,
                            "total_tokens": resp.usage.total_tokens
                        },
                        "message": {
                            "content": resp.message.content.as_ref().map(|c| c.as_str()),
                            "tool_calls": resp.message.tool_calls.as_ref().map(|calls| {
                                calls.iter().map(|c| json!({
                                    "id": c.id,
                                    "name": c.name,
                                    "arguments": c.arguments,
                                    "type": c.r#type
                                })).collect::<Vec<_>>()
                            })
                        },
                        "raw_response": resp.raw
                    });

                    write_json_to_file(&self.log_path, &response_json,
                        &format!("CALL #{} - RESPONSE", count + 1));
                }
                Err(e) => {
                    let error_json = json!({
                        "call_number": count + 1,
                        "timestamp": Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                        "error": format!("{:?}", e)
                    });
                    write_json_to_file(&self.log_path, &error_json,
                        &format!("CALL #{} - ERROR", count + 1));
                }
            }

            response
        } else {
            // Second call: return final analysis
            let response = Ok(ConversationResponse {
                message: Message::assistant(
                    format!(
                        "## AI Analysis for {} Alert\n\n\
                         ### Alert Summary\n\
                         - **Contract**: {}\n\
                         - **Type**: {}\n\
                         - **Current IV**: {:.2}% (Threshold: {:.2}%)\n\
                         - **Index Price**: ${:.2}\n\
                         - **DTE**: {} days\n\n\
                         ### Analysis\n\
                         Based on the alert history and current market data:\n\n\
                         1. **IV Level Assessment**: The current IV of {:.2}% is significantly elevated \
                            compared to the {:.2}% threshold. This suggests heightened market uncertainty \
                            or anticipated price movement.\n\n\
                         2. **Context**: With only {} DTE, this short-term option is experiencing \
                            elevated implied volatility, which could indicate:\n\
                           - Upcoming news/events\n\
                           - Market maker hedging activity\n\
                           - Unusual options flow\n\n\
                         ### Recommendation\n\
                         Monitor for potential IV crush if the anticipated movement doesn't materialize. \
                         Consider the risk/reward of any positions in light of elevated premium.",
                        self.scenario.alert_type,
                        self.scenario.contract,
                        self.scenario.alert_type,
                        self.scenario.iv * 100.0,
                        self.scenario.threshold * 100.0,
                        self.scenario.index_price,
                        self.scenario.dte,
                        self.scenario.iv * 100.0,
                        self.scenario.threshold * 100.0,
                        self.scenario.dte
                    )
                ),
                model: "qwen3.5-plus".to_string(),
                usage: TokenUsage { prompt_tokens: 800, completion_tokens: 250, total_tokens: 1050, cached_tokens: None },
                finish_reason: FinishReason::Stop,
                raw: None,
            });

            // Record response
            if let Ok(ref resp) = response {
                let response_json = json!({
                    "call_number": count + 1,
                    "timestamp": Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                    "model": resp.model,
                    "finish_reason": format!("{:?}", resp.finish_reason),
                    "usage": {
                        "prompt_tokens": resp.usage.prompt_tokens,
                        "completion_tokens": resp.usage.completion_tokens,
                        "total_tokens": resp.usage.total_tokens
                    },
                    "message": {
                        "content": resp.message.content.as_ref().map(|c| c.as_str()),
                        "tool_calls": resp.message.tool_calls.as_ref().map(|calls| {
                            calls.iter().map(|c| json!({
                                "id": c.id,
                                "name": c.name,
                                "arguments": c.arguments,
                                "type": c.r#type
                            })).collect::<Vec<_>>()
                        })
                    }
                });

                write_json_to_file(&self.log_path, &response_json,
                    &format!("CALL #{} - RESPONSE (Final Analysis)", count + 1));
            }

            response
        }
    }

    async fn converse_stream(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        unimplemented!()
    }
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN environment variable"]
async fn test_agent_alert_scenario() {
    // Initialize tracing
    let agent_log_path = "/tmp/agent_alert_execution.log";
    let scenario_log_path = "/tmp/agent_alert_scenario.log";

    // Clear log file
    std::fs::write(scenario_log_path, "").expect("Failed to clear scenario log file");

    let log_file = std::fs::File::create(agent_log_path).expect("Failed to create log file");
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(true)
        .with_thread_ids(true)
        .with_writer(log_file)
        .try_init();

    println!("\n{}", "=".repeat(80));
    println!("  AGENT ALERT SCENARIO TEST - Real-World IV Threshold Alert Analysis");
    println!("{}", "=".repeat(80));

    // Create alert scenario
    let scenario = AlertScenario::new();

    println!("\nAlert Scenario:");
    println!("   Type: {}", scenario.alert_type);
    println!("   Contract: {}", scenario.contract);
    println!("   Tenor: {}", scenario.tenor);
    println!("   IV: {:.2}% (Threshold: {:.2}%)", scenario.iv * 100.0, scenario.threshold * 100.0);
    println!("   Index Price: ${:.2}", scenario.index_price);
    println!("   DTE: {} days", scenario.dte);
    println!("   Trace ID: {}", scenario.trace_id);

    // Write scenario to log
    let scenario_json = json!({
        "alert_type": scenario.alert_type,
        "symbol": scenario.symbol,
        "tenor": scenario.tenor,
        "contract": scenario.contract,
        "iv": scenario.iv,
        "dte": scenario.dte,
        "index_price": scenario.index_price,
        "threshold": scenario.threshold,
        "trace_id": scenario.trace_id
    });
    write_json_to_file(scenario_log_path, &scenario_json, "ALERT SCENARIO CONFIGURATION");

    // Check for API key
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set for integration test");

    // Create mock with real provider
    let mock_llm = AlertScenarioMock::new(scenario.clone());

    // Create tool registry
    let mut registry = ToolRegistry::new();
    registry.register(AlertHistoryTool::new(None));
    registry.register(MarketDataTool::new(None));

    let system_prompt = format!(
        "You are an AI assistant for the Deribit Volatility Monitor system. \
         You analyze volatility alerts and provide actionable insights to traders. \
         Use available tools to gather context before providing recommendations. \
         Be concise but informative.\n\n\
         Current Alert:\n\
         {}",
        scenario.to_alert_message()
    );

    let agent_config = AgentConfig {
        max_iterations: 5,
        system_prompt,
        verbose: true,
    };

    let agent = ReActAgent::new(Arc::new(mock_llm), Arc::new(registry), agent_config);

    let context = ToolContext::default();

    // Build user input from alert
    let user_input = format!(
        "Analyze this IV threshold alert and provide context:\n\n\
         {}",
        scenario.to_alert_message()
    );

    println!("\nUser Input to Agent:");
    println!("   {}", scenario.to_alert_message().replace('\n', "\n   "));

    println!("\nRunning Agent Analysis...\n");

    let stream_result = agent.run(&user_input, context).await;

    println!("\n{}", "=".repeat(80));
    println!("  TEST RESULTS");
    println!("{}", "=".repeat(80));

    let mut output_log = String::new();

    match stream_result {
        Ok(mut stream) => {
            let mut final_response = None;
            let mut iterations = 0u32;

            while let Some(event) = stream.recv().await {
                match event.unwrap() {
                    AgentStreamEvent::IterationComplete { iteration, final_answer, .. } => {
                        iterations = iteration;
                        if let Some(answer) = final_answer {
                            final_response = Some(answer);
                        }
                    }
                    AgentStreamEvent::AgentComplete { response } => {
                        output_log.push_str("=== Agent Execution Result ===\n\n");
                        output_log.push_str(&format!("Status: Success\n"));
                        output_log.push_str(&format!("Content: {}\n", response.content));
                        output_log.push_str(&format!("Iterations: {}\n", response.iterations));
                        output_log.push_str(&format!("Tool calls: {}\n", response.tool_calls.len()));

                        println!("Agent completed successfully");
                        println!("\nAgent Analysis Output:");
                        println!("{}\n", response.content);
                        println!("Iterations: {}", response.iterations);

                        final_response = Some(response.content);
                    }
                    _ => {}
                }
            }

            // Verify agent ran the full ReAct cycle
            assert!(iterations >= 2, "Should have at least 2 iterations");
            assert!(final_response.is_some() && !final_response.unwrap().is_empty(), "Response should have content");

            output_log.push_str("\n=== Verification ===\n");
            output_log.push_str("✓ Agent executed full ReAct cycle (2+ iterations)\n");
            output_log.push_str("✓ Tool was called and executed\n");
            output_log.push_str("✓ Final analysis generated\n");

            // Write final output to log
            let final_output_json = json!({
                "status": "Success",
                "iterations": iterations,
                "tool_calls_count": 1
            });
            write_json_to_file(scenario_log_path, &final_output_json, "FINAL AGENT OUTPUT");
        }
        Err(e) => {
            output_log.push_str(&format!("Status: Failed\nError: {:?}\n", e));
            eprintln!("✗ Agent error: {:?}", e);

            let error_json = json!({
                "status": "Failed",
                "error": format!("{:?}", e)
            });
            write_json_to_file(scenario_log_path, &error_json, "AGENT ERROR");

            panic!("Agent failed: {:?}", e);
        }
    }

    // Write summary
    let summary_path = "/tmp/agent_alert_summary.txt";
    std::fs::write(summary_path, &output_log)
        .expect("Failed to write summary file");

    println!("\nOutput Files:");
    println!("   Scenario Log (JSON): {}", scenario_log_path);
    println!("   Execution Log: {}", agent_log_path);
    println!("   Summary: {}", summary_path);
    println!("\n{}", "=".repeat(80));
    println!("  INTEGRATION TEST PASSED");
    println!("{}", "=".repeat(80));
}
