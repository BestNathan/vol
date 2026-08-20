//! vol-llm-agent-tool: AgentTool 派发工具 + AgentInjector 上下文贡献。
//!
//! 高层组合 crate：依赖 vol-llm-agent（ReAct 编排 / AgentLoader）、
//! vol-session（会话持久化）、vol-llm-tool（工具协议）等底层实现，
//! 被 vol-llm-runtime 依赖并注册为内置工具。

pub mod agent_tool;
pub use agent_tool::AgentTool;
