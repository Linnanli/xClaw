//! Capability-based workspace filesystem boundary.
//!
//! Provides [`WorkspaceCapability`], a thin wrapper over [`cap_std::fs::Dir`]
//! that enforces at the **OS kernel level** (via `openat` + symlink
//! resolution controls) that every filesystem access stays within a single
//! root directory.
//!
//! This is the *application layer* of the ironclaw sandbox stack described
//! in ADR-002 (`docs/plans/architecture-refactor/adr-002-...md`). It
//! protects the host Rust process itself from path-escape bugs (`..`,
//! symlinks, TOCTOU). The *kernel layer* (Landlock / sandbox-exec /
//! Restricted Token) binds child processes spawned by the LLM — the two
//! layers are complementary and stack together.
//!
//! # Guarantees
//!
//! - Absolute paths are **rejected** (cap-std returns an error).
//! - `..` components that would escape the root are **rejected**.
//! - Symlinks that point outside the root are **rejected** at open time.
//! - TOCTOU races on the root itself are eliminated because the root is
//!   held open as a capability (directory file descriptor), not looked up
//!   by name on every access.
//!
//! # Non-guarantees (by design)
//!
//! - Does not restrict *child processes* — that is the kernel sandbox
//!   layer's job.
//! - Does not enforce DLP / approval policy — that is the
//!   `ironclaw_safety` tier.
//! - Does not cap file size / count — caller must enforce before passing
//!   data in.
//!
//! # Example
//!
//! ```no_run
//! use ironclaw_workspace_cap::WorkspaceCapability;
//!
//! let ws = WorkspaceCapability::open("/home/user/project").unwrap();
//! ws.write("notes/todo.md", b"hello").unwrap();
//! let data = ws.read("notes/todo.md").unwrap();
//! assert_eq!(data, b"hello");
//!
//! // `..` or absolute paths error out, enforced by cap-std.
//! assert!(ws.read("../../../etc/passwd").is_err());
//! assert!(ws.read("/etc/passwd").is_err());
//! ```

use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;

/// Errors returned by workspace capability operations.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceCapError {
    /// The requested path violates the capability boundary
    /// (absolute path, `..` escape, or symlink pointing outside root).
    #[error("path escapes workspace root: {0}")]
    PolicyViolation(String),

    /// Underlying I/O error (disk, permissions, not found, etc.).
    #[error("workspace I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A handle to a workspace directory with capability-based access control.
///
/// All methods take **relative paths**. Absolute paths and paths that
/// escape the root are rejected by `cap-std` (which we treat as
/// [`WorkspaceCapError::PolicyViolation`]).
///
/// Cloning is cheap (clones the underlying directory file descriptor).
pub struct WorkspaceCapability {
    dir: Dir,
    /// Root path, kept purely for display / logging. Never used for path
    /// resolution — that happens through the `Dir` capability.
    root: PathBuf,
}

impl WorkspaceCapability {
    /// Open a workspace capability rooted at `root`.
    ///
    /// The directory must already exist. Consumes `ambient_authority()`,
    /// which is the moment at which we "pay" for filesystem access; after
    /// this call all FS operations go through the returned capability.
    ///
    /// Call this **once** at startup per workspace; clone the returned
    /// handle for cheap sharing.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, WorkspaceCapError> {
        let root = root.as_ref().to_path_buf();
        let dir = Dir::open_ambient_dir(&root, ambient_authority())?;
        Ok(Self { dir, root })
    }

    /// Display-only root path. Do not use for path manipulation.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Read a file at `rel` (relative path). Absolute / escaping paths
    /// return [`WorkspaceCapError::PolicyViolation`].
    pub fn read(&self, rel: impl AsRef<Path>) -> Result<Vec<u8>, WorkspaceCapError> {
        let rel = rel.as_ref();
        self.dir.read(rel).map_err(|e| translate(rel, e))
    }

    /// Write `data` to `rel`, creating parent directories as needed.
    pub fn write(&self, rel: impl AsRef<Path>, data: &[u8]) -> Result<(), WorkspaceCapError> {
        let rel = rel.as_ref();
        if let Some(parent) = rel.parent() {
            if !parent.as_os_str().is_empty() {
                self.dir
                    .create_dir_all(parent)
                    .map_err(|e| translate(parent, e))?;
            }
        }
        self.dir.write(rel, data).map_err(|e| translate(rel, e))
    }

    /// Create `rel` as a directory (recursively, idempotent).
    pub fn create_dir_all(&self, rel: impl AsRef<Path>) -> Result<(), WorkspaceCapError> {
        let rel = rel.as_ref();
        self.dir.create_dir_all(rel).map_err(|e| translate(rel, e))
    }

    /// Test whether `rel` exists inside the workspace.
    ///
    /// Returns `false` on any error (including policy violation), matching
    /// the conservative "if we cannot verify it exists safely, treat as
    /// absent" default.
    pub fn exists(&self, rel: impl AsRef<Path>) -> bool {
        self.dir.try_exists(rel.as_ref()).unwrap_or(false)
    }

    /// Borrow the underlying [`cap_std::fs::Dir`] for advanced use (e.g.
    /// opening a file in a specific mode). The returned reference is
    /// still capability-bound to this workspace.
    pub fn dir(&self) -> &Dir {
        &self.dir
    }
}

