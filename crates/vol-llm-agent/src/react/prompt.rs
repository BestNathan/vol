//! System prompt templates.

/// Default system prompt
pub fn default_system_prompt() -> &'static str {
    r#"你是一个专业的衍生品市场风险分析师。

你的任务是分析监控系统的告警，为用户提供深入的市场洞察和风险评估。

## 可用工具

你可以使用以下工具获取额外信息：

- `alert_history(symbol, tenor?, alert_type?)`: 查询历史告警
- `iv_curve(symbol, tenor?)`: 获取 IV 曲线数据
- `market_data(symbol, data_type?)`: 获取市场数据
- `rule_info(alert_type)`: 查询告警规则

## 工作流程

1. **分析告警** - 理解告警的类型、标的、期限
2. **决定行动** - 判断是否需要调用工具获取更多信息
3. **综合结论** - 基于所有信息给出分析结论

## 输出格式

当你需要调用工具时，请使用工具调用格式。
当你有足够信息时，直接给出最终分析结论。

## 注意事项

- 只调用必要的工具
- 如果一次工具调用不足以得出结论，可以进行多轮查询
- 最终结论应该清晰、可操作，包括风险等级和具体建议"#
}

/// System prompt builder
pub struct SystemPromptBuilder {
    available_tools: String,
    custom_instructions: Option<String>,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self {
            available_tools: String::new(),
            custom_instructions: None,
        }
    }

    pub fn with_tools(mut self, tools: &[vol_llm_core::ToolDefinition]) -> Self {
        let tools_desc = tools
            .iter()
            .map(|t| {
                format!(
                    "- `{}`: {}",
                    t.name,
                    t.description.as_deref().unwrap_or("无描述")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        self.available_tools = tools_desc;
        self
    }

    pub fn with_instructions(mut self, instructions: &str) -> Self {
        self.custom_instructions = Some(instructions.to_string());
        self
    }

    pub fn build(self) -> String {
        let base = default_system_prompt();
        let mut prompt = base.to_string();

        if let Some(instructions) = self.custom_instructions {
            prompt.push_str(&format!("\n\n## 额外指示\n\n{}", instructions));
        }

        prompt
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
