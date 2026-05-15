//! Filesystem-aware path resolver — Phase 3.1.g Step 6.1 (ADR-150).
//!
//! Adds the **minimum IO surface** needed by the symlink-escape pass
//! that wraps `path_validation::validate_command_paths`. The IO surface
//! is intentionally tiny:
//!
//! 1. [`FsResolver`] — three methods (`lstat_kind`, `readlink_absolute`,
//!    `realpath`), each `Option`-returning. Returning `None` from any
//!    method causes the outer walker to **Fail-Safe → Ask** for the
//!    originating target (see ADR-150 §5).
//! 2. [`FsEntryKind`] — file / dir / symlink / special. Special covers
//!    FIFO / socket / char / block devices so the walker never
//!    `realpath`s those (which would block on a missing writer).
//! 3. [`NoopFsResolver`] — every method returns `None`. Equivalent to
//!    "no IO available"; preserves the Phase 3.1.A behaviour (lexical
//!    only) when callers do not want to pay the IO cost.
//! 4. [`resolve_path_chain`] — mirrors upstream
//!    `getPathsForPermissionCheck` (`fsOperations.ts:288-383`); walks
//!    the symlink chain at most [`SYMLOOP_MAX`] hops, returning **all**
//!    on-disk targets the original path could resolve to.
//!
//! POSIX-specific real implementation lives in
//! [`real_posix`][crate::fs_resolver::real_posix]; it is gated behind
//! `cfg(unix)`. Windows lives in a separate step (ADR-150 §6 Step 6.2).
//!
//! # Fail-Safe contract
//!
//! Per AGENTS.md "安全功能必须 Fail-Safe": every IO failure path here
//! returns `None`/empty walk results. The outer
//! `validate_command_paths_with_fs` then escalates the target to
//! `PathValidationOutcome::Ask` (never `Allow`/`Passthrough`).

use std::path::{Path, PathBuf};

/// Maximum number of symlink hops to traverse before giving up.
///
/// Matches upstream `fsOperations.ts:298` `SYMLOOP_MAX = 40` so an
/// audit trail of `chain.len() == 40` here means the same thing as
/// upstream. POSIX systems differ (Linux: 40, macOS: 32, Windows: no
/// defined limit); 40 is the maximum across the three.
pub const SYMLOOP_MAX: usize = 40;

/// Coarse-grained classification of a filesystem entry.
///
/// Mirrors the discriminant set used by upstream `safeResolvePath`
/// (`fsOperations.ts:131-150`). The walker treats `Special` specially:
/// it stops the chain walk without calling `realpath` (FIFOs would
/// otherwise hang waiting for a writer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEntryKind {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Symbolic link (or Windows reparse point with `IO_REPARSE_TAG_SYMLINK`).
    Symlink,
    /// FIFO, socket, character or block device. Walker stops here.
    Special,
}

/// Minimum filesystem surface the symlink-escape walker needs.
///
/// All methods MUST be infallible-from-the-caller's-perspective —
/// callers translate `None` into [`PathValidationOutcome::Ask`][crate::path_validation::PathValidationOutcome::Ask].
///
/// **Why `Option`, not `Result`**: the walker does not need to
/// distinguish ENOENT from EACCES from ELOOP — they all mean "we
/// cannot trust this path → Fail-Safe Ask". Propagating a typed error
/// up would only enable callers to relax the contract, which the
/// safety design forbids (see ADR-150 §5).
pub trait FsResolver {
    /// Equivalent of POSIX `lstat(2)` — does **not** follow the final
    /// symlink. Returns `None` for ENOENT, EACCES, ELOOP, EIO, or any
    /// other IO error.
    fn lstat_kind(&self, path: &Path) -> Option<FsEntryKind>;

    /// Equivalent of POSIX `readlink(2)`. Relative targets must be
    /// resolved against the symlink's **parent** directory (matches
    /// upstream `path.resolve(dirname(p), target)` semantics in
    /// `fsOperations.ts:319-324`). Returns `None` if the path is not
    /// a symlink, the link is broken, or IO fails.
    fn readlink_absolute(&self, path: &Path) -> Option<PathBuf>;

