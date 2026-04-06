# CLAUDE.md 配置文档重构设计

**日期:** 2026-04-06
**状态:** 实施中

---

## 1. 概述

### 目标

将 CLAUDE.md 中的配置相关内容进行整合，大块的配置描述和配置清单移至 docs 目录，CLAUDE.md 只保留关键约束和常用命令，通过引用指向完整文档。

### 设计原则

| 原则 | 说明 |
|------|------|
| **CLAUDE.md 极简** | 只保留 3 步快速启动、核心环境变量、常用命令 |
| **docs 权威** | docs/CONFIGURATION.md 作为完整配置文档 |
| **不重复** | 相同内容只在一处维护 |
| **易查找** | CLAUDE.md 提供清晰的文档索引 |

---

## 2. CLAUDE.md 结构

### 保留内容（约 40 行）

```markdown
## Configuration

⚠️ **Credentials must be injected via environment variables. Never commit secrets.**

### Quick Start
```bash
cp .env.example .env && vim .env
./scripts/run-dev.sh dev
```

### Required Environment Variables
| Variable | Purpose |
|----------|---------|
| `DERIBIT_CLIENT_ID` | Deribit API client ID |
| `DERIBIT_CLIENT_SECRET` | Deribit API client secret |
| `FEISHU_APP_ID` | Feishu app ID |
| `FEISHU_APP_SECRET` | Feishu app secret |
| `FEISHU_RECEIVE_ID` | Feishu recipient ID |

### Common Commands
```bash
# Run with config
./target/release/vol-monitor --config config.dev.toml

# Kubernetes deploy
kubectl apply -f k8s/

# Restart deployment
kubectl -n deribit rollout restart deployment/vol-monitor
```

### Full Documentation
- **Configuration Guide**: [docs/CONFIGURATION.md](docs/CONFIGURATION.md)
- **Design Document**: [docs/superpowers/specs/2026-04-06-config-separation-design.md](docs/superpowers/specs/2026-04-06-config-separation-design.md)
```

### 移除内容

- ❌ 配置结构 TOML 示例（25 行）
- ❌ Dev vs Prod 对比表（7 行）
- ❌ K8s Secret 创建详细步骤（10 行）
- ❌ Pod Spec YAML 示例（20 行）
- ❌ Management Commands 详细列表（20 行）
- ❌ Migration from v0.4.x 指南（20 行）

---

## 3. docs/CONFIGURATION.md 结构

### 保留并优化的内容

| 章节 | 说明 |
|------|------|
| Overview | 配置文件总览表 |
| Quick Start | 本地开发、K8s 部署详细步骤 |
| Configuration Files | config.dev.toml、config.prod.toml 详解 |
| Environment Variables | 完整表（Required + Optional） |
| Security Considerations | 敏感数据处理、Git 安全 |
| Configuration Differences | 冷却时间、告警阈值、日志对比表 |
| Troubleshooting | 常见问题排查 |
| Migration from v0.3.x | 迁移指南 |

---

## 4. 实施步骤

1. 备份当前 CLAUDE.md
2. 替换 Configuration 部分为极简版本
3. 验证 docs/CONFIGURATION.md 内容完整
4. 提交更改

---

## 5. 预期效果

### Before
- CLAUDE.md Configuration 部分：~230 行
- 与 docs/CONFIGURATION.md 大量重复
- 维护成本高

### After
- CLAUDE.md Configuration 部分：~40 行
- 内容清晰分离，无重复
- 快速参考 + 完整文档双层结构
