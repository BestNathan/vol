# 多配置分离设计文档

**日期:** 2026-04-06
**状态:** 已实施

---

## 1. 概述

### 1.1 目标

将敏感凭证（Deribit API 凭证、Feishu 凭证）从配置文件中分离，通过环境变量注入，实现：
- **开发/生产配置分离** - 不同的阈值、冷却时间、日志级别
- **敏感信息隔离** - 凭证不进入版本控制
- **K8s 原生支持** - 使用 Secrets 管理敏感数据

### 1.2 设计原则

| 原则 | 说明 |
|------|------|
| **敏感信息不入git** | 所有凭证通过环境变量或 K8s Secrets 注入 |
| **配置可测试** | 本地开发可以模拟生产配置 |
| **向后兼容** | 保留文件配置作为默认值 |
| **环境优先** | 环境变量覆盖配置文件中的值 |

---

## 2. 配置文件结构

### 2.1 文件列表

```
project-root/
├── config.toml              # 默认配置（生产模式，环境变量注入）
├── config.dev.toml          # 本地开发配置
├── config.prod.toml         # 生产配置（K8s 用）
├── .env.example             # 环境变量模板
├── .env                     # 本地环境变量（gitignore）
├── scripts/
│   └── run-dev.sh           # 本地开发启动脚本
└── k8s/
    ├── configmap.yaml       # 非敏感配置
    ├── secrets.yaml         # 敏感凭证模板
    ├── deployment.yaml      # 部署配置（环境变量注入）
    └── namespace.yaml       # 命名空间
```

### 2.2 配置文件对比

| 配置项 | config.dev.toml | config.prod.toml |
|--------|-----------------|------------------|
| **冷却时间** | 短 (60-600s) | 长 (300-14400s) |
| **IV 阈值** | 宽松 (0.70-0.90) | 严格 (0.51-0.75) |
| **日志级别** | debug | info |
| **日志格式** | 人类可读 | JSON |
| **Feishu** | 默认禁用 | 启用 |
| **OpenTelemetry** | 禁用 | 启用 |

---

## 3. 环境变量注入机制

### 3.1 支持的环境变量

```rust
// Deribit 凭证
DERIBIT_CLIENT_ID        // 覆盖 [clients.deribit.auth.client_id]
DERIBIT_CLIENT_SECRET    // 覆盖 [clients.deribit.auth.client_secret]

// Feishu 凭证
FEISHU_APP_ID            // 覆盖 notifications[].app_id
FEISHU_APP_SECRET        // 覆盖 notifications[].app_secret
FEISHU_RECEIVE_ID        // 覆盖 notifications[].receive_id

// 其他配置
HTTPS_PROXY              // 网络代理
RUST_LOG                 // 日志级别
OTEL_ENDPOINT            // OpenTelemetry 端点
```

### 3.2 实现代码

**vol-config/src/client.rs:**
```rust
impl DeribitAuthConfig {
    pub fn client_id(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_ID")
            .ok()
            .or_else(|| self.client_id.clone())
    }

    pub fn client_secret(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_SECRET")
            .ok()
            .or_else(|| self.client_secret.clone())
    }
}
```

**vol-config/src/notification.rs:**
```rust
impl FeishuNotificationConfig {
    pub fn app_id(&self) -> String {
        std::env::var("FEISHU_APP_ID")
            .ok()
            .unwrap_or_else(|| self.app_id.clone())
    }
    // ... app_secret(), receive_id() 同理
}
```

### 3.3 优先级顺序

```
环境变量 > 配置文件 > 默认值
```

---

## 4. Kubernetes 部署

### 4.1 Secret 创建

```bash
kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<actual-id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<actual-secret> \
  --from-literal=FEISHU_APP_ID=<actual-app-id> \
  --from-literal=FEISHU_APP_SECRET=<actual-app-secret> \
  --from-literal=FEISHU_RECEIVE_ID=<actual-receive-id> \
  -n deribit
```

### 4.2 Deployment 环境变量注入

```yaml
env:
- name: DERIBIT_CLIENT_ID
  valueFrom:
    secretKeyRef:
      name: vol-monitor-secrets
      key: DERIBIT_CLIENT_ID
- name: DERIBIT_CLIENT_SECRET
  valueFrom:
    secretKeyRef:
      name: vol-monitor-secrets
      key: DERIBIT_CLIENT_SECRET
- name: FEISHU_APP_ID
  valueFrom:
    secretKeyRef:
      name: vol-monitor-secrets
      key: FEISHU_APP_ID
```

### 4.3 ConfigMap 内容

