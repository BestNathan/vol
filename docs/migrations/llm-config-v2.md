# LLM Config Migration Guide (v1 → v2)

## Overview

This migration updates `LLMConfig` to use the new `Secret` type for flexible API key configuration and adds multi-provider support through `LLMProviderRegistry`.

## What Changed

### Key Features

1. **Secret Type**: API keys can now be literal values or environment variable references
2. **Multi-Provider**: Configure multiple LLM providers with unique IDs
3. **Provider Registry**: Agent Advice references providers by ID instead of embedding config

## Breaking Changes

### Old Format (v1)

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_AUTH_TOKEN"
endpoint = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
```

### New Format (v2)

```toml
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
llm_provider_id = "anthropic-main"
```

## Migration Steps

### Step 1: Replace `[llm]` with `[[llm_providers]]`

Change from single `[llm]` table to array of providers:

```toml
# Before
[llm]
provider = "anthropic"

# After
[[llm_providers]]
id = "anthropic-main"  # Add unique ID
provider = "anthropic"
```

### Step 2: Update API Key Format

Change `api_key_env` to `api_key` with environment variable syntax:

```toml
# Before
api_key_env = "ANTHROPIC_AUTH_TOKEN"

# After
api_key = "${ANTHROPIC_AUTH_TOKEN}"
```

### Step 3: Rename `endpoint` to `base_url`

```toml
# Before
endpoint = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

# After
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"
```

### Step 4: Add `llm_provider_id` to Agent Advice

```toml
[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
llm_provider_id = "anthropic-main"  # Add this line
```

## Secret Value Formats

The new `api_key` field supports three formats:

| Format | Syntax | Example |
|--------|--------|---------|
| Literal | Direct string | `api_key = "sk-xxx-key"` |
| Environment Variable | `${VAR_NAME}` | `api_key = "${API_KEY}"` |
| Environment with Default | `${VAR_NAME:default}` | `api_key = "${API_KEY:sk-fallback}"` |

## Multiple Providers Example

Configure multiple providers for failover or different models:

```toml
# Primary provider
[[llm_providers]]
id = "anthropic-primary"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

# Backup provider
[[llm_providers]]
id = "openai-backup"
provider = "openai"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY:sk-fallback-key}"
base_url = "https://api.openai.com/v1"

# Agent Advice uses primary
[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
llm_provider_id = "anthropic-primary"
```

## Environment Variables

Add LLM API keys to your `.env` file:

```bash
# LLM API Keys
ANTHROPIC_AUTH_TOKEN="sk-xxx-actual-key"
OPENAI_API_KEY="sk-xxx-actual-key"
```

See `.env.example` for the full template.

## Testing After Migration

1. **Verify config loads:**
   ```bash
   cargo run -- --config config.toml
   ```
   
   Look for:
   ```
   Initializing LLM providers: 1 configured
   Available LLM providers: ["anthropic-main"]
   AgentAdvice will use provider: anthropic-main
   ```

2. **Check provider initializes:**
   Look for "AgentAdviceService started" in logs

3. **Test alert processing:**
   Trigger a test alert and verify analysis is generated

## Rollback

If issues occur, restore the backup:

```bash
# Restore old config
cp config.toml.bak config.toml

# Rebuild
cargo build
```

Note: You must use a version of the code before the LLMConfig changes if rolling back.

## Code Changes Required

If you have custom code that uses `LLMConfig`:

### Before (v1)

```rust
let config = LLMConfig {
    provider: LLMProvider::Anthropic,
    model: "claude-test".to_string(),
    api_key_env: "TEST_API_KEY".to_string(),
    endpoint: Some("https://api.test.com".to_string()),
};
```

### After (v2)

```rust
use vol_llm_provider::Secret;

let config = LLMConfig::with_env_key(
    LLMProvider::Anthropic,
    "claude-test",
    "TEST_API_KEY",
    "https://api.test.com",
);

// Or with literal key (for testing):
let config = LLMConfig::with_literal_key(
    LLMProvider::Anthropic,
    "claude-test",
    "sk-test-key",
    "https://api.test.com",
);
```

## Additional Resources

- Configuration examples: `config/llm.example.toml`
- Full documentation: `docs/CONFIGURATION.md` (LLM Configuration section)
- Environment template: `.env.example`
