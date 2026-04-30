//! AGENTS.md / CLAUDE.md multi-layer project doc loader.
//!
//! W3 issues #54 (trait surface), #55 (single-dir priority), #56 (3-layer merge).
//! Downstream:
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

/// Filenames searched in priority order within a project / cwd directory.
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

/// User-global filename priority chain (relative to `$HOME`).
///
/// Mirrors ADR-106 dual-read: `~/.dasclaw/AGENTS.md` is the new primary,
/// `~/.codex/AGENTS.md` retained for codex-fork compatibility.
const USER_GLOBAL_FILENAME_PRIORITY: &[&str] = &[".dasclaw/AGENTS.md", ".codex/AGENTS.md"];

/// Read a single doc, skipping unreadable / whitespace-only files.
///
/// Centralises the “best-effort, never panic” I/O policy mandated by the
/// [`ProjectDocLoader`] trait contract. Whitespace-only files are skipped to
/// match codex `agents_md.rs` behaviour.
fn read_doc_at(path: PathBuf, layer: DocLayer) -> Option<ProjectDoc> {
    let content = std::fs::read_to_string(&path).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    let bytes = content.len();
    Some(ProjectDoc {
        content,
        source_path: path,
        layer,
        bytes,
    })
}

/// Walk a filename priority chain under `base`, returning the first match.
fn first_priority_doc(base: &Path, chain: &[&str], layer: DocLayer) -> Option<ProjectDoc> {
    chain
        .iter()
        .find_map(|relative| read_doc_at(base.join(relative), layer))
}

/// Single-directory priority loader (#55).
///
/// Walks [`DOC_FILENAME_PRIORITY`] in order under `cwd` and returns the first
/// readable, non-empty doc. I/O errors fall through silently per the
/// [`ProjectDocLoader`] contract.
///
/// Multi-layer merge (#56), upward search (#57), and byte-cap truncation
/// (#58) are intentionally out of scope here.
#[derive(Debug, Default, Clone, Copy)]
pub struct PriorityProjectDocLoader;

impl ProjectDocLoader for PriorityProjectDocLoader {
    fn load(&self, cwd: &Path) -> Vec<ProjectDoc> {
        first_priority_doc(cwd, DOC_FILENAME_PRIORITY, DocLayer::Cwd)
            .map(|doc| vec![doc])
            .unwrap_or_default()
    }
}

/// 3-layer merge loader (#56).
///
/// Emits up to 3 docs in load order: `UserGlobal` → `Project` → `Cwd`.
/// Each layer reuses [`DOC_FILENAME_PRIORITY`] (or
/// [`USER_GLOBAL_FILENAME_PRIORITY`] for the user-global layer) and skips
/// empty / unreadable files. When `cwd` is the same path as `project_root`
/// the duplicate `Cwd` emit is suppressed so callers see a single
/// `Project`-layer doc instead of two copies.
///
/// `user_home` and `project_root` are optional so callers can disable the
/// upper layers without composing a different loader (e.g. ephemeral /
/// non-repo `cwd`s in tests). Recursive upward search to find
/// `project_root` is the responsibility of issue #57.
#[derive(Debug, Default, Clone)]
pub struct LayeredProjectDocLoader {
    user_home: Option<PathBuf>,
    project_root: Option<PathBuf>,
}

impl LayeredProjectDocLoader {
    /// Construct a layered loader with explicit user-home and project-root paths.
    pub fn new(user_home: Option<PathBuf>, project_root: Option<PathBuf>) -> Self {
        Self {
            user_home,
            project_root,
        }
    }
}

impl ProjectDocLoader for LayeredProjectDocLoader {
    fn load(&self, cwd: &Path) -> Vec<ProjectDoc> {
        let mut docs = Vec::with_capacity(3);

        let user_doc = self.user_home.as_deref().and_then(|home| {
            first_priority_doc(home, USER_GLOBAL_FILENAME_PRIORITY, DocLayer::UserGlobal)
        });
        if let Some(doc) = user_doc {
            docs.push(doc);
        }

        let project_doc = self
            .project_root
            .as_deref()
            .and_then(|root| first_priority_doc(root, DOC_FILENAME_PRIORITY, DocLayer::Project));
        if let Some(doc) = project_doc {
            docs.push(doc);
        }

        let cwd_is_project_root = self.project_root.as_deref() == Some(cwd);
        if !cwd_is_project_root {
            if let Some(doc) = first_priority_doc(cwd, DOC_FILENAME_PRIORITY, DocLayer::Cwd) {
                docs.push(doc);
            }
        }

        docs
    }
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

    // ───────────────────────── #56 LayeredProjectDocLoader ─────────────────────────

    #[test]
    fn req_project_docs_56_three_layers_emit_in_order() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let cwd = project.path().join("subdir");
        fs::create_dir_all(&cwd).unwrap();

