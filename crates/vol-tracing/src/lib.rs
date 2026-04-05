mod with_span;
mod macros;
mod traced_event;

pub use with_span::WithSpan;
pub use traced_event::TracedEvent;
// Re-export tracing core types for downstream crates
pub use tracing::Instrument;
pub use tracing::instrument;
// macros are exported via #[macro_export] automatically

/// Generate a new trace_id (UUID v4, hyphenated format)
///
/// # Example
/// ```
/// let trace_id = vol_tracing::new_trace_id();
/// assert_eq!(trace_id.len(), 36); // 8-4-4-4-12 format
/// ```
pub fn new_trace_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
