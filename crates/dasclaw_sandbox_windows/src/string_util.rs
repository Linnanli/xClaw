// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/utils/string/src/lib.rs
//     - `take_bytes_at_char_boundary`
//     - `sanitize_metric_tag_value`
// SPDX-License-Identifier: Apache-2.0

//! String helpers ported from `codex-utils-string`.
//!
//! Only the two helpers actually consumed by `windows-sandbox-rs` are ported:
//! `sanitize_metric_tag_value` (used by `setup_error.rs`) and
//! `take_bytes_at_char_boundary` (used by `logging.rs`). The remaining
//! upstream surface (UUID extraction, markdown-hash-location normalization,
//! token-budget truncation) is intentionally **not** ported because it is
//! unused on the Windows sandbox path; reintroduce only when a porting PR
//! actually needs it.

/// Truncate a `&str` to a byte budget at a UTF-8 character boundary, returning
/// the longest prefix that fits within `maxb` bytes without splitting a
/// multi-byte codepoint.
#[inline]
pub fn take_bytes_at_char_boundary(s: &str, maxb: usize) -> &str {
    if s.len() <= maxb {
        return s;
    }
    let mut last_ok = 0;
    for (i, ch) in s.char_indices() {
        let nb = i + ch.len_utf8();
        if nb > maxb {
            break;
        }
        last_ok = nb;
    }
    &s[..last_ok]
}

/// Sanitize a tag value to comply with metric tag validation rules: only
/// ASCII alphanumeric, '.', '_', '-', and '/' are allowed.
///
/// Invalid characters become '_'. The result is trimmed of leading and
/// trailing underscores. If the trimmed result is empty or contains no
/// alphanumeric characters, returns `"unspecified"`. Results longer than 256
/// bytes are truncated.
pub fn sanitize_metric_tag_value(value: &str) -> String {
    const MAX_LEN: usize = 256;
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() || trimmed.chars().all(|ch| !ch.is_ascii_alphanumeric()) {
        return "unspecified".to_string();
    }
    if trimmed.len() <= MAX_LEN {
        trimmed.to_string()
    } else {
        trimmed[..MAX_LEN].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn take_bytes_at_char_boundary_returns_input_when_within_budget() {
        assert_eq!(take_bytes_at_char_boundary("hello", 10), "hello");
        assert_eq!(take_bytes_at_char_boundary("hello", 5), "hello");
    }

    #[test]
    fn take_bytes_at_char_boundary_truncates_ascii() {
        assert_eq!(take_bytes_at_char_boundary("hello", 3), "hel");
    }

    #[test]
    fn take_bytes_at_char_boundary_does_not_split_multibyte_codepoint() {
        // "🙂" is 4 bytes (U+1F642, F0 9F 99 82).
        let s = "ab🙂c";
        assert_eq!(s.len(), 7);
        // Budget 3 → "ab" (next char would push to 6 > 3).
        assert_eq!(take_bytes_at_char_boundary(s, 3), "ab");
        // Budget 5 → still "ab", because the smiley is 4 bytes and 2+4=6>5.
        assert_eq!(take_bytes_at_char_boundary(s, 5), "ab");
        // Budget 6 → "ab🙂".
        assert_eq!(take_bytes_at_char_boundary(s, 6), "ab🙂");
    }

    #[test]
    fn take_bytes_at_char_boundary_zero_budget_returns_empty() {
        assert_eq!(take_bytes_at_char_boundary("hello", 0), "");
    }

    #[test]
    fn sanitize_metric_tag_value_replaces_invalid_chars() {
        assert_eq!(sanitize_metric_tag_value("bad value!"), "bad_value");
    }

    #[test]
    fn sanitize_metric_tag_value_returns_unspecified_for_empty_after_trim() {
        assert_eq!(sanitize_metric_tag_value("///"), "unspecified");
    }

    #[test]
    fn sanitize_metric_tag_value_returns_unspecified_when_only_underscores_remain() {
        assert_eq!(sanitize_metric_tag_value("???"), "unspecified");
    }

    #[test]
    fn sanitize_metric_tag_value_keeps_allowed_punctuation() {
        assert_eq!(
            sanitize_metric_tag_value("foo.bar-baz_qux/quux"),
            "foo.bar-baz_qux/quux"
        );
    }

    #[test]
    fn sanitize_metric_tag_value_truncates_at_max_len() {
        let input: String = "a".repeat(300);
        let out = sanitize_metric_tag_value(&input);
        assert_eq!(out.len(), 256);
        assert!(out.chars().all(|c| c == 'a'));
    }
}
