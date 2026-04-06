# CLAUDE.md 与文档结构重构设计

**日期:** 2026-04-06
**状态:** 实施中

---

## 1. 概述

### 目标

将 CLAUDE.md 重构为项目入口文档，采用分层结构：CLAUDE.md 作为快速参考和文档索引，详细内容移至 docs/ 目录结构化存储。

### 设计原则

| 原则 | 说明 |
|------|------|
| **CLAUDE.md 精简** | ~100 行，作为项目入口和快速参考 |
| **docs/ 结构化** | 按主题组织：architecture/、deployment/、integration/ |
| **链接清晰** | CLAUDE.md 提供文档索引，指向详细内容 |
| **渐进迁移** | 先创建 docs 结构，再移动内容 |

---

## 2. CLAUDE.md 新结构

### 保留内容（~100 行）

```markdown
# CLAUDE.md

## Quick Start
- 3 步启动开发环境
- 核心环境变量表

## Project Structure
- 根目录结构图
- crates/ 目录概览表

## Architecture Overview
- 系统架构图（ASCII）
- 数据流图
- 关键设计模式

## Common Commands
- Build & Test
- Run locally
- Deploy to K8s

## Documentation Index
| Topic | Location |
|-------|----------|
| Configuration | docs/CONFIGURATION.md |
| Architecture | docs/architecture/overview.md |
| Docker Build | docs/deployment/docker-build.md |
| K8s Deployment | docs/deployment/k8s-deployment.md |
| Deribit API | docs/integration/deribit.md |
| Tracing | docs/tracing.md |
```

### 移至 docs/ 的内容

| 内容 | 目标位置 | 状态 |
|------|----------|------|
| Docker Build 完整流程 | docs/deployment/docker-build.md | 待创建 |
| K8s 部署详细步骤 | docs/deployment/k8s-deployment.md | 待创建 |
| Deribit 集成细节 | docs/integration/deribit.md | 待创建 |
| Workspace/Crates 详解 | docs/architecture/crates.md | 待创建 |
| Common Modifications | docs/development/common-modifications.md | 待创建 |

---

## 3. docs/ 目录结构

### 新建目录

```
docs/
├── architecture/          # 新目录
│   ├── overview.md        # 系统架构
│   ├── data-flow.md       # 数据流
│   └── crates.md          # Workspace 详解
├── deployment/            # 新目录
│   ├── docker-build.md    # Docker 构建
│   ├── k8s-deployment.md  # K8s 部署
│   └── local-dev.md       # 本地开发
├── integration/           # 新目录
│   └── deribit.md         # Deribit 集成
├── development/           # 新目录
│   └── common-modifications.md  # 常见修改
├── deribit/               # 保留（现有 API 文档）
├── superpowers/           # 保留（OpenSpec 工件）
├── CONFIGURATION.md       # 保留（配置文档）
└── tracing.md             # 保留（追踪文档）
```

### 文档内容来源

| 新文档 | 内容来源 |
|--------|----------|
| architecture/overview.md | CLAUDE.md Architecture Overview + 扩展 |
| architecture/crates.md | CLAUDE.md Workspace Structure + vol-tracing/vol-engine |
| deployment/docker-build.md | CLAUDE.md Docker Build（完整保留） |
| deployment/k8s-deployment.md | CLAUDE.md K8s + docs/CONFIGURATION.md K8s 部分 |
| integration/deribit.md | CLAUDE.md Deribit Integration + vol-deribit 模块说明 |
| development/common-modifications.md | CLAUDE.md Common Modifications |

---

## 4. 实施步骤

### Phase 1: 创建 docs 目录结构
1. 创建 architecture/、deployment/、integration/、development/ 目录
2. 迁移现有文档到对应位置

### Phase 2: 创建新文档
1. 从 CLAUDE.md 移动内容到新文档
2. 补充缺失的信息（vol-tracing、vol-engine 等）

### Phase 3: 重构 CLAUDE.md
1. 替换详细内容为文档索引
2. 添加项目目录结构图
3. 保留快速参考命令

### Phase 4: 验证
1. 检查所有链接有效
2. 验证文档完整性

---

## 5. 预期效果

### CLAUDE.md 前后对比

| 指标 | 之前 | 之后 |
|------|------|------|
| 总行数 | ~363 | ~100 |
| Architecture 章节 | 46 行简略 | 链接到 docs/architecture/ |
| Docker Build | 67 行详细 | 链接到 docs/deployment/ |
| Deribit Integration | 50 行模块细节 | 链接到 docs/integration/ |

### docs/ 结构

| 目录 | 文件数 | 说明 |
|------|--------|------|
| architecture/ | 3 | 系统架构、数据流、Crate 详解 |
| deployment/ | 3 | Docker、K8s、本地开发 |
| integration/ | 1 | Deribit 集成 |
| development/ | 1 | 常见修改指南 |
| deribit/ | (现有) | API 参考文档 |
| superpowers/ | (现有) | OpenSpec 工件 |
| 单文件 | CONFIGURATION.md, tracing.md | 配置、追踪 |

---

## 6. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 链接断裂 | 实施后全面检查所有内部链接 |
| 内容丢失 | 迁移前后对比行数/内容 |
| 历史提交记录 | 使用 git mv 保留历史 |
