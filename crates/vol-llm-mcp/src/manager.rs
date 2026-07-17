//! McpManager — manages MCP server connection lifecycles.
//!
//! Tracks per-server connection state, spawns background reconnect tasks
//! on failure, and caches discovered capabilities at connect time.

use rmcp::model::{ClientInfo, Prompt, Resource, ResourceTemplate, Tool};
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use rmcp::transport::TokioChildProcess;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing;

use crate::config::{McpServerConfig, McpTransport};
use crate::error::McpError;
use crate::session::{sanitize_name, McpToolInfo};

/// Connection state of a single MCP server.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Connected,
    Disconnected,
    Connecting,
    Error(String),
}

/// Per-server connection state.
struct ServerState {
    config: McpServerConfig,
    status: ServerStatus,
    retry_count: usize,
    running_service: Option<RunningService<RoleClient, ClientInfo>>,
    cancel_token: CancellationToken,
    cached_tools: Vec<McpToolInfo>,
    cached_resources: Vec<Resource>,
    cached_resource_templates: Vec<ResourceTemplate>,
    cached_prompts: Vec<Prompt>,
    reconnect_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ServerState {
    fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            status: ServerStatus::Disconnected,
            retry_count: 0,
            running_service: None,
            cancel_token: CancellationToken::new(),
            cached_tools: Vec::new(),
            cached_resources: Vec::new(),
            cached_resource_templates: Vec::new(),
            cached_prompts: Vec::new(),
            reconnect_handle: None,
        }
    }

    fn clear_caches(&mut self) {
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_resource_templates.clear();
        self.cached_prompts.clear();
    }
}

/// MCP connection lifecycle manager.
#[derive(Clone)]
pub struct McpManager {
    servers: Arc<RwLock<HashMap<String, ServerState>>>,
    max_retries: usize,
    backoff_min: Duration,
    backoff_max: Duration,
}

impl McpManager {
    pub fn new(configs: Vec<McpServerConfig>) -> Self {
        let servers = configs
            .into_iter()
            .map(|c| (sanitize_name(&c.name), ServerState::new(c)))
            .collect();
        Self {
            servers: Arc::new(RwLock::new(servers)),
            max_retries: 5,
            backoff_min: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
        }
    }

    pub fn with_max_retries(mut self, max: usize) -> Self {
        self.max_retries = max;
        self
    }

    pub fn with_backoff(mut self, min: Duration, max: Duration) -> Self {
        self.backoff_min = min;
        self.backoff_max = max;
        self
    }