    /// Equivalent of POSIX `realpath(3)` — best-effort canonicalisation
    /// of the deepest existing ancestor. Mirrors upstream
    /// `resolveDeepestExistingAncestorSync` (`fsOperations.ts:217-262`).
    ///
    /// Returns `None` if even the path's first existing ancestor
    /// cannot be canonicalised; callers then Ask.
    fn realpath(&self, path: &Path) -> Option<PathBuf>;
}

/// IO-disabled resolver: every method returns `None`.
///
/// `validate_command_paths_with_fs(.., &NoopFsResolver)` collapses to
/// the pure-string behaviour from Phase 3.1.A — the walker emits an
/// empty chain and the outer validator runs unchanged.
///
/// Useful for:
/// - Callers that do not want to pay the IO cost (e.g. early gates).
/// - Unit tests of the pure-string layer.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopFsResolver;

impl FsResolver for NoopFsResolver {
    fn lstat_kind(&self, _path: &Path) -> Option<FsEntryKind> {
        None
    }
    fn readlink_absolute(&self, _path: &Path) -> Option<PathBuf> {
        None
    }
    fn realpath(&self, _path: &Path) -> Option<PathBuf> {
        None
    }
}

/// Outcome of [`resolve_path_chain`] — the on-disk targets a single
/// input path can resolve to, plus a flag indicating whether the
/// chain walk gave up (max-depth, IO error, or Special-file early
/// exit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedChain {
    /// Distinct absolute paths visited by the walker, in walk order.
    /// The first element is always the original input (after
    /// lexical normalisation by the caller). The last element is the
    /// final resolved target if `terminated_cleanly` is `true`.
    pub steps: Vec<PathBuf>,
    /// `false` when the walker bailed out (loop, max-depth, IO
    /// failure). Callers MUST treat the chain as untrustworthy in
    /// that case (Fail-Safe Ask).
    pub terminated_cleanly: bool,
}

/// Walks the symlink chain rooted at `start`, returning every
/// distinct path the walker visits.
///
/// Algorithm mirrors upstream `getPathsForPermissionCheck`:
/// 1. `lstat` current step.
/// 2. If it's a regular file/dir → stop (clean).
/// 3. If it's a Special (FIFO/socket/etc.) → stop (clean, do NOT
///    realpath; matches upstream `safeResolvePath:131-150`).
/// 4. If it's a symlink → `readlink_absolute`, push the target,
///    loop.
/// 5. If the target was visited before → loop detected, stop (NOT
///    clean).
/// 6. If `lstat_kind` / `readlink_absolute` returns `None` → IO
///    failure, fall back to `realpath(start)` for one last attempt;
///    if that's also `None`, give up (NOT clean).
/// 7. If we exceed [`SYMLOOP_MAX`] hops → give up (NOT clean).
///
/// Returns the walk **in addition to** the original `start` path
/// (which is always `steps[0]`), so callers can run
/// `path_in_workspace` on every step without separately remembering
/// the original.
#[must_use]
pub fn resolve_path_chain(start: &Path, fs: &dyn FsResolver) -> ResolvedChain {
    let mut steps: Vec<PathBuf> = Vec::with_capacity(2);
    steps.push(start.to_path_buf());

    let mut current = start.to_path_buf();

    for _hop in 0..SYMLOOP_MAX {
        match fs.lstat_kind(&current) {
            Some(FsEntryKind::Symlink) => {
                let Some(target) = fs.readlink_absolute(&current) else {
                    return ResolvedChain {
                        steps,
                        terminated_cleanly: false,
                    };
                };
                if steps.iter().any(|s| s == &target) {
                    // Loop detected — record the offending step but
                    // do NOT append it again (avoid unbounded growth
                    // on synthetic cycles in tests).
                    return ResolvedChain {
                        steps,
                        terminated_cleanly: false,
                    };
                }
                steps.push(target.clone());
                current = target;
            }
            Some(FsEntryKind::File | FsEntryKind::Dir | FsEntryKind::Special) => {
                return ResolvedChain {
                    steps,
                    terminated_cleanly: true,
                };
            }
            None => {
                // IO failed (or path doesn't exist). Last-ditch:
                // realpath the deepest existing ancestor.
                if let Some(rp) = fs.realpath(&current) {
                    if !steps.iter().any(|s| s == &rp) {
                        steps.push(rp);
                    }
                    return ResolvedChain {
                        steps,
                        terminated_cleanly: true,
                    };
                }
                return ResolvedChain {
                    steps,
                    terminated_cleanly: false,
                };
            }
        }
    }
    // Exceeded SYMLOOP_MAX.
    ResolvedChain {
        steps,
        terminated_cleanly: false,
    }
}

