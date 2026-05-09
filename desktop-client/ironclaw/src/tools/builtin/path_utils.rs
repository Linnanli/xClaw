//! Shared path validation utilities for tools that access the filesystem.
//!
//! This module provides secure path validation to prevent directory traversal
//! attacks and ensure paths stay within allowed sandboxes.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::context::JobContext;
use crate::tools::tool::ToolError;

/// Resolve the effective base directory for file operations.
///
/// Priority: tool's compile-time `base_dir` > `ctx.metadata["workspace_root"]` > `None`.
/// This allows tools registered without a base_dir to still respect the
/// per-conversation workspace root injected at runtime.
pub fn effective_base_dir<'a>(
    tool_base_dir: Option<&'a Path>,
    ctx: &'a JobContext,
) -> Option<PathBuf> {
    if let Some(dir) = tool_base_dir {
        return Some(dir.to_path_buf());
    }
    let result = ctx
        .metadata
        .get("workspace_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    if result.is_none() {
        tracing::warn!(
            conversation_id = ?ctx.conversation_id,
            metadata_keys = ?ctx.metadata.as_object().map(|o| o.keys().collect::<Vec<_>>()),
            "No workspace_root in job context — file tools will reject relative paths"
        );
    }
    result
}

/// Normalize a path by resolving `.` and `..` components lexically (no filesystem access).
///
/// This is critical for security: `std::fs::canonicalize` only works on paths that exist,
/// so for new files we must normalize without touching the filesystem.
pub fn normalize_lexical(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Only pop if there's a normal component to pop (don't escape root/prefix)
                if components
                    .last()
                    .is_some_and(|c| matches!(c, std::path::Component::Normal(_)))
                {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

/// Canonicalize a non-existing path by walking up to the nearest existing
/// ancestor, canonicalizing it, then re-appending the remaining tail.
///
/// This handles macOS `/var` → `/private/var` symlink and similar cases
/// where the leaf path doesn't exist yet but its parent directory does.
///
/// NOTE: Uses sync `exists()` + `canonicalize()` in a loop. Acceptable in
/// async context because typical path depth is ≤ 10 and each call is a
/// single stat/realpath syscall (microseconds). If profiling shows contention,
/// wrap call-site in `tokio::task::spawn_blocking`.
fn canonicalize_via_ancestor(path: &Path) -> PathBuf {
    let mut ancestor = path;
    let mut tail_parts: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if ancestor.exists() {
            let canonical = ancestor
                .canonicalize()
                .unwrap_or_else(|_| ancestor.to_path_buf());
            let mut result = canonical;
            for part in tail_parts.into_iter().rev() {
                result = result.join(part);
            }
            return result;
        }
        if let Some(name) = ancestor.file_name() {
            tail_parts.push(name);
        }
        match ancestor.parent() {
            Some(parent) if parent != ancestor => ancestor = parent,
            _ => return normalize_lexical(path),
        }
    }
}

/// Validate that a path is safe (no traversal attacks).
///
/// For sandboxed paths (base_dir is set), we normalize the joined path lexically
/// and then verify it lives under the canonical base. This prevents escapes through
/// non-existent parent directories where `canonicalize()` would fall back to the
/// raw (un-normalized) path.
///
/// # Arguments
/// * `path_str` - The path to validate
/// * `base_dir` - Optional base directory for sandboxing
///
/// # Returns
/// * `Ok(resolved_path)` - The canonicalized, validated path
/// * `Err(ToolError)` - If path escapes sandbox or is invalid
pub fn validate_path(path_str: &str, base_dir: Option<&Path>) -> Result<PathBuf, ToolError> {
    // First pass: reject null bytes and URL-encoded traversal
    // Note: We don't block `..` here because validate_path handles it by
    // normalizing lexically and checking sandbox containment
    if !is_path_safe_minimal(path_str) {
        return Err(ToolError::NotAuthorized(format!(
            "Path contains forbidden characters or sequences: {}",
            path_str
        )));
    }

    let path = PathBuf::from(path_str);

    // Resolve to absolute path
    let resolved = if path.is_absolute() {
        path.canonicalize()
            .unwrap_or_else(|_| normalize_lexical(&path))
    } else if let Some(base) = base_dir {
        let joined = base.join(&path);
        joined
            .canonicalize()
            .unwrap_or_else(|_| normalize_lexical(&joined))
    } else {
        // Relative path without a workspace — refuse instead of silently
        // resolving against the process CWD, which is almost never the
        // directory the user intended.
        return Err(ToolError::ExecutionFailed(format!(
            "Cannot resolve relative path '{}' — no workspace directory is set for this \
             conversation. Please import a workspace first.",
            path_str
        )));
    };

    // If base_dir is set, ensure the resolved path is within it
    if let Some(base) = base_dir {
        let base_canonical = base
            .canonicalize()
            .unwrap_or_else(|_| normalize_lexical(base));

        // For existing paths, canonicalize to resolve symlinks.
        // For non-existent paths, the lexical normalization above already removed
        // all `..` components, so starts_with is reliable.
        let check_path = if resolved.exists() {
            resolved.canonicalize().unwrap_or_else(|_| resolved.clone())
        } else {
            // Walk up to the nearest existing ancestor directory, canonicalize it,
            // then re-append the remaining tail. This handles the case where a
            // symlink sits above the new file.
            let mut ancestor = resolved.as_path();
            let mut tail_parts: Vec<&std::ffi::OsStr> = Vec::new();
            loop {
                if ancestor.exists() {
                    let canonical_ancestor = ancestor
                        .canonicalize()
                        .unwrap_or_else(|_| ancestor.to_path_buf());
                    let mut result = canonical_ancestor;
                    for part in tail_parts.into_iter().rev() {
                        result = result.join(part);
                    }
                    break result;
                }
                if let Some(name) = ancestor.file_name() {
                    tail_parts.push(name);
                }
                match ancestor.parent() {
                    Some(parent) if parent != ancestor => ancestor = parent,
                    _ => break resolved.clone(),
                }
            }
        };

        if !check_path.starts_with(&base_canonical) {
            return Err(ToolError::NotAuthorized(format!(
                "Path escapes sandbox: {}",
                path_str
            )));
        }
    }

    Ok(resolved)
}

