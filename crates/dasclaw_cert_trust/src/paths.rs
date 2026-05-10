//! Filesystem layout for dasclaw cert trust state.
//!
//! Per ADR-139 §4.3 dasclaw stores the fingerprint of the
//! currently-installed CA at `$CODEX_HOME/proxy/installed-ca.fingerprint`
//! so subsequent invocations can locate it inside the OS trust store
//! without iterating every entry.
//!
//! Resolution order:
//! 1. `$CODEX_HOME/proxy/` if `CODEX_HOME` is set (and points to an existing
//!    directory). Honoring the env var keeps tests hermetic and matches the
//!    contract documented by `dasclaw_utils_home_dir`.
//! 2. `~/.codex/proxy/` otherwise.
//!
//! Unlike `dasclaw_utils_home_dir::find_codex_home`, this helper does
//! **not** require the directory to exist — `install_ca` will create it.

use std::path::PathBuf;

use crate::Result;
use crate::error::Error;

const FINGERPRINT_FILENAME: &str = "installed-ca.fingerprint";

/// Returns the directory `$CODEX_HOME/proxy/` (creating nothing).
pub(crate) fn proxy_dir() -> Result<PathBuf> {
    let mut base = match std::env::var("CODEX_HOME").ok().filter(|v| !v.is_empty()) {
        Some(val) => PathBuf::from(val),
        None => dirs::home_dir()
            .ok_or_else(|| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "could not resolve home directory for $CODEX_HOME fallback",
                ))
            })?
            .join(".codex"),
    };
    base.push("proxy");
    Ok(base)
}

/// Returns the fingerprint sentinel path.
pub(crate) fn fingerprint_path() -> Result<PathBuf> {
    Ok(proxy_dir()?.join(FINGERPRINT_FILENAME))
}

/// Persist the SHA-256 hex fingerprint of the just-installed CA.
///
/// Creates `$CODEX_HOME/proxy/` on demand. Overwrites any prior value
/// (rotation is install-after-uninstall). Writes are **atomic** via
/// tempfile + rename so a crash mid-write leaves either the old
/// fingerprint or the new one — never a half-written file. This is
/// load-bearing: the keychain backends rely on the sentinel being
/// authoritative; a truncated file would make `status()` falsely report
/// "not installed" while the real CA is still trusted by the OS.
///
/// # Permissions (issue #376 §3.1)
///
/// On Unix the sentinel is created with mode `0o600`. The default umask
/// would yield `0o644`, which lets any process running as the same user
/// overwrite the file and trick `status()` / `uninstall()` into looking
/// for the wrong CA in the keychain. Tightening to `0o600` does not add
/// a real privilege boundary (anyone in the same user context can call
/// the keychain APIs directly), but it makes intra-user tampering
/// detectable: [`read_fingerprint`] also asserts the mode and ownership
/// match what we wrote.
///
/// On Windows we rely on the default ACL (`CurrentUser` profile only),
/// which already restricts access to the same user.
pub(crate) fn write_fingerprint(fp_hex: &str) -> Result<()> {
    let path = fingerprint_path()?;
    let parent = path.parent().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "fingerprint path has no parent directory",
        ))
    })?;
    std::fs::create_dir_all(parent)?;
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        builder.permissions(std::fs::Permissions::from_mode(0o600));
    }
    let mut tmp = builder.tempfile_in(parent)?;
    {
        use std::io::Write as _;
        tmp.write_all(fp_hex.as_bytes())?;
        tmp.flush()?;
    }
    // `persist` does an atomic rename on POSIX; on Windows it falls back
    // to MoveFileEx which is also atomic for same-volume renames.
    tmp.persist(&path).map_err(|err| Error::Io(err.error))?;
    Ok(())
}

/// Read the previously-persisted SHA-256 fingerprint.
///
/// Returns `Ok(None)` when the file does not exist (= no CA installed).
///
/// # Permissions (issue #376 §3.1)
///
/// On Unix this also verifies the file is owned by the current uid and
/// has mode `0o600` — the same constraints [`write_fingerprint`]
/// applies. A mismatch returns [`Error::Io`] with `PermissionDenied`,
/// because a sentinel that another process tampered with cannot be
/// trusted to point at our CA. Windows skips the check and relies on
/// the default profile ACL.
pub(crate) fn read_fingerprint() -> Result<Option<String>> {
    let path = fingerprint_path()?;
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            verify_sentinel_permissions(&path)?;
            let trimmed = content.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(Error::Io(err)),
    }
}

