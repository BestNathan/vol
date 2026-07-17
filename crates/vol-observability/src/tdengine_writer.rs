//! TDengine batch writer — buffers metrics and flushes to TDengine in batches.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::event::ExtractedMetric;

// -- Table name constants --

const TABLE_AGENT_RUN: &str = "agent_run";
const TABLE_LLM_CALL: &str = "llm_call";
const TABLE_TOOL_CALL: &str = "tool_call";

// -- Command sent into the writer channel --

pub enum TdengineCommand {
    Metric(ExtractedMetric),
    Flush,
}

// -- Health tracking --

#[derive(Clone, Default)]
pub struct TdengineWriterHealth {
    pub last_flush_ok: Arc<AtomicBool>,
}

// -- SQL helper --

fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

// -- Public API --

/// Spawn a background task that buffers `ExtractedMetric` items and flushes
/// them to TDengine when the buffer reaches `batch_size` or after
/// `flush_interval_ms`.
///
/// Returns a `(sender, health)` tuple.
pub fn spawn_tdengine_writer(
    base_url: String,
    user: String,
    password: String,
    database: String,
    batch_size: usize,
    flush_interval_ms: u64,
) -> (mpsc::Sender<TdengineCommand>, TdengineWriterHealth) {
    let (tx, mut rx) = mpsc::channel::<TdengineCommand>(1000);
    let health = TdengineWriterHealth::default();
    let health_clone = health.clone();

    tokio::spawn(async move {
        let mut buffer: Vec<ExtractedMetric> = Vec::new();
        let client = reqwest::Client::new();
        let flush_interval = Duration::from_millis(flush_interval_ms);
        let mut ticker = tokio::time::interval(flush_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(TdengineCommand::Metric(metric)) => {
                            buffer.push(metric);
                            if buffer.len() >= batch_size {
                                let to_send = std::mem::take(&mut buffer);
                                flush_to_tdengine(
                                    &client, &base_url, &user, &password,
                                    &database, to_send, &health_clone,
                                ).await;
                            }
                        }
                        Some(TdengineCommand::Flush) => {
                            if !buffer.is_empty() {
                                let to_send = std::mem::take(&mut buffer);
                                flush_to_tdengine(
                                    &client, &base_url, &user, &password,
                                    &database, to_send, &health_clone,
                                ).await;
                            }
                        }
                        None => {
                            if !buffer.is_empty() {
                                let to_send = std::mem::take(&mut buffer);
                                flush_to_tdengine(
                                    &client, &base_url, &user, &password,
                                    &database, to_send, &health_clone,
                                ).await;
                            }
                            tracing::info!("tdengine writer: channel closed, exiting");
                            break;
                        }
                    }
                }

                _ = ticker.tick() => {
                    if !buffer.is_empty() {
                        let to_send = std::mem::take(&mut buffer);
                        flush_to_tdengine(
                            &client, &base_url, &user, &password,
                            &database, to_send, &health_clone,
                        ).await;
                    }
                }
            }
        }
    });

    (tx, health)
}

// -- Internal flush helper --