/// Validate a path with both sandbox + admin-configured policy.
///
/// Calls `validate_path` first (sandbox check), then applies `PathPolicy` rules
/// (denied patterns within sandbox, external read-only whitelist).
///
/// When `policy` is `None`, behaves identically to `validate_path`.
pub fn validate_path_with_policy(
    path_str: &str,
    base_dir: Option<&Path>,
    policy: Option<&PathPolicy>,
    mode: AccessMode,
) -> Result<PathBuf, ToolError> {
    // When no base_dir and path is outside sandbox, validate_path won't block it.
    // With a policy, we still need to validate and then apply policy checks.
    let resolved = if base_dir.is_some() {
        // Normal sandbox validation — may return Ok (inside sandbox) or Err (escape)
        match validate_path(path_str, base_dir) {
            Ok(path) => path,
            Err(e) => {
                // Path escapes sandbox — policy may allow external read
                if let Some(pol) = policy
                    && mode == AccessMode::Read
                {
                    let fallback = validate_path(path_str, None)?;
                    // Canonicalize to resolve symlinks — prevents TOCTOU where
                    // a symlink is created after basic validation but before
                    // the policy starts_with check.
                    let fallback = if fallback.exists() {
                        fallback.canonicalize().unwrap_or(fallback)
                    } else {
                        canonicalize_via_ancestor(&fallback)
                    };
                    pol.check(&fallback, base_dir, mode)?;
                    return Ok(fallback);
                }
                return Err(e);
            }
        }
    } else {
        validate_path(path_str, None)?
    };

    if let Some(pol) = policy {
        pol.check(&resolved, base_dir, mode)?;
    }

    Ok(resolved)
}

/// Basic path safety check without requiring a base directory.
///
/// This is a fallback check that blocks obvious traversal attempts:
/// - Contains `..` components
/// - Contains null bytes
/// - Uses URL encoding to hide traversal
///
/// For stronger security, use validate_path() with a base_dir.
pub fn is_path_safe_basic(path: &str) -> bool {
    // Block path traversal
    if path.contains("..") {
        return false;
    }

    // Block null bytes (would panic in Path)
    if path.contains('\0') {
        return false;
    }

    // Block URL-encoded traversal attempts
    let lower = path.to_lowercase();
    if lower.contains("%2e") || lower.contains("%2f") || lower.contains("%5c") {
        return false;
    }

    true
}

