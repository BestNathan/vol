//! `{{env.VAR}}` placeholder substitution.
//!
//! Replaces every occurrence of `{{env.VAR}}` in the input string with the
//! value of the local environment variable `VAR`. Unknown vars become empty
//! strings and emit a `tracing::warn!`. Unknown namespaces (e.g. `{{foo.X}}`)
//! are left untouched but emit a warning. Escape literal `{{` with `\{{`.

/// Interpolate `{{env.VAR}}` placeholders in a single string.
///
/// Lookup is performed via the provided closure so callers can inject
/// test doubles without touching process env.
pub fn interpolate_with<F>(input: &str, lookup: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        // escape sequence `\{{` → literal `{{`: consume BOTH opening braces so the
        // remainder of the input isn't misread as a placeholder.
        if c == '\\' && chars.peek() == Some(&'{') {
            let mut maybe = chars.clone();
            maybe.next();
            if maybe.peek() == Some(&'{') {
                chars.next(); // consume first `{`
                chars.next(); // consume second `{`
                out.push('{');
                out.push('{');
                continue;
            }
        }

        if c == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second `{`
                          // read until `}}`
            let mut tag = String::new();
            let mut closed = false;
            while let Some(ch) = chars.next() {
                if ch == '}' && chars.peek() == Some(&'}') {
                    chars.next();
                    closed = true;
                    break;
                }
                tag.push(ch);
            }
            if !closed {
                // malformed: emit as-is
                out.push_str("{{");
                out.push_str(&tag);
                continue;
            }
            // parse `namespace.name`
            if let Some(dot_pos) = tag.find('.') {
                let ns = &tag[..dot_pos];
                let var = &tag[dot_pos + 1..];
                match ns {
                    "env" => match lookup(var) {
                        Some(v) => out.push_str(&v),
                        None => {
                            tracing::warn!(var, "env var not set, substituting empty");
                        }
                    },
                    other => {
                        tracing::warn!(
                            namespace = other,
                            var,
                            "unknown placeholder namespace, leaving intact"
                        );
                        out.push_str("{{");
                        out.push_str(&tag);
                        out.push_str("}}");
                    }
                }
            } else {
                // no dot: unknown form, leave intact
                tracing::warn!(tag = %tag, "malformed placeholder (missing namespace)");
                out.push_str("{{");
                out.push_str(&tag);
                out.push_str("}}");
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Interpolate using the real process environment.
pub fn interpolate(input: &str) -> String {
    interpolate_with(input, |var| std::env::var(var).ok())
}

/// Apply interpolation to every value in a HashMap, returning a new HashMap.
/// Uses the provided lookup closure (useful for testing without touching process env).
pub fn interpolate_map_with<F>(
    m: &std::collections::HashMap<String, String>,
    lookup: F,
) -> std::collections::HashMap<String, String>
where
    F: Fn(&str) -> Option<String>,
{
    m.iter()
        .map(|(k, v)| (k.clone(), interpolate_with(v, &lookup)))
        .collect()
}

/// Apply interpolation to every value in a HashMap, returning a new HashMap.
pub fn interpolate_map(
    m: &std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, String> {
    interpolate_map_with(m, |var| std::env::var(var).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(var: &str) -> Option<String> {
        match var {
            "HOME" => Some("/home/alice".into()),
            "USER" => Some("alice".into()),
            _ => None,
        }
    }

    #[test]
    fn replaces_known_var() {
        assert_eq!(
            interpolate_with("path={{env.HOME}}/bin", lookup),
            "path=/home/alice/bin"
        );
    }

    #[test]
    fn missing_var_becomes_empty() {
        assert_eq!(interpolate_with("x={{env.MISSING}}y", lookup), "x=y");
    }

    #[test]
    fn multiple_vars_in_one_string() {
        assert_eq!(
            interpolate_with("{{env.USER}}@{{env.HOME}}", lookup),
            "alice@/home/alice"
        );
    }

    #[test]
    fn escaped_braces_become_literal() {
        assert_eq!(
            interpolate_with("literal \\{{env.HOME}}", lookup),
            "literal {{env.HOME}}"
        );
    }

    #[test]
    fn unknown_namespace_left_intact() {
        let out = interpolate_with("keep {{other.X}} intact", lookup);
        assert_eq!(out, "keep {{other.X}} intact");
    }

    #[test]
    fn malformed_no_closing_braces() {
        let out = interpolate_with("oops {{env.HOME", lookup);
        assert_eq!(out, "oops {{env.HOME");
    }

    #[test]
    fn no_placeholders_passes_through() {
        assert_eq!(interpolate_with("plain text", lookup), "plain text");
    }

    #[test]
    fn interpolate_map_applies_to_all_values() {
        let mut m: HashMap<String, String> = HashMap::new();
        m.insert("A".into(), "{{env.HOME}}/a".into());
        m.insert("B".into(), "literal".into());
        let out_map = interpolate_map_with(&m, lookup);
        assert_eq!(out_map.get("A").map(String::as_str), Some("/home/alice/a"));
        assert_eq!(out_map.get("B").map(String::as_str), Some("literal"));
    }
}
