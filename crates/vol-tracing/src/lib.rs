mod macros;
mod with_span;
mod traced_event;

use tracing::Span;
pub use with_span::WithSpan;
pub use traced_event::TracedEvent;
// Re-export tracing core types for downstream crates
pub use tracing::instrument;
pub use tracing::Instrument;
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

/// Get the current trace_id from the active span context.
///
/// Note: The tracing crate does not support reading field values from spans.
/// This function returns a new trace_id as a fallback.
/// For proper trace propagation, prefer passing trace_id explicitly through
/// Alert or TracedEvent wrappers.
pub fn current_trace_id() -> String {
    // tracing::Span doesn't support reading field values at runtime.
    // Return a new trace_id as fallback.
    // For proper trace context propagation, use TracedEvent wrappers or pass trace_id explicitly.
    new_trace_id()
}
