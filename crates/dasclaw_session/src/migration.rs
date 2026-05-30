//! Version-to-version migration scaffolding for [`SessionSnapshot`].
//!
//! **Current status:** `SESSION_VERSION == 1`, so no real migration runs.
//! This module exists as a template so that the first contributor bumping
//! [`SESSION_VERSION`] to `2` does not have to invent the migration
//! architecture from scratch — they copy [`migrate_v1_to_v2`], replace the
//! `NotImplemented` body with the actual transformation, and add a new
//! `match` arm in [`migrate_to_latest`].
//!
//! See ADR-160 / doc 56 §2.2 and §4 for the policy that rejects an
//! `extensions: serde_json::Value` slot and instead requires an explicit
//! `version` bump + migrate function per breaking change.

use thiserror::Error;

use crate::id::SESSION_VERSION;
use crate::snapshot::SessionSnapshot;

/// Errors produced by the migration entry points in this module.
///
/// Kept separate from [`crate::SessionError`] because migration is a pure,
/// in-memory transform with no I/O or serde involvement — callers can
/// handle "this snapshot is from a version I don't know how to read"
/// without having to match against I/O variants.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrationError {
    /// Reached when the migration path between two adjacent versions has
    /// not been written yet. Currently returned by [`migrate_v1_to_v2`]
    /// because v2 does not exist.
    #[error("session migration {from} → {to} not implemented: {reason}")]
    NotImplemented {
        /// Source schema version.
        from: u32,
        /// Target schema version.
        to: u32,
        /// Human-readable reason; static so the variant stays cheap to clone.
        reason: &'static str,
    },

    /// Returned when the snapshot's recorded `version` does not match what
    /// the called migration function expects (e.g. caller invoked
    /// `migrate_v1_to_v2` on a v3 snapshot), or when [`migrate_to_latest`]
    /// sees a version it has no arm for.
    #[error("session version {found} does not match expected migration source {expected}")]
    VersionMismatch {
        /// Version the migration function was prepared to consume.
        expected: u32,
        /// Version actually carried by the snapshot.
        found: u32,
    },
}

/// Migrates a v1 [`SessionSnapshot`] to v2.
///
/// **Status: placeholder.** v2 does not exist yet. This function exists so
/// the first contributor bumping [`SESSION_VERSION`] to `2` has a template
/// to copy and does not have to invent the migration architecture from
/// scratch.
///
/// When v2 lands, this function will:
///
/// 1. Validate that `old.version == 1`, returning
///    [`MigrationError::VersionMismatch`] otherwise.
/// 2. Transform v1-shaped fields into their v2 equivalents.
/// 3. Set `version = 2` on the returned snapshot.
/// 4. Be paired with an inverse `migrate_v2_to_v1` if downgrade is
///    supported (none of the reference implementations do — see
///    `claw-code/rust/crates/runtime/src/session.rs` and
///    `codex-cli-main/codex-rs/core/src/state/session.rs`, which both
///    record `version` without offering downgrade paths).
///
/// See ADR-160 §2.2 and §4 for the version-bump policy.
pub fn migrate_v1_to_v2(_old: SessionSnapshot) -> Result<SessionSnapshot, MigrationError> {
    Err(MigrationError::NotImplemented {
        from: 1,
        to: 2,
        reason: "v2 schema not yet defined; bump SESSION_VERSION and replace this stub when v2 lands",
    })
}

/// Routes a snapshot through every known migration step until it reaches
/// the current [`SESSION_VERSION`].
///
/// At v1 this function is effectively the identity: snapshots that already
/// match the current version pass through unchanged, snapshots from a
/// future version (loader is older than the file) or an unknown past
/// version return [`MigrationError::VersionMismatch`].
///
/// When v2 ships, the future contributor adds an arm:
///
/// ```ignore
/// match snapshot.version {
///     SESSION_VERSION => Ok(snapshot),
///     1 => migrate_v1_to_v2(snapshot).and_then(migrate_to_latest),
///     found => Err(MigrationError::VersionMismatch { expected: SESSION_VERSION, found }),
/// }
/// ```
///
/// Recursing through `migrate_to_latest` after each step means a v1
/// snapshot will be lifted through v1 → v2 → v3 → … → latest without the
/// caller having to know the chain length.
pub fn migrate_to_latest(snapshot: SessionSnapshot) -> Result<SessionSnapshot, MigrationError> {
    match snapshot.version {
        SESSION_VERSION => Ok(snapshot),
        found => Err(MigrationError::VersionMismatch {
            expected: SESSION_VERSION,
            found,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_with_version(version: u32) -> SessionSnapshot {
        let mut snap = SessionSnapshot::fresh();
        snap.version = version;
        snap
    }

    #[test]
    fn migrate_v1_to_v2_returns_not_implemented() {
        let result = migrate_v1_to_v2(snapshot_with_version(1));
        assert_eq!(
            result.unwrap_err(),
            MigrationError::NotImplemented {
                from: 1,
                to: 2,
                reason: "v2 schema not yet defined; bump SESSION_VERSION and replace this stub when v2 lands",
            }
        );
    }

    #[test]
    fn migrate_to_latest_is_identity_on_current_version() {
        let snap = snapshot_with_version(SESSION_VERSION);
        let session_id = snap.session_id.clone();
        let out = migrate_to_latest(snap).expect("identity on current version");
        assert_eq!(out.version, SESSION_VERSION);
        assert_eq!(out.session_id, session_id);
    }

    #[test]
    fn migrate_to_latest_rejects_future_version() {
        let future = SESSION_VERSION
            .checked_add(1)
            .expect("SESSION_VERSION leaves room for +1");
        let result = migrate_to_latest(snapshot_with_version(future));
        assert_eq!(
            result.unwrap_err(),
            MigrationError::VersionMismatch {
                expected: SESSION_VERSION,
                found: future,
            }
        );
    }

    #[test]
    fn migrate_to_latest_rejects_unknown_old_version() {
        let result = migrate_to_latest(snapshot_with_version(999));
        assert_eq!(
            result.unwrap_err(),
            MigrationError::VersionMismatch {
                expected: SESSION_VERSION,
                found: 999,
            }
        );
    }
}
