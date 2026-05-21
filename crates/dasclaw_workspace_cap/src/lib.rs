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
//! use dasclaw_workspace_cap::WorkspaceCapability;
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

pub mod chunker;
pub mod document;
pub mod layer;
pub mod policy;
pub mod privacy;
pub use policy::{
    NetworkAccess, SandboxPolicy, WritableRoot, default_read_only_subpaths_for_writable_root,
};

use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;

/// A streaming file handle inside a capability-bound workspace.
///
/// Implements `std::io::Read` / `Write` / `Seek` — use it for large files
/// where loading the whole contents into a `Vec<u8>` is wasteful.
pub type WorkspaceFile = cap_std::fs::File;

/// A single entry returned by [`WorkspaceCapability::list_dir`].
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// File name (not a full path). Safe to join back onto the caller's
    /// relative directory context.
    pub name: std::ffi::OsString,
    /// `true` if the entry is a regular directory (not a symlink to one).
    pub is_dir: bool,
    /// `true` if the entry is a regular file.
    pub is_file: bool,
    /// `true` if the entry is a symbolic link (following not attempted).
    pub is_symlink: bool,
    /// File length in bytes. `0` for directories and symlinks.
    pub len: u64,
}

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
        if let Some(parent) = rel.parent()
            && !parent.as_os_str().is_empty()
        {
            self.dir
                .create_dir_all(parent)
                .map_err(|e| translate(parent, e))?;
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

    /// List entries directly under `rel`. Does **not** recurse.
    ///
    /// Passing `""` or `"."` lists the workspace root itself. Each
    /// returned [`DirEntry`] contains only the file name (no path
    /// prefix); the caller joins it back onto its own relative context.
    ///
    /// Entry order is OS-dependent (typically inode / insertion order);
    /// callers that need a stable order must sort.
    pub fn list_dir(&self, rel: impl AsRef<Path>) -> Result<Vec<DirEntry>, WorkspaceCapError> {
        let rel = rel.as_ref();
        let read_dir = if rel.as_os_str().is_empty() || rel == Path::new(".") {
            self.dir.entries().map_err(|e| translate(rel, e))?
        } else {
            self.dir.read_dir(rel).map_err(|e| translate(rel, e))?
        };

        let mut out = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|e| translate(rel, e))?;
            let metadata = entry.metadata().map_err(|e| translate(rel, e))?;
            let file_type = metadata.file_type();
            out.push(DirEntry {
                name: entry.file_name(),
                is_dir: file_type.is_dir(),
                is_file: file_type.is_file(),
                is_symlink: file_type.is_symlink(),
                len: if file_type.is_file() {
                    metadata.len()
                } else {
                    0
                },
            });
        }
        Ok(out)
    }

    /// Remove the file at `rel`. Refuses directories (use
    /// [`Self::remove_dir_all`] for that).
    pub fn remove_file(&self, rel: impl AsRef<Path>) -> Result<(), WorkspaceCapError> {
        let rel = rel.as_ref();
        self.dir.remove_file(rel).map_err(|e| translate(rel, e))
    }

    /// Remove a directory and all its contents recursively.
    ///
    /// Separate from [`Self::remove_file`] so callers make an explicit
    /// destructive choice — deleting a populated directory accidentally
    /// is the kind of mistake a capability API should make harder, not
    /// easier.
    pub fn remove_dir_all(&self, rel: impl AsRef<Path>) -> Result<(), WorkspaceCapError> {
        let rel = rel.as_ref();
        self.dir.remove_dir_all(rel).map_err(|e| translate(rel, e))
    }

    /// Open `rel` for streaming reads. Use this instead of [`Self::read`]
    /// when the file is large and you want to avoid buffering it all in
    /// memory.
    pub fn open_read(&self, rel: impl AsRef<Path>) -> Result<WorkspaceFile, WorkspaceCapError> {
        let rel = rel.as_ref();
        self.dir.open(rel).map_err(|e| translate(rel, e))
    }

    /// Open `rel` for streaming writes (truncates if exists, creates if
    /// not). Parent directories are **not** auto-created — call
    /// [`Self::create_dir_all`] first if needed.
    pub fn open_write(&self, rel: impl AsRef<Path>) -> Result<WorkspaceFile, WorkspaceCapError> {
        let rel = rel.as_ref();
        self.dir.create(rel).map_err(|e| translate(rel, e))
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
            .expect("cap-std Dir::try_clone failed — likely FD exhaustion"); // safety: only fails on fd exhaustion; process is already unhealthy and Clone has no fallible signature
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

    // -- list_dir / remove_file / streaming --

    #[test]
    fn list_dir_empty_root() {
        let (ws, _tmp) = new_workspace();
        let entries = ws.list_dir("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn list_dir_root_with_dot() {
        let (ws, _tmp) = new_workspace();
        ws.write("a.txt", b"1").unwrap();
        let entries = ws.list_dir(".").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a.txt");
    }

    #[test]
    fn list_dir_mixed_content() {
        let (ws, _tmp) = new_workspace();
        ws.write("file1.txt", b"hello").unwrap();
        ws.write("file2.log", b"world!").unwrap();
        ws.create_dir_all("subdir").unwrap();
        let mut entries = ws.list_dir("").unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "file1.txt");
        assert!(entries[0].is_file);
        assert_eq!(entries[0].len, 5);
        assert_eq!(entries[1].name, "file2.log");
        assert_eq!(entries[1].len, 6);
        assert_eq!(entries[2].name, "subdir");
        assert!(entries[2].is_dir);
        assert_eq!(entries[2].len, 0);
    }

    #[test]
    fn list_dir_subdirectory() {
        let (ws, _tmp) = new_workspace();
        ws.write("a/b/x.txt", b"x").unwrap();
        ws.write("a/b/y.txt", b"yy").unwrap();
        let mut entries = ws.list_dir("a/b").unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "x.txt");
        assert_eq!(entries[1].name, "y.txt");
    }

    #[test]
    fn list_dir_absolute_path_rejected() {
        let (ws, _tmp) = new_workspace();
        let err = ws.list_dir("/etc").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn list_dir_escape_rejected() {
        let (ws, _tmp) = new_workspace();
        let err = ws.list_dir("../..").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn list_dir_nonexistent_is_io_error() {
        let (ws, _tmp) = new_workspace();
        let err = ws.list_dir("does/not/exist").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::Io(_)));
    }

    #[test]
    fn remove_file_removes_existing_file() {
        let (ws, _tmp) = new_workspace();
        ws.write("doomed.txt", b"x").unwrap();
        assert!(ws.exists("doomed.txt"));
        ws.remove_file("doomed.txt").unwrap();
        assert!(!ws.exists("doomed.txt"));
    }

    #[test]
    fn remove_file_nonexistent_is_io_error() {
        let (ws, _tmp) = new_workspace();
        let err = ws.remove_file("never.txt").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::Io(_)));
    }

    #[test]
    fn remove_file_absolute_path_rejected() {
        let (ws, _tmp) = new_workspace();
        let err = ws.remove_file("/etc/passwd").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn remove_file_refuses_directory() {
        let (ws, _tmp) = new_workspace();
        ws.create_dir_all("dir").unwrap();
        // remove_file on a directory is an I/O error (IsADirectory /
        // PermissionDenied depending on platform). Either way the dir is
        // still there.
        let _ = ws.remove_file("dir");
        assert!(ws.exists("dir"));
    }

    #[test]
    fn remove_dir_all_removes_populated_directory() {
        let (ws, _tmp) = new_workspace();
        ws.write("keep/me/inner.txt", b"data").unwrap();
        assert!(ws.exists("keep/me/inner.txt"));
        ws.remove_dir_all("keep").unwrap();
        assert!(!ws.exists("keep"));
    }

    #[test]
    fn remove_dir_all_absolute_path_rejected() {
        let (ws, _tmp) = new_workspace();
        let err = ws.remove_dir_all("/tmp").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn open_read_streams_file_contents() {
        use std::io::Read;
        let (ws, _tmp) = new_workspace();
        ws.write("big.bin", &[0xAB; 1024]).unwrap();
        let mut file = ws.open_read("big.bin").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf.len(), 1024);
        assert!(buf.iter().all(|b| *b == 0xAB));
    }

    #[test]
    fn open_write_streams_writes_and_truncates() {
        use std::io::{Read, Write};
        let (ws, _tmp) = new_workspace();
        ws.write("mut.txt", b"OLD LONG CONTENT").unwrap();

        {
            let mut file = ws.open_write("mut.txt").unwrap();
            file.write_all(b"new").unwrap();
            file.flush().unwrap();
        }

        // open_write should truncate — expect "new" not "new LONG CONTENT".
        let mut buf = Vec::new();
        ws.open_read("mut.txt")
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();
        assert_eq!(buf, b"new");
    }

    #[test]
    fn open_read_absolute_path_rejected() {
        let (ws, _tmp) = new_workspace();
        let err = ws.open_read("/etc/passwd").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }

    #[test]
    fn open_write_absolute_path_rejected() {
        let (ws, _tmp) = new_workspace();
        let err = ws.open_write("/tmp/evil").unwrap_err();
        assert!(matches!(err, WorkspaceCapError::PolicyViolation(_)));
    }
}
