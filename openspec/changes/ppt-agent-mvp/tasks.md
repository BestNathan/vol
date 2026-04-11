## 1. 项目设置与依赖

- [ ] 1.1 添加 `ppt-rs` 依赖到 `crates/vol-llm-agents/Cargo.toml`
- [ ] 1.2 创建 `crates/vol-llm-agents/src/ppt/` 目录结构
- [ ] 1.3 创建 `mod.rs` 导出公共类型
- [ ] 1.4 更新 `crates/vol-llm-agents/src/lib.rs` 导出 ppt 模块

## 2. 类型定义与配置

- [ ] 2.1 实现 `types.rs`：定义 `PptInput`, `PptOutput`, `Slide`, `SlideLayout` 等类型
- [ ] 2.2 实现 `config.rs`：定义 `PptAgentConfig`
- [ ] 2.3 实现 `prompt.rs`：定义 System Prompt 和 User Prompt 模板

## 3. 模板系统

- [ ] 3.1 定义模板 YAML 格式（`template.rs`）
- [ ] 3.2 实现 `TemplateRegistry` 加载与管理
- [ ] 3.3 创建 3 个预定义模板（商务正式、简洁科技、学术报告）
- [ ] 3.4 实现 LLM 驱动的场景分析（`extract_template_criteria`）
- [ ] 3.5 实现向量相似度匹配（可选，MVP 可简化为规则匹配）

## 4. PPTX 渲染器

- [ ] 4.1 实现 `renderer.rs`：封装 `ppt-rs` 库
- [ ] 4.2 实现封面页生成（标题、副标题、日期）
- [ ] 4.3 实现目录页生成
- [ ] 4.4 实现内容页生成（标题 + 要点列表）
- [ ] 4.5 实现模板样式应用（配色、字体）

## 5. 工具集实现

- [ ] 5.1 实现 `tools/outline.rs`：`OutlineGeneratorTool`
- [ ] 5.2 实现 `tools/content.rs`：`ContentGeneratorTool`
- [ ] 5.3 实现 `tools/template.rs`：`TemplateMatcherTool`
- [ ] 5.4 实现 `tools/renderer.rs`：`PptxRendererTool`
- [ ] 5.5 注册工具到 `ToolRegistry`

## 6. PptAgent 核心

- [ ] 6.1 实现 `agent.rs`：`PptAgent` 主结构
- [ ] 6.2 实现 `service.rs`：生成流程编排（分析→大纲→模板→内容→渲染）
- [ ] 6.3 实现输入验证与错误处理
- [ ] 6.4 集成 LLM 调用（大纲生成、内容生成）

## 7. CLI 工具

- [ ] 7.1 添加 `ppt-agent` 子命令到主程序或创建独立 binary
- [ ] 7.2 实现 `generate` 命令（`--text`, `--template`, `--output` 参数）
- [ ] 7.3 实现 `templates list` 命令
- [ ] 7.4 实现 `templates preview` 命令

## 8. 测试与验证

- [ ] 8.1 编写单元测试（工具函数、模板匹配）
- [ ] 8.2 编写集成测试（端到端 PPT 生成）
- [ ] 8.3 手动验证：生成实际 `.pptx` 文件并用 PowerPoint 打开
- [ ] 8.4 编写文档（README、使用示例）