impl Clone for WorkspaceCapability {
    fn clone(&self) -> Self {
        // `Dir::try_clone` duplicates the underlying file descriptor.
        // Failure here would mean the FS layer is in a bad state; panicking
        // in `Clone` is acceptable because this only happens under fd
        // exhaustion, at which point the process is already unhealthy.
        let dir = self
            .dir
            .try_clone()
            .expect("cap-std Dir::try_clone failed — likely FD exhaustion");
        Self {
            dir,
            root: self.root.clone(),
        }
    }
}

impl std::fmt::Debug for WorkspaceCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceCapability")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// Translate a raw `io::Error` from cap-std into our error enum. Absolute
/// paths and escape attempts surface as `PermissionDenied` /
/// `InvalidInput` from cap-std; we relabel those as `PolicyViolation` so
/// callers can distinguish security failures from real I/O issues.
fn translate(rel: &Path, err: std::io::Error) -> WorkspaceCapError {
    use std::io::ErrorKind;
    match err.kind() {
        ErrorKind::PermissionDenied | ErrorKind::InvalidInput => {
            WorkspaceCapError::PolicyViolation(format!(
                "path {} rejected by capability: {err}",
                rel.display()
            ))
        }
        _ => WorkspaceCapError::Io(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_workspace() -> (WorkspaceCapability, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let ws = WorkspaceCapability::open(tmp.path()).unwrap();
        (ws, tmp)
    }

    #[test]
    fn read_write_roundtrip_relative() {
        let (ws, _tmp) = new_workspace();
        ws.write("hello.txt", b"world").unwrap();
        assert_eq!(ws.read("hello.txt").unwrap(), b"world");
    }

    #[test]
    fn write_creates_parent_dirs() {
        let (ws, _tmp) = new_workspace();
        ws.write("a/b/c/file.txt", b"nested").unwrap();
        assert_eq!(ws.read("a/b/c/file.txt").unwrap(), b"nested");
        assert!(ws.exists("a/b/c"));
    }

    #[test]
    fn absolute_path_rejected_on_read() {
        let (ws, _tmp) = new_workspace();
        let err = ws.read("/etc/passwd").unwrap_err();
        assert!(
            matches!(err, WorkspaceCapError::PolicyViolation(_)),
            "expected PolicyViolation, got {err:?}"
        );
    }

    #[test]
    fn absolute_path_rejected_on_write() {
        let (ws, _tmp) = new_workspace();
        let err = ws.write("/tmp/should-not-work", b"x").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn parent_escape_rejected() {
        let (ws, _tmp) = new_workspace();
        let err = ws.read("../../../etc/passwd").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn parent_escape_in_subdir_rejected() {
        let (ws, _tmp) = new_workspace();
        ws.create_dir_all("sub").unwrap();
        let err = ws.read("sub/../../outside").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn nonexistent_file_not_policy_violation() {
        let (ws, _tmp) = new_workspace();
        let err = ws.read("does/not/exist.txt").unwrap_err();
        // "not found" is a real I/O error, not a security violation.
        assert!(
            matches!(err, WorkspaceCapError::Io(_)),
            "expected Io(NotFound), got {err:?}"
        );
    }

    #[test]
    fn exists_returns_false_for_absolute_path() {
        let (ws, _tmp) = new_workspace();
        assert!(!ws.exists("/etc/passwd"));
        assert!(!ws.exists("../../../etc/passwd"));
    }

    #[test]
    fn exists_returns_true_after_write() {
        let (ws, _tmp) = new_workspace();
        assert!(!ws.exists("later.txt"));
        ws.write("later.txt", b"x").unwrap();
        assert!(ws.exists("later.txt"));
    }

    #[test]
    fn clone_shares_same_root() {
        let (ws, _tmp) = new_workspace();
        ws.write("a.txt", b"1").unwrap();
        let ws2 = ws.clone();
        assert_eq!(ws2.read("a.txt").unwrap(), b"1");
        ws2.write("b.txt", b"2").unwrap();
        assert_eq!(ws.read("b.txt").unwrap(), b"2");
    }

    /// Symlink escape test — the whole point of cap-std.
    #[cfg(unix)]
    #[test]
    fn symlink_pointing_outside_root_is_rejected() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"TOPSECRET").unwrap();

        let (ws, tmp) = new_workspace();
        // Create a symlink INSIDE the workspace that points OUTSIDE.
        symlink(
            outside.path().join("secret.txt"),
            tmp.path().join("link.txt"),
        )
        .unwrap();

        // cap-std must refuse to follow this symlink at open time.
        let err = ws.read("link.txt").unwrap_err();
        assert!(
            matches!(err, WorkspaceCapError::PolicyViolation(_)),
            "symlink escape must be rejected, got {err:?}"
        );
    }

    /// Sanity: the root() method is display-only, not a security boundary.
    #[test]
    fn root_is_informational_only() {
        let (ws, tmp) = new_workspace();
        assert_eq!(ws.root(), tmp.path());
    }
}
