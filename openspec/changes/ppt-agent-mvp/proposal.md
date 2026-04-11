## Why

为 Deribit 监控系统添加 PPT 报告自动生成能力，让用户可以通过文字描述或多模态输入（文字 + 图片）快速生成专业的 PowerPoint 报告。当前系统能够生成文字分析建议（AdviceAgent），但缺乏可视化报告输出能力，用户需要手动制作 PPT 进行汇报。

## What Changes

- **新增 `vol-llm-agents/src/ppt/` 模块**：独立的 PPT Agent 实现，不影响现有 ReActAgent 架构
- **新增 PPT 生成工具链**：基于 `ppt-rs` 库实现 `.pptx` 文件生成
- **新增多模态输入支持**：支持文字 + 图片输入，使用 Qwen-VL 进行图片理解
- **新增智能模板匹配系统**：LLM 分析场景 + 向量相似度匹配，推荐合适模板
- **新增 CLI 工具**：`ppt-agent generate` 命令，支持文字、图片、模板选择
- **新增 5 个 Agent 工具**：OutlineGenerator、ContentGenerator、TemplateMatcher、ImageAnalyzer、PptxRenderer

## Capabilities

### New Capabilities
- `ppt-agent-core`: PPT Agent 核心实现，包括输入处理、流程编排、输出渲染
- `ppt-template-system`: 模板定义、注册、智能匹配（LLM 推荐 + 向量相似度）
- `ppt-multimodal`: 多模态输入处理，图片理解与内容提取
- `pptx-renderer`: 基于 ppt-rs 的 PPTX 文件生成器
- `ppt-tools`: PPT 生成专用工具集（大纲、内容、模板、图片、渲染）

### Modified Capabilities
<!-- 无修改的现有能力，PPT Agent 完全独立 -->

## Impact

- **新增依赖**：`ppt-rs` crate（PPTX 生成）
- **复用现有基础设施**：`vol-llm-provider`（多模态 LLM 接入）、`vol-llm-agent`（Plugin 系统）
- **新增 CLI 入口**：可扩展为主命令或独立子命令
- **不影响现有 Agent**：AdviceAgent、QaAgent 保持不变
