//! Small utility helpers used by `context` (and only by `context`).
//!
//! `floor_char_boundary` is a verbatim port of the same helper in
//! `desktop-client/ironclaw/src/util.rs` (UTF-8 boundary polyfill for the
//! nightly-only `str::floor_char_boundary`). Duplicated here to keep slice A''
//! self-contained — the desktop copy stays in place for callers there.

pub(crate) fn floor_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut i = pos;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
