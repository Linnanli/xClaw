//! File operation guard — security-enhanced wrappers for filesystem access.
//!
//! Builds on top of `path_utils::validate_path` to add:
//! - Symlink escape detection (resolves symlinks and re-checks boundary)
//! - Binary file detection (NUL byte sniffing)
//! - File size limit enforcement
//! - Line-ending normalization

use std::path::{Path, PathBuf};

use crate::path_utils::validate_path;
use dasclaw_tool::ToolError;

/// Default maximum file size: 10 MB.
const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Number of bytes to sample for binary detection.
pub const BINARY_SNIFF_SIZE: usize = 8192;

// ── Symlink Escape ──────────────────────────────────────────────────────

/// Verify that `path` does not escape `workspace_root` via symlink resolution.
///
/// This resolves the **full** symlink chain (using `std::fs::canonicalize`)
/// and then checks that the canonical path still lives under `workspace_root`.
/// Non-existent paths are allowed (the caller may be about to create them)
/// as long as their nearest existing ancestor resolves within bounds.
pub fn check_symlink_escape(path: &Path, workspace_root: &Path) -> Result<PathBuf, ToolError> {
    // Delegate heavy lifting to validate_path which already handles
    // canonicalize + ancestor walk-up for non-existent paths.
    validate_path(&path.to_string_lossy(), Some(workspace_root))
}

// ── Binary Detection ────────────────────────────────────────────────────

/// Detect whether `content` is binary by scanning for NUL bytes.
///
/// Checks only the first `BINARY_SNIFF_SIZE` bytes to keep the cost O(1).
pub fn is_binary(content: &[u8]) -> bool {
    let check_len = content.len().min(BINARY_SNIFF_SIZE);
    content[..check_len].contains(&0)
}

// ── Size Limit ──────────────────────────────────────────────────────────

/// Reject the file at `path` if its size exceeds `max_bytes`.
///
/// Falls back to `DEFAULT_MAX_FILE_SIZE` when `max_bytes` is `None`.
pub fn check_size_limit(path: &Path, max_bytes: Option<u64>) -> Result<u64, ToolError> {
    let limit = max_bytes.unwrap_or(DEFAULT_MAX_FILE_SIZE);
    let meta = std::fs::metadata(path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Cannot stat {}: {}", path.display(), e))
    })?;
    let size = meta.len();
    if size > limit {
        return Err(ToolError::ExecutionFailed(format!(
            "File {} exceeds size limit ({} bytes > {} bytes)",
            path.display(),
            size,
            limit
        )));
    }
    Ok(size)
}

// ── Line-Ending Normalisation ───────────────────────────────────────────

/// Normalize CRLF (`\r\n`) and bare CR (`\r`) to LF (`\n`).
pub fn normalize_line_endings(content: &str) -> String {
    // Two-pass to avoid double-replacement: first CRLF → LF, then bare CR → LF.
    content.replace("\r\n", "\n").replace('\r', "\n")
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // ── is_binary ───────────────────────────────────────────────────

    #[test]
    fn text_content_detected_as_non_binary() {
        assert!(!is_binary(b"Hello, world!\n"));
        assert!(!is_binary(b"fn main() { }"));
    }

    #[test]
    fn nul_byte_detected_as_binary() {
        assert!(is_binary(b"ELF\x00\x01\x02"));
        assert!(is_binary(&[0u8; 100]));
    }

    #[test]
    fn empty_content_is_not_binary() {
        assert!(!is_binary(b""));
    }

    // ── check_size_limit ────────────────────────────────────────────

    #[test]
    fn file_within_limit_passes() {
        let dir = tempdir().expect("tempdir");
        let f = dir.path().join("small.txt");
        fs::write(&f, "hello").expect("write");
        assert!(check_size_limit(&f, Some(1024)).is_ok());
    }

    #[test]
    fn file_exceeding_limit_rejected() {
        let dir = tempdir().expect("tempdir");
        let f = dir.path().join("big.txt");
        fs::write(&f, vec![b'x'; 2048]).expect("write");
        assert!(check_size_limit(&f, Some(1024)).is_err());
    }

    #[test]
    fn missing_file_returns_error() {
        let dir = tempdir().expect("tempdir");
        let f = dir.path().join("nope.txt");
        assert!(check_size_limit(&f, None).is_err());
    }

    // ── check_symlink_escape ────────────────────────────────────────

    #[test]
    fn path_inside_workspace_passes() {
        let dir = tempdir().expect("tempdir");
        let f = dir.path().join("ok.txt");
        fs::write(&f, "ok").expect("write");
        assert!(check_symlink_escape(&f, dir.path()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escaping_workspace_rejected() {
        let workspace = tempdir().expect("workspace");
        let outside = tempdir().expect("outside");
        let target = outside.path().join("secret.txt");
        fs::write(&target, "secret").expect("write");

        let link = workspace.path().join("escape_link");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        assert!(check_symlink_escape(&link, workspace.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_within_workspace_passes() {
        let workspace = tempdir().expect("workspace");
        let target = workspace.path().join("real.txt");
        fs::write(&target, "data").expect("write");

        let link = workspace.path().join("alias.txt");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        assert!(check_symlink_escape(&link, workspace.path()).is_ok());
    }

    // ── normalize_line_endings ──────────────────────────────────────

    #[test]
    fn crlf_normalized_to_lf() {
        assert_eq!(normalize_line_endings("a\r\nb\r\n"), "a\nb\n");
    }

    #[test]
    fn bare_cr_normalized_to_lf() {
        assert_eq!(normalize_line_endings("a\rb\r"), "a\nb\n");
    }

    #[test]
    fn lf_unchanged() {
        assert_eq!(normalize_line_endings("a\nb\n"), "a\nb\n");
    }

    #[test]
    fn mixed_endings_normalized() {
        assert_eq!(normalize_line_endings("a\r\nb\rc\n"), "a\nb\nc\n");
    }
}
