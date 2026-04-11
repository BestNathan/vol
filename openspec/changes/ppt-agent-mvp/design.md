## Context

当前 Deribit 监控系统已具备：
- 实时市场数据监控（vol-engine）
- 告警规则匹配（vol-alert）
- 文字分析建议（AdviceAgent via vol-llm-agents）
- 多通道通知（Feishu, stdout）

**缺失能力**: 可视化报告生成。用户需要手动制作 PPT 进行汇报，无法将分析结果直接转化为可分享的演示文稿。

**约束条件**:
- Rust 技术栈
- 复用现有 `vol-llm-provider` 多模态 LLM 能力
- 独立于现有 ReActAgent，不影响 AdviceAgent/QaAgent
- MVP 优先，快速验证

## Goals / Non-Goals

**Goals:**
- 支持文字描述生成 PPTX 报告
- 基础模板系统（3-5 个预定义模板）+ 智能匹配
- LLM 驱动的大纲和内容生成
- CLI 工具 `ppt-agent generate`
- 独立 Agent 架构，可独立演进

**Non-Goals:**
- 多模态图片理解（MVP 后迭代）
- 复杂图表生成（柱状图、折线图等）
- HTTP API 服务
- 用户自定义模板上传界面
- 实时协作编辑

## Decisions

### 1. PPTX 生成库选型

| 选项 | 优点 | 缺点 | 选择 |
|------|------|------|------|
| `ppt-rs` | 2025 年 12 月更新，作者声称"真正能用"，支持读写、图表 | 相对较新，社区较小 | ✅ 首选 |
| `pptx` | 2026 年 2 月更新，纯 Rust | 功能可能不如 ppt-rs 完整 | 备选 |
| Python FFI | `python-pptx` 成熟 | 增加部署复杂度，需要 Python 环境 | ❌ MVP 不考虑 |

**Rationale**: MVP 优先快速验证，`ppt-rs` 功能完备且最近活跃更新。

### 2. 多模态支持策略

| 阶段 | 能力 |
|------|------|
| MVP | 仅文字输入 |
| v1.1 | 图片输入 + Qwen-VL 理解 |
| v1.2 | 数据图表自动生成 |

**Rationale**: 多模态增加复杂度，MVP 聚焦核心文字生成能力。

### 3. 模板系统设计

**混合模式**:
1. LLM 分析用户描述，提取场景特征（occasion, industry, style, audience）
2. 基于特征过滤候选模板
3. 向量相似度排序（embedding 语义搜索）
4. 返回 Top 1 + 备选

**Rationale**: 纯 LLM 推荐可能不稳定，纯向量匹配缺乏可解释性。混合模式兼顾两者。

### 4. Agent 架构

**独立 PptAgent**，不继承 ReActAgent：
- 复用 `vol-llm-agent` 的 Plugin 系统、Session 管理
- 自定义生成流程（5 阶段：分析→大纲→模板→内容→渲染）
- 独立工具集

**Rationale**: PPT 生成流程与标准 ReAct 循环差异较大，独立实现更灵活。

### 5. 输出粒度

**MVP**: 每页生成
- 标题
- 要点列表（bullet points）
- 建议布局类型

**v1.1+**: 增加
- 图片占位符
- 图表建议
- 演讲者备注

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| `ppt-rs` 库不成熟，关键功能缺失 | 高 | 备选 `pptx` crate，或降级为 Markdown 输出 |
| LLM 生成内容质量不稳定 | 中 | 增加用户确认环节，提供编辑能力 |
| 模板匹配不准确 | 低 | 允许用户手动指定模板，提供预览 |
| 多模态图片处理复杂度高 | 中 | MVP 阶段暂缓，先做文字输入 |
| 大文件生成内存占用高 | 低 | 流式写入，分批处理 |

## Migration Plan

**部署步骤**:
1. 添加 `ppt-rs` 依赖
2. 创建 `ppt` 模块（不影响现有代码）
3. 预定义 3-5 个基础模板
4. 实现 CLI 工具
5. 测试验证

**无迁移需求**: 独立新功能，不影响现有系统。

**回滚策略**: 未使用则无回滚需求。

## Open Questions

1. **CLI 集成方式**: 独立命令 `ppt-agent` vs 子命令 `vol-monitor ppt`？
2. **模板存储位置**: 代码内嵌 vs 外部 YAML 文件 vs 运行时加载？
3. **输出路径**: 固定目录 vs 用户指定 vs 临时文件？