    async fn connect_single(
        config: &McpServerConfig,
        cancel_token: &CancellationToken,
    ) -> Result<
        (
            RunningService<RoleClient, ClientInfo>,
            Vec<Tool>,
            Vec<Resource>,
            Vec<ResourceTemplate>,
            Vec<Prompt>,
        ),
        McpError,
    > {
        let service = match &config.transport {
            McpTransport::Stdio { command, args, env } => {
                connect_stdio(command, args, env, config, cancel_token).await?
            }
            McpTransport::Http { url, headers, env } => {
                connect_http(url, headers.as_ref(), env, config, cancel_token).await?
            }
        };

        let peer = service.peer();

        let tools = peer.list_all_tools().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list tools for '{}': {}", config.name, e);
            Vec::new()
        });

        let resources = peer.list_all_resources().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list resources for '{}': {}", config.name, e);
            Vec::new()
        });

        let resource_templates = peer
            .list_all_resource_templates()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to list resource templates for '{}': {}",
                    config.name,
                    e
                );
                Vec::new()
            });

        let prompts = peer.list_all_prompts().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list prompts for '{}': {}", config.name, e);
            Vec::new()
        });

        Ok((service, tools, resources, resource_templates, prompts))
    }

    pub async fn connect(&self) -> Result<(), McpError> {
        let server_names: Vec<String>;
        {
            let servers = self.servers.read().await;
            server_names = servers.keys().cloned().collect();
        }

        for name in server_names {
            self.connect_server(&name).await;
        }

        Ok(())
    }

    /// Reconnect all servers that are not currently Connected.
    /// Called before agent runs and when a tool call finds a dead connection.
    pub async fn reconnect_all(&self) {
        let to_reconnect: Vec<String> = {
            let servers = self.servers.read().await;
            servers
                .iter()
                .filter(|(_, s)| s.status != ServerStatus::Connected)
                .map(|(n, _)| n.clone())
                .collect()
        };
        for name in to_reconnect {
            tracing::info!(server = %name, "MCP reconnecting (not connected)");
            // Reset retry count so reconnect can proceed
            {
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(&name) {
                    state.retry_count = 0;
                }
            }
            self.connect_server(&name).await;
        }
    }

    async fn connect_server(&self, name: &str) {
        let config;
        let max_retries;
        let backoff_min;
        let backoff_max;
        let cancel_token = {
            let mut servers = self.servers.write().await;
            let Some(state) = servers.get_mut(name) else {
                return;
            };
            state.status = ServerStatus::Connecting;
            state.cancel_token = CancellationToken::new();
            let ct = state.cancel_token.clone();
            config = state.config.clone();
            max_retries = self.max_retries;
            backoff_min = self.backoff_min;
            backoff_max = self.backoff_max;
            ct
        };

        match Self::connect_single(&config, &cancel_token).await {
            Ok((service, tools, resources, resource_templates, prompts)) => {
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.running_service = Some(service);
                    state.cached_tools = tools
                        .iter()
                        .map(|t| McpToolInfo {
                            name: t.name.to_string(),
                            description: t
                                .description
                                .as_ref()
                                .map(std::string::ToString::to_string),
                            input_schema: Some(t.schema_as_json_value()),
                        })
                        .collect();
                    state.cached_resources = resources;
                    state.cached_resource_templates = resource_templates;
                    state.cached_prompts = prompts;
                    state.status = ServerStatus::Connected;
                    state.retry_count = 0;
                    tracing::info!(server = name, "MCP server connected");
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                tracing::error!(server = name, error = %err_msg, "MCP server connection failed");
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    // Don't retry if the binary simply doesn't exist.
                    if err_msg.contains("No such file or directory") {
                        state.status = ServerStatus::Error(err_msg);
                        tracing::warn!(
                            server = name,
                            "MCP server binary not found, skipping retries"
                        );
                        return;
                    }
                    state.retry_count += 1;
                    if state.retry_count >= max_retries {
                        state.clear_caches();
                        state.status = ServerStatus::Error(
                            "max retries exceeded, retrying with delay".to_string(),
                        );
                        tracing::error!(
                            server = name,
                            retries = state.retry_count,
                            "MCP server max retries exceeded, will retry with delay"
                        );
                        // Still spawn reconnect — it handles the delay internally
                        self.spawn_reconnect(name, max_retries, backoff_min, backoff_max);
                    } else {
                        state.status = ServerStatus::Error(err_msg);
                        self.spawn_reconnect(name, max_retries, backoff_min, backoff_max);
                    }
                }
            }
        }
    }

    fn spawn_reconnect(
        &self,
        name: &str,
        max_retries: usize,
        backoff_min: Duration,
        backoff_max: Duration,
    ) {
        let name = name.to_string();
        let servers = self.servers.clone();

        // Clone name for the handle-storing spawn
        let name_for_handle = name.clone();
        let handle = tokio::spawn(async move {
            loop {
                let (config, cancel_token, current_retry) = {
                    let mut srv = servers.write().await;
                    let Some(state) = srv.get_mut(&name) else {
                        break;
                    };
                    if state.retry_count >= max_retries {
                        state.clear_caches();
                        state.status = ServerStatus::Error("max retries exceeded".to_string());
                        break;
                    }
                    state.cancel_token.cancel();
                    state.cancel_token = CancellationToken::new();
                    let config = state.config.clone();
                    (config, state.cancel_token.clone(), state.retry_count)
                };

                let delay = exponential_backoff(current_retry, backoff_min, backoff_max);
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = cancel_token.cancelled() => break,
                }

                match tokio::time::timeout(
                    Duration::from_secs(10),
                    McpManager::connect_single(&config, &cancel_token),
                )
                .await
                {
                    Ok(Ok((service, tools, resources, resource_templates, prompts))) => {
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.running_service = Some(service);
                            state.cached_tools = tools
                                .iter()
                                .map(|t| McpToolInfo {
                                    name: t.name.to_string(),
                                    description: t
                                        .description
                                        .as_ref()
                                        .map(std::string::ToString::to_string),
                                    input_schema: Some(t.schema_as_json_value()),
                                })
                                .collect();
                            state.cached_resources = resources;
                            state.cached_resource_templates = resource_templates;
                            state.cached_prompts = prompts;
                            state.status = ServerStatus::Connected;
                            state.retry_count = 0;
                            tracing::info!(server = &name, "MCP server reconnected");
                        }
                        break;
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(server = &name, error = %e, "MCP reconnect failed");
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.retry_count += 1;
                            state.status = ServerStatus::Error(e.to_string());
                            if state.retry_count >= max_retries {
                                state.clear_caches();
                                state.status =
                                    ServerStatus::Error("max retries exceeded".to_string());
                                tracing::error!(
                                    server = &name,
                                    retries = state.retry_count,
                                    "MCP server max retries exceeded"
                                );
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        tracing::warn!(server = &name, "MCP reconnect timed out");
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.retry_count += 1;
                            state.status = ServerStatus::Error("connection timeout".to_string());
                            if state.retry_count >= max_retries {
                                state.clear_caches();
                                state.status =
                                    ServerStatus::Error("max retries exceeded".to_string());
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Store handle
        let servers_clone = self.servers.clone();
        tokio::spawn(async move {
            let mut srv = servers_clone.write().await;
            if let Some(state) = srv.get_mut(&name_for_handle) {
                state.reconnect_handle = Some(handle);
            }
        });
    }

    pub async fn list_all_tools(&self) -> Vec<(String, McpToolInfo)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state
                    .cached_tools
                    .iter()
                    .map(|tool| (server.clone(), tool.clone()))
            })
            .collect()
    }

    // ── Tool call ──────────────────────────────────────────────────────

    #[tracing::instrument(skip(self, args), fields(mcp.server = server, mcp.tool = tool_name))]
    pub async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String, McpError> {
        use rmcp::model::{CallToolRequestParams, JsonObject};

        // If server is not connected, try to reconnect first.
        {
            let servers = self.servers.read().await;
            let needs_reconnect = servers
                .get(server)
                .map(|s| s.status != ServerStatus::Connected)
                .unwrap_or(false);
            drop(servers);
            if needs_reconnect {
                tracing::info!(%server, "MCP server not connected, attempting reconnect before tool call");
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(server) {
                    state.retry_count = 0;
                }
                drop(servers);
                self.connect_server(server).await;
            }
        }

        let (peer, server_name) = {
            let servers = self.servers.read().await;
            let state = servers
                .get(server)
                .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;

            if state.status != ServerStatus::Connected {
                return Err(McpError::ServerDisconnected(server.to_string()));
            }

            let service = state
                .running_service
                .as_ref()
                .ok_or_else(|| McpError::ServerDisconnected(server.to_string()))?;

            (service.peer().clone(), state.config.name.clone())
        };

        let arguments = match args {
            serde_json::Value::Object(obj) => JsonObject::from_iter(obj),
            _ => JsonObject::new(),
        };

        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments);

        let result = peer
            .call_tool(params)
            .await
            .map_err(|e: rmcp::service::ServiceError| McpError::ToolCallFailed {
                server: server_name.clone(),
                tool: tool_name.to_string(),
                detail: e.to_string(),
            })?;

        Ok(Self::format_call_tool_result(&result))
    }

    fn format_call_tool_result(result: &rmcp::model::CallToolResult) -> String {
        use rmcp::model::{RawContent, ResourceContents};

        if result.is_error == Some(true) {
            let text = result
                .content
                .iter()
                .filter_map(|c| match &c.raw {
                    RawContent::Text(text_block) => Some(text_block.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            return format!(
                "MCP tool error: {}",
                if text.is_empty() {
                    "unknown error"
                } else {
                    &text
                }
            );
        }

        result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                RawContent::Text(text_block) => Some(text_block.text.clone()),
                RawContent::Image(_) => Some("[image content]".to_string()),
                RawContent::Resource(resource_block) => {
                    let uri = match &resource_block.resource {
                        ResourceContents::TextResourceContents { uri, .. } => uri,
                        ResourceContents::BlobResourceContents { uri, .. } => uri,
                    };
                    Some(format!("[resource: {uri}]"))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    // ── Resource protocol ──────────────────────────────────────────────

    pub async fn list_all_resources(&self) -> Vec<(String, rmcp::model::Resource)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state
                    .cached_resources
                    .iter()
                    .map(|r| (server.clone(), r.clone()))
            })
            .collect()
    }

    pub async fn list_all_resource_templates(
        &self,
    ) -> Vec<(String, rmcp::model::ResourceTemplate)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state
                    .cached_resource_templates
                    .iter()
                    .map(|t| (server.clone(), t.clone()))
            })
            .collect()
    }

    pub async fn read_resource(&self, uri: &str) -> Result<String, McpError> {
        let servers = self.servers.read().await;
        for (_server, state) in servers.iter() {
            if state.status != ServerStatus::Connected {
                continue;
            }
            if state.cached_resources.iter().any(|r| r.uri == uri)
                || state
                    .cached_resource_templates
                    .iter()
                    .any(|t| uri.starts_with(&t.uri_template))
            {
                if let Some(service) = &state.running_service {
                    return Self::read_resource_from_peer(service.peer(), &state.config.name, uri)
                        .await;
                }
            }
        }
        Err(McpError::ServerNotFound(uri.to_string()))
    }

    async fn read_resource_from_peer(
        peer: &rmcp::service::Peer<rmcp::service::RoleClient>,
        server_name: &str,
        uri: &str,
    ) -> Result<String, McpError> {
        use rmcp::model::{ReadResourceRequestParams, ResourceContents};

        let params = ReadResourceRequestParams::new(uri);
        let result =
            peer.read_resource(params)
                .await
                .map_err(
                    |e: rmcp::service::ServiceError| McpError::ResourceReadFailed {
                        server: server_name.to_string(),
                        uri: uri.to_string(),
                        detail: e.to_string(),
                    },
                )?;

        #[allow(clippy::unnecessary_filter_map)]
        let texts: Vec<String> = result
            .contents
            .into_iter()
            .filter_map(|c| match c {
                ResourceContents::TextResourceContents { text, .. } => Some(text),
                ResourceContents::BlobResourceContents {
                    blob, mime_type, ..
                } => Some(format!(
                    "[blob content, {} bytes, mime: {}]",
                    blob.len(),
                    mime_type.as_deref().unwrap_or("unknown")
                )),
            })
            .collect();

        Ok(texts.join("\n"))
    }

    // ── Prompt protocol ────────────────────────────────────────────────

    pub async fn list_all_prompts(&self) -> Vec<(String, rmcp::model::Prompt)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state
                    .cached_prompts
                    .iter()
                    .map(|p| (server.clone(), p.clone()))
            })
            .collect()
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        args: Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<(Option<String>, Vec<rmcp::model::PromptMessage>), McpError> {
        let servers = self.servers.read().await;
        for (_server, state) in servers.iter() {
            if state.status != ServerStatus::Connected {
                continue;
            }
            if state.cached_prompts.iter().any(|p| p.name == name) {
                if let Some(service) = &state.running_service {
                    return Self::get_prompt_from_peer(
                        service.peer(),
                        &state.config.name,
                        name,
                        args,
                    )
                    .await;
                }
            }
        }
        Err(McpError::ServerNotFound(name.to_string()))
    }

    async fn get_prompt_from_peer(
        peer: &rmcp::service::Peer<rmcp::service::RoleClient>,
        server_name: &str,
        name: &str,
        args: Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<(Option<String>, Vec<rmcp::model::PromptMessage>), McpError> {
        use rmcp::model::{GetPromptRequestParams, JsonObject};

        let arguments = match args {
            Some(args) if !args.is_empty() =>
            {
                #[allow(clippy::map_identity)]
                Some(JsonObject::from_iter(args.into_iter().map(|(k, v)| (k, v))))
            }
            _ => None,
        };

        let params = match arguments {
            Some(args) => GetPromptRequestParams::new(name).with_arguments(args),
            None => GetPromptRequestParams::new(name),
        };

        let result = peer
            .get_prompt(params)
            .await
            .map_err(|e: rmcp::service::ServiceError| McpError::PromptGetFailed {
                server: server_name.to_string(),
                name: name.to_string(),
                detail: e.to_string(),
            })?;

        Ok((result.description, result.messages))
    }

    pub async fn complete_prompt(
        &self,
        name: &str,
        argument_name: &str,
        value: &str,
    ) -> Result<rmcp::model::CompletionInfo, McpError> {
        let servers = self.servers.read().await;
        for (server, state) in servers.iter() {
            if state.status != ServerStatus::Connected {
                continue;
            }
            if state.cached_prompts.iter().any(|p| p.name == name) {
                if let Some(service) = &state.running_service {
                    return service
                        .peer()
                        .complete_prompt_argument(name, argument_name, value, None)
                        .await
                        .map_err(|e: rmcp::service::ServiceError| McpError::PromptGetFailed {
                            server: server.to_string(),
                            name: name.to_string(),
                            detail: e.to_string(),
                        });
                }
            }
        }
        Err(McpError::ServerNotFound(name.to_string()))
    }

    // ── Disconnect ─────────────────────────────────────────────────────

    pub async fn disconnect(&self) -> Result<(), McpError> {
        let server_names: Vec<String>;
        {
            let servers = self.servers.read().await;
            server_names = servers.keys().cloned().collect();
        }

        for name in server_names {
            self.disconnect_server(&name).await?;
        }

        Ok(())
    }

    pub async fn disconnect_server(&self, name: &str) -> Result<(), McpError> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;

        state.cancel_token.cancel();

        if let Some(mut service) = state.running_service.take() {
            if let Err(e) = service.close().await {
                tracing::warn!("Error closing MCP server '{}': {}", name, e);
            }
        }

        state.clear_caches();
        state.status = ServerStatus::Disconnected;

        tracing::info!(server = name, "MCP server disconnected");
        Ok(())
    }

    // ── Status + reconnect ─────────────────────────────────────────────

    pub fn server_status(&self) -> std::collections::HashMap<String, ServerStatus> {
        if let Ok(guard) = self.servers.try_read() {
            guard
                .iter()
                .map(|(name, state)| (name.clone(), state.status.clone()))
                .collect()
        } else {
            std::collections::HashMap::new()
        }
    }

    pub async fn server_status_async(&self) -> std::collections::HashMap<String, ServerStatus> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .map(|(name, state)| (name.clone(), state.status.clone()))
            .collect()
    }

    pub async fn reconnect(&self, name: &str) -> Result<(), McpError> {
        {
            let mut servers = self.servers.write().await;
            let state = servers
                .get_mut(name)
                .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;

            state.cancel_token.cancel();

            if state.status == ServerStatus::Connected {
                if let Some(mut service) = state.running_service.take() {
                    let _ = service.close().await;
                }
                state.clear_caches();
            }

            state.retry_count = 0;
        }

        self.connect_server(name).await;

        let servers = self.servers.read().await;
        let state = servers
            .get(name)
            .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;
        match &state.status {
            ServerStatus::Connected => Ok(()),
            ServerStatus::Error(e) => Err(McpError::ConnectionFailed {
                server: name.to_string(),
                detail: e.clone(),
            }),
            _ => Err(McpError::ConnectionFailed {
                server: name.to_string(),
                detail: "reconnect did not complete".to_string(),
            }),
        }
    }
}

