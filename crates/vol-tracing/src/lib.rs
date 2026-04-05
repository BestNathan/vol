mod with_span;
mod macros;
mod traced_event;

/// @deprecated Use `TracedEvent` instead.
///
/// `WithSpan` only carries a `Span` but does not explicitly store the `trace_id`,
/// which is required for distributed tracing across async boundaries.
///
/// # Example
/// ```
/// // Old (deprecated):
/// // let traced = WithSpan::new(event, span);
///
/// // New (recommended):
/// use vol_tracing::{TracedEvent, new_trace_id};
/// use tracing::info_span;
///
/// let span = info_span!("my_event");
/// let trace_id = new_trace_id();
/// let traced = TracedEvent::new(event, span, trace_id);
/// ```
#[deprecated(since = "0.5.0", note = "Use TracedEvent instead, which explicitly stores trace_id for distributed tracing")]
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
