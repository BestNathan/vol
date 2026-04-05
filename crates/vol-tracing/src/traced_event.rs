//! TracedEvent - wrapper for propagating trace context across channel boundaries.
//!
//! Unlike WithSpan which only carries a span, TracedEvent explicitly stores
//! the trace_id to ensure consistent tracing across async boundaries.

use tracing::Span;

/// Wrapper for sending events across channel boundaries with explicit trace context.
///
/// # Type Parameters
/// * `T` - The wrapped event type (e.g., VolatilityData, Alert, MonitoringEvent)
#[derive(Clone)]
pub struct TracedEvent<T> {
    /// The wrapped event data
    value: T,
    /// Parent span for establishing causal relationships
    parent_span: Option<Span>,
    /// Explicit trace ID for distributed tracing
    trace_id: String,
}

impl<T> TracedEvent<T> {
    /// Create a new TracedEvent with the given value, span, and trace_id.
    ///
    /// # Arguments
    /// * `value` - The event data to wrap
    /// * `span` - The current span for establishing causal relationships
    /// * `trace_id` - The trace identifier for distributed tracing
    ///
    /// # Example
    /// ```
    /// use tracing::Span;
    /// use vol_tracing::TracedEvent;
    ///
    /// let span = Span::current();
    /// let event = TracedEvent::new(42, span, "trace-123".to_string());
    /// ```
    pub fn new(value: T, span: Span, trace_id: String) -> Self {
        Self {
            value,
            parent_span: Some(span),
            trace_id,
        }
    }

    /// Create a TracedEvent without a parent span (generates new trace_id).
    ///
    /// # Arguments
    /// * `value` - The event data to wrap
    ///
    /// # Returns
    /// A new TracedEvent with a freshly generated trace_id and no parent span.
    pub fn without_span(value: T) -> Self {
        Self {
            value,
            parent_span: None,
            trace_id: crate::new_trace_id(),
        }
    }

    /// Create a TracedEvent with explicit trace_id (for continuing a trace).
    ///
    /// # Arguments
    /// * `value` - The event data to wrap
    /// * `span` - Optional parent span for establishing causal relationships
    /// * `trace_id` - The trace identifier to associate with this event
    ///
    /// # Example
    /// ```
    /// use tracing::Span;
    /// use vol_tracing::TracedEvent;
    ///
    /// let span = Span::current();
    /// let event = TracedEvent::with_trace_id(42, Some(span), "existing-trace-456".to_string());
    /// ```
    pub fn with_trace_id(value: T, span: Option<Span>, trace_id: String) -> Self {
        Self {
            value,
            parent_span: span,
            trace_id,
        }
    }

    /// Split the wrapper to get the value, optional span, and trace_id.
    ///
    /// # Returns
    /// A tuple of (value, parent_span, trace_id)
    pub fn split(self) -> (T, Option<Span>, String) {
        (self.value, self.parent_span, self.trace_id)
    }

    /// Get a reference to the trace_id.
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    /// Get a reference to the wrapped value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Get a reference to the parent span.
    pub fn parent_span(&self) -> Option<&Span> {
        self.parent_span.as_ref()
    }

    /// Unwrap and return the value, consuming the wrapper.
    pub fn into_value(self) -> T {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_traced_event_with_all_fields() {
        let span = Span::current();
        let trace_id = "test-trace-123".to_string();
        let event = TracedEvent::new(42, span.clone(), trace_id.clone());

        assert_eq!(event.trace_id(), &trace_id);
        assert_eq!(*event.value(), 42);
    }

    #[test]
    fn test_without_span_generates_trace_id() {
        let event: TracedEvent<i32> = TracedEvent::without_span(99);

        assert_eq!(*event.value(), 99);
        assert_eq!(event.trace_id().len(), 36); // UUID v4 hyphenated format
    }

    #[test]
    fn test_with_trace_id_uses_provided_id() {
        let span = Span::current();
        let custom_trace_id = "custom-trace-abc".to_string();
        let event = TracedEvent::with_trace_id("data", Some(span), custom_trace_id.clone());

        assert_eq!(event.trace_id(), &custom_trace_id);
        assert_eq!(event.value(), &"data");
    }

    #[test]
    fn test_with_trace_id_without_span() {
        let custom_trace_id = "custom-trace-def".to_string();
        let event: TracedEvent<&str> = TracedEvent::with_trace_id("data", None, custom_trace_id.clone());

        assert_eq!(event.trace_id(), &custom_trace_id);
    }

    #[test]
    fn test_split_returns_all_fields() {
        let span = Span::current();
        let trace_id = "split-test-789".to_string();
        let event = TracedEvent::new("test-value", span.clone(), trace_id.clone());

        let (value, parent_span, returned_trace_id) = event.split();

        assert_eq!(value, "test-value");
        assert!(parent_span.is_some());
        assert_eq!(returned_trace_id, trace_id);
    }

    #[test]
    fn test_into_value_consumes_wrapper() {
        let event = TracedEvent::new("hello".to_string(), Span::current(), "trace".to_string());
        let value: String = event.into_value();

        assert_eq!(value, "hello");
    }

    #[test]
    fn test_clone_preserves_all_fields() {
        let span = Span::current();
        let trace_id = "clone-test-xyz".to_string();
        let event1 = TracedEvent::new(123, span.clone(), trace_id.clone());
        let event2 = event1.clone();

        assert_eq!(event1.trace_id(), event2.trace_id());
        assert_eq!(event1.value(), event2.value());
    }
}