        write(home.path(), ".dasclaw/AGENTS.md", "user-body");
        write(project.path(), "AGENTS.md", "project-body");
        write(&cwd, "AGENTS.md", "cwd-body");

        let loader = LayeredProjectDocLoader::new(
            Some(home.path().to_path_buf()),
            Some(project.path().to_path_buf()),
        );
        let docs = loader.load(&cwd);

        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].layer, DocLayer::UserGlobal);
        assert_eq!(docs[0].content, "user-body");
        assert_eq!(docs[1].layer, DocLayer::Project);
        assert_eq!(docs[1].content, "project-body");
        assert_eq!(docs[2].layer, DocLayer::Cwd);
        assert_eq!(docs[2].content, "cwd-body");
    }

    #[test]
    fn req_project_docs_56_user_plus_project_two_layers() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let cwd = project.path().join("subdir");
        fs::create_dir_all(&cwd).unwrap();

        write(home.path(), ".dasclaw/AGENTS.md", "user-body");
        write(project.path(), "AGENTS.md", "project-body");
        // No doc at cwd.

        let loader = LayeredProjectDocLoader::new(
            Some(home.path().to_path_buf()),
            Some(project.path().to_path_buf()),
        );
        let docs = loader.load(&cwd);

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].layer, DocLayer::UserGlobal);
        assert_eq!(docs[1].layer, DocLayer::Project);
    }

    #[test]
    fn req_project_docs_56_project_plus_cwd_skips_missing_user() {
        let project = tempdir().unwrap();
        let cwd = project.path().join("subdir");
        fs::create_dir_all(&cwd).unwrap();

        write(project.path(), "AGENTS.md", "project-body");
        write(&cwd, "AGENTS.md", "cwd-body");

        let loader = LayeredProjectDocLoader::new(None, Some(project.path().to_path_buf()));
        let docs = loader.load(&cwd);

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].layer, DocLayer::Project);
        assert_eq!(docs[1].layer, DocLayer::Cwd);
    }

    #[test]
    fn req_project_docs_56_cwd_only_when_no_user_no_project() {
        let cwd = tempdir().unwrap();
        write(cwd.path(), "AGENTS.md", "cwd-body");

        let loader = LayeredProjectDocLoader::new(None, None);
        let docs = loader.load(cwd.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].layer, DocLayer::Cwd);
    }

    #[test]
    fn req_project_docs_56_returns_empty_when_no_docs_anywhere() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let cwd = project.path().join("subdir");
        fs::create_dir_all(&cwd).unwrap();

        let loader = LayeredProjectDocLoader::new(
            Some(home.path().to_path_buf()),
            Some(project.path().to_path_buf()),
        );
        assert!(loader.load(&cwd).is_empty());
    }

    #[test]
    fn req_project_docs_56_cwd_equal_to_project_root_dedups_to_single_project_doc() {
        let project = tempdir().unwrap();
        write(project.path(), "AGENTS.md", "project-body");

        let loader = LayeredProjectDocLoader::new(None, Some(project.path().to_path_buf()));
        let docs = loader.load(project.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].layer, DocLayer::Project);
        assert_eq!(docs[0].content, "project-body");
    }

    #[test]
    fn req_project_docs_56_user_global_dasclaw_wins_over_codex_compat() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        write(home.path(), ".dasclaw/AGENTS.md", "user-dasclaw");
        write(home.path(), ".codex/AGENTS.md", "user-codex");

        let loader = LayeredProjectDocLoader::new(Some(home.path().to_path_buf()), None);
        let docs = loader.load(cwd.path());

        let user_docs: Vec<_> = docs
            .iter()
            .filter(|d| d.layer == DocLayer::UserGlobal)
            .collect();
        assert_eq!(user_docs.len(), 1);
        assert_eq!(user_docs[0].content, "user-dasclaw");
    }

    #[test]
    fn req_project_docs_56_user_global_falls_back_to_codex_compat() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        write(home.path(), ".codex/AGENTS.md", "user-codex");

        let loader = LayeredProjectDocLoader::new(Some(home.path().to_path_buf()), None);
        let docs = loader.load(cwd.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].layer, DocLayer::UserGlobal);
        assert_eq!(docs[0].content, "user-codex");
        assert_eq!(docs[0].source_path, home.path().join(".codex/AGENTS.md"));
    }

    #[test]
    fn req_project_docs_56_project_layer_uses_full_priority_chain() {
        let project = tempdir().unwrap();
        // Only CLAUDE.md at project root — should still emit at Project layer.
        write(project.path(), "CLAUDE.md", "claude-project");
        let cwd = project.path().join("subdir");
        fs::create_dir_all(&cwd).unwrap();

        let loader = LayeredProjectDocLoader::new(None, Some(project.path().to_path_buf()));
        let docs = loader.load(&cwd);

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].layer, DocLayer::Project);
        assert_eq!(docs[0].content, "claude-project");
        assert_eq!(docs[0].source_path, project.path().join("CLAUDE.md"));
    }
}
