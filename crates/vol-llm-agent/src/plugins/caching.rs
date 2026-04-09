//! Caching plugin with semantic cache support.

use crate::react::plugin::*;
use crate::react::run_context::RunContext;
use crate::{AgentResponse, AgentError};
use std::sync::Arc;
use std::collections::HashMap;
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
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
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
}

#[async_trait::async_trait]
impl AgentPlugin for CachingPlugin {
    fn id(&self) -> PluginId {
        "caching".to_string()
    }

    fn priority(&self) -> u32 {
        20
    }

    async fn on_start(&self, ctx: &RunContext) -> PluginAction<()> {
        let key = self.cache.cache_key(&ctx.user_input);

        if let Some(cached_response) = self.cache.get(&key).await {
            tracing::info!(
                run_id = %ctx.run_id,
                cache_key = %key,
                "Cache hit"
            );
            let _ = ctx.set("cache.hit", true).await;
            return PluginAction::ShortCircuit(cached_response);
        }

        let _ = ctx.set("cache.hit", false).await;
        PluginAction::Continue(())
    }

    async fn intercept(
        &self,
        event: crate::react::plugin::StreamEvent,
        _ctx: &RunContext,
    ) -> PluginAction<Option<crate::react::plugin::StreamEvent>> {
        PluginAction::Continue(Some(event))
    }

    async fn on_complete(
        &self,
        ctx: &RunContext,
        final_response: &AgentResponse,
    ) -> PluginAction<()> {
        // Skip if cache hit (response was from cache)
        if ctx.get::<bool>("cache.hit").await.unwrap_or(false) {
            return PluginAction::Continue(());
        }

        // Cache the final response
        let key = self.cache.cache_key(&ctx.user_input);
        self.cache
            .set(key, final_response.clone(), self.ttl_secs)
            .await;
        tracing::info!(run_id = %ctx.run_id, "Cached response");

        PluginAction::Continue(())
    }

    async fn on_error(
        &self,
        _ctx: &RunContext,
        _error: &AgentError,
    ) -> PluginAction<()> {
        // Don't cache errors
        PluginAction::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};
    use crate::react::AgentConfig;

    fn create_test_run_context() -> RunContext {
        RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        )
    }

    #[tokio::test]
    async fn test_caching_plugin_shortcircuit() {
        let cache = SemanticCache::new();
        let plugin = CachingPlugin::new(300).with_cache(cache.clone());

        let ctx = create_test_run_context();

        // First call - cache miss
        match plugin.on_start(&ctx).await {
            PluginAction::Continue(()) => {
                assert_eq!(ctx.get::<bool>("cache.hit").await, Some(false));
            }
            _ => panic!("Expected Continue on cache miss"),
        }

        // Populate cache
        let response = AgentResponse {
            content: "cached response".to_string(),
            reasoning: String::new(),
            iterations: 1,
            tool_calls: Vec::new(),
        };
        cache
            .set(
                plugin.cache.cache_key("test input"),
                response.clone(),
                300,
            )
            .await;

        // Second call - cache hit, should short-circuit
        let plugin2 = CachingPlugin::new(300).with_cache(cache);
        let ctx2 = create_test_run_context();

        match plugin2.on_start(&ctx2).await {
            PluginAction::ShortCircuit(cached) => {
                assert_eq!(cached.content, "cached response");
            }
            _ => panic!("Expected ShortCircuit on cache hit"),
        }
    }

    #[tokio::test]
    async fn test_caching_plugin_on_complete() {
        let cache = SemanticCache::new();
        let plugin = CachingPlugin::new(300).with_cache(cache.clone());

        let ctx = create_test_run_context();

        // Mark as cache miss
        let _ = ctx.set("cache.hit", false).await;

        let response = AgentResponse {
            content: "new response".to_string(),
            reasoning: String::new(),
            iterations: 1,
            tool_calls: Vec::new(),
        };

        plugin.on_complete(&ctx, &response).await;

        // Verify response was cached
        let key = plugin.cache.cache_key("test input");
        let cached = cache.get(&key).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().content, "new response");
    }
}
