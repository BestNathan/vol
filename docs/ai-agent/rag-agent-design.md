# RAG Agent 设计与实现

## 概述

本文档描述了 RAG（Retrieval-Augmented Generation）智能体的设计与实现。RAG 能力作为独立模块实现在 `vol-llm-agent` crate 中，与 ReActAgent 并列，业务方按需使用。

## 架构设计

### 核心思想

RAG 是一种将检索增强与生成式 AI 结合的技术：
1. **检索（Retrieval）**：根据用户查询从知识库中检索相关文档
2. **增强（Augmented）**：将检索到的文档作为上下文注入 prompt
3. **生成（Generation）**：LLM 基于上下文生成准确的回答

### 架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    RagAgent                                 │
│                                                             │
│  - llm: Arc<dyn LLMClient>                                 │
│  - store: Arc<dyn EmbeddingStore>                          │
│  - embedder: Arc<dyn Embedder>                             │
│                                                             │
│  + retrieve(query) -> Vec<Document>                        │
│  + generate(query, docs) -> String                         │
│  + query(query) -> RagResponse                             │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐    ┌────────────────┐   ┌─────────────────┐
│ Embedder      │    │ EmbeddingStore │   │ LLMClient       │
│ (DashScope)   │    │ (InMemory)     │   │ (Anthropic)     │
└───────────────┘    └────────────────┘   └─────────────────┘
```

### 设计原则

1. **分离关注点**：`vol-llm-agent`（基础设施）与 `vol-llm-agents`（业务智能体）分离
2. **基于 trait 的抽象**：用户可实现 `Embedder`/`EmbeddingStore` 对接自定义后端
3. **灵活的 API**：提供分离的 `retrieve()` 和 `generate()` 方法，支持高级用法
4. **无额外依赖**：core 模块不引入 embedding 相关依赖，保持轻量

## 核心组件

### 1. Embedder Trait

位置：`crates/vol-llm-agent/src/rag/embedding.rs`

```rust
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}
```

**设计说明**：
- 异步 trait，支持 async/await
- Send + Sync 约束，支持多线程环境
- 提供单条和批量 embedding 生成接口
- 默认 `embed_batch` 实现为串行调用，可由实现者优化

### 2. DashScopeEmbedder

位置：`crates/vol-llm-agent/src/rag/dashscope_embedder.rs`

```rust
pub struct DashScopeEmbedder {
    client: Client,
    config: DashScopeConfig,
}

pub enum DashScopeModel {
    TextEmbeddingV2,  // 1536 dimensions
    TextEmbeddingV3,  // 1024 dimensions
}
```

**配置项**：
- `api_key`：DashScope API 密钥
- `model`：embedding 模型选择
- `base_url`：API 端点
- `timeout_secs`：请求超时

**使用方法**：
```rust
let embedder = DashScopeEmbedder::new("your-api-key");
let embedding = embedder.embed("Hello, world!").await.unwrap();
```

### 3. EmbeddingStore Trait

位置：`crates/vol-llm-agent/src/rag/store.rs`

```rust
#[async_trait]
pub trait EmbeddingStore: Send + Sync {
    async fn search(&self, query: &[f32], k: usize) -> Result<Vec<Document>>;
    async fn insert(&self, document: Document, embedding: Vec<f32>) -> Result<()>;
    async fn insert_batch(&self, documents: &[(Document, Vec<f32>)]) -> Result<()>;
}
```

**设计说明**：
- 支持向量相似度搜索
- 支持文档插入（批量和单条）
- 用户可实现对接 Qdrant、ChromaDB、Milvus 等向量数据库

### 4. InMemoryStore

位置：`crates/vol-llm-agent/src/rag/memory_store.rs`

```rust
pub struct InMemoryStore {
    documents: RwLock<Vec<StoredDocument>>,
}
```

**特点**：
- 内存存储，无需外部依赖
- 使用 `RwLock` 实现线程安全
- 余弦相似度计算
- 适合测试、演示和小规模应用

**核心算法**：
```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}
```

### 5. Document

位置：`crates/vol-llm-agent/src/rag/document.rs`

```rust
pub struct Document {
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub score: Option<f32>,
}
```

**字段说明**：
- `content`：文档内容
- `metadata`：元数据（来源、创建时间、类别等）
- `score`：相似度分数（检索时设置）

### 6. RagConfig

位置：`crates/vol-llm-agent/src/rag/config.rs`

```rust
pub struct RagConfig {
    pub top_k: usize,              // 检索文档数量
    pub similarity_threshold: f32, // 相似度阈值
    pub return_scores: bool,       // 是否返回分数
    pub max_tokens: u32,           // 最大生成 token 数
    pub temperature: f32,          // 生成温度（低值更准确）
}
```

**默认配置**：
- `top_k`: 5
- `similarity_threshold`: 0.3
- `temperature`: 0.1（RAG 需要准确性，使用低温）

### 7. RagAgent

位置：`crates/vol-llm-agent/src/rag/agent.rs`

```rust
pub struct RagAgent {
    llm: Arc<dyn LLMClient>,
    store: Arc<dyn EmbeddingStore>,
    embedder: Arc<dyn Embedder>,
    config: RagConfig,
}

