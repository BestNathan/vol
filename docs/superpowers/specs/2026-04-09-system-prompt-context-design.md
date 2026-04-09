# System Prompt Context 组件设计

**日期：** 2026-04-09
**作者：** Claude Code
**状态：** 设计完成，待实现

---

## 概述

设计一个系统提示词上下文组件，支持模块化拼装和复用，最大化 LLM 服务端缓存命中率，降低成本支出。

### 核心目标

1. **缓存友好** - 固定内容放在 System 消息，最大化命中 LLM 缓存
2. **模块化拼装** - 支持模板、工具清单、规则等组件化定义和注入
3. **上下文管控** - 管理历史消息长度，控制 token 消耗
4. **灵活扩展** - 支持不同 Agent 类型的提示词需求

---

## 架构设计

### 核心组件

```
┌────────────────────────────────────────────────────────────────┐
│                      PromptContext                             │
│  (提示词上下文管理器 - 管理固定片段和动态注入)                    │
├────────────────────────────────────────────────────────────────┤
│  - templates: HashMap<String, Template>                        │
│  - fragments: HashMap<String, String>                          │
│  - cache_key: String                                           │
├────────────────────────────────────────────────────────────────┤
│  + with_template(name, content) -> Self                        │
│  + with_fragment(name, content) -> Self                        │
│  + with_tools(tools) -> Self                                   │
│  + build_system() -> String                                    │
│  + build_user(context) -> String                               │
│  + cache_key() -> String                                       │
└────────────────────────────────────────────────────────────────┘
                              │
                              ↓
┌────────────────────────────────────────────────────────────────┐
│                    MessageAssembler                            │
│  (消息组装器 - 将拼装后的内容放到正确的消息位置)                   │
├────────────────────────────────────────────────────────────────┤
│  + assemble(prompt_ctx, user_input) -> Vec<Message>            │
│  + assemble_with_history(prompt_ctx, user_input, history)      │
└────────────────────────────────────────────────────────────────┘
```

### 数据流

```
用户定义模板 ──→ PromptContext ──→ MessageAssembler ──→ Vec<Message>
     ↓              ↓                    ↓
  固定片段      动态注入            System/User 分离
  (tools)      (RAG, query)          (缓存优化)
```

---

## 组件详细设计

### 1. PromptTemplate - 模板定义

```rust
/// 提示词模板 - 用户定义的完整模板，支持命名注入点
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    /// 模板唯一标识（用于缓存）
    pub id: String,
    
    /// 模板内容，支持 {name} 占位符
    pub content: String,
    
    /// 定义的注入点名称列表
    pub injections: Vec<String>,
}

impl PromptTemplate {
    /// 从字符串创建模板，自动解析占位符
    pub fn new(id: &str, content: &str) -> Self {
        let injections = Self::parse_injection_points(content);
        Self {
            id: id.to_string(),
            content: content.to_string(),
            injections,
        }
    }

    /// 从文件加载模板
    pub fn from_file(id: &str, path: &str) -> Result<Self, TemplateError> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::new(id, &content))
    }

    fn parse_injection_points(content: &str) -> Vec<String> {
        // 解析 {name} 格式的占位符
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        re.captures_iter(content)
            .map(|cap| cap[1].to_string())
            .collect()
    }
}
```

### 2. PromptFragment - 可复用片段

```rust
/// 提示词片段 - 可复用的内容块
#[derive(Debug, Clone)]
pub struct PromptFragment {
    pub id: String,
    pub content: String,
    /// 片段类型（用于分类管理）
    pub fragment_type: FragmentType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FragmentType {
    /// 角色定义
    Role,
    /// 工具清单
    Tools,
    /// 行为规则
    Rules,
    /// 输出格式
    Format,
    /// 自定义
    Custom,
}

impl PromptFragment {
    pub fn new(id: &str, content: &str, fragment_type: FragmentType) -> Self {
        Self {
            id: id.to_string(),
            content: content.to_string(),
            fragment_type,
        }
    }

    /// 从工具定义自动生成工具清单片段
    pub fn from_tools(tools: &[ToolDefinition]) -> Self {
        let content = tools
            .iter()
            .map(|t| format!("- `{}`: {}", t.name, t.description.as_deref().unwrap_or("无描述")))
            .collect::<Vec<_>>()
            .join("\n");
        Self::new("tools", &content, FragmentType::Tools)
    }
}
```

### 3. PromptContext - 上下文管理器

