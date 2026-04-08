# RAG Agent 设计与实现文档

## 文档信息
- **创建时间**: 2026-04-08
- **位置**: `docs/rag-agent-design.md`
- **作者**: Claude Code

## 快速导航

### 核心组件
1. **Embedder Trait** - Embedding 生成接口
2. **DashScopeEmbedder** - 阿里云 DashScope 实现
3. **EmbeddingStore Trait** - 向量存储接口
4. **InMemoryStore** - 内存向量存储（余弦相似度）
5. **RagAgent** - RAG 核心引擎
6. **QaAgent** - 业务层问答智能体

### 架构图
```
┌─────────────────────────────────────┐
│           RagAgent                  │
│  - llm: Arc<dyn LLMClient>         │
│  - store: Arc<dyn EmbeddingStore>  │
│  - embedder: Arc<dyn Embedder>     │
│  + retrieve() / generate() / query()│
└─────────────────────────────────────┘
```

### 使用示例

#### 基础 RAG
```rust
use vol_llm_agent::{RagAgent, RagConfig};
use vol_llm_agent::rag::{DashScopeEmbedder, InMemoryStore};

let embedder = Arc::new(DashScopeEmbedder::from_env());
let store = Arc::new(InMemoryStore::new());
let rag = RagAgent::new(llm, store, embedder, config);
let response = rag.query("什么是 Delta？").await?;
```

#### QaAgent
```rust
use vol_llm_agents::{QaAgent, QaAgentConfig};

let agent = QaAgent::new(llm, store, embedder, config);
let response = agent.ask("如何进行 Delta 对冲？").await?;
```

### 测试状态
- vol-llm-agent: 21 个单元测试 + 3 个文档测试 ✅
- vol-llm-agents: 11 个单元测试 ✅
- 示例：rag_example.rs, qa_agent_example.rs ✅

### 扩展指南

#### 自定义 Embedder
```rust
struct OpenAIEmbedder { client: OpenAIClient }
#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.client.embeddings(text).await?)
    }
}
```

#### 自定义 EmbeddingStore
实现 `EmbeddingStore` trait 对接 Qdrant/ChromaDB/Milvus 等

### 完整文档
完整设计文档包含：
- 详细架构设计
- 所有组件 API 文档
- 设计决策说明
- 性能考虑
- 文件结构

查看完整文档：`docs/rag-agent-design.md`

### 运行命令
```bash
# 运行测试
cargo test -p vol-llm-agent -p vol-llm-agents

# 运行示例
cargo run --example rag_example -p vol-llm-agent
cargo run --example qa_agent_example -p vol-llm-agents
```
