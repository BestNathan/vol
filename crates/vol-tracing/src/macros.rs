/// Record multiple fields from a value into a span.
///
/// Usage: record_tags!(span, data, iv, symbol, mark_price)
/// This will call span.record("iv", &data.iv), span.record("symbol", &data.symbol), etc.
#[macro_export]
macro_rules! record_tags {
    ($span:expr, $value:expr, $($field:ident),+ $(,)?) => {{
        $(
            $span.record(stringify!($field), &$value.$field);
        )+
    }};
}
