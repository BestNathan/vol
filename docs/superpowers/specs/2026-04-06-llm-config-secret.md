# Design: LLMConfig with Secret Support and Multi-Provider

## Why

当前 `LLMConfig` 使用 `api_key_env` 字段从环境变量读取 API Key，存在以下问题：
1. **配置不灵活**：无法在配置文件中直接指定 API Key（如测试场景）
2. **不支持默认值**：环境变量不存在时没有 fallback 机制
3. **单一 Provider**：只能配置一个 LLM Provider，无法支持多 Provider 切换

需要通过 `Secret` 类型支持直接值和环境变量引用，并通过配置 ID 支持多 Provider 架构。

## What Changes

### 新增 `Secret` 类型

```rust
/// A secret value that can be either a literal string or an environment variable reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Secret {
    /// Direct literal value (e.g., "sk-xxx")
    Literal(String),
    /// Environment variable reference: "${VAR_NAME}" or "${VAR_NAME:default}"
    Env { env: String, default: Option<String> },
}
```

### 更新 `LLMConfig`

```rust
pub struct LLMConfig {
    pub provider: LLMProvider,
    pub model: String,
    pub api_key: Secret,      // 改为 Secret 类型
    pub base_url: String,     // 移除 Option，始终需要
}
```

### 新增 `LLMProviderConfig` 支持多 Provider

```rust
/// Named LLM provider configuration
pub struct LLMProviderConfig {
    pub id: String,           // 配置 ID，如 "anthropic-main"
    pub config: LLMConfig,
}
```

### 新增 `LLMProviderRegistry`

```rust
pub struct LLMProviderRegistry {
    providers: HashMap<String, Box<dyn LLMClient>>,
}

impl LLMProviderRegistry {
    pub fn from_config(configs: &[LLMProviderConfig]) -> Result<Self>;
    pub fn get(&self, id: &str) -> Option<&dyn LLMClient>;
    pub fn ids(&self) -> Vec<&str>;
}
```

### 更新 `AgentAdviceConfig`

```rust
pub struct AgentAdviceConfig {
    pub enabled: bool,
    pub cooldown_secs: u64,
    pub max_analyses_per_hour: u32,
    pub llm_provider_id: String,  // 引用 llm_providers 配置的 id
}
```

## Configuration Examples

### TOML 配置文件

```toml
# 多个 LLM Provider 配置
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[[llm_providers]]
id = "openai-backup"
provider = "openai"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY:sk-fallback-key}"
base_url = "https://api.openai.com/v1"

# Agent 配置引用 Provider ID
[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
llm_provider_id = "anthropic-main"
```

### 使用方式

```rust
// 加载配置
let provider_configs = config.llm_providers;
let registry = LLMProviderRegistry::from_config(&provider_configs)?;

// 获取指定 Provider
let llm = registry.get("anthropic-main")
    .ok_or_else(|| Error::UnknownProvider("anthropic-main".to_string()))?;

// 创建 Agent
let agent = ReActAgent::new(llm, tools, agent_config);
```

## Capabilities

### New Capabilities
- `vol-llm-provider`: `Secret` 类型，支持字面值和环境变量引用
- `vol-llm-provider`: `LLMProviderRegistry` 管理多 Provider 实例
- `vol-config`: `[[llm_providers]]` 数组配置支持

### Modified Capabilities
- `vol-llm-provider`: `LLMConfig` 使用 `Secret` 替代 `api_key_env`
- `vol-llm-bridge`: `AgentAdviceConfig` 使用 `llm_provider_id` 引用配置

## Impact

- **Breaking**: `LLMConfig` 结构变更，现有代码需要更新
- **Breaking**: `api_key_env` 字段移除，改为 `api_key: Secret`
- **配置变更**: 新增 `[[llm_providers]]` 配置段
- **迁移路径**: 
  - 旧：`api_key_env = "ANTHROPIC_AUTH_TOKEN"`
  - 新：`api_key = "${ANTHROPIC_AUTH_TOKEN}"`

## Testing Strategy

1. **单元测试**:
   - `Secret::resolve()` 字面值解析
   - `Secret::resolve()` 环境变量解析
   - `Secret::resolve()` 带默认值的环境变量解析
   - `LLMProviderRegistry` 注册和获取

2. **集成测试**:
   - 从 TOML 加载多 Provider 配置
   - 验证 Agent 使用正确的 Provider

## Open Questions

- 是否需要支持 Provider 运行时切换（failover）？
- 是否需要将 Provider 配置持久化到数据库？
