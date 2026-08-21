# Configuration & Environment Variables

## Overview

Config files live under `configs/` — one example per server type. Secrets go in `.env` (gitignored).

| File | Purpose |
|------|---------|
| `configs/vol-agent-server.example.toml` | Agent server config example (data/control plane) |
| `configs/vol-agent-server.env.example` | Env template for agent server |
| `.env` | Local secrets (gitignored, **never commit**) |

**Quick Start — Agent Server**

```bash
cp configs/vol-agent-server.env.example .env   # edit API keys
source .env
cargo run -p vol-agent-server -- --config configs/vol-agent-server.example.toml
```

## Subsystem B — LLM Agent Framework

Configuration for LLM providers, ReAct agents, MCP servers, and skills.

### B.1 LLM Provider Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_AUTH_TOKEN` | Anthropic API key (or DashScope proxy token) |
| `OPENAI_API_KEY` | OpenAI API key |

### B.2 `[[llm_providers]]` — Provider Definitions

Define one or more LLM providers in the TOML config. The `api_key` field supports three formats:

| Format | Example | Behavior |
|--------|---------|----------|
| Literal | `"sk-abc123"` | Use the value directly |
| Env var | `"${ANTHROPIC_AUTH_TOKEN}"` | Read from environment variable |
| Env + fallback | `"${OPENAI_API_KEY:sk-default}"` | Use env var, fall back to literal if unset |

```toml
# Anthropic via DashScope proxy
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

# Local model service
[[llm_providers]]
id = "qwen-local"
provider = "openai"
model = "qwen3.6-plus"
api_key = "not-needed"
base_url = "http://192.168.2.162:31693/v1"
```

| Key | Type | Description |
|-----|------|-------------|
| `id` | string | Unique ID referenced by agents |
| `provider` | string | `"anthropic"` or `"openai"` |
| `model` | string | Model name |
| `api_key` | string | API key (literal or `${ENV_VAR}`) |
| `base_url` | string | API base URL |

### B.3 Agent Server Configuration

See `configs/vol-agent-server.example.toml` for the full annotated example covering server roles
(standalone data-plane, standalone control-plane, combined), control-plane node registration and
routing, data-plane identity, runtime store configuration, and tracing.

### B.4 Model Service

The default model service runs at `http://192.168.2.162:31693` with these available models:

| Model ID | Provider Type |
|----------|---------------|
| `gpt5.5` | openai-compatible |
| `coding` | openai-compatible |
| `qwen3.6-plus` | openai-compatible |
| `glm5.1` | openai-compatible |

Configure in `[[llm_providers]]` with `provider = "openai"` and the appropriate `base_url`.

---

## Kubernetes Deployment

### Security Checklist

- [ ] `.env` is in `.gitignore`
- [ ] No credentials in ConfigMap — only env var references
- [ ] K8s Secrets used, not ConfigMap literals
- [ ] Consider `sealed-secrets` or `external-secrets` for production
- [ ] Rotate credentials after team changes
