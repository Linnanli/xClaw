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
pub(crate) fn write_fingerprint(fp_hex: &str) -> Result<()> {
    let path = fingerprint_path()?;
    let parent = path.parent().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "fingerprint path has no parent directory",
        ))
    })?;
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
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
pub(crate) fn read_fingerprint() -> Result<Option<String>> {
    let path = fingerprint_path()?;
    match std::fs::read_to_string(&path) {
        Ok(content) => {
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
}