```rust
/// 提示词上下文管理器
/// 
/// 管理固定片段和动态注入，生成缓存友好的提示词
pub struct PromptContext {
    /// 主模板
    template: PromptTemplate,
    
    /// 已注册的片段（固定内容）
    fragments: HashMap<String, PromptFragment>,
    
    /// 动态内容（每轮不同）
    dynamic_vars: HashMap<String, String>,
    
    /// 缓存标识（基于固定内容计算）
    cache_key: String,
}

impl PromptContext {
    pub fn new(template: PromptTemplate) -> Self {
        let cache_key = Self::compute_cache_key(&template, &HashMap::new());
        Self {
            template,
            fragments: HashMap::new(),
            dynamic_vars: HashMap::new(),
            cache_key,
        }
    }

    /// 添加固定片段
    pub fn with_fragment(mut self, fragment: PromptFragment) -> Self {
        let id = fragment.id.clone();
        self.fragments.insert(id, fragment);
        self.recompute_cache_key();
        self
    }

    /// 添加工具清单
    pub fn with_tools(mut self, tools: &[ToolDefinition]) -> Self {
        let fragment = PromptFragment::from_tools(tools);
        self.fragments.insert(fragment.id.clone(), fragment);
        self.recompute_cache_key();
        self
    }

    /// 设置动态变量（不计入缓存 key）
    pub fn with_dynamic(mut self, name: &str, value: &str) -> Self {
        self.dynamic_vars.insert(name.to_string(), value.to_string());
        self
    }

    /// 构建 System 消息内容（仅固定内容，缓存友好）
    pub fn build_system(&self) -> String {
        let mut content = self.template.content.clone();
        
        // 替换固定片段
        for (id, fragment) in &self.fragments {
            content = content.replace(&format!("{{{}}}", id), &fragment.content);
        }
        
        // 替换动态变量（如果未提供则留空或默认值）
        for injection in &self.template.injections {
            if !self.fragments.contains_key(injection) {
                let placeholder = format!("{{{}}}", injection);
                if content.contains(&placeholder) {
                    let dynamic_value = self.dynamic_vars.get(injection).cloned().unwrap_or_default();
                    content = content.replace(&placeholder, &dynamic_value);
                }
            }
        }
        
        content
    }

    /// 构建 User 消息内容（动态内容）
    pub fn build_user(&self, query: &str, rag_context: Option<&str>) -> String {
        let mut parts = Vec::new();
        
        // RAG 上下文（如果有）
        if let Some(ctx) = rag_context {
            parts.push(format!("参考资料:\n{}\n", ctx));
        }
        
        // 用户问题
        parts.push(format!("问题：{}", query));
        
        parts.join("\n\n")
    }

    /// 获取缓存标识（用于 LLM 缓存命中）
    pub fn cache_key(&self) -> &str {
        &self.cache_key
    }

    fn recompute_cache_key(&mut self) {
        self.cache_key = Self::compute_cache_key(&self.template, &self.fragments);
    }

    fn compute_cache_key(template: &PromptTemplate, fragments: &HashMap<String, PromptFragment>) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        template.id.hash(&mut hasher);
        template.content.hash(&mut hasher);
        
        // 按 ID 排序后 hash，保证一致性
        let mut ids: Vec<_> = fragments.keys().collect();
        ids.sort();
        for id in ids {
            id.hash(&mut hasher);
            fragments[id].content.hash(&mut hasher);
        }
        
        format!("prompt_{}", hasher.finish())
    }
}
```

### 4. MessageAssembler - 消息组装器

```rust
/// 消息组装器
/// 
/// 将提示词上下文转换为正确的消息格式
pub struct MessageAssembler;

impl MessageAssembler {
    /// 组装基础消息（System + User）
    pub fn assemble(ctx: &PromptContext, user_input: &str) -> Vec<Message> {
        vec![
            Message::system(ctx.build_system()),
            Message::user(user_input.to_string()),
        ]
    }

    /// 组装带历史消息的对话
    pub fn assemble_with_history(
        ctx: &PromptContext,
        user_input: &str,
        history: &[Message],
    ) -> Vec<Message> {
        let mut messages = Vec::new();
        
        // System 消息（只放一次）
        messages.push(Message::system(ctx.build_system()));
        
        // 历史消息（受 max_history_messages 限制）
        messages.extend_from_slice(history);
        
        // 当前 User 消息（包含 RAG 上下文）
        let user_msg = ctx.build_user(user_input, None);
        messages.push(Message::user(user_msg));
        
        messages
    }

    /// 组装带 RAG 上下文的查询
    pub fn assemble_with_rag(
        ctx: &PromptContext,
        user_input: &str,
        rag_docs: &[Document],
    ) -> Vec<Message> {
        let rag_context = rag_docs
            .iter()
            .map(|d| d.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        
        let user_msg = ctx.build_user(user_input, Some(&rag_context));
        
        vec![
            Message::system(ctx.build_system()),
            Message::user(user_msg),
        ]
    }
}
```

### 5. Agent 集成

```rust
// AgentConfig 支持 PromptContext
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    
    // 新增：提示词上下文管理
    pub prompt_context: PromptContext,
    
    pub plugin_registry: PluginRegistry,
}

// Agent 内部使用
impl ReActAgent {
    pub async fn run(&self, user_input: &str, context: ToolContext) -> Result<...> {
        // 使用 PromptContext 组装消息
        let system_prompt = self.config.prompt_context.build_system();
        let user_msg = self.config.prompt_context.build_user(user_input, None);
        
        messages.push(Message::system(system_prompt));
        messages.push(Message::user(user_msg));
        
        // ... 后续处理
    }
}
```

---

## 使用示例

### 示例 1：基础用法

