//! Loki HTTP client with batched background writer.
//!
//! Spawns a background tokio task that buffers log entries and flushes them
//! to Loki via the Push API when the buffer reaches `batch_size` or after
//! `flush_interval_ms`. Entries are sent via an mpsc channel (non-blocking).

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::mpsc;

/// A single log entry ready to be sent to Loki.
#[derive(Debug, Clone)]
pub struct LokiEntry {
    /// Nanosecond-precision Unix timestamp.
    pub timestamp_nanos: i64,
    /// The log line content (JSON string).
    pub line: String,
    /// Labels for this entry (merged with base labels).
    pub labels: HashMap<String, String>,
}

/// Batched Loki writer.
///
/// Holds the sender side of an mpsc channel. Entries are sent via `send()`
/// and a background task handles buffering and HTTP POST to Loki.
#[derive(Clone)]
pub struct LokiWriter {
    tx: mpsc::Sender<LokiEntry>,
}

impl LokiWriter {
    /// Spawn a background task that buffers entries and flushes to Loki.
    ///
    /// Returns a `LokiWriter` whose `send()` method queues entries non-blocking.
    /// When the last clone of the writer is dropped, the background task
    /// flushes remaining entries and exits.
    pub fn spawn(
        url: String,
        batch_size: usize,
        flush_interval_ms: u64,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<LokiEntry>(1000);

        tokio::spawn(async move {
            let mut buffer: Vec<LokiEntry> = Vec::with_capacity(batch_size);
            let client = reqwest::Client::new();
            let flush_interval = Duration::from_millis(flush_interval_ms);
            let mut ticker = tokio::time::interval(flush_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let push_url = format!("{}/loki/api/v1/push", url.trim_end_matches('/'));

            loop {
                tokio::select! {
                    entry = rx.recv() => {
                        match entry {
                            Some(entry) => {
                                buffer.push(entry);
                                if buffer.len() >= batch_size {
                                    let to_send = std::mem::take(&mut buffer);
                                    flush(&client, &push_url, to_send).await;
                                }
                            }
                            None => {
                                // Channel closed — flush remaining and exit.
                                if !buffer.is_empty() {
                                    let to_send = std::mem::take(&mut buffer);
                                    flush(&client, &push_url, to_send).await;
                                }
                                tracing::info!("loki writer: channel closed, exiting");
                                break;
                            }
                        }
                    }

                    _ = ticker.tick() => {
                        if !buffer.is_empty() {
                            let to_send = std::mem::take(&mut buffer);
                            flush(&client, &push_url, to_send).await;
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    /// Send a log entry to the background writer (non-blocking).
    ///
    /// If the channel is full, the entry is dropped with a warning.
    pub async fn send(&self, entry: LokiEntry) {
        if let Err(e) = self.tx.send(entry).await {
            tracing::warn!(error = %e, "loki writer: channel closed, entry dropped");
        }
    }
}

/// Group entries by label set and POST to Loki.
#[derive(Serialize)]
struct LokiPushRequest {
    streams: Vec<LokiStream>,
}

#[derive(Serialize)]
struct LokiStream {
    stream: HashMap<String, String>,
    values: Vec<[String; 2]>,
}

async fn flush(client: &reqwest::Client, url: &str, entries: Vec<LokiEntry>) {
    if entries.is_empty() {
        return;
    }

    let mut streams: HashMap<Vec<(String, String)>, Vec<[String; 2]>> = HashMap::new();
    for entry in entries {
        let mut sorted_labels: Vec<(String, String)> = entry.labels.into_iter().collect();
        sorted_labels.sort_by(|a, b| a.0.cmp(&b.0));
        streams
            .entry(sorted_labels)
            .or_default()
            .push([entry.timestamp_nanos.to_string(), entry.line]);
    }

    let loki_streams: Vec<LokiStream> = streams
        .into_iter()
        .map(|(sorted_labels, values)| LokiStream {
            stream: sorted_labels.into_iter().collect(),
            values,
        })
        .collect();

    let request = LokiPushRequest {
        streams: loki_streams,
    };

    match client.post(url).json(&request).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                tracing::debug!(status = %status, "loki flush ok");
            } else {
                let body = resp.text().await.unwrap_or_default();
                tracing::error!(status = %status, body = %body, "loki flush failed");
            }
        }
        Err(err) => {
            tracing::error!(error = %err, "loki flush error");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loki_request_serialization() {
        let mut labels = HashMap::new();
        labels.insert("namespace".to_string(), "agent".to_string());
        labels.insert("agent".to_string(), "coding".to_string());

        let streams = vec![LokiStream {
            stream: labels,
            values: vec![
                ["1714370000000000000".to_string(), r#"{"event":"AgentStart"}"#.to_string()],
                ["1714370001000000000".to_string(), r#"{"event":"ToolCallBegin"}"#.to_string()],
            ],
        }];

        let request = LokiPushRequest { streams };
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("\"namespace\":\"agent\""));
        assert!(json.contains("\"agent\":\"coding\""));
        assert!(json.contains("AgentStart"));
        assert!(json.contains("ToolCallBegin"));
    }
}
