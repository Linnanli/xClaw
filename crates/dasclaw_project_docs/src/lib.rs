//! AGENTS.md / CLAUDE.md multi-layer project doc loader.
//!
//! W3 issue #54 — public surface only. Downstream issues fill the impl:
//! - #55: priority `AGENTS.md > CLAUDE.md > .codex/agents.md`
//! - #56: 3-layer merge (user / project / cwd)
//! - #57: recursive upward search to repo root / `$HOME`
//! - #58: `max_bytes` truncation
//! - #59: `OnSessionStart` hook injection
//!
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 and
//! `32-execution-plan.md` W3 task 3 for the full design.

use std::path::{Path, PathBuf};

/// Origin tier of a [`ProjectDoc`].
///
/// The 3-layer model mirrors codex `agents_md.rs`: globally-applied user
/// instructions, repo-scoped project instructions, and request-scoped
/// `cwd` instructions. Order in this enum matches load priority — last
/// layer wins on conflict in the merge step (#56).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocLayer {
    /// Per-user global doc (e.g. `$HOME/.codex/AGENTS.md`).
    UserGlobal,
    /// Repo-scoped doc found by walking up to repo root (e.g. `<repo>/AGENTS.md`).
    Project,
    /// Doc adjacent to the request `cwd` (e.g. `<cwd>/AGENTS.md`).
    Cwd,
}

/// A single project-doc fragment loaded from disk.
///
/// `bytes` is the size of `content` *after* truncation (#58); the raw on-disk
/// size is intentionally not exposed — downstream consumers must not assume
/// `content.len() == file_size`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDoc {
    /// Doc body, possibly truncated by `max_bytes` (#58).
    pub content: String,
    /// Absolute path the doc was read from.
    pub source_path: PathBuf,
    /// Which layer this doc was discovered in.
    pub layer: DocLayer,
    /// Length of `content` in bytes after truncation.
    pub bytes: usize,
}

/// Loader contract for project-scoped instruction docs.
///
/// W3 placeholder: the default implementation returns an empty `Vec`.
/// Downstream issues (#55-#59) plug in real loaders without changing this
/// surface.
pub trait ProjectDocLoader {
    /// Load all docs visible from `cwd`, in load-priority order.
    ///
    /// Implementations must not panic on missing files or permission errors:
    /// project-doc loading is a best-effort enrichment path, not a hard
    /// dependency.
    fn load(&self, cwd: &Path) -> Vec<ProjectDoc>;
}

/// W3 placeholder loader — returns an empty `Vec`.
///
/// Used as the default wiring point until #55 lands the real layered loader.
/// Kept in the public API so downstream crates can compose against the trait
/// without depending on a private struct.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyProjectDocLoader;

impl ProjectDocLoader for EmptyProjectDocLoader {
    fn load(&self, _cwd: &Path) -> Vec<ProjectDoc> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_loader_returns_no_docs() {
        let loader = EmptyProjectDocLoader;
        assert!(loader.load(Path::new("/tmp")).is_empty());
    }
}