impl RagAgent {
    pub async fn retrieve(&self, query: &str) -> Result<Vec<Document>>;
    pub async fn generate(&self, query: &str, docs: &[Document]) -> Result<String>;
    pub async fn query(&self, query: &str) -> Result<RagResponse>;
}
```

**方法说明**：

| 方法 | 功能 | 使用场景 |
|------|------|----------|
| `retrieve()` | 仅检索文档 | 需要检查/过滤检索结果 |
| `generate()` | 仅生成回答 | 已有检索文档，只需生成 |
| `query()` | 完整 RAG 流程 | 标准问答场景 |

**RAG Prompt 模板**：
```
你是一名知识助手。请根据提供的参考资料回答问题。

要求：
1. 只基于参考资料回答，不要编造信息
2. 如果参考资料不足以回答问题，明确告知用户
3. 回答时注明信息来源

参考资料：
{context}

用户问题：{query}
```

## 业务层封装

### QaAgent

位置：`crates/vol-llm-agents/src/qa/service.rs`

```rust
pub struct QaAgent<E, S>
where
    E: Embedder + 'static,
    S: EmbeddingStore + 'static,
{
    rag: RagAgent,
    config: QaAgentConfig,
    _embedder: PhantomData<E>,
    _store: PhantomData<S>,
}

impl QaAgent {
    pub async fn ask(&self, question: &str) -> Result<QaResponse>;
}
```

**设计说明**：
- 泛型设计，支持不同的 Embedder/Store 实现
- 使用 `PhantomData` 保持类型参数
- 封装 RagAgent，提供业务层 API
- 可扩展为特定领域的问答助手（客服、内部知识库等）

## 使用示例

### 基础 RAG 示例

```rust
use vol_llm_agent::{RagAgent, RagConfig};
use vol_llm_agent::rag::{DashScopeEmbedder, InMemoryStore, Document};

#[tokio::main]
async fn main() {
    // 1. 创建 embedder 和 store
    let embedder = Arc::new(DashScopeEmbedder::from_env());
    let store = Arc::new(InMemoryStore::new());
    
    // 2. 添加知识
    let doc = Document::new("Delta 是期权希腊值之一...".to_string())
        .with_metadata("source", "options_trading");
    let embedding = embedder.embed(&doc.content).await?;
    store.insert(doc, embedding).await?;
    
    // 3. 创建 RagAgent
    let llm = Arc::new(anthropic_client);
    let config = RagConfig::default().with_top_k(3);
    let rag = RagAgent::new(llm, store, embedder, config);
    
    // 4. 问答
    let response = rag.query("什么是 Delta？").await?;
    println!("Answer: {}", response.answer);
}
```

### QaAgent 示例

```rust
use vol_llm_agents::{QaAgent, QaAgentConfig};
use vol_llm_agent::rag::{DashScopeEmbedder, InMemoryStore};

#[tokio::main]
async fn main() {
    let embedder = Arc::new(DashScopeEmbedder::from_env());
    let store = Arc::new(InMemoryStore::new());
    
    // 创建 QaAgent
    let config = QaAgentConfig::default()
        .with_name("trading-assistant");
    let agent = QaAgent::new(llm, store, embedder, config);
    
    // 问答
    let response = agent.ask("如何进行 Delta 对冲？").await?;
    println!("{}", response.with_sources());
}
```

## 扩展指南

### 自定义 Embedder

```rust
struct OpenAIEmbedder {
    client: OpenAIClient,
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 调用 OpenAI API
        Ok(self.client.embeddings(text).await?)
    }
}
```

### 自定义 EmbeddingStore

```rust
struct QdrantStore {
    client: QdrantClient,
}

