//! AGENTS.md / CLAUDE.md multi-layer project doc loader.
//!
//! W3 issue #54 — public surface; #55 — single-directory priority routing.
//! Downstream issues fill the rest of the impl:
//! - #56: 3-layer merge (user / project / cwd)
//! - #57: recursive upward search to repo root / `$HOME`
//! - #58: `max_bytes` truncation
//! - #59 (#59a): `assemble_section` helper — joins layered docs into a
//!   single `system_prompt` section. Loaded into the prompt at session
//!   construction time via constructor DI per
//!   [ADR-115](../../docs/plans/architecture-refactor/adr-115-project-docs-not-in-hook.md);
//!   never enters the `dasclaw_hooks` system.
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
    /// Per-user global doc (e.g. `$HOME/.dasclaw/AGENTS.md`, fallback `$HOME/.codex/AGENTS.md`).
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
/// Used as the default wiring point for downstream crates that want to opt
/// out of project-doc loading entirely. Real loading goes through
/// [`PriorityProjectDocLoader`] (#55) or its layered successors (#56-#59).
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyProjectDocLoader;

impl ProjectDocLoader for EmptyProjectDocLoader {
    fn load(&self, _cwd: &Path) -> Vec<ProjectDoc> {
        Vec::new()
    }
}

/// Filenames searched in priority order within a single directory.
///
/// Mirrors ADR-106 with the `.codex/` → `.dasclaw/` migration (issues #99/#100):
/// the project's own namespace (`.dasclaw/AGENTS.md`) outranks third-party
/// conventions (`CLAUDE.md`); `.codex/AGENTS.md` stays read-only for
/// codex-fork compatibility at the bottom.
const DOC_FILENAME_PRIORITY: &[&str] = &[
    "AGENTS.md",
    ".dasclaw/AGENTS.md",
    "CLAUDE.md",
    ".codex/AGENTS.md",
];

/// Single-directory priority loader (#55).
///
/// Walks [`DOC_FILENAME_PRIORITY`] in order under `cwd` and returns the first
/// readable, non-empty doc. Whitespace-only files are skipped (matches codex
/// `agents_md.rs` behaviour). I/O errors fall through silently per the
/// [`ProjectDocLoader`] contract.
///
/// Multi-layer merge (#56), upward search (#57), and byte-cap truncation
/// (#58) are intentionally out of scope here.
#[derive(Debug, Default, Clone, Copy)]
pub struct PriorityProjectDocLoader;

impl ProjectDocLoader for PriorityProjectDocLoader {
    fn load(&self, cwd: &Path) -> Vec<ProjectDoc> {
        for relative in DOC_FILENAME_PRIORITY {
            let path = cwd.join(relative);
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            if content.trim().is_empty() {
                continue;
            }
            let bytes = content.len();
            return vec![ProjectDoc {
                content,
                source_path: path,
                layer: DocLayer::Cwd,
                bytes,
            }];
        }
        Vec::new()
    }
}

/// Separator inserted between adjacent project-doc fragments when assembling
/// them into a single `system_prompt` section.
///
/// Locked to codex `agents_md.rs::AGENTS_MD_SEPARATOR` so prompt diffs across
/// `x-claw`, `codex` and `claw-code` stay readable side-by-side.
pub const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

/// Assemble multi-layer project docs into a single `system_prompt` section.
///
/// Behaviour (locked by `tests/assemble_section.rs`):
///
/// - Returns `None` when `docs` is empty or every fragment has empty content
///   (so callers never emit an orphan separator into the prompt).
/// - Preserves the input order — the [`ProjectDocLoader`] trait already
///   defines load-priority ordering, and re-sorting here would silently
///   override caller-side composition (e.g. cwd overrides).
/// - Skips blank fragments mid-list rather than dropping the whole section,
///   so a missing project-layer doc doesn't create two adjacent separators.
/// - Does not re-truncate `content`; truncation is `max_bytes`'s job (#58).
///
/// Per [ADR-115](../../docs/plans/architecture-refactor/adr-115-project-docs-not-in-hook.md)
/// this helper is invoked at session-construction time via constructor DI;
/// it must not be wired into the `dasclaw_hooks` system.
pub fn assemble_section(docs: &[ProjectDoc]) -> Option<String> {
    let parts: Vec<&str> = docs
        .iter()
        .map(|d| d.content.as_str())
        .filter(|content| !content.is_empty())
        .collect();

    if parts.is_empty() {
        return None;
    }

    Some(parts.join(PROJECT_DOC_SEPARATOR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, body).expect("write fixture");
    }

    #[test]
    fn empty_loader_returns_no_docs() {
        let loader = EmptyProjectDocLoader;
        assert!(loader.load(Path::new("/tmp")).is_empty());
    }

    #[test]
    fn req_project_docs_55_priority_picks_agents_md_when_all_present() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), "AGENTS.md", "agents-body");
        write(tmp.path(), "CLAUDE.md", "claude-body");
        write(tmp.path(), ".dasclaw/AGENTS.md", "dasclaw-body");
        write(tmp.path(), ".codex/AGENTS.md", "codex-body");

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "agents-body");
        assert_eq!(docs[0].source_path, tmp.path().join("AGENTS.md"));
        assert_eq!(docs[0].layer, DocLayer::Cwd);
        assert_eq!(docs[0].bytes, "agents-body".len());
    }

    #[test]
    fn req_project_docs_55_falls_back_to_claude_md_when_agents_absent() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), "CLAUDE.md", "claude-body");

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "claude-body");
        assert_eq!(docs[0].source_path, tmp.path().join("CLAUDE.md"));
    }

    #[test]
    fn req_project_docs_55_falls_back_to_dasclaw_namespace() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".dasclaw/AGENTS.md", "dasclaw-body");

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "dasclaw-body");
        assert_eq!(docs[0].source_path, tmp.path().join(".dasclaw/AGENTS.md"));
    }

    #[test]
    fn req_project_docs_55_falls_back_to_codex_compat_namespace() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".codex/AGENTS.md", "codex-body");

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "codex-body");
        assert_eq!(docs[0].source_path, tmp.path().join(".codex/AGENTS.md"));
    }

    #[test]
    fn req_project_docs_55_dasclaw_wins_over_codex_compat() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".dasclaw/AGENTS.md", "dasclaw-body");
        write(tmp.path(), ".codex/AGENTS.md", "codex-body");

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "dasclaw-body");
    }

    #[test]
    fn req_project_docs_55_dasclaw_namespace_wins_over_claude_md() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".dasclaw/AGENTS.md", "dasclaw-body");
        write(tmp.path(), "CLAUDE.md", "claude-body");

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "dasclaw-body");
        assert_eq!(docs[0].source_path, tmp.path().join(".dasclaw/AGENTS.md"));
    }

    #[test]
    fn req_project_docs_55_returns_empty_when_no_docs_present() {
        let tmp = tempdir().unwrap();

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert!(docs.is_empty());
    }

    #[test]
    fn req_project_docs_55_skips_whitespace_only_file_and_falls_through() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), "AGENTS.md", "   \n\t\n");
        write(tmp.path(), "CLAUDE.md", "claude-body");

        let docs = PriorityProjectDocLoader.load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "claude-body");
        assert_eq!(docs[0].source_path, tmp.path().join("CLAUDE.md"));
    }

    #[test]
    fn req_project_docs_55_missing_directory_returns_empty_without_panic() {
        let docs = PriorityProjectDocLoader.load(Path::new("/nonexistent/x-claw-test-path"));
        assert!(docs.is_empty());
    }
}
