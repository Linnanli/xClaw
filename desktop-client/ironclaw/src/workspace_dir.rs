//! Workspace directory management.
//!
//! Each conversation thread is associated with a workspace directory on disk.
//! When the user has imported an external project, the workspace points to that
//! real directory. Otherwise, a sandboxed directory under `~/.ironclaw/projects/`
//! is automatically created.
//!
//! The workspace path is stored in conversation metadata as `"workspace_root"`
//! and propagated to tool execution via `JobContext.metadata["workspace_root"]`.

use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::bootstrap::ironclaw_base_dir;

/// Return the base directory for auto-created workspaces: `~/.ironclaw/projects/`.
pub fn projects_base() -> PathBuf {
    ironclaw_base_dir().join("projects")
}

/// Create a sandboxed workspace directory for a conversation thread.
///
/// Returns `~/.ironclaw/projects/{thread_id}/` after ensuring it exists.
pub fn create_sandbox_workspace(thread_id: Uuid) -> Result<PathBuf, std::io::Error> {
    let base = projects_base();
    std::fs::create_dir_all(&base)?;
    let dir = base.join(thread_id.to_string());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Resolve the effective workspace root for a thread.
///
/// Priority:
/// 1. User-imported workspace path (from conversation metadata)
/// 2. Auto-created sandbox `~/.ironclaw/projects/{thread_id}/`
///
/// Returns the path and whether it was newly created.
pub fn resolve_workspace(
    user_workspace: Option<&str>,
    thread_id: Uuid,
) -> Result<PathBuf, std::io::Error> {
    if let Some(path) = user_workspace {
        let p = PathBuf::from(path);
        if p.is_dir() {
            return Ok(p);
        }
        tracing::warn!(
            path = %p.display(),
            "User workspace directory does not exist, falling back to sandbox"
        );
    }
    create_sandbox_workspace(thread_id)
}

/// Validate that an imported workspace path is a real, accessible directory.
///
/// Returns the canonicalized path on success.
pub fn validate_import_path(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", p.display()));
    }
    if !p.is_dir() {
        return Err(format!("Path is not a directory: {}", p.display()));
    }
    p.canonicalize()
        .map_err(|e| format!("Cannot resolve path {}: {}", p.display(), e))
}

/// Extract `workspace_root` from conversation metadata JSON.
pub fn workspace_from_metadata(metadata: &serde_json::Value) -> Option<&str> {
    metadata.get("workspace_root").and_then(|v| v.as_str())
}

/// Check whether a path is inside the sandbox projects directory.
pub fn is_sandbox_path(path: &Path) -> bool {
    let base = projects_base();
    // Canonicalize both to handle symlinks; fall back to prefix check if
    // canonicalization fails (directory might not exist yet).
    match (base.canonicalize(), path.canonicalize()) {
        (Ok(cb), Ok(cp)) => cp.starts_with(&cb),
        _ => path.starts_with(&base),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_sandbox_workspace() {
        let id = Uuid::new_v4();
        let dir = create_sandbox_workspace(id).expect("should create dir");
        assert!(dir.is_dir());
        assert!(dir.ends_with(id.to_string()));
        // Cleanup
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_resolve_workspace_with_user_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let id = Uuid::new_v4();
        let result =
            resolve_workspace(Some(tmp.path().to_str().unwrap()), id).expect("should resolve");
        assert_eq!(result, tmp.path());
    }

    #[test]
    fn test_resolve_workspace_fallback_to_sandbox() {
        let id = Uuid::new_v4();
        let result = resolve_workspace(None, id).expect("should create sandbox");
        assert!(result.is_dir());
        assert!(is_sandbox_path(&result));
        // Cleanup
        let _ = fs::remove_dir(&result);
    }

    #[test]
    fn test_resolve_workspace_invalid_user_path_falls_back() {
        let id = Uuid::new_v4();
        let result = resolve_workspace(Some("/nonexistent/path/xyz"), id)
            .expect("should fall back to sandbox");
        assert!(is_sandbox_path(&result));
        // Cleanup
        let _ = fs::remove_dir(&result);
    }

    #[test]
    fn test_validate_import_path_valid() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = validate_import_path(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_import_path_nonexistent() {
        let result = validate_import_path("/nonexistent/directory/abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_validate_import_path_file_not_dir() {
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let result = validate_import_path(tmp.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a directory"));
    }

    #[test]
    fn test_workspace_from_metadata() {
        let meta = serde_json::json!({"workspace_root": "/some/path"});
        assert_eq!(workspace_from_metadata(&meta), Some("/some/path"));
    }

    #[test]
    fn test_workspace_from_metadata_missing() {
        let meta = serde_json::json!({"other": "value"});
        assert_eq!(workspace_from_metadata(&meta), None);
    }

    #[test]
    fn test_workspace_from_metadata_null() {
        let meta = serde_json::Value::Null;
        assert_eq!(workspace_from_metadata(&meta), None);
    }

    #[test]
    fn test_is_sandbox_path_positive() {
        let id = Uuid::new_v4();
        let dir = create_sandbox_workspace(id).expect("should create");
        assert!(is_sandbox_path(&dir));
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_is_sandbox_path_negative() {
        assert!(!is_sandbox_path(Path::new("/tmp/some/random")));
    }
}