/// Check for null bytes, URL-encoded traversal, and Unicode confusable characters.
/// Unlike is_path_safe_basic, this allows `..` in paths since validate_path
/// handles that by normalizing lexically and checking sandbox containment.
fn is_path_safe_minimal(path: &str) -> bool {
    if path.contains('\0') {
        return false;
    }

    let lower = path.to_lowercase();
    if lower.contains("%2e") || lower.contains("%2f") || lower.contains("%5c") {
        return false;
    }

    // Reject Unicode confusable characters that could bypass path validation:
    // - Fullwidth period (U+FF0E ．) may resolve to '.' on some filesystems
    // - Fullwidth solidus (U+FF0F ／) may resolve to '/'
    // - Fullwidth reverse solidus (U+FF3C ＼) may resolve to '\'
    // - Two-dot leader (U+2025 ‥) may be confused with '..'
    // - Halfwidth solidus (U+FF0F) and other separator lookalikes
    for ch in path.chars() {
        if matches!(
            ch,
            '\u{FF0E}' | '\u{FF0F}' | '\u{FF3C}' | '\u{2025}'
            | '\u{2024}' // One dot leader
            | '\u{FE52}' // Small full stop
            | '\u{2044}' // Fraction slash
            | '\u{2215}' // Division slash
            | '\u{29F8}' // Big solidus
        ) {
            return false;
        }
    }

    true
}

/// Admin-configurable path access policy.
///
/// Controls which paths the AI agent can access beyond the base sandbox:
/// - `denied_patterns`: Glob patterns for paths **within** the sandbox that are forbidden
///   (e.g. `.env*`, `**/*.pem`, `**/secrets/**`). Takes priority over allowed patterns.
/// - `external_read_only`: Absolute paths outside the sandbox that are readable (never writable).
///
/// Empty `denied_patterns` = no extra restrictions within sandbox.
/// Empty `external_read_only` = no access outside sandbox.
#[derive(Debug, Clone)]
pub struct PathPolicy {
    denied_globs: GlobSet,
    external_read_only: Vec<PathBuf>,
}

/// Whether the operation is a read or write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
}

impl PathPolicy {
    pub fn new(
        denied_patterns: &[String],
        external_read_only: &[String],
    ) -> Result<Self, ToolError> {
        let mut builder = GlobSetBuilder::new();
        for pattern in denied_patterns {
            let glob = Glob::new(pattern).map_err(|e| {
                ToolError::InvalidParameters(format!("Invalid denied pattern '{}': {}", pattern, e))
            })?;
            builder.add(glob);
        }
        let denied_globs = builder.build().map_err(|e| {
            ToolError::InvalidParameters(format!("Failed to build denied glob set: {}", e))
        })?;

        let external_read_only = external_read_only.iter().map(PathBuf::from).collect();

        Ok(Self {
            denied_globs,
            external_read_only,
        })
    }