async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
    config: &McpServerConfig,
    cancel_token: &CancellationToken,
) -> Result<RunningService<RoleClient, ClientInfo>, McpError> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit());

    let child =
        TokioChildProcess::new(cmd).map_err(|e: std::io::Error| McpError::ConnectionFailed {
            server: config.name.clone(),
            detail: e.to_string(),
        })?;

    let client_info = ClientInfo::default();
    tokio::time::timeout(
        Duration::from_secs(10),
        client_info.serve_with_ct(child, cancel_token.clone()),
    )
    .await
    .map_err(|_| McpError::InitializeTimeout {
        server: config.name.clone(),
    })?
    .map_err(|e| McpError::ConnectionFailed {
        server: config.name.clone(),
        detail: e.to_string(),
    })
}

async fn connect_http(
    url: &str,
    headers: Option<&HashMap<String, String>>,
    env: &HashMap<String, String>,
    config: &McpServerConfig,
    cancel_token: &CancellationToken,
) -> Result<RunningService<RoleClient, ClientInfo>, McpError> {
    use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

    let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url);

    if let Some(hdrs) = headers {
        if !hdrs.is_empty() {
            let mut http_headers: std::collections::HashMap<http::HeaderName, http::HeaderValue> =
                std::collections::HashMap::new();
            for (name, value) in hdrs {
                if let (Ok(name), Ok(value)) = (
                    http::HeaderName::from_bytes(name.as_bytes()),
                    http::HeaderValue::from_str(value),
                ) {
                    http_headers.insert(name, value);
                }
            }
            transport_config = transport_config.custom_headers(http_headers);
        }
    }

    // Auto-detect proxy from HTTPS_PROXY environment variable.
    // Only apply proxy for remote HTTPS URLs — skip for HTTP/local addresses
    // so internal services (e.g. nhome.local) are reachable directly.
    let proxy_url = if url.starts_with("https://") {
        env.get("HTTPS_PROXY")
            .or_else(|| env.get("https_proxy"))
            .cloned()
            .or_else(|| std::env::var("HTTPS_PROXY").ok())
            .or_else(|| std::env::var("https_proxy").ok())
    } else if url.starts_with("http://") {
        env.get("HTTP_PROXY")
            .or_else(|| env.get("http_proxy"))
            .cloned()
            .or_else(|| std::env::var("HTTP_PROXY").ok())
            .or_else(|| std::env::var("http_proxy").ok())
    } else {
        env.get("ALL_PROXY")
            .or_else(|| env.get("all_proxy"))
            .cloned()
            .or_else(|| std::env::var("ALL_PROXY").ok())
            .or_else(|| std::env::var("all_proxy").ok())
    };

    let transport = if let Some(ref proxy) = proxy_url {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .proxy(
                reqwest::Proxy::https(proxy).map_err(|e| McpError::ConnectionFailed {
                    server: config.name.clone(),
                    detail: format!("invalid proxy URL '{proxy}': {e}"),
                })?,
            )
            .build()
            .map_err(|e| McpError::ConnectionFailed {
                server: config.name.clone(),
                detail: format!("failed to build reqwest client with proxy: {e}"),
            })?;
        tracing::info!(
            server = %config.name,
            proxy = %proxy,
            "connecting MCP server via HTTPS with proxy"
        );
        rmcp::transport::StreamableHttpClientTransport::with_client(client, transport_config)
    } else {
        rmcp::transport::StreamableHttpClientTransport::from_config(transport_config)
    };

    let client_info = ClientInfo::default();
    tokio::time::timeout(
        Duration::from_secs(10),
        client_info.serve_with_ct(transport, cancel_token.clone()),
    )
    .await
    .map_err(|_| McpError::InitializeTimeout {
        server: config.name.clone(),
    })?
    .map_err(|e| McpError::ConnectionFailed {
        server: config.name.clone(),
        detail: e.to_string(),
    })
}

