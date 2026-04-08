//! DashScope Embedder configuration.

/// DashScope embedding models
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum DashScopeModel {
    /// text-embedding-v2 (1536 dimensions)
    #[default]
    TextEmbeddingV2,
    /// text-embedding-v3 (1024 dimensions)
    TextEmbeddingV3,
}

impl DashScopeModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DashScopeModel::TextEmbeddingV2 => "text-embedding-v2",
            DashScopeModel::TextEmbeddingV3 => "text-embedding-v3",
        }
    }

    /// Get the output dimension for this model
    pub fn dimensions(&self) -> usize {
        match self {
            DashScopeModel::TextEmbeddingV2 => 1536,
            DashScopeModel::TextEmbeddingV3 => 1024,
        }
    }
}

/// DashScope Embedder configuration
#[derive(Debug, Clone)]
pub struct DashScopeConfig {
    /// API key
    pub api_key: String,
    /// Model to use
    pub model: DashScopeModel,
    /// API base URL
    pub base_url: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl Default for DashScopeConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("DASHSCOPE_API_KEY")
                .unwrap_or_else(|_| String::new()),
            model: DashScopeModel::default(),
            base_url: "https://dashscope.aliyuncs.com/api/v1".to_string(),
            timeout_secs: 30,
        }
    }
}

impl DashScopeConfig {
    pub fn with_api_key(mut self, key: &str) -> Self {
        self.api_key = key.to_string();
        self
    }

    pub fn with_model(mut self, model: DashScopeModel) -> Self {
        self.model = model;
        self
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}