    /// Check if a resolved path is allowed under this policy.
    ///
    /// - `resolved`: the canonicalized absolute path
    /// - `base_dir`: the sandbox root (if set)
    /// - `mode`: read or write
    pub fn check(
        &self,
        resolved: &Path,
        base_dir: Option<&Path>,
        mode: AccessMode,
    ) -> Result<(), ToolError> {
        // If the path is inside the sandbox, check denied patterns
        if let Some(base) = base_dir {
            let base_canonical = base
                .canonicalize()
                .unwrap_or_else(|_| normalize_lexical(base));
            // Canonicalize resolved; for non-existing paths, walk up to
            // nearest existing ancestor (handles macOS /var → /private/var).
            let resolved_canonical = if resolved.exists() {
                resolved
                    .canonicalize()
                    .unwrap_or_else(|_| normalize_lexical(resolved))
            } else {
                canonicalize_via_ancestor(resolved)
            };

            if resolved_canonical.starts_with(&base_canonical) {
                let relative = resolved_canonical
                    .strip_prefix(&base_canonical)
                    .unwrap_or(resolved);
                let rel_str = relative.to_string_lossy();

                if self.denied_globs.is_match(rel_str.as_ref()) {
                    return Err(ToolError::NotAuthorized(format!(
                        "Path denied by policy: {}",
                        rel_str,
                    )));
                }
                return Ok(());
            }
        }

        // Path is outside the sandbox — check external read-only list
        if mode == AccessMode::Write {
            return Err(ToolError::NotAuthorized(
                "Write access outside sandbox is not allowed".to_string(),
            ));
        }

        let resolved_canonical = resolved
            .canonicalize()
            .unwrap_or_else(|_| normalize_lexical(resolved));

        for allowed in &self.external_read_only {
            let allowed_canonical = allowed
                .canonicalize()
                .unwrap_or_else(|_| normalize_lexical(allowed));

            // Allow exact match or child path (for directory patterns)
            if resolved_canonical == allowed_canonical
                || resolved_canonical.starts_with(&allowed_canonical)
            {
                return Ok(());
            }
        }

        Err(ToolError::NotAuthorized(format!(
            "Path outside sandbox not in external read-only list: {}",
            resolved.display(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_is_path_safe_basic_allows_normal_paths() {
        assert!(is_path_safe_basic("/tmp/file.txt"));
        assert!(is_path_safe_basic("documents/report.pdf"));
        assert!(is_path_safe_basic("my-file.png"));
    }

    #[test]
    fn test_is_path_safe_basic_rejects_traversal() {
        assert!(!is_path_safe_basic("../etc/passwd"));
        assert!(!is_path_safe_basic("foo/../bar"));
        assert!(!is_path_safe_basic("foo/bar/../../secret"));
    }

    #[test]
    fn test_is_path_safe_basic_rejects_null_bytes() {
        assert!(!is_path_safe_basic("file\0.txt"));
        assert!(!is_path_safe_basic("/tmp/test\0.txt"));
    }

    #[test]
    fn test_is_path_safe_basic_rejects_url_encoding() {
        assert!(!is_path_safe_basic("%2e%2e%2fetc/passwd"));
        assert!(!is_path_safe_basic("foo%2fbar"));
        assert!(!is_path_safe_basic("test%5cpath"));
    }

    #[test]
    fn test_validate_path_allows_within_sandbox() {
        let dir = tempdir().unwrap();
        let result = validate_path("subdir/file.txt", Some(dir.path()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_rejects_traversal_nonexistent_parent() {
        let dir = tempdir().unwrap();
        // Create a sibling directory structure to test escape
        // Try to escape to parent and access /etc/passwd
        let result = validate_path("../etc/passwd", Some(dir.path()));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_rejects_relative_traversal() {
        let dir = tempdir().unwrap();
        let result = validate_path("../../etc/passwd", Some(dir.path()));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_allows_valid_nested_write() {
        let dir = tempdir().unwrap();
        let result = validate_path("subdir/newfile.txt", Some(dir.path()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_allows_dot_dot_within_sandbox() {
        let dir = tempdir().unwrap();
        // This should be allowed as it stays within the sandbox
        let result = validate_path("a/b/../c.txt", Some(dir.path()));
        assert!(result.is_ok());
    }

    // ── Unicode confusable tests ──

    #[test]
    fn test_rejects_fullwidth_period() {
        // U+FF0E fullwidth full stop — could be confused with '.'
        assert!(!is_path_safe_minimal("src/\u{FF0E}\u{FF0E}/etc/passwd"));
    }

    #[test]
    fn test_rejects_fullwidth_solidus() {
        // U+FF0F fullwidth solidus — could be confused with '/'
        assert!(!is_path_safe_minimal("src\u{FF0F}secret"));
    }

    #[test]
    fn test_rejects_two_dot_leader() {
        // U+2025 two dot leader — could be confused with '..'
        assert!(!is_path_safe_minimal("\u{2025}/etc/passwd"));
    }

    #[test]
    fn test_rejects_division_slash() {
        // U+2215 division slash — could be confused with '/'
        assert!(!is_path_safe_minimal("src\u{2215}secret"));
    }

    #[test]
    fn test_allows_normal_unicode_filenames() {
        // CJK characters in filenames are legitimate
        assert!(is_path_safe_minimal("src/文档/readme.md"));
        assert!(is_path_safe_minimal("测试.txt"));
    }

    // ── PathPolicy tests ──

    #[test]
    fn test_policy_denies_patterns_within_sandbox() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET=x").unwrap();

        let policy = PathPolicy::new(&[".env*".to_string(), "**/*.pem".to_string()], &[]).unwrap();

        // .env is denied
        let resolved = dir.path().join(".env");
        assert!(
            policy
                .check(&resolved, Some(dir.path()), AccessMode::Read)
                .is_err()
        );

        // src/foo.rs is allowed
        let resolved = dir.path().join("src/foo.rs");
        assert!(
            policy
                .check(&resolved, Some(dir.path()), AccessMode::Write)
                .is_ok()
        );
    }

    #[test]
    fn test_policy_denies_pem_by_glob() {
        let dir = tempdir().unwrap();
        let policy = PathPolicy::new(&["**/*.pem".to_string()], &[]).unwrap();

        let resolved = dir.path().join("certs/server.pem");
        assert!(
            policy
                .check(&resolved, Some(dir.path()), AccessMode::Read)
                .is_err()
        );
    }

    #[test]
    fn test_policy_blocks_write_outside_sandbox() {
        let dir = tempdir().unwrap();
        let policy = PathPolicy::new(&[], &["/etc/hosts".to_string()]).unwrap();

        // Read outside sandbox is allowed if in external list
        assert!(
            policy
                .check(Path::new("/etc/hosts"), Some(dir.path()), AccessMode::Read)
                .is_ok()
        );

        // Write outside sandbox is always denied
        assert!(
            policy
                .check(Path::new("/etc/hosts"), Some(dir.path()), AccessMode::Write)
                .is_err()
        );
    }

    #[test]
    fn test_policy_rejects_unlisted_external_path() {
        let dir = tempdir().unwrap();
        let policy = PathPolicy::new(&[], &["/etc/hosts".to_string()]).unwrap();

        // /etc/passwd is NOT in external_read_only
        assert!(
            policy
                .check(Path::new("/etc/passwd"), Some(dir.path()), AccessMode::Read)
                .is_err()
        );
    }

    #[test]
    fn test_policy_empty_allows_all_within_sandbox() {
        let dir = tempdir().unwrap();
        let policy = PathPolicy::new(&[], &[]).unwrap();

        let resolved = dir.path().join(".env");
        assert!(
            policy
                .check(&resolved, Some(dir.path()), AccessMode::Read)
                .is_ok()
        );
        assert!(
            policy
                .check(&resolved, Some(dir.path()), AccessMode::Write)
                .is_ok()
        );
    }

    // ── validate_path_with_policy tests ──

    #[test]
    fn test_with_policy_none_same_as_validate_path() {
        let dir = tempdir().unwrap();
        let result =
            validate_path_with_policy("subdir/file.txt", Some(dir.path()), None, AccessMode::Read);
        assert!(result.is_ok());
    }

    #[test]
    fn test_with_policy_denies_within_sandbox() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "x").unwrap();
        let policy = PathPolicy::new(&[".env*".to_string()], &[]).unwrap();

        let result =
            validate_path_with_policy(".env", Some(dir.path()), Some(&policy), AccessMode::Read);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("denied by policy"),);
    }

    #[test]
    fn test_with_policy_allows_external_read() {
        let dir = tempdir().unwrap();
        // Create a real file outside sandbox for this test
        let external_dir = tempdir().unwrap();
        let external_file = external_dir.path().join("config.toml");
        std::fs::write(&external_file, "key=val").unwrap();

        let policy = PathPolicy::new(&[], &[external_file.to_string_lossy().to_string()]).unwrap();

        let result = validate_path_with_policy(
            external_file.to_str().unwrap(),
            Some(dir.path()),
            Some(&policy),
            AccessMode::Read,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_with_policy_blocks_external_write() {
        let dir = tempdir().unwrap();
        let external_dir = tempdir().unwrap();
        let external_file = external_dir.path().join("config.toml");
        std::fs::write(&external_file, "key=val").unwrap();

        let policy = PathPolicy::new(&[], &[external_file.to_string_lossy().to_string()]).unwrap();

        let result = validate_path_with_policy(
            external_file.to_str().unwrap(),
            Some(dir.path()),
            Some(&policy),
            AccessMode::Write,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_effective_base_dir_prefers_tool_base() {
        let tool_base = PathBuf::from("/tool/base");
        let ctx = JobContext::with_user("u", "t", "d");
        let result = effective_base_dir(Some(&tool_base), &ctx);
        assert_eq!(result, Some(PathBuf::from("/tool/base")));
    }

    #[test]
    fn test_effective_base_dir_falls_back_to_ctx_workspace() {
        let mut ctx = JobContext::with_user("u", "t", "d");
        ctx.metadata = serde_json::json!({"workspace_root": "/home/user/project"});
        let result = effective_base_dir(None, &ctx);
        assert_eq!(result, Some(PathBuf::from("/home/user/project")));
    }

    #[test]
    fn test_effective_base_dir_none_when_both_absent() {
        let ctx = JobContext::with_user("u", "t", "d");
        let result = effective_base_dir(None, &ctx);
        assert_eq!(result, None);
    }

    #[test]
    fn test_effective_base_dir_tool_overrides_ctx() {
        let tool_base = PathBuf::from("/tool/base");
        let mut ctx = JobContext::with_user("u", "t", "d");
        ctx.metadata = serde_json::json!({"workspace_root": "/ctx/workspace"});
        let result = effective_base_dir(Some(&tool_base), &ctx);
        assert_eq!(result, Some(PathBuf::from("/tool/base")));
    }

    #[test]
    fn test_validate_path_rejects_relative_without_base() {
        let result = validate_path("some/file.txt", None);
        assert!(
            result.is_err(),
            "Relative path without base_dir should be rejected"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("workspace"),
            "Error message should mention workspace: {err_msg}"
        );
    }

    #[test]
    fn test_validate_path_allows_absolute_without_base() {
        // Absolute paths don't need a base_dir to resolve
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "data").unwrap();
        let result = validate_path(file.to_str().unwrap(), None);
        assert!(
            result.is_ok(),
            "Absolute path without base_dir should be allowed"
        );
    }
}