fn exponential_backoff(retry_count: usize, min: Duration, max: Duration) -> Duration {
    #[allow(clippy::cast_possible_truncation)]
    let delay = min.mul_f64(2f64.powi(retry_count as i32));
    delay.min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_config() {
        let mgr = McpManager::new(vec![]);
        mgr.connect().await.unwrap();
        assert!(mgr.list_all_tools().await.is_empty());
    }

    #[tokio::test]
    async fn test_max_retry_exhaustion() {
        // Use a command that doesn't exist — every connect attempt will fail
        let config = McpServerConfig {
            name: "failing-server".to_string(),
            transport: McpTransport::Stdio {
                command: "nonexistent-command-that-will-fail".to_string(),
                args: vec![],
                env: std::collections::HashMap::new(),
            },
        };

        let mgr = McpManager::new(vec![config])
            .with_max_retries(2)
            .with_backoff(Duration::from_millis(10), Duration::from_millis(50));

        let _ = mgr.connect().await;

        // Allow some time for background reconnect attempts
        tokio::time::sleep(Duration::from_millis(300)).await;

        let status = mgr.server_status_async().await;
        let failing_status = status.get("failing-server").expect("server should exist");

        // After 2 retries + initial attempt = 3 total attempts, should be in Error state
        assert!(
            matches!(failing_status, ServerStatus::Error(msg) if msg.contains("max retries")),
            "expected max retries error, got: {failing_status:?}"
        );

        // No tools should be available
        let tools = mgr.list_all_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_manual_reconnect_after_exhaustion() {
        let config = McpServerConfig {
            name: "failing-server".to_string(),
            transport: McpTransport::Stdio {
                command: "nonexistent-command".to_string(),
                args: vec![],
                env: std::collections::HashMap::new(),
            },
        };

        let mgr = McpManager::new(vec![config])
            .with_max_retries(1)
            .with_backoff(Duration::from_millis(10), Duration::from_millis(50));

        let _ = mgr.connect().await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify exhaustion
        let status = mgr.server_status_async().await;
        assert!(
            matches!(status.get("failing-server"), Some(ServerStatus::Error(msg)) if msg.contains("max retries")),
            "expected max retries, got: {:?}",
            status.get("failing-server")
        );

        // Manual reconnect resets retry counter and attempts again
        mgr.reconnect("failing-server").await.unwrap_err();

        // Should be in Error state again (command still doesn't exist)
        let status = mgr.server_status_async().await;
        assert!(
            matches!(status.get("failing-server"), Some(ServerStatus::Error(_))),
            "expected error after reconnect, got: {:?}",
            status.get("failing-server")
        );
    }
}
