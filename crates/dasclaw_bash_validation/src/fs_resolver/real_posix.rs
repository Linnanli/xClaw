//! POSIX implementation of [`FsResolver`][super::FsResolver].
//!
//! Unix-only (`cfg(unix)`). Uses `std::fs::symlink_metadata` for
//! `lstat`, `std::fs::read_link` for `readlink`, and a
//! deepest-existing-ancestor walk for `realpath` (mirrors upstream
//! `resolveDeepestExistingAncestorSync` in `fsOperations.ts:217-262`).
//!
//! All errors collapse to `None` per the [`FsResolver`][super::FsResolver]
//! contract; no `unwrap`/`expect` in production paths.

use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use super::{FsEntryKind, FsResolver};

/// Real POSIX filesystem resolver.
///
/// **Cost notice**: every call performs at least one syscall. Callers
/// inside hot loops (e.g. permission gates per command) should batch
/// or cache, not call this per `Path`. The walker in
/// [`resolve_path_chain`][super::resolve_path_chain] makes at most
/// `SYMLOOP_MAX = 40` calls per starting path, which is acceptable
/// for per-Bash-invocation validation.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFsResolver;

impl FsResolver for RealFsResolver {
    fn lstat_kind(&self, path: &Path) -> Option<FsEntryKind> {
        let md = fs::symlink_metadata(path).ok()?;
        let ft = md.file_type();
        if ft.is_symlink() {
            Some(FsEntryKind::Symlink)
        } else if ft.is_dir() {
            Some(FsEntryKind::Dir)
        } else if ft.is_file() {
            Some(FsEntryKind::File)
        } else if ft.is_fifo() || ft.is_socket() || ft.is_char_device() || ft.is_block_device() {
            Some(FsEntryKind::Special)
        } else {
            // Unknown type — treat conservatively as Special so the
            // walker stops without dereferencing.
            Some(FsEntryKind::Special)
        }
    }

    fn readlink_absolute(&self, path: &Path) -> Option<PathBuf> {
        let target = fs::read_link(path).ok()?;
        if target.is_absolute() {
            Some(target)
        } else {
            // Relative — resolve against the symlink's parent
            // (matches upstream `path.resolve(dirname(p), target)`).
            let parent = path.parent()?;
            Some(parent.join(target))
        }
    }

    fn realpath(&self, path: &Path) -> Option<PathBuf> {
        // Fast path: canonicalize works → done.
        if let Ok(rp) = fs::canonicalize(path) {
            return Some(rp);
        }
        // Slow path: walk up until we find an existing ancestor,
        // canonicalize that, rejoin the missing tail. Mirrors
        // `resolveDeepestExistingAncestorSync`.
        let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
        let mut cursor = path;
        loop {
            let parent = cursor.parent()?;
            if let Some(name) = cursor.file_name() {
                tail.push(name);
            }
            if let Ok(rp) = fs::canonicalize(parent) {
                let mut out = rp;
                for seg in tail.iter().rev() {
                    out.push(seg);
                }
                return Some(out);
            }
            if parent.as_os_str().is_empty() || parent == cursor {
                return None;
            }
            cursor = parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    #[test]
    fn real_resolver_classifies_regular_file_as_file() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("a.txt");
        fs::write(&f, b"x").unwrap();
        assert_eq!(RealFsResolver.lstat_kind(&f), Some(FsEntryKind::File));
    }

    #[test]
    fn real_resolver_classifies_directory_as_dir() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            RealFsResolver.lstat_kind(dir.path()),
            Some(FsEntryKind::Dir)
        );
    }

    #[test]
    fn real_resolver_classifies_symlink_as_symlink() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("real");
        fs::write(&target, b"x").unwrap();
        let link = dir.path().join("alias");
        symlink(&target, &link).unwrap();
        assert_eq!(RealFsResolver.lstat_kind(&link), Some(FsEntryKind::Symlink));
    }

    #[test]
    fn real_resolver_lstat_missing_returns_none() {
        assert!(RealFsResolver
            .lstat_kind(Path::new("/nonexistent/3.1.g.1/probe"))
            .is_none());
    }

    #[test]
    fn real_resolver_readlink_absolutifies_relative_target() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("real");
        fs::write(&target, b"x").unwrap();
        let link = dir.path().join("alias");
        // Create a *relative* symlink so we can verify the
        // parent-relative resolution rule.
        symlink("real", &link).unwrap();
        let got = RealFsResolver.readlink_absolute(&link).unwrap();
        assert_eq!(got, dir.path().join("real"));
    }

    #[test]
    fn real_resolver_readlink_keeps_absolute_target_absolute() {
        let dir = TempDir::new().unwrap();
        let link = dir.path().join("abslink");
        symlink("/tmp/some-abs-target", &link).unwrap();
        let got = RealFsResolver.readlink_absolute(&link).unwrap();
        assert_eq!(got, PathBuf::from("/tmp/some-abs-target"));
    }

    #[test]
    fn real_resolver_realpath_resolves_existing_path() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("real");
        fs::write(&f, b"x").unwrap();
        let got = RealFsResolver.realpath(&f).unwrap();
        // canonicalize resolves the temp dir prefix (e.g. macOS
        // /var → /private/var); we only assert it agrees with
        // canonicalize().
        assert_eq!(got, fs::canonicalize(&f).unwrap());
    }

    #[test]
    fn real_resolver_realpath_walks_to_deepest_existing_ancestor() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("does-not-exist/leaf");
        let got = RealFsResolver.realpath(&missing).unwrap();
        // The existing ancestor is `dir.path()`; the tail
        // `does-not-exist/leaf` is appended in original order.
        let expected = fs::canonicalize(dir.path())
            .unwrap()
            .join("does-not-exist")
            .join("leaf");
        assert_eq!(got, expected);
    }
}
