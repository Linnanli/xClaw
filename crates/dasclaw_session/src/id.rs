//! Session identifier generation.
//!
//! Mirrors the `claw-code::session::generate_session_id` strategy
//! (`session-{unix_millis}-{counter}`) so future jsonl exports can be
//! cross-read with claw-code if that ever becomes a goal.
//!
//! See `claw-code/rust/crates/runtime/src/session.rs` (function
//! `generate_session_id`) for the reference implementation.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Wire-format version for [`SessionSnapshot`].
///
/// Increment when the on-disk / serialized shape of `SessionSnapshot`
/// changes in a way that would break existing readers. Snapshot loaders
/// must check this field and either upgrade or reject older snapshots.
pub const SESSION_VERSION: u32 = 1;

static SESSION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a fresh session identifier with a `session-` prefix.
///
/// Format: `session-{unix_millis}-{counter}`. The counter disambiguates
/// IDs created inside the same millisecond.
#[must_use]
pub fn generate_session_id() -> String {
    let millis = current_time_millis();
    let counter = SESSION_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("session-{millis}-{counter}")
}

/// Current Unix time in milliseconds.
///
/// Falls back to `0` if the system clock is somehow before the Unix
/// epoch; that is a degenerate but well-defined value (no panic).
#[must_use]
pub fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