ConfigMap 仅包含非敏感配置：
- 告警阈值
- 冷却时间
- 数据源配置
- OpenTelemetry 端点（非敏感部分）

---

## 5. 本地开发工作流

### 5.1 快速启动

```bash
# 1. 复制环境变量模板
cp .env.example .env

# 2. 编辑 .env 填入凭证
vim .env

# 5. 启动开发模式
./scripts/run-dev.sh dev
```

### 5.2 测试生产配置

```bash
# 使用生产配置本地运行（需要完整凭证）
./scripts/run-dev.sh prod
```

### 5.3 直接使用 cargo

```bash
source .env
cargo run --release -- --config config.dev.toml
```

---

## 6. 命令行参数

```bash
# 使用指定配置文件
./target/release/vol-monitor --config config.prod.toml

# 简写形式
./target/release/vol-monitor -c config.dev.toml

# 查看帮助
./target/release/vol-monitor --help
```

---

## 7. 安全考虑

### 7.1 Git 安全

```bash
# .gitignore 已包含
.env
.env.local
.env.*.local
```

### 7.2 凭证轮换

- **本地开发**: 更新 `.env` 文件后重启服务
- **K8s**: 更新 Secret 后重启 Pod

```bash
kubectl rollout restart deployment/vol-monitor -n deribit
```

### 7.3 生产建议

1. **使用 sealed-secrets**: 自动加密 Secret
2. **使用外部密钥管理**: AWS Secrets Manager、HashiCorp Vault
3. **定期轮换凭证**: 至少每季度一次
4. **最小权限原则**: API 凭证只授予必要权限

---

## 8. 迁移指南

### 8.1 从旧配置迁移

**旧格式 (v0.3.x):**
```toml
[datasources.deribit]
client_id = "xxx"
client_secret = "xxx"
```

**新格式 (v0.4.0+):**
```toml
# config.toml
[clients.deribit]
ws_url = "wss://www.deribit.com/ws/api/v2"
# 凭证通过环境变量注入

# .env
DERIBIT_CLIENT_ID="xxx"
DERIBIT_CLIENT_SECRET="xxx"
```

### 8.2 迁移步骤

1. 备份现有配置：`cp config.toml config.toml.backup`
2. 复制凭证到 `.env` 或 K8s Secrets
3. 更新配置文件移除敏感字段
4. 重启服务验证

---

## 9. 故障排查

### 9.1 凭证未生效

```bash
# 检查环境变量是否加载
echo $DERIBIT_CLIENT_ID

# K8s 检查 Secret 是否存在
kubectl get secret vol-monitor-secrets -n deribit

# 检查 Pod 环境变量
kubectl exec -it deployment/vol-monitor -n deribit -- env | grep DERIBIT
```

### 9.2 配置文件未找到

```bash
# 确认当前工作目录
pwd

# 使用绝对路径
./target/release/vol-monitor --config /absolute/path/to/config.toml
```

### 9.3 Feishu 通知不发送

1. 确认 Feishu 凭证正确
2. 确认 receive_id 格式正确（`oc_`、`ou_`或 `og_` 开头）
3. 确认机器人已添加到群聊
4. 检查日志中的错误信息

---

## 10. 文件清单

| 文件 | 状态 | 用途 |
|------|------|------|
| `config.toml` | ✅ 已更新 | 默认配置（环境变量模式） |
| `config.dev.toml` | ✅ 已创建 | 本地开发配置 |
| `config.prod.toml` | ✅ 已创建 | 生产配置 |
| `.env.example` | ✅ 已创建 | 环境变量模板 |
| `.gitignore` | ✅ 已更新 | 忽略.env 文件 |
| `scripts/run-dev.sh` | ✅ 已创建 | 开发启动脚本 |
| `k8s/secrets.yaml` | ✅ 已创建 | K8s Secret 模板 |
| `k8s/configmap.yaml` | ✅ 已更新 | 移除敏感信息 |
| `k8s/deployment.yaml` | ✅ 已更新 | 环境变量注入 |
| `docs/CONFIGURATION.md` | ✅ 已创建 | 用户文档 |
| `crates/vol-config/src/notification.rs` | ✅ 已更新 | Feishu 环境变量支持 |
| `crates/vol-monitor/src/main.rs` | ✅ 已更新 | 命令行参数支持 |

---

## 11. 后续改进

- [ ] 添加配置验证命令 `vol-monitor --validate-config`
- [ ] 集成 sealed-secrets for K8s
- [ ] 添加配置热重载测试
- [ ] 文档：CI/CD 配置注入流程
