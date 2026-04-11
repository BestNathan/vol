## ADDED Requirements

### Requirement: PPT Agent Core Implementation
系统应实现 PPT Agent 核心功能，包括输入处理、流程编排和输出渲染。

#### Scenario: 用户通过文字描述生成 PPT
- **WHEN** 用户提供文字描述（如"做一个期权周报，包含 IV 分析、RV 分析、交易建议"）
- **THEN** 系统生成包含封面、目录、内容页的完整 PPTX 文件

#### Scenario: 用户指定输出路径
- **WHEN** 用户指定输出文件路径
- **THEN** 系统将生成的 PPTX 文件保存到指定路径

---

### Requirement: Template System
系统应实现模板注册表、模板定义格式和智能匹配能力。

#### Scenario: 预定义模板加载
- **WHEN** PPT Agent 启动时
- **THEN** 系统加载所有预定义模板（至少 3 个：商务正式、简洁科技、学术报告）

#### Scenario: 模板智能匹配
- **WHEN** 用户提供文字描述但未指定模板
- **THEN** 系统通过 LLM 分析 + 向量相似度匹配推荐最佳模板

#### Scenario: 用户手动指定模板
- **WHEN** 用户通过 `--template` 参数指定模板 ID
- **THEN** 系统使用用户指定的模板，跳过智能匹配

---

### Requirement: Multimodal Input Support (文字输入)
MVP 阶段支持纯文字输入。

#### Scenario: 纯文字输入处理
- **WHEN** 用户仅提供文字描述
- **THEN** 系统正常处理并生成 PPT

---

### Requirement: PPTX Renderer
系统应实现基于 `ppt-rs` 库的 PPTX 文件生成器。

#### Scenario: 生成封面页
- **WHEN** 生成 PPT 时
- **THEN** 系统创建包含标题、副标题、日期的封面页

#### Scenario: 生成内容页
- **WHEN** 生成 PPT 时
- **THEN** 系统创建包含标题和要点列表的内容页

#### Scenario: 生成目录页
- **WHEN** 生成 PPT 时
- **THEN** 系统创建包含章节标题的目录页

---

### Requirement: PPT Tools
系统应实现 5 个 PPT 生成专用工具。

#### Scenario: OutlineGeneratorTool 生成大纲
- **WHEN** Agent 调用 OutlineGeneratorTool
- **THEN** 工具返回层级化的 PPT 大纲结构

#### Scenario: ContentGeneratorTool 生成内容
- **WHEN** Agent 调用 ContentGeneratorTool 并传入大纲节点
- **THEN** 工具返回该节点的详细内容（标题、要点）

#### Scenario: TemplateMatcherTool 匹配模板
- **WHEN** Agent 调用 TemplateMatcherTool 并传入用户描述
- **THEN** 工具返回匹配的模板 ID 和备选列表

#### Scenario: PptxRendererTool 渲染文件
- **WHEN** Agent 调用 PptxRendererTool 并传入幻灯片数据
- **THEN** 工具生成 .pptx 文件并返回文件路径
