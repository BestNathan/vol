use std::clone::Clone;
use tracing::Span;

/// Wrapper for sending events across channel boundaries with span context.
///
/// Carries the trace_id explicitly for distributed tracing across channel boundaries.
#[derive(Clone)]
pub struct WithSpan<T> {
    value: T,
    parent_span: Option<Span>,
    trace_id: String,
}

impl<T> WithSpan<T> {
    /// Create a new WithSpan with an attached span and trace_id.
    pub fn new(value: T, span: Span, trace_id: String) -> Self {
        Self {
            value,
            parent_span: Some(span),
            trace_id,
        }
    }

    /// Create a WithSpan without a span (for events that don't need tracing).
    /// Generates a new trace_id.
    pub fn without_span(value: T) -> Self {
        Self {
            value,
            parent_span: None,
            trace_id: crate::new_trace_id(),
        }
    }

    /// Create a WithSpan with explicit trace_id (for continuing a trace from another component).
    pub fn with_trace_id(value: T, span: Option<Span>, trace_id: String) -> Self {
        Self {
            value,
            parent_span: span,
            trace_id,
        }
    }

    /// Split the wrapper to get the value, optional span, and trace_id.
    pub fn split(self) -> (T, Option<Span>, String) {
        (self.value, self.parent_span, self.trace_id)
    }

    /// Get the trace_id for this event.
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    /// Enter a new span that follows from the parent span.
    /// The caller provides the new span (created via tracing::info_span! at the call site).
    /// The closure receives the new span so you can record attributes.
    ///
    /// # Example
    /// ```no_run
    /// # use tracing::Span;
    /// # use vol_tracing::WithSpan;
    /// # let event = ();
    /// # let parent_span = Span::current();
    /// # let trace_id = "tr_abc123".to_string();
    /// let traced = WithSpan::new(event, parent_span, trace_id);
    /// traced.enter_span(tracing::info_span!("rule_evaluate"), |span| {
    ///     span.record("rule.id", &"my-rule");
    ///     // process(&event)
    /// });
    /// ```
    pub fn enter_span<F, R>(self, new_span: Span, f: F) -> R
    where
        F: FnOnce(Span) -> R,
    {
        let (_value, parent_span, _trace_id) = self.split();

        // Establish causal relationship with parent span
        if let Some(parent) = parent_span {
            new_span.follows_from(parent.id());
        }

        let _guard = new_span.enter();
        f(new_span.clone())
    }

    /// Get reference to the wrapped value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Unwrap and return the value.
    pub fn into_value(self) -> T {
        self.value
    }
}