#[cfg(unix)]
pub mod real_posix;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// In-memory FS for hermetic walker tests. Maps absolute path →
    /// (kind, optional symlink target).
    #[derive(Default)]
    struct MockFs {
        entries: HashMap<PathBuf, (FsEntryKind, Option<PathBuf>)>,
    }

    impl MockFs {
        fn add(&mut self, path: &str, kind: FsEntryKind, link_target: Option<&str>) {
            self.entries
                .insert(PathBuf::from(path), (kind, link_target.map(PathBuf::from)));
        }
    }

    impl FsResolver for MockFs {
        fn lstat_kind(&self, path: &Path) -> Option<FsEntryKind> {
            self.entries.get(path).map(|(k, _)| *k)
        }
        fn readlink_absolute(&self, path: &Path) -> Option<PathBuf> {
            self.entries.get(path).and_then(|(_, t)| t.clone())
        }
        fn realpath(&self, path: &Path) -> Option<PathBuf> {
            // Mock: realpath returns the path itself if and only if
            // it exists; otherwise None. Real impls follow more
            // complex ancestor-walking semantics — covered in
            // `real_posix`.
            self.entries.contains_key(path).then(|| path.to_path_buf())
        }
    }

    #[test]
    fn noop_resolver_returns_none_everywhere() {
        let n = NoopFsResolver;
        assert!(n.lstat_kind(Path::new("/x")).is_none());
        assert!(n.readlink_absolute(Path::new("/x")).is_none());
        assert!(n.realpath(Path::new("/x")).is_none());
    }

    #[test]
    fn req_safety_490_3_1_g_chain_t_sym_1_workspace_link_to_etc() {
        // T-SYM-1: ws/leak -> /etc; reading /ws/leak must surface
        // /etc to the workspace gate.
        let mut fs = MockFs::default();
        fs.add("/ws/leak", FsEntryKind::Symlink, Some("/etc"));
        fs.add("/etc", FsEntryKind::Dir, None);
        let chain = resolve_path_chain(Path::new("/ws/leak"), &fs);
        assert!(chain.terminated_cleanly);
        assert_eq!(chain.steps[0], PathBuf::from("/ws/leak"));
        assert_eq!(chain.steps.last().unwrap(), &PathBuf::from("/etc"));
    }

    #[test]
    fn req_safety_490_3_1_g_chain_t_sym_2_dangling_link_realpath_none() {
        // T-SYM-2: dangling symlink — lstat None on the target.
        let mut fs = MockFs::default();
        fs.add("/ws/evil", FsEntryKind::Symlink, Some("/missing"));
        // /missing not registered → lstat returns None → realpath
        // also None → Fail-Safe.
        let chain = resolve_path_chain(Path::new("/ws/evil"), &fs);
        assert!(!chain.terminated_cleanly);
    }

    #[test]
    fn req_safety_490_3_1_g_chain_t_sym_3_multi_hop_links_all_visited() {
        // T-SYM-3: A -> B -> /etc/shadow; the middle hop must be
        // visible to the gate too.
        let mut fs = MockFs::default();
        fs.add("/ws/a", FsEntryKind::Symlink, Some("/ws/b"));
        fs.add("/ws/b", FsEntryKind::Symlink, Some("/etc/shadow"));
        fs.add("/etc/shadow", FsEntryKind::File, None);
        let chain = resolve_path_chain(Path::new("/ws/a"), &fs);
        assert!(chain.terminated_cleanly);
        assert_eq!(chain.steps.len(), 3);
        assert_eq!(chain.steps[1], PathBuf::from("/ws/b"));
        assert_eq!(chain.steps[2], PathBuf::from("/etc/shadow"));
    }

    #[test]
    fn req_safety_490_3_1_g_chain_t_sym_4_loop_terminates_uncleanly() {
        // T-SYM-4: A -> B -> A loop; walker must give up without
        // OOM and mark the chain unclean.
        let mut fs = MockFs::default();
        fs.add("/ws/a", FsEntryKind::Symlink, Some("/ws/b"));
        fs.add("/ws/b", FsEntryKind::Symlink, Some("/ws/a"));
        let chain = resolve_path_chain(Path::new("/ws/a"), &fs);
        assert!(!chain.terminated_cleanly);
        // Loop detected on the 3rd hop attempt — steps capped at 2.
        assert_eq!(chain.steps.len(), 2);
    }

    #[test]
    fn req_safety_490_3_1_g_chain_t_sym_7_special_file_stops_walk_cleanly() {
        // T-SYM-7: FIFO target — walker must stop without realpath,
        // chain is clean (Special files have well-defined identity).
        let mut fs = MockFs::default();
        fs.add("/ws/pipe", FsEntryKind::Special, None);
        let chain = resolve_path_chain(Path::new("/ws/pipe"), &fs);
        assert!(chain.terminated_cleanly);
        assert_eq!(chain.steps.len(), 1);
    }

    #[test]
    fn req_safety_490_3_1_g_chain_t_sym_8_tilde_expanded_path_walks_normally() {
        // T-SYM-8: caller already expanded `~/.cache` → /home/u/.cache
        // before invoking the walker. The walker just sees an
        // absolute path and follows.
        let mut fs = MockFs::default();
        fs.add("/home/u/.cache", FsEntryKind::Symlink, Some("/etc/secret"));
        fs.add("/etc/secret", FsEntryKind::File, None);
        let chain = resolve_path_chain(Path::new("/home/u/.cache"), &fs);
        assert!(chain.terminated_cleanly);
        assert_eq!(chain.steps.last().unwrap(), &PathBuf::from("/etc/secret"));
    }

    #[test]
    fn req_safety_490_3_1_g_chain_max_depth_terminates_uncleanly() {
        // Deep chain longer than SYMLOOP_MAX must Fail-Safe even
        // without an explicit cycle.
        let mut fs = MockFs::default();
        for i in 0..(SYMLOOP_MAX + 5) {
            let from = format!("/ws/n{i}");
            let to = format!("/ws/n{}", i + 1);
            fs.add(&from, FsEntryKind::Symlink, Some(&to));
        }
        let chain = resolve_path_chain(Path::new("/ws/n0"), &fs);
        assert!(!chain.terminated_cleanly);
        assert!(chain.steps.len() <= SYMLOOP_MAX + 1);
    }

    #[test]
    fn req_safety_490_3_1_g_chain_realpath_fallback_used_when_lstat_none() {
        // When lstat returns None, the walker tries realpath once.
        // Custom resolver: lstat always None, realpath returns
        // /resolved.
        struct Resolver;
        impl FsResolver for Resolver {
            fn lstat_kind(&self, _: &Path) -> Option<FsEntryKind> {
                None
            }
            fn readlink_absolute(&self, _: &Path) -> Option<PathBuf> {
                None
            }
            fn realpath(&self, _: &Path) -> Option<PathBuf> {
                Some(PathBuf::from("/resolved"))
            }
        }
        let chain = resolve_path_chain(Path::new("/missing"), &Resolver);
        assert!(chain.terminated_cleanly);
        assert_eq!(chain.steps.last().unwrap(), &PathBuf::from("/resolved"));
    }

    #[test]
    fn req_safety_490_3_1_g_noop_resolver_produces_unclean_singleton() {
        // NoopFsResolver: lstat None, realpath None → unclean chain
        // with just the original path. Caller can then choose to
        // Pass (preserving 3.1.A behaviour) or Ask, but the walker
        // itself flags the chain untrustworthy.
        let chain = resolve_path_chain(Path::new("/anything"), &NoopFsResolver);
        assert!(!chain.terminated_cleanly);
        assert_eq!(chain.steps.len(), 1);
    }
}
