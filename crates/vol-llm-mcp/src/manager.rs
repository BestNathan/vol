//! McpManager — manages MCP server connection lifecycles.
//!
//! Tracks per-server connection state, spawns background reconnect tasks
//! on failure, and caches discovered capabilities at connect time.

use rmcp::model::{ClientInfo, Tool, Resource, ResourceTemplate, Prompt};
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

use crate::config::McpServerConfig;
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
    ) -> Result<(RunningService<RoleClient, ClientInfo>, Vec<Tool>, Vec<Resource>, Vec<ResourceTemplate>, Vec<Prompt>), McpError> {
        let mut command = Command::new(&config.command);
        command.args(&config.args);
        for (key, value) in &config.env {
            command.env(key, value);
        }
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::inherit());

        let child = TokioChildProcess::new(command).map_err(|e: std::io::Error| {
            McpError::ConnectionFailed {
                server: config.name.clone(),
                detail: e.to_string(),
            }
        })?;

        let client_info = ClientInfo::default();
        let service = tokio::time::timeout(
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
        })?;

        let peer = service.peer();

        let tools = peer.list_all_tools().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list tools for '{}': {}", config.name, e);
            Vec::new()
        });

        let resources = peer.list_all_resources().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list resources for '{}': {}", config.name, e);
            Vec::new()
        });

        let resource_templates = peer.list_all_resource_templates().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list resource templates for '{}': {}", config.name, e);
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

    async fn connect_server(&self, name: &str) {
        let config;
        let max_retries;
        let backoff_min;
        let backoff_max;
        {
            let mut servers = self.servers.write().await;
            let Some(state) = servers.get_mut(name) else { return };
            state.status = ServerStatus::Connecting;
            state.cancel_token = CancellationToken::new();
            config = state.config.clone();
            max_retries = self.max_retries;
            backoff_min = self.backoff_min;
            backoff_max = self.backoff_max;
        }

        match Self::connect_single(&config, &CancellationToken::new()).await {
            Ok((service, tools, resources, resource_templates, prompts)) => {
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.running_service = Some(service);
                    state.cached_tools = tools.iter().map(|t| McpToolInfo {
                        name: t.name.to_string(),
                        description: t.description.as_ref().map(|s| s.to_string()),
                        input_schema: Some(t.schema_as_json_value()),
                    }).collect();
                    state.cached_resources = resources;
                    state.cached_resource_templates = resource_templates;
                    state.cached_prompts = prompts;
                    state.status = ServerStatus::Connected;
                    state.retry_count = 0;
                    tracing::info!(server = name, "MCP server connected");
                }
            }
            Err(e) => {
                tracing::error!(server = name, error = %e, "MCP server connection failed");
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.retry_count += 1;
                    if state.retry_count >= max_retries {
                        state.clear_caches();
                        state.status = ServerStatus::Error("max retries exceeded".to_string());
                        tracing::error!(server = name, retries = state.retry_count, "MCP server max retries exceeded");
                    } else {
                        state.status = ServerStatus::Error(e.to_string());
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
        let backoff_min = backoff_min;
        let backoff_max = backoff_max;

        let handle = {
            let name = name.clone();
            tokio::spawn(async move {
            loop {
                let (config, cancel_token, current_retry) = {
                    let mut srv = servers.write().await;
                    let Some(state) = srv.get_mut(&name) else { break };
                    if state.retry_count >= max_retries {
                        state.clear_caches();
                        state.status = ServerStatus::Error("max retries exceeded".to_string());
                        break;
                    }
                    // Cancel previous reconnect, create new token
                    state.cancel_token.cancel();
                    state.cancel_token = CancellationToken::new();
                    let config = state.config.clone();
                    (config, state.cancel_token.clone(), state.retry_count)
                };

                if cancel_token.is_cancelled() {
                    break;
                }

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
                            state.cached_tools = tools.iter().map(|t| McpToolInfo {
                                name: t.name.to_string(),
                                description: t.description.as_ref().map(|s| s.to_string()),
                                input_schema: Some(t.schema_as_json_value()),
                            }).collect();
                            state.cached_resources = resources;
                            state.cached_resource_templates = resource_templates;
                            state.cached_prompts = prompts;
                            state.status = ServerStatus::Connected;
                            state.retry_count = 0;
                            tracing::info!(server = name, "MCP server reconnected");
                        }
                        break;
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(server = name, error = %e, "MCP reconnect failed");
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.retry_count += 1;
                            state.status = ServerStatus::Error(e.to_string());
                            if state.retry_count >= max_retries {
                                state.clear_caches();
                                state.status = ServerStatus::Error("max retries exceeded".to_string());
                                tracing::error!(server = name, retries = state.retry_count, "MCP server max retries exceeded");
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        tracing::warn!(server = name, "MCP reconnect timed out");
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.retry_count += 1;
                            state.status = ServerStatus::Error("connection timeout".to_string());
                            if state.retry_count >= max_retries {
                                state.clear_caches();
                                state.status = ServerStatus::Error("max retries exceeded".to_string());
                                break;
                            }
                        }
                    }
                }
            }
        })
        };

        let name_clone = name.clone();
        let servers_clone = self.servers.clone();
        tokio::spawn(async move {
            let mut srv = servers_clone.write().await;
            if let Some(state) = srv.get_mut(&name_clone) {
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
                state.cached_tools.iter().map(|tool| {
                    (server.clone(), tool.clone())
                })
            })
            .collect()
    }
}

fn exponential_backoff(retry_count: usize, min: Duration, max: Duration) -> Duration {
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
}