/// Group metrics by table, build true multi-row INSERT statements, and POST
/// to TDengine REST API. On failure, logs ERROR and drops the batch.
async fn flush_to_tdengine(
    client: &reqwest::Client,
    base_url: &str,
    user: &str,
    password: &str,
    database: &str,
    metrics: Vec<ExtractedMetric>,
    health: &TdengineWriterHealth,
) {
    if metrics.is_empty() {
        return;
    }

    // Group by table name, collecting (tags, values) pairs.
    let mut agent_tags_values: Vec<(String, String)> = Vec::new();
    let mut llm_tags_values: Vec<(String, String)> = Vec::new();
    let mut tool_tags_values: Vec<(String, String)> = Vec::new();

    for metric in metrics {
        match metric {
            ExtractedMetric::AgentRun {
                run_id,
                session_id,
                agent_id,
                agent_type,
                timestamp,
                duration_ms,
                iterations,
                tool_calls,
                final_answer_len,
                status,
            } => {
                let ts = timestamp.timestamp_millis();
                let tags = format!(
                    "('{}','{}','{}','{}')",
                    sql_escape(&run_id),
                    sql_escape(&session_id),
                    sql_escape(&agent_id),
                    sql_escape(&agent_type),
                );
                let values = format!(
                    "({ts},{duration_ms},{iterations},{tool_calls},{final_answer_len},{status})",
                );
                agent_tags_values.push((tags, values));
            }
            ExtractedMetric::LlmCall {
                run_id,
                session_id,
                agent_id,
                agent_type,
                timestamp,
                duration_ms,
                iteration,
                input_tokens,
                output_tokens,
                total_tokens,
                model,
                is_error,
            } => {
                let ts = timestamp.timestamp_millis();
                let error_flag = if is_error { -1 } else { 0 };
                let tags = format!(
                    "('{}','{}','{}','{}','{}')",
                    sql_escape(&run_id),
                    sql_escape(&session_id),
                    sql_escape(&agent_id),
                    sql_escape(&agent_type),
                    sql_escape(&model),
                );
                let values = format!(
                    "({ts},{duration_ms},{iteration},{input_tokens},{output_tokens},{total_tokens},{error_flag})",
                );
                llm_tags_values.push((tags, values));
            }
            ExtractedMetric::ToolCall {
                run_id,
                session_id,
                agent_id,
                agent_type,
                timestamp,
                duration_ms,
                status,
                tool_name,
            } => {
                let ts = timestamp.timestamp_millis();
                let tags = format!(
                    "('{}','{}','{}','{}','{}')",
                    sql_escape(&run_id),
                    sql_escape(&session_id),
                    sql_escape(&agent_id),
                    sql_escape(&agent_type),
                    sql_escape(&tool_name),
                );
                let values = format!("({ts},{duration_ms},{status})",);
                tool_tags_values.push((tags, values));
            }
        }
    }

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{base_url}/rest/sql/{database}");

    let mut ok = true;

    // Build and send multi-row INSERT per table.
    for (table, tags_values) in [
        (TABLE_AGENT_RUN, agent_tags_values),
        (TABLE_LLM_CALL, llm_tags_values),
        (TABLE_TOOL_CALL, tool_tags_values),
    ] {
        if tags_values.is_empty() {
            continue;
        }

        // Group rows by identical tags to produce true multi-row VALUES.
        // Key = tags string, Value = list of value tuples.
        let mut by_tags: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for (tags, values) in tags_values {
            by_tags.entry(tags).or_default().push(values);
        }

        let sql_parts: Vec<String> = by_tags
            .into_iter()
            .map(|(tags, values)| {
                format!(
                    "{} USING {} TAGS {} VALUES {}",
                    table,
                    table,
                    tags,
                    values.join(" ")
                )
            })
            .collect();

        let sql = sql_parts.join(" ");
        match client
            .post(&url)
            .basic_auth(user, Some(password))
            .header("Content-Type", "text/plain")
            .body(sql)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    tracing::error!("tdengine flush failed: table={}, status={}", table, status,);
                    ok = false;
                }
            }
            Err(err) => {
                tracing::error!("tdengine flush failed: table={}, error={}", table, err);
                ok = false;
            }
        }
    }

    health.last_flush_ok.store(ok, Ordering::SeqCst);
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_escape() {
        assert_eq!(sql_escape("hello"), "hello");
        assert_eq!(sql_escape("it's"), "it''s");
        assert_eq!(sql_escape("a'b'c"), "a''b''c");
        assert_eq!(sql_escape(""), "");
    }

    #[test]
    fn test_flush_groups_by_tags() {
        let now = chrono::Utc::now();
        let metrics = vec![
            ExtractedMetric::AgentRun {
                run_id: "r1".to_string(),
                session_id: "s1".to_string(),
                agent_id: "a1".to_string(),
                agent_type: "CodingAgent".to_string(),
                timestamp: now,
                duration_ms: 500,
                iterations: 3,
                tool_calls: 2,
                final_answer_len: 100,
                status: 0,
            },
            ExtractedMetric::AgentRun {
                run_id: "r1".to_string(),
                session_id: "s1".to_string(),
                agent_id: "a1".to_string(),
                agent_type: "CodingAgent".to_string(),
                timestamp: now,
                duration_ms: 600,
                iterations: 4,
                tool_calls: 3,
                final_answer_len: 200,
                status: 0,
            },
            ExtractedMetric::LlmCall {
                run_id: "r1".to_string(),
                session_id: "s1".to_string(),
                agent_id: "a1".to_string(),
                agent_type: "CodingAgent".to_string(),
                timestamp: now,
                duration_ms: 200,
                iteration: 1,
                input_tokens: 100,
                output_tokens: 50,
                total_tokens: 150,
                model: "qwen3.5-plus".to_string(),
                is_error: false,
            },
            ExtractedMetric::ToolCall {
                run_id: "r1".to_string(),
                session_id: "s1".to_string(),
                agent_id: "a1".to_string(),
                agent_type: "CodingAgent".to_string(),
                timestamp: now,
                duration_ms: 150,
                status: 0,
                tool_name: "bash".to_string(),
            },
        ];

        // Group metrics the same way flush_to_tdengine does.
        let mut agent_tags_values: Vec<(String, String)> = Vec::new();
        let mut llm_tags_values: Vec<(String, String)> = Vec::new();
        let mut tool_tags_values: Vec<(String, String)> = Vec::new();

        for metric in metrics {
            match metric {
                ExtractedMetric::AgentRun {
                    run_id,
                    session_id,
                    agent_id,
                    agent_type,
                    timestamp,
                    duration_ms,
                    iterations,
                    tool_calls,
                    final_answer_len,
                    status,
                } => {
                    let ts = timestamp.timestamp_millis();
                    let tags = format!(
                        "('{}','{}','{}','{}')",
                        sql_escape(&run_id),
                        sql_escape(&session_id),
                        sql_escape(&agent_id),
                        sql_escape(&agent_type)
                    );
                    let values = format!(
                        "({ts},{duration_ms},{iterations},{tool_calls},{final_answer_len},{status})"
                    );
                    agent_tags_values.push((tags, values));
                }
                ExtractedMetric::LlmCall {
                    run_id,
                    session_id,
                    agent_id,
                    agent_type,
                    timestamp,
                    duration_ms,
                    iteration,
                    input_tokens,
                    output_tokens,
                    total_tokens,
                    model,
                    is_error,
                } => {
                    let ts = timestamp.timestamp_millis();
                    let tags = format!(
                        "('{}','{}','{}','{}','{}')",
                        sql_escape(&run_id),
                        sql_escape(&session_id),
                        sql_escape(&agent_id),
                        sql_escape(&agent_type),
                        sql_escape(&model)
                    );
                    let error_flag = if is_error { -1 } else { 0 };
                    let values = format!(
                        "({ts},{duration_ms},{iteration},{input_tokens},{output_tokens},{total_tokens},{error_flag})"
                    );
                    llm_tags_values.push((tags, values));
                }
                ExtractedMetric::ToolCall {
                    run_id,
                    session_id,
                    agent_id,
                    agent_type,
                    timestamp,
                    duration_ms,
                    status,
                    tool_name,
                } => {
                    let ts = timestamp.timestamp_millis();
                    let tags = format!(
                        "('{}','{}','{}','{}','{}')",
                        sql_escape(&run_id),
                        sql_escape(&session_id),
                        sql_escape(&agent_id),
                        sql_escape(&agent_type),
                        sql_escape(&tool_name)
                    );
                    let values = format!("({ts},{duration_ms},{status})");
                    tool_tags_values.push((tags, values));
                }
            }
        }

        // Two AgentRun metrics share tags → should group into one multi-row VALUES
        assert_eq!(agent_tags_values.len(), 2);
        let mut agent_by_tags: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for (tags, values) in agent_tags_values {
            agent_by_tags.entry(tags).or_default().push(values);
        }
        assert_eq!(
            agent_by_tags.len(),
            1,
            "identical tags should group into one entry"
        );
        let values = agent_by_tags.values().next().unwrap();
        assert_eq!(values.len(), 2, "should have two value sets grouped");

        // One LlmCall, one ToolCall → each their own group
        assert_eq!(llm_tags_values.len(), 1);
        assert_eq!(tool_tags_values.len(), 1);
    }
}