/// Unix: the sentinel must be a regular file owned by the current uid
/// with mode `0o600`. Anything else means another process touched it
/// and we should refuse to trust the value. On Windows / unsupported
/// targets this is a no-op.
#[cfg(unix)]
fn verify_sentinel_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "fingerprint sentinel is not a regular file",
        )));
    }
    let mode = meta.mode() & 0o777;
    if mode != 0o600 {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("fingerprint sentinel mode {mode:o} (expected 600)"),
        )));
    }
    // `getuid()` is `unsafe` only because libc declares it so; the
    // syscall itself has no preconditions and always succeeds.
    let current_uid = unsafe { libc::getuid() };
    if meta.uid() != current_uid {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "fingerprint sentinel owned by uid {}, expected {current_uid}",
                meta.uid()
            ),
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_sentinel_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

/// Delete the fingerprint file. Idempotent — succeeds when the file is
/// already absent.
pub(crate) fn clear_fingerprint() -> Result<()> {
    let path = fingerprint_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Error::Io(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Use a fresh `CODEX_HOME` per test so they can run in parallel.
    /// SAFETY: `set_var` is unsafe in edition 2024; the helper isolates
    /// each test via `tempfile::TempDir` so the env var is only read by
    /// `proxy_dir()` inside the same scope.
    fn with_codex_home(td: &TempDir) {
        // SAFETY: tests are single-threaded per nextest process model
        // (one test = one process), so racing readers cannot exist here.
        unsafe { std::env::set_var("CODEX_HOME", td.path()) };
    }

    #[test]
    fn proxy_dir_uses_codex_home_when_set() {
        let td = TempDir::new().expect("tempdir");
        with_codex_home(&td);
        let dir = proxy_dir().expect("proxy_dir");
        assert_eq!(dir, td.path().join("proxy"));
    }

    #[test]
    fn write_then_read_round_trips() {
        let td = TempDir::new().expect("tempdir");
        with_codex_home(&td);
        let fp = "abc123";
        write_fingerprint(fp).expect("write");
        let got = read_fingerprint().expect("read");
        assert_eq!(got.as_deref(), Some(fp));
    }

    #[test]
    fn read_fingerprint_returns_none_when_missing() {
        let td = TempDir::new().expect("tempdir");
        with_codex_home(&td);
        let got = read_fingerprint().expect("read");
        assert!(got.is_none());
    }

    #[test]
    fn clear_fingerprint_is_idempotent() {
        let td = TempDir::new().expect("tempdir");
        with_codex_home(&td);
        // First call — file does not exist yet.
        clear_fingerprint().expect("first clear");
        write_fingerprint("xyz").expect("write");
        // Second call — file exists, gets removed.
        clear_fingerprint().expect("second clear");
        // Third call — file is gone again.
        clear_fingerprint().expect("third clear");
        assert!(read_fingerprint().expect("read").is_none());
    }

    #[test]
    fn write_fingerprint_overwrites_atomically() {
        // Atomic-rename contract: rewriting the same path with a new
        // value yields the new value, never a half-written merge.
        let td = TempDir::new().expect("tempdir");
        with_codex_home(&td);
        write_fingerprint("aaaa").expect("first write");
        write_fingerprint("bbbb").expect("second write");
        assert_eq!(read_fingerprint().expect("read").as_deref(), Some("bbbb"));
        // No `.tmp` litter must be left in the proxy dir.
        let dir = proxy_dir().expect("proxy_dir");
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("read_dir")
            .filter_map(|e| {
                e.ok()
                    .map(|e| e.file_name().into_string().unwrap_or_default())
            })
            .collect();
        assert_eq!(
            entries,
            vec![FINGERPRINT_FILENAME.to_string()],
            "atomic rename must leave only the sentinel file: {entries:?}"
        );
    }

    /// Issue #376 §3.1 — sentinel must be created with mode 0o600.
    #[cfg(unix)]
    #[test]
    fn write_fingerprint_uses_mode_0600_on_unix() {
        use std::os::unix::fs::PermissionsExt as _;
        let td = TempDir::new().expect("tempdir");
        with_codex_home(&td);
        write_fingerprint("abc").expect("write");
        let path = fingerprint_path().expect("path");
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "expected mode 0600, got {mode:o}");
    }

    /// Issue #376 §3.1 — read must reject a tampered (world-readable)
    /// sentinel even when its content would otherwise round-trip.
    #[cfg(unix)]
    #[test]
    fn read_fingerprint_rejects_world_readable_sentinel() {
        use std::os::unix::fs::PermissionsExt as _;
        let td = TempDir::new().expect("tempdir");
        with_codex_home(&td);
        write_fingerprint("abc").expect("write");
        let path = fingerprint_path().expect("path");
        // Loosen the mode behind our back, simulating a tampering
        // process running as the same user.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("relax mode");
        let err = read_fingerprint().expect_err("must reject loose mode");
        match err {
            Error::Io(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::PermissionDenied);
                assert!(io_err.to_string().contains("644"), "msg: {io_err}");
            }
            other => panic!("expected Error::Io, got {other:?}"),
        }
    }
}
