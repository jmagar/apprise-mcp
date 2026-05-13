/// Maximum response size in bytes (~10K tokens).
pub const MAX_RESPONSE_BYTES: usize = 40_000;

/// Maximum notification body size before truncation with a warning.
pub const MAX_BODY_BYTES: usize = 8_000;

/// Truncate a response string to [`MAX_RESPONSE_BYTES`] with an informative suffix.
#[must_use]
pub fn truncate_response(text: &str) -> String {
    if text.len() <= MAX_RESPONSE_BYTES {
        return text.to_string();
    }
    let truncated = &text[..MAX_RESPONSE_BYTES];
    format!(
        "{truncated}\n\n[TRUNCATED: response exceeded 40KB (10K token) limit. \
         Use more specific filters or pagination to narrow results.]"
    )
}

/// If `body` exceeds [`MAX_BODY_BYTES`], truncate it and prepend a warning to
/// the *returned* body. Returns `(body, warning_message)` where `warning_message`
/// is `Some(...)` only when truncation occurred.
#[must_use]
pub fn truncate_body(body: &str) -> (String, Option<String>) {
    if body.len() <= MAX_BODY_BYTES {
        return (body.to_string(), None);
    }
    let truncated = &body[..MAX_BODY_BYTES];
    let warning = format!(
        "WARNING: notification body was truncated from {} bytes to {} bytes \
         (max allowed for a single notification).",
        body.len(),
        MAX_BODY_BYTES
    );
    (truncated.to_string(), Some(warning))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_response_is_unchanged() {
        let s = "hello world";
        assert_eq!(truncate_response(s), s);
    }

    #[test]
    fn long_response_is_truncated_with_suffix() {
        let s = "x".repeat(MAX_RESPONSE_BYTES + 100);
        let result = truncate_response(&s);
        assert!(result.len() > MAX_RESPONSE_BYTES);
        assert!(result.contains("[TRUNCATED:"));
    }

    #[test]
    fn short_body_unchanged() {
        let (body, warn) = truncate_body("hello");
        assert_eq!(body, "hello");
        assert!(warn.is_none());
    }

    #[test]
    fn long_body_truncated_with_warning() {
        let s = "y".repeat(MAX_BODY_BYTES + 50);
        let (body, warn) = truncate_body(&s);
        assert_eq!(body.len(), MAX_BODY_BYTES);
        assert!(warn.is_some());
    }
}
