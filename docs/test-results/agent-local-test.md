# Agent Advice 本地测试指南

本文档说明如何在本地环境中测试 Agent Advice 功能的接入流程。

## 前提条件

1. **Rust 工具链** - 已安装 `rustc` 和 `cargo`
2. **Deribit API 凭证** - 测试环境的 Client ID 和 Secret
3. **LLM API Key** - Anthropic 或其他支持的 LLM provider
4. **网络代理** - 如果在受限环境需要配置代理

## 快速开始

### 1. 配置环境变量

复制并编辑 `.env` 文件：

```bash
cp .env.example .env
vim .env
```

必要的环境变量：

```bash
# Deribit API
DERIBIT_CLIENT_ID="your-client-id"
DERIBIT_CLIENT_SECRET="your-client-secret"

# LLM API (Agent Advice 必需)
ANTHROPIC_AUTH_TOKEN="sk-xxx-actual-key"

# 代理 (如需要)
HTTPS_PROXY="http://192.168.2.98:8890"
```

### 2. 使用测试配置

项目已包含预配置的测试文件 `config.agent-test.toml`：

```toml
# 特点：
# - 宽松的告警阈值 (容易触发)
# - 较短的冷却时间 (快速测试)
# - 仅 stdout 通知 (无需 Feishu)
# - Agent Advice 启用
```

### 3. 运行测试脚本

```bash
# Dry run 模式 - 验证配置
./scripts/test-agent.sh --dry-run

# 实际运行测试
./scripts/test-agent.sh

# 使用详细日志
RUST_LOG=debug ./scripts/test-agent.sh --verbose
```

### 4. 观察日志输出

成功启动后，应该看到以下日志：

```
===========================================
  Deribit Volatility Monitor v0.3.0
===========================================

Initializing LLM providers: 1 configured
Available LLM providers: ["anthropic-main"]
TDengine client initialized
Tool registry initialized with 1 tools
Feishu notification initialized
Added AgentAdviceService notification handler
...
Monitoring engine started successfully
```

## 关键日志检查点

| 日志 | 说明 | 预期结果 |
|------|------|----------|
| `TDengine client initialized` | TDengine 客户端初始化 | 应该看到 |
| `Tool registry initialized with N tools` | 工具注册表初始化 | N >= 1 |
| `Feishu notification initialized` | Feishu 通知初始化 | 应该看到 (即使未配置凭证) |
| `Added AgentAdviceService notification handler` | Agent 服务注册 | 应该看到 |
| `AgentAdviceService started` | Agent 服务启动 | 启用时应该看到 |

## 测试场景

### 场景 1: 基础初始化测试

验证所有组件正确初始化：

```bash
./scripts/test-agent.sh --dry-run
```

**预期结果:**
- 配置文件验证通过
- 必要的环境变量存在
- 二进制文件编译成功

### 场景 2: 运行时集成测试

测试完整的 Agent 工作流程：

```bash
# 设置详细日志
export RUST_LOG="info,vol_llm_bridge=debug,vol_llm_agent=debug"

# 运行
./scripts/test-agent.sh
```

**观察点:**
1. WebSocket 连接成功
2. 市场数据开始流入
3. 告警触发时的处理流程
4. Agent 分析建议生成

### 场景 3: 手动触发告警测试

修改配置文件中的阈值，使其更容易触发：

```toml
[[rules]]
id = "absolute-iv-btc"
type = "absolute-iv"
symbol = "BTC"
short_threshold = 0.01  # 极低的阈值
enabled = true
```

然后运行测试，观察 Agent 是否响应告警。

## 故障排查

### 问题 1: "Missing DERIBIT_CLIENT_ID"

**原因:** 环境变量未加载

**解决:**
```bash
source .env
# 或重新运行脚本
./scripts/test-agent.sh
```

### 问题 2: "Failed to initialize LLM providers"

**原因:** API Key 无效或网络问题

**解决:**
1. 检查 `ANTHROPIC_AUTH_TOKEN` 是否正确
2. 验证代理配置
3. 测试 API 连接:
   ```bash
   curl -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN" \
        https://coding.dashscope.aliyuncs.com/apps/anthropic
   ```

### 问题 3: "AgentAdviceService provider not found"

**原因:** 配置中的 `llm_provider_id` 与 `[[llm_providers]]` 中的 ID 不匹配

**解决:** 检查配置文件：
```toml
[[llm_providers]]
id = "anthropic-main"  # 这个 ID

[agent_advice]
llm_provider_id = "anthropic-main"  # 必须匹配上面的 ID
```

### 问题 4: WebSocket 连接失败

**原因:** 网络问题或凭证错误

**解决:**
1. 检查代理配置
2. 验证 Deribit 凭证
3. 测试 WebSocket 连接:
   ```bash
   websocat "wss://test.deribit.com/ws/api/v2"
   ```

## 手动测试步骤

如果不使用测试脚本，可以手动执行：

```bash
# 1. 加载环境变量
source .env

# 2. 设置日志级别
export RUST_LOG="info,vol_llm_bridge=debug,vol_llm_agent=debug"

# 3. 运行程序
./target/release/vol-monitor --config config.agent-test.toml
```

## 清理

测试完成后：

```bash
# 停止程序
Ctrl+C

# 清理日志 (可选)
rm -rf logs/

# 恢复敏感配置
vim .env  # 移除或注释掉实际密钥
```

## 下一步

测试通过后：

1. **调整告警阈值** - 根据实际需求配置合适的阈值
2. **启用 Feishu 通知** - 配置 Feishu 凭证接收 AI 分析
3. **配置 TDengine** - 启用历史数据查询功能
4. **生产部署** - 使用 `config.prod.toml` 部署到 Kubernetes

## 参考文档

- [CONFIGURATION.md](CONFIGURATION.md) - 完整配置说明
- [config.agent-test.toml](config.agent-test.toml) - 测试配置示例
- [scripts/test-agent.sh](scripts/test-agent.sh) - 测试脚本源码
