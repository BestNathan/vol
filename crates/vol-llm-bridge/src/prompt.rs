//! System prompt templates for alert analysis.

use vol_core::AlertType;

/// Default system prompt for the volatility analysis agent
pub fn system_prompt() -> &'static str {
    r#"你是一名衍生品市场风险分析师。

收到预警后，你需要：
1. 分析预警数据（IV、期限、symbol 等）
2. 结合历史数据了解趋势
3. 给出风险评估和操作建议

输出格式（严格遵循）：

🔔 预警分析建议

预警：{alert_type} - {symbol}
当前 IV: {iv} (阈值：{threshold})

📊 历史数据分析:
- 过去 1 小时 IV 变化 {change}%
- 过去 24 小时 IV 分位数：{percentile}%

⚠️ 风险等级：[高/中/低]

💡 建议:
{1-3 条具体操作建议}"#
}

/// Build a user prompt for alert analysis
pub fn build_user_prompt(alert_type: &str, symbol: &str, iv: f64, threshold: f64, history_summary: &str) -> String {
    format!(
        r#"请分析以下预警：

预警类型：{}
标的：{}
当前 IV: {:.4}
阈值：{:.4}

历史数据：
{}

请根据上述信息提供风险评估和操作建议。"#,
        alert_type, symbol, iv, threshold, history_summary
    )
}

/// Extract threshold value from AlertType
pub fn get_threshold_from_alert(alert_type: &AlertType) -> f64 {
    match alert_type {
        AlertType::AbsoluteIv { threshold } => *threshold,
        AlertType::RateChange { change_pct, .. } => *change_pct,
        AlertType::TermStructure { spread_pct } => *spread_pct,
        AlertType::Skew { skew_pct } => *skew_pct,
        AlertType::PortfolioMargin { threshold, .. } => *threshold,
        AlertType::PortfolioBalance { threshold, .. } => *threshold,
        AlertType::PortfolioDelta { .. } => 0.0, // No threshold
        AlertType::PortfolioPnL { threshold, .. } => *threshold,
        AlertType::PortfolioGreek { threshold, .. } => *threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_not_empty() {
        assert!(!system_prompt().is_empty());
    }

    #[test]
    fn test_user_prompt_builder() {
        let prompt = build_user_prompt(
            "absolute_iv",
            "BTC",
            0.55,
            0.50,
            "过去 1 小时 IV 上涨 5%",
        );
        assert!(prompt.contains("BTC"));
        assert!(prompt.contains("absolute_iv"));
    }

    #[test]
    fn test_get_threshold_from_alert() {
        let alert = AlertType::AbsoluteIv { threshold: 0.5 };
        assert_eq!(get_threshold_from_alert(&alert), 0.5);
    }
}
