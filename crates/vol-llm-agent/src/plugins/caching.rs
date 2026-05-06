//! Caching plugin with semantic cache support.

use crate::react::plugin::*;
use crate::{AgentResponse, AgentStreamEvent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache entry with TTL
#[derive(Debug, Clone)]
pub struct CacheEntry {
    response: AgentResponse,
    expires_at: u64,
}

impl CacheEntry {
    pub fn new(response: AgentResponse, ttl_secs: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            response,
            expires_at: now + ttl_secs,
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }
}

/// Semantic cache with TTL
#[derive(Clone)]
pub struct SemanticCache {
    entries: Arc<tokio::sync::RwLock<HashMap<String, CacheEntry>>>,
}

impl SemanticCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub fn cache_key(&self, input: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        format!("cache_{}", hasher.finish())
    }

    pub async fn get(&self, key: &str) -> Option<AgentResponse> {
        let entries = self.entries.read().await;
        entries
            .get(key)
            .filter(|e| !e.is_expired())
            .map(|e| e.response.clone())
    }

    pub async fn set(&self, key: String, response: AgentResponse, ttl_secs: u64) {
        let entry = CacheEntry::new(response, ttl_secs);
        self.entries.write().await.insert(key, entry);
    }
}

impl Default for SemanticCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Caching plugin
pub struct CachingPlugin {
    cache: SemanticCache,
    ttl_secs: u64,
}

impl CachingPlugin {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            cache: SemanticCache::new(),
            ttl_secs,
        }
    }

    pub fn with_cache(mut self, cache: SemanticCache) -> Self {
        self.cache = cache;
        self
    }

    /// Check cache for input and return cached response if available
    pub async fn check_cache(&self, input: &str) -> Option<AgentResponse> {
        let key = self.cache.cache_key(input);
        self.cache.get(&key).await
    }

    /// Store response in cache
    pub async fn store(&self, input: &str, response: AgentResponse) {
        let key = self.cache.cache_key(input);
        self.cache.set(key, response, self.ttl_secs).await;
    }
}

#[async_trait::async_trait]
impl AgentPlugin for CachingPlugin {
    fn id(&self) -> PluginId {
        "caching".to_string()
    }

    fn priority(&self) -> u32 {
        20
    }

    /// Interceptor hook - no-op for caching (cache logic handled externally)
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    /// Listener hook - logs caching events
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                tracing::debug!(
                    run_id = %ctx.run_id,
                    input = %input,
                    "Caching: checking cache"
                );
            }
            AgentStreamEvent::AgentComplete { .. } => {
                tracing::info!(
                    run_id = %ctx.run_id,
                    "Caching: agent complete"
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react::{AgentConfig, RunContext};
    use vol_session::{InMemoryEntryStore, Session};
    use std::sync::Arc;

    fn create_test_run_context() -> RunContext {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                Arc::new(InMemoryEntryStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
            20,
            "test-model".to_string(),
        );
        ctx
    }

    #[tokio::test]
    async fn test_caching_plugin_cache_operations() {
        let cache = SemanticCache::new();
        let plugin = CachingPlugin::new(300).with_cache(cache.clone());

        // Cache miss
        let result = plugin.check_cache("test input").await;
        assert!(result.is_none());

        // Store response
        let response = AgentResponse {
            content: "cached response".to_string(),
            reasoning: vec![],
            run_id: "test-run".to_string(),
            session_id: "test-session".to_string(),
            iterations: 1,
            tool_calls: Vec::new(),
            error: None,
        };
        plugin.store("test input", response.clone()).await;

        // Cache hit
        let result = plugin.check_cache("test input").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().content, "cached response");
    }

    #[test]
    fn test_caching_plugin_id() {
        let plugin = CachingPlugin::new(300);
        assert_eq!(plugin.id(), "caching");
    }

    #[test]
    fn test_caching_plugin_priority() {
        let plugin = CachingPlugin::new(300);
        assert_eq!(plugin.priority(), 20);
    }

    #[tokio::test]
    async fn test_caching_plugin_intercept() {
        let plugin = CachingPlugin::new(300);
        let ctx = create_test_run_context();

        let event = AgentStreamEvent::agent_start("test".to_string());
        match plugin.intercept(&event, &ctx).await {
            PluginDecision::Continue => {}
            _ => panic!("Expected Continue"),
        }
    }
}