#[async_trait]
impl EmbeddingStore for QdrantStore {
    async fn search(&self, query: &[f32], k: usize) -> Result<Vec<Document>> {
        // 调用 Qdrant API
        Ok(docs)
    }
    
    async fn insert(&self, doc: Document, embedding: Vec<f32>) -> Result<()> {
        // 存储到 Qdrant
        Ok(())
    }
}
```

## 测试

### 单元测试

```bash
# vol-llm-agent 测试
cargo test -p vol-llm-agent

# vol-llm-agents 测试
cargo test -p vol-llm-agents
```

### 运行示例

```bash
# 基础 RAG 示例
cargo run --example rag_example -p vol-llm-agent

# QaAgent 示例
cargo run --example qa_agent_example -p vol-llm-agents
```

## 依赖关系

```toml
[dependencies]
vol-llm-core = { path = "crates/vol-llm-core" }
async-trait = "0.1"
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }
serde = { version = "1.0", features = ["derive"] }
uuid = "1.0"
```

## 文件结构

```
crates/vol-llm-agent/src/rag/
├── mod.rs              # 模块入口，导出所有类型
├── agent.rs            # RagAgent 核心实现
├── config.rs           # RagConfig 配置
├── document.rs         # Document 数据结构
├── embedding.rs        # Embedder trait
├── store.rs            # EmbeddingStore trait
├── memory_store.rs     # InMemoryStore 实现
└── dashscope_embedder.rs  # DashScopeEmbedder 实现

crates/vol-llm-agents/src/qa/
├── mod.rs              # QA 模块入口
└── service.rs          # QaAgent 实现

crates/vol-llm-agent/examples/
└── rag_example.rs      # 基础 RAG 示例

crates/vol-llm-agents/examples/
└── qa_agent_example.rs # QaAgent 示例
```

## 设计决策

### 为什么 Embedder 和 EmbeddingStore 是 trait？

1. **灵活性**：用户可选择不同的 embedding 服务（DashScope、OpenAI、本地模型）
2. **可测试性**：测试时使用 MockEmbedder，无需真实 API
3. **可扩展性**：新增 embedding 服务无需修改核心代码

### 为什么提供分离的 retrieve() 和 generate()？

1. **灵活性**：高级用户可能需要在生成前过滤/检查文档
2. **可组合性**：可复用检索结果进行多次生成
3. **可观察性**：便于调试和监控检索质量

### 为什么 InMemoryStore 使用余弦相似度？

1. **标准化**：余弦相似度是 NLP 领域最常用的相似度度量
2. **归一化**：结果在 [-1, 1] 范围，便于设置阈值
3. **高效**：简单向量运算，无需复杂计算

### 为什么 temperature 默认为 0.1？

1. **准确性优先**：RAG 需要基于事实回答，低温度减少幻觉
2. **一致性**：相同输入产生相同输出
3. **可调整**：用户可根据场景调整

## 性能考虑

### 当前实现

- InMemoryStore 使用 `RwLock`，读多写少场景性能良好
- 串行 `embed_batch` 实现，适合低频使用
- 无缓存机制

### 未来优化方向

1. **批量 embedding**：DashScopeEmbedder 可优化为并行/批量 API 调用
2. **缓存**：缓存常见查询的 embedding 结果
3. **索引优化**：引入 HNSW 等近似最近邻索引加速搜索
4. **异步队列**：大批量插入时使用异步队列

## 总结

RAG Agent 实现了一个灵活、可扩展的检索增强生成框架：

- **核心能力**：RagAgent 提供 retrieve/generate/query 三合一 API
- **可插拔设计**：Embedder 和 EmbeddingStore 基于 trait，易于扩展
- **开箱即用**：提供 DashScopeEmbedder 和 InMemoryStore 实现
- **业务封装**：QaAgent 提供问答场景的业务层 API
- **完整测试**：单元测试 + 文档测试 + 运行示例

用户可以通过实现 trait 对接自己的 embedding 服务和向量存储，快速构建 RAG 应用。
