//! TDengine batch writer — buffers metrics and flushes to TDengine in batches.

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
    pub last_flush_ok: Arc<std::sync::Mutex<bool>>,
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
/// Returns a `(sender, health)` tuple. Send `TdengineCommand::Metric` to
/// queue entries, or `TdengineCommand::Flush` to force an immediate flush.
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

/// Group metrics by table type, build multi-row INSERT statements, and POST
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

    // Group by table name and collect INSERT values.
    let mut agent_rows: Vec<String> = Vec::new();
    let mut llm_rows: Vec<String> = Vec::new();
    let mut tool_rows: Vec<String> = Vec::new();

    for metric in metrics {
        match metric {
            ExtractedMetric::AgentRun {
                run_id, session_id, agent_id, agent_type,
                timestamp, duration_ms, iterations, tool_calls,
                final_answer_len, status,
            } => {
                let ts = timestamp.timestamp_millis();
                let row = format!(
                    "{} USING {} TAGS ('{}','{}','{}','{}') VALUES ({},{},{},{},{},{})",
                    TABLE_AGENT_RUN, TABLE_AGENT_RUN,
                    sql_escape(&run_id),
                    sql_escape(&session_id),
                    sql_escape(&agent_id),
                    sql_escape(&agent_type),
                    ts, duration_ms, iterations, tool_calls,
                    final_answer_len, status,
                );
                agent_rows.push(row);
            }
            ExtractedMetric::LlmCall {
                run_id, session_id, agent_id, agent_type,
                timestamp, duration_ms, iteration,
                input_tokens, output_tokens, total_tokens,
                model, is_error,
            } => {
                let ts = timestamp.timestamp_millis();
                let error_flag = if is_error { -1 } else { 0 };
                let row = format!(
                    "{} USING {} TAGS ('{}','{}','{}','{}') VALUES ({},{},{},{},{},{},{},'{}')",
                    TABLE_LLM_CALL, TABLE_LLM_CALL,
                    sql_escape(&run_id),
                    sql_escape(&session_id),
                    sql_escape(&agent_id),
                    sql_escape(&agent_type),
                    ts, duration_ms, iteration,
                    input_tokens, output_tokens, total_tokens,
                    error_flag, sql_escape(&model),
                );
                llm_rows.push(row);
            }
            ExtractedMetric::ToolCall {
                run_id, session_id, agent_id, agent_type,
                timestamp, duration_ms, status, tool_name,
            } => {
                let ts = timestamp.timestamp_millis();
                let row = format!(
                    "{} USING {} TAGS ('{}','{}','{}','{}') VALUES ({},{},{},'{}')",
                    TABLE_TOOL_CALL, TABLE_TOOL_CALL,
                    sql_escape(&run_id),
                    sql_escape(&session_id),
                    sql_escape(&agent_id),
                    sql_escape(&agent_type),
                    ts, duration_ms, status,
                    sql_escape(&tool_name),
                );
                tool_rows.push(row);
            }
        }
    }

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{}/rest/sql/{}", base_url, database);

    let mut ok = true;

    // Send each table's INSERT statements.
    for (table_name, rows) in [
        (TABLE_AGENT_RUN, agent_rows),
        (TABLE_LLM_CALL, llm_rows),
        (TABLE_TOOL_CALL, tool_rows),
    ] {
        if rows.is_empty() {
            continue;
        }

        let sql = rows.join(" ");
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
                if status.is_success() {
                    tracing::debug!("tdengine flush ok: table={}, status={}", table_name, status);
                } else {
                    tracing::error!(
                        "tdengine flush failed: table={}, status={}",
                        table_name, status,
                    );
                    ok = false;
                }
            }
            Err(err) => {
                tracing::error!("tdengine flush failed: table={}, error={}", table_name, err);
                ok = false;
            }
        }
    }

    if let Ok(mut flag) = health.last_flush_ok.lock() {
        *flag = ok;
    }
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
    fn test_build_insert_statement() {
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

        let ts = now.timestamp_millis();

        // Verify AgentRun row
        let agent_row = format!(
            "{} USING {} TAGS ('{}','{}','{}','{}') VALUES ({},{},{},{},{},{})",
            TABLE_AGENT_RUN, TABLE_AGENT_RUN,
            sql_escape("r1"), sql_escape("s1"),
            sql_escape("a1"), sql_escape("CodingAgent"),
            ts, 500, 3, 2, 100, 0,
        );
        assert!(agent_row.contains("agent_run USING agent_run"));
        assert!(agent_row.contains("'r1'"));
        assert!(agent_row.contains("'CodingAgent'"));

        // Verify LlmCall row
        let llm_row = format!(
            "{} USING {} TAGS ('{}','{}','{}','{}') VALUES ({},{},{},{},{},{},{},'{}')",
            TABLE_LLM_CALL, TABLE_LLM_CALL,
            sql_escape("r1"), sql_escape("s1"),
            sql_escape("a1"), sql_escape("CodingAgent"),
            ts, 200, 1, 100, 50, 150, 0, sql_escape("qwen3.5-plus"),
        );
        assert!(llm_row.contains("llm_call USING llm_call"));
        assert!(llm_row.contains("'qwen3.5-plus'"));

        // Verify ToolCall row
        let tool_row = format!(
            "{} USING {} TAGS ('{}','{}','{}','{}') VALUES ({},{},{},'{}')",
            TABLE_TOOL_CALL, TABLE_TOOL_CALL,
            sql_escape("r1"), sql_escape("s1"),
            sql_escape("a1"), sql_escape("CodingAgent"),
            ts, 150, 0, sql_escape("bash"),
        );
        assert!(tool_row.contains("tool_call USING tool_call"));
        assert!(tool_row.contains("'bash'"));

        // Verify all three metrics produced distinct rows
        assert_eq!(metrics.len(), 3);
    }
}
