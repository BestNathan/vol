# LLM Provider Dynamic Configuration Design

**Date:** 2026-05-15
**Status:** Draft

## Problem

当前 LLM provider 配置存在两个问题：

1. **参数僵化**: `LLMConfig` 只有 4 个固定字段（provider, model, api_key, base_url），无法配置 provider 特有参数（如 Anthropic 的 `anthropic-version` header、OpenAI 的 `response_format`）
2. **配置耦合**: provider 配置嵌在 `config.toml` 的 `[[llm_providers]]` 中，与监控配置混在一起，且缺乏分层加载能力
3. **通用参数硬编码**: `ModelConfig` 包含所有可能参数的 Option 字段，但实际使用的参数取决于 provider，未使用的字段浪费序列化空间

## Requirements

- 用独立配置文件完全替代 `config.toml` 中的 `[[llm_providers]]`
- 文件名作为 provider ID，内容为 provider 配置（body 参数 + headers）
- 支持项目级 (`.agents/providers/`) 和用户级 (`~/.agents/providers/`) 双层加载，项目级覆盖用户级
- body 参数作为 provider 默认值，运行时 ConversationRequest 可以覆盖
- headers 附加到 HTTP 请求

## Design

### 1. 目录结构

```
.agents/providers/
  anthropic-dashscope.toml
  openai-gpt.toml
```

每个 `.toml` 文件的文件名（不含扩展名）即为 provider ID。

### 2. 文件格式

```toml
# .agents/providers/anthropic-dashscope.toml
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[body]
max_tokens = 8192
temperature = 0.7
top_p = 0.9

[headers]
"anthropic-version" = "2023-06-01"
```

- `provider`, `model`, `api_key`, `base_url`: 必需字段，与现有 `LLMConfig` 兼容
- `body`: 可选，动态 body 参数，key-value 对，运行时合并到请求
- `headers`: 可选，动态 HTTP headers，附加到每个请求

### 3. 配置结构

```rust
// crates/vol-llm-provider/src/config.rs

/// 文件级配置（从单个 TOML 文件解析）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderFileConfig {
    pub provider: LLMProvider,
    pub model: String,
    pub api_key: Secret,
    pub base_url: String,
    #[serde(default)]
    pub body: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

/// 运行时 provider 实例，携带默认 body 参数和 headers
#[derive(Debug, Clone)]
pub struct ProviderRuntime {
    pub config: LLMConfig,
    pub body_defaults: HashMap<String, serde_json::Value>,
    pub headers: HashMap<String, String>,
}
```

### 4. 配置加载

类似 MCP 模式 (`crates/vol-llm-mcp/src/config.rs`)：

```rust
// crates/vol-llm-provider/src/loader.rs (new file)

const PROVIDERS_DIR: &str = ".agents/providers";
const USER_PROVIDERS_DIR: &str = ".agents/providers";

pub struct ProviderLoader {
    providers: HashMap<String, ProviderRuntime>,
}

impl ProviderLoader {
    /// 加载项目级 + 用户级配置，项目级覆盖用户级（per-key by filename）
    pub fn load(working_dir: Option<&Path>) -> Result<Self, ProviderError>;

    /// 按 ID 获取 provider
    pub fn get(&self, id: &str) -> Option<&ProviderRuntime>;

    /// 获取所有 provider IDs
    pub fn ids(&self) -> Vec<&str>;
}
```

加载流程：
1. 扫描 `~/.agents/providers/*.toml`（用户级，低优先级）
2. 扫描 `<working_dir>/.agents/providers/*.toml`（项目级，高优先级）
3. 按文件名合并，同名项目级覆盖用户级
4. 解析每个文件为 `ProviderFileConfig`，构建 `ProviderRuntime`
5. `api_key` 通过 `Secret::resolve()` 处理环境变量引用

### 5. 运行时参数合并

`ConversationRequest` 中的 `model_config` 参数覆盖 provider body defaults：

```
最终参数 = provider.body_defaults ∪ request.model_config
```

合并逻辑在 provider 的 `converse()` / `converse_stream()` 中实现：

```rust
// 在 AnthropicProvider::converse() 中：
let mut body = build_base_body_from_defaults(&self.body_defaults);
// 然后按现有逻辑应用 request.model_config 中的参数
// model_config 中的值覆盖 body_defaults 中的同名 key
```

headers 在构建 HTTP 请求时附加：

```rust
for (key, value) in &self.headers {
    request = request.header(key, value);
}
```

### 6. 变更影响

#### `crates/vol-llm-provider/src/config.rs`

- `LLMConfig` 保持不变（向后兼容）
- 新增 `ProviderFileConfig` 结构

#### `crates/vol-llm-provider/src/registry.rs`

- `LLMProviderRegistry` 改为从 `ProviderLoader` 获取配置
- 移除 `LLMProviderConfig` 的 `#[serde(flatten)]` flatten 模式

#### `crates/vol-llm-provider/src/factory.rs`

- `create_provider` 接受 `ProviderFileConfig` 而非 `LLMConfig`

#### `crates/vol-llm-provider/src/anthropic.rs`

- `AnthropicProvider` 存储 `body_defaults` 和 `headers`
- `converse()` / `converse_stream()` 中使用 body defaults 并附加 headers

#### `crates/vol-config/src/lib.rs`

- 移除 `Config.llm_providers: Vec<vol_llm_provider::LLMProviderConfig>` 字段
- 移除 `AgentAdviceConfig.llm_provider_id` 中引用的 provider ID 改为从 `.agents/providers/` 加载的 ID

#### `crates/vol-llm-core/src/model.rs`

- `ModelConfig` 保持不变（作为运行时覆盖层）

#### 新增文件

- `crates/vol-llm-provider/src/loader.rs` — 配置加载和合并逻辑

### 7. 迁移

1. 将 `config/llm.example.toml` 中的 `[[llm_providers]]` 示例改写为 `.agents/providers/` 下的 TOML 文件
2. 更新 `config.toml` / `config.dev.toml` / `config.prod.toml` 移除 `[[llm_providers]]` 段
3. 更新所有测试中手动构造 `LLMProviderConfig` 的地方，改用 `ProviderLoader` 或 `ProviderFileConfig`
4. 删除 `config/llm.example.toml`

### 8. 示例

```
.agents/providers/
  anthropic-dashscope.toml
  openai-backup.toml
```

`anthropic-dashscope.toml`:
```toml
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[body]
max_tokens = 8192
temperature = 0.7

[headers]
"anthropic-version" = "2023-06-01"
```

`openai-backup.toml`:
```toml
provider = "openai"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"

[body]
max_tokens = 4096
temperature = 0.7
frequency_penalty = 0.0

[headers]
"OpenAI-Beta" = "assistants=v2"
```

## Open Questions

- 是否需要支持 YAML 格式？当前只支持 TOML
- body 参数是否需要类型校验（如 temperature 必须在 0-2 之间）？
