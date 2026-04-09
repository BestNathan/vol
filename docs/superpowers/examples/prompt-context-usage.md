# Prompt Context 使用示例

本文档提供 `vol-llm-agent` 的 `prompt_context` 模块的使用示例。

## 目录

- [基础用法](#基础用法)
- [RAG 场景](#rag-场景)
- [多轮对话](#多轮对话)
- [缓存优化](#缓存优化)

---

## 基础用法

### 1. 定义模板和片段

```rust
use vol_llm_agent::prompt_context::{
    PromptContext, PromptTemplate, PromptFragment, FragmentType,
};
use vol_llm_core::ToolDefinition;

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

// 2. 定义工具清单
let tool_definitions = vec![
    ToolDefinition {
        name: "get_volatility_surface".to_string(),
        description: Some("获取期权波动率曲面数据".to_string()),
        parameters: None,
    },
    ToolDefinition {
        name: "calculate_greeks".to_string(),
        description: Some("计算期权 Greeks 参数".to_string()),
        parameters: None,
    },
];

// 3. 创建上下文并添加片段
let prompt_ctx = PromptContext::new(template)
    .with_fragment(PromptFragment::new(
        "role",
        "衍生品市场风险分析师",
        FragmentType::Role,
    ))
    .with_tools(&tool_definitions)
    .with_fragment(PromptFragment::new(
        "rules",
        "只基于事实回答，不编造信息。引用数据时注明来源。",
        FragmentType::Rules,
    ))
    .with_fragment(PromptFragment::new(
        "format",
        "使用 Markdown 格式输出。关键数据使用表格展示。",
        FragmentType::Format,
    ));

// 4. 获取 System 消息
let system_message = prompt_ctx.build_system();
println!("{}", system_message);
```

输出:
```
你是一名衍生品市场风险分析师。

## 可用工具
- `get_volatility_surface`: 获取期权波动率曲面数据
- `calculate_greeks`: 计算期权 Greeks 参数

## 行为准则
只基于事实回答，不编造信息。引用数据时注明来源。

## 输出格式
使用 Markdown 格式输出。关键数据使用表格展示。
```

### 2. 创建 Agent

```rust
use vol_llm_agent::{ReActAgent, AgentConfig};
use vol_llm_agent::session::{Session, InMemorySessionStore, InMemoryMessageStore};
use std::sync::Arc;

// 创建 Session
let session_store = Arc::new(InMemorySessionStore::new());
let message_store = Arc::new(InMemoryMessageStore::new());
let session = Arc::new(Session::new(
    "session-1".to_string(),
    session_store,
    message_store,
));

// 创建 Agent 配置
let config = AgentConfig {
    // 注意：当前 AgentConfig 可能还没有 prompt_context 字段
    // 这是未来的集成方式
    ..Default::default()
};

// 创建 Agent
let agent = ReActAgent::builder()
    .with_llm(llm_client)
    .with_tools(tool_definitions)
    .with_session(session)
    .build()
    .unwrap();

// 使用 prompt_ctx 构建消息
use vol_llm_agent::prompt_context::MessageAssembler;

let messages = MessageAssembler::assemble(&prompt_ctx, "解释 IV 和 CV 的区别");
```

---

## RAG 场景

### 使用 RAG 检索增强生成

```rust
use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate, MessageAssembler};
use vol_llm_agent::rag::{Document, RagAgent};

// 1. 创建 Prompt 上下文
let template = PromptTemplate::new(
    "rag-assistant",
    "你是一个知识库助手。\n\n## 能力\n{capabilities}"
);

let prompt_ctx = PromptContext::new(template)
    .with_fragment(PromptFragment::new(
        "capabilities",
        "- 检索知识库文档\n- 基于检索内容回答问题",
        FragmentType::Tools,
    ));

// 2. 使用 RAG Agent 检索相关文档
let rag_agent = RagAgent::new(embedding_model, document_store);
let rag_docs = rag_agent.retrieve("期权波动率微笑").await?;

// 示例：手动创建检索结果
let rag_docs = vec![
    Document::new(
        "波动率微笑是指 IV 随执行价格变化呈现微笑形状的现象。".to_string()
    )
    .with_metadata("source", "options_theory")
    .with_score(0.92),
    
    Document::new(
        "深度实值和深度虚值期权的 IV 通常高于平值期权。".to_string()
    )
    .with_metadata("source", "volatility_trading")
    .with_score(0.87),
];

// 3. 组装消息
let messages = MessageAssembler::assemble_with_rag(
    &prompt_ctx,
    "解释期权波动率微笑现象",
    &rag_docs,
);

// 4. 消息结构
// System: [固定模板 + 能力清单，缓存友好]
// User: [RAG 文档内容 + 用户问题，动态部分]
```

### 消息内容示例

**System 消息:**
```
你是一个知识库助手。

## 能力
- 检索知识库文档
- 基于检索内容回答问题
```

**User 消息:**
```
参考资料:
波动率微笑是指 IV 随执行价格变化呈现微笑形状的现象。

---

深度实值和深度虚值期权的 IV 通常高于平值期权。

问题：解释期权波动率微笑现象
```

---

## 多轮对话

### 基本多轮对话

```rust
use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate, MessageAssembler};
use vol_llm_core::Message;

// 创建 Prompt 上下文（固定）
let template = PromptTemplate::new("assistant", "你是一名有帮助的 AI 助手。");
let prompt_ctx = PromptContext::new(template);

// === 第一轮 ===
let msg1 = MessageAssembler::assemble(&prompt_ctx, "什么是 IV?");
// msg1[0]: System - "你是一名有帮助的 AI 助手。"
// msg1[1]: User - "问题：什么是 IV?"

let response1 = llm.converse(msg1).await?;
// response1: Assistant - "IV 是 Implied Volatility（隐含波动率）..."

// === 第二轮（携带历史）===
let history = vec![
    msg1[0].clone(),  // System
    msg1[1].clone(),  // User 第一轮
    response1.clone(), // Assistant 第一轮
];

let msg2 = MessageAssembler::assemble_with_history(&prompt_ctx, "那 CV 呢？", &history);
// msg2[0]: System - "你是一名有帮助的 AI 助手。" (只出现一次)
// msg2[1]: User - "问题：什么是 IV?" (历史)
// msg2[2]: Assistant - "IV 是 Implied Volatility..." (历史)
// msg2[3]: User - "问题：那 CV 呢？" (当前)

let response2 = llm.converse(msg2).await?;

// === 第三轮（继续累积历史）===
let history2 = msg2; // 包含之前的所有消息
let msg3 = MessageAssembler::assemble_with_history(&prompt_ctx, "如何交易波动率？", &history2);
```

### 多轮对话 + RAG

```rust
use vol_llm_agent::prompt_context::{PromptContext, PromptTemplate, MessageAssembler};
use vol_llm_agent::rag::Document;
use vol_llm_core::Message;

let template = PromptTemplate::new("rag-assistant", "你是 RAG 助手。\n\n## 工具\n{tools}");
let prompt_ctx = PromptContext::new(template)
    .with_fragment(PromptFragment::new("tools", "- 检索知识库", FragmentType::Tools));

// === 第一轮：RAG 查询 ===
let docs1 = vec![
    Document::new("Gamma 衡量 Delta 的变化率。".to_string()),
];

let msg1 = MessageAssembler::assemble_with_rag(&prompt_ctx, "什么是 Gamma?", &docs1);
let response1 = llm.converse(msg1).await?;

// === 第二轮：历史中自然携带 RAG 上下文 ===
let history = vec![
    msg1[0].clone(),  // System
    msg1[1].clone(),  // User (包含 RAG 内容)
    response1,        // Assistant
];

let msg2 = MessageAssembler::assemble_with_history(&prompt_ctx, "它和 Delta 有什么关系？", &history);
// 注意：第二轮不需要重新检索 RAG，因为历史中已包含第一轮的 RAG 上下文
// LLM 可以从历史中理解上下文

// === 第三轮：可选重新检索 ===
let docs3 = vec![
    Document::new("Vega 衡量对波动率变化的敏感性。".to_string()),
];

// 如果第三轮需要新的 RAG 内容，可以手动构建
let mut new_history = history.clone();
new_history.push(Message::assistant("Gamma 是 Delta 的变化率...".to_string()));
new_history.push(Message::user("问题：那 Vega 呢？".to_string()));

// 或者使用带 RAG 的历史构建（需要自定义实现）
```

---

## 缓存优化

### System 消息缓存策略

LLM API 通常支持基于 System 消息的缓存。使用 `prompt_context` 可以最大化缓存命中率。

| 场景 | System 内容 | 缓存命中 |
|------|------------|---------|
| 相同 Agent，相同工具 | 完全相同 | ✅ 命中 |
| 相同 Agent，不同工具 | 工具清单不同 | ❌ Miss |
| 不同 Agent | 模板不同 | ❌ Miss |

### 缓存标识计算

```rust
// cache_key = hash(template_id + template_content + all_fragments)
// 动态变量不计入 cache_key

let template = PromptTemplate::new("market-analyst", "你是一名{role}...");

let ctx1 = PromptContext::new(template.clone())
    .with_fragment(PromptFragment::new("role", "分析师", FragmentType::Role))
    .with_tools(&tools);

let ctx2 = PromptContext::new(template)
    .with_fragment(PromptFragment::new("role", "分析师", FragmentType::Role))
    .with_tools(&tools);

// 相同配置产生相同 cache_key
assert_eq!(ctx1.cache_key(), ctx2.cache_key());

// 动态变量不影响 cache_key
let ctx3 = ctx1.clone().with_dynamic("query", "不同的查询内容");
assert_eq!(ctx1.cache_key(), ctx3.cache_key());
```

### 缓存友好实践

1. **固定内容放 System**: 角色定义、工具清单、行为准则等固定内容应放在 System 消息
2. **动态内容放 User**: RAG 检索结果、用户查询等动态内容应放在 User 消息
3. **复用 PromptContext**: 多轮对话中复用同一个 `PromptContext` 实例
4. **避免频繁修改片段**: 片段内容改变会导致缓存失效

---

## API 参考

### PromptTemplate

```rust
// 创建模板
let template = PromptTemplate::new(id: &str, content: &str);

// 解析占位符
let injections = template.injections; // Vec<String>
```

### PromptFragment

```rust
// 创建片段
let fragment = PromptFragment::new(id: &str, content: &str, fragment_type: FragmentType);

// 从工具生成片段
let tools_fragment = PromptFragment::from_tools(&[ToolDefinition]);
```

### PromptContext

```rust
// 创建上下文
let ctx = PromptContext::new(template);

// 添加片段
let ctx = ctx
    .with_fragment(fragment)
    .with_tools(&tools)
    .with_dynamic(name, value);

// 构建消息
let system = ctx.build_system();
let user = ctx.build_user(query, rag_context);
let cache_key = ctx.cache_key();
```

### MessageAssembler

```rust
// 基础消息
let messages = MessageAssembler::assemble(&ctx, user_input);

// 带历史
let messages = MessageAssembler::assemble_with_history(&ctx, user_input, &history);

// 带 RAG
let messages = MessageAssembler::assemble_with_rag(&ctx, user_input, &rag_docs);
```

---

## 相关文档

- [系统设计文档](../specs/2026-04-09-system-prompt-context-design.md)
- [vol-llm-agent API 文档](../../crates/vol-llm-agent/README.md)
- [vol-llm-core API 文档](../../crates/vol-llm-core/README.md)
