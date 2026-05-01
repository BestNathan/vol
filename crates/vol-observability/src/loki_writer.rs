//! Loki batch writer — buffers log entries and flushes to Loki in batches.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::mpsc;

use crate::event::{IngestEvent, LokiLogEntry};

// -- Loki Push API types (serialize only) --

#[derive(Serialize)]
struct LokiPushRequest {
    streams: Vec<LokiStream>,
}

#[derive(Serialize)]
struct LokiStream {
    stream: HashMap<String, String>,
    values: Vec<[String; 2]>, // [timestamp_nanos, line]
}

// -- Command sent into the writer channel --

pub enum LokiCommand {
    Event(IngestEvent),
    Flush,
}

// -- Health tracking --

#[derive(Clone, Default)]
pub struct LokiWriterHealth {
    pub last_flush_ok: Arc<std::sync::Mutex<bool>>,
}

// -- Public API --

/// Spawn a background task that buffers `LokiLogEntry` items and flushes them
/// to Loki when the buffer reaches `batch_size` or after `flush_interval_ms`.
///
/// Returns a `(sender, health)` tuple. Send `LokiCommand::Event` to queue
/// entries, or `LokiCommand::Flush` to force an immediate flush.
pub fn spawn_loki_writer(
    url: String,
    batch_size: usize,
    flush_interval_ms: u64,
) -> (mpsc::Sender<LokiCommand>, LokiWriterHealth) {
    let (tx, mut rx) = mpsc::channel::<LokiCommand>(1000);
    let health = LokiWriterHealth::default();
    let health_clone = health.clone();

    tokio::spawn(async move {
        let mut buffer: Vec<LokiLogEntry> = Vec::new();
        let client = reqwest::Client::new();
        let flush_interval = Duration::from_millis(flush_interval_ms);
        let mut ticker = tokio::time::interval(flush_interval);
        // Don't fire immediately — wait for the first interval.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Channel receives an entry or explicit flush.
                cmd = rx.recv() => {
                    match cmd {
                        Some(LokiCommand::Event(event)) => {
                            let entry = event.to_loki_entry();
                            buffer.push(entry);
                            if buffer.len() >= batch_size {
                                let to_send = std::mem::take(&mut buffer);
                                flush_to_loki(&client, &url, to_send, &health_clone).await;
                            }
                        }
                        Some(LokiCommand::Flush) => {
                            if !buffer.is_empty() {
                                let to_send = std::mem::take(&mut buffer);
                                flush_to_loki(&client, &url, to_send, &health_clone).await;
                            }
                        }
                        None => {
                            // Channel closed — flush remaining and exit.
                            if !buffer.is_empty() {
                                let to_send = std::mem::take(&mut buffer);
                                flush_to_loki(&client, &url, to_send, &health_clone).await;
                            }
                            tracing::info!("loki writer: channel closed, exiting");
                            break;
                        }
                    }
                }

                // Periodic flush timer.
                _ = ticker.tick() => {
                    if !buffer.is_empty() {
                        let to_send = std::mem::take(&mut buffer);
                        flush_to_loki(&client, &url, to_send, &health_clone).await;
                    }
                }
            }
        }
    });

    (tx, health)
}

// -- Internal flush helper --

/// Group entries by label set, build a `LokiPushRequest`, and POST to Loki.
/// Retries once on failure, then drops the batch and logs ERROR.
async fn flush_to_loki(
    client: &reqwest::Client,
    url: &str,
    entries: Vec<LokiLogEntry>,
    health: &LokiWriterHealth,
) {
    if entries.is_empty() {
        return;
    }

    // Group by sorted label key-value map.
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
        .map(|(sorted_labels, values)| {
            let stream: HashMap<String, String> =
                sorted_labels.into_iter().collect();
            LokiStream { stream, values }
        })
        .collect();

    let request = LokiPushRequest {
        streams: loki_streams,
    };

    let push_url = format!("{}/loki/api/v1/push", url.trim_end_matches('/'));

    // Retry once on failure, then drop.
    let mut attempt = 0;
    let max_attempts = 2;
    let mut last_err = None;

    while attempt < max_attempts {
        attempt += 1;
        match client.post(&push_url).json(&request).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    tracing::debug!("loki flush ok: status={}", status);
                    if let Ok(mut ok) = health.last_flush_ok.lock() {
                        *ok = true;
                    }
                    return;
                }
                last_err = Some(format!("status={}", status));
            }
            Err(err) => {
                last_err = Some(err.to_string());
            }
        }
        if attempt < max_attempts {
            tracing::warn!("loki flush failed (attempt {}), retrying: {}", attempt, last_err.as_ref().unwrap());
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // All attempts exhausted.
    tracing::error!("loki flush failed after {} attempts: {}", max_attempts, last_err.as_ref().unwrap());
    if let Ok(mut ok) = health.last_flush_ok.lock() {
        *ok = false;
    }
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loki_request_serialization() {
        let mut labels_a = HashMap::new();
        labels_a.insert("app".to_string(), "vol-monitor".to_string());
        labels_a.insert("env".to_string(), "dev".to_string());

        let mut labels_b = HashMap::new();
        labels_b.insert("app".to_string(), "vol-monitor".to_string());
        labels_b.insert("env".to_string(), "prod".to_string());

        let streams = vec![
            LokiStream {
                stream: labels_a,
                values: vec![
                    ["1714370000000000000".to_string(), "line one".to_string()],
                    ["1714370001000000000".to_string(), "line two".to_string()],
                ],
            },
            LokiStream {
                stream: labels_b,
                values: vec![
                    ["1714370002000000000".to_string(), "line three".to_string()],
                ],
            },
        ];

        let request = LokiPushRequest { streams };
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("\"app\":\"vol-monitor\""));
        assert!(json.contains("\"env\":\"dev\""));
        assert!(json.contains("\"env\":\"prod\""));
        assert!(json.contains("line one"));
        assert!(json.contains("line two"));
        assert!(json.contains("line three"));
        assert!(json.contains("1714370000000000000"));
    }
}
