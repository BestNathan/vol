//! JSON-RPC agent service over WebSocket.
//!
//! Exposes agent operations via JSON-RPC 2.0 over WebSocket:
//! - `agent.submit` — submit input, get req_id
//! - `agent.cancel` — cancel a running agent
//! - `agent.approve` — approve/reject tool call
//! - `file.list` / `file.read` — filesystem access
//! - `log.list` / `log.read` — log access
//! - `session.list` / `session.resume` — session management
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example jsonrpc_agent_service -p vol-llm-agent-channel
//! ```
//!
//! Connect with any JSON-RPC WebSocket client to `ws://localhost:3001`.

use std::sync::Arc;

use jsonrpsee::server::{ServerBuilder, RpcModule};
use tokio::net::TcpListener;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, ReActAgent};
use vol_llm_agent_channel::jsonrpc::handler::{JsonRpcHandler, JsonRpcContext};
use vol_llm_agent_channel::AgentDispatcher;
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create LLM provider from env
    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "claude-sonnet-4-6",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    info!(model = "claude-sonnet-4-6", "LLM provider created");

    // Build agent
    let def = AgentDef::new(
        "general-assistant",
        "You are a helpful AI assistant. Answer questions concisely.",
    )
    .with_type("general-assistant");

    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let mut config = AgentConfig::new(Arc::from(llm), tools, session);
    config.def = Some(def);
    let agent = ReActAgent::new(config);

    let dispatcher = Arc::new(AgentDispatcher::new(agent));

    // Create JSON-RPC handler context
    let ctx = JsonRpcContext::new(
        dispatcher,
        ".".to_string(),
        "/tmp/vol-llm-store".to_string(),
    );
    let handler = Arc::new(JsonRpcHandler::new(ctx));

    // Build RPC module with methods
    let mut module = RpcModule::from_arc(handler);

    module
        .register_async_method("agent.submit", |params, handler, _| async move {
            let params: vol_llm_agent_channel::jsonrpc::handler::SubmitParams =
                params.parse().map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                        e.to_string(),
                        None::<()>,
                    )
                })?;
            let resp = handler.agent_submit(params).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("agent.cancel", |params, handler, _| async move {
            let params: vol_llm_agent_channel::jsonrpc::handler::CancelParams =
                params.parse().map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                        e.to_string(),
                        None::<()>,
                    )
                })?;
            let resp = handler.agent_cancel(params).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("agent.approve", |params, handler, _| async move {
            let params: vol_llm_agent_channel::jsonrpc::handler::ApproveParams =
                params.parse().map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                        e.to_string(),
                        None::<()>,
                    )
                })?;
            let resp = handler.agent_approve(params).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("file.list", |params, handler, _| async move {
            let params: vol_llm_agent_channel::jsonrpc::handler::FileListParams =
                params.parse().map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                        e.to_string(),
                        None::<()>,
                    )
                })?;
            let resp = handler.file_list(params).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("file.read", |params, handler, _| async move {
            let params: vol_llm_agent_channel::jsonrpc::handler::FileReadParams =
                params.parse().map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                        e.to_string(),
                        None::<()>,
                    )
                })?;
            let resp = handler.file_read(params).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("log.list", |params, handler, _| async move {
            let resp = handler.log_list(params.parse()?).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("log.read", |params, handler, _| async move {
            let params: vol_llm_agent_channel::jsonrpc::handler::LogReadParams =
                params.parse().map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                        e.to_string(),
                        None::<()>,
                    )
                })?;
            let resp = handler.log_read(params).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("session.list", |params, handler, _| async move {
            let resp = handler.session_list(params.parse()?).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    module
        .register_async_method("session.resume", |params, handler, _| async move {
            let params: vol_llm_agent_channel::jsonrpc::handler::SessionResumeParams =
                params.parse().map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                        e.to_string(),
                        None::<()>,
                    )
                })?;
            let resp = handler.session_resume(params).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e,
                    None::<()>,
                )
            })?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!(resp))
        })
        .unwrap();

    // Build JSON-RPC server
    let server = ServerBuilder::default()
        .build("0.0.0.0:3001")
        .await
        .expect("failed to start JSON-RPC server");

    let handle = server.start(module);

    info!("JSON-RPC server started on ws://localhost:3001");
    info!("  Methods: agent.submit, agent.cancel, agent.approve");
    info!("           file.list, file.read");
    info!("           log.list, log.read");
    info!("           session.list, session.resume");

    // Wait for stop signal
    tokio::signal::ctrl_c().await.expect("ctrl-c failed");
    handle.stop().unwrap();
    info!("Shutting down");
}