```rust
// 1. 定义模板
let template = PromptTemplate::new(
    "market-analyst",
    r#"你是一名{role}。

## 可用工具
{tools}

## 行为准则
{rules}

## 输出格式
{format}"#
);

// 2. 创建上下文并添加片段
let prompt_ctx = PromptContext::new(template)
    .with_fragment(PromptFragment::new("role", "衍生品市场风险分析师", FragmentType::Role))
    .with_tools(&tool_definitions)
    .with_fragment(PromptFragment::new("rules", "只基于事实回答，不编造信息", FragmentType::Rules))
    .with_fragment(PromptFragment::new("format", "使用 Markdown 格式输出", FragmentType::Format));

// 3. 使用上下文创建 Agent
let config = AgentConfig {
    prompt_context: prompt_ctx,
    ..Default::default()
};

let agent = ReActAgent::new(llm, tools, config, session);
```

### 示例 2：RAG 场景

```rust
// RAG Agent 使用
let rag_docs = rag_agent.retrieve("期权波动率微笑").await?;

let messages = MessageAssembler::assemble_with_rag(
    &prompt_ctx,
    "解释期权波动率微笑现象",
    &rag_docs,
);

// System: [固定模板 + 工具清单，缓存友好]
// User: [RAG 文档内容 + 用户问题，动态部分]
```

### 示例 3：多轮对话

```rust
// 第一轮
let msg1 = MessageAssembler::assemble_with_history(&ctx, "什么是 IV?", &[]);
let response1 = llm.converse(msg1).await?;

// 第二轮（携带历史）
let history = vec![msg1[0].clone(), msg1[1].clone(), response1.clone()];
let msg2 = MessageAssembler::assemble_with_history(&ctx, "那 CV 呢？", &history);
// System 消息只出现一次，历史消息自然传递 RAG 上下文
```

---

## 缓存优化策略

### System 消息缓存

| 场景 | System 内容 | 缓存命中 |
|------|------------|---------|
| 相同 Agent，相同工具 | 完全相同 | ✅ 命中 |
| 相同 Agent，不同工具 | 工具清单不同 | ❌ Miss |
| 不同 Agent | 模板不同 | ❌ Miss |

### 缓存标识计算

```rust
// cache_key = hash(template_id + template_content + all_fragments)
// 动态变量不计入 cache_key

cache_key = hash(
    "market-analyst" +
    "你是一名{role}..." +
    "tools:" + "1. get_weather..." +
    "rules:" + "只基于事实..."
)
```

---

## 测试计划

### 单元测试

1. `PromptTemplate::parse_injection_points` - 正确解析占位符
2. `PromptFragment::from_tools` - 从工具定义生成片段
3. `PromptContext::build_system` - 正确替换占位符
4. `PromptContext::cache_key` - 相同配置产生相同 key
5. `MessageAssembler::assemble` - 正确分离 System/User

### 集成测试

1. 缓存命中验证 - 相同配置调用两次，确认 cache_key 相同
2. RAG 场景 - 验证 RAG 内容在 User 消息，历史传递正确
3. 多轮对话 - 验证 System 只出现一次，历史消息累积

---

## 实施计划

### Task 1: 创建核心类型
- `PromptTemplate` - 模板定义和解析
- `PromptFragment` - 片段定义和类型
- `FragmentType` - 片段类型枚举

### Task 2: 实现 PromptContext
- 基础结构和方法
- 缓存 key 计算
- `build_system()` / `build_user()`

### Task 3: 实现 MessageAssembler
- 基础消息组装
- 带历史的组装
- RAG 场景支持

### Task 4: 集成到 AgentConfig
- 添加 `prompt_context` 字段
- 更新 `ReActAgent::run()` 使用新组件
- 更新 Builder

### Task 5: 集成到 RAG Agent
- 更新 `RagAgent::build_rag_prompt()` 使用新组件
- 验证 System/User 分离正确

### Task 6: 测试和文档
- 单元测试
- 集成测试
- 使用示例文档

---

## 设计决策记录

### 决策 1：System vs User 分离

**决策：** 固定内容 → System，动态内容 → User

**理由：**
- LLM 缓存通常基于 System 消息的稳定性
- RAG 内容每轮不同，放在 User 是合理的
- 历史消息自然传递 RAG 上下文

### 决策 2：占位符语法

**决策：** 使用 `{name}` 格式

**理由：**
- 简洁直观
- 与 Python f-string、Rust 格式化语法一致
- 易于解析和验证

### 决策 3：缓存 key 计算

**决策：** 仅基于固定内容（模板 + 片段），动态变量不计入

**理由：**
- 动态变量每轮不同，计入会导致缓存失效
- 固定内容相同就应该命中缓存
- 调用方可通过复用 PromptContext 实现缓存友好

---

## 验收标准

1. ✅ 支持模板定义和占位符注入
2. ✅ 支持工具清单自动注入
3. ✅ 支持自定义片段注入
4. ✅ System/User 正确分离
5. ✅ 缓存 key 计算正确（相同配置 = 相同 key）
6. ✅ RAG 场景支持（内容在 User 消息）
7. ✅ 多轮对话支持（历史消息传递）
8. ✅ 所有现有测试通过
