use tracing::Span;

/// Wrapper for sending events across channel boundaries with span context.
pub struct WithSpan<T> {
    value: T,
    parent_span: Option<Span>,
}

impl<T> WithSpan<T> {
    /// Create a new WithSpan with an attached span.
    pub fn new(value: T, span: Span) -> Self {
        Self {
            value,
            parent_span: Some(span),
        }
    }

    /// Create a WithSpan without a span (for events that don't need tracing).
    pub fn without_span(value: T) -> Self {
        Self {
            value,
            parent_span: None,
        }
    }

    /// Split the wrapper to get the value and optional span.
    pub fn split(self) -> (T, Option<Span>) {
        (self.value, self.parent_span)
    }

    /// Enter a new span that follows from the parent span.
    /// The caller provides the new span (created via tracing::info_span! at the call site).
    /// The closure receives the new span so you can record attributes.
    ///
    /// # Example
    /// ```
    /// let traced = WithSpan::new(event, parent_span);
    /// traced.enter_span(tracing::info_span!("rule_evaluate"), |span| {
    ///     span.record("rule.id", &self.id);
    ///     process(&event)
    /// });
    /// ```
    pub fn enter_span<F, R>(self, new_span: Span, f: F) -> R
    where
        F: FnOnce(Span) -> R,
    {
        let (_value, parent_span) = self.split();

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
