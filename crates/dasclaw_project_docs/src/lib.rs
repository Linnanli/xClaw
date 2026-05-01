//! AGENTS.md / CLAUDE.md multi-layer project doc loader.
//!
//! W3 issues #54 (trait surface), #55 (single-dir priority), #56 (3-layer merge),
//! #58 (`max_bytes` truncation).
//! Downstream:
//! - #59 (#59a): `assemble_section` helper — joins layered docs into a
//!   single `system_prompt` section. Loaded into the prompt at session
//!   construction time via constructor DI per
//!   [ADR-115](../../docs/plans/architecture-refactor/adr-115-project-docs-not-in-hook.md);
//!   never enters the `dasclaw_hooks` system.
//!
//! Note: #57 (recursive upward search) was closed as obsolete — desktop-client
//! uses an explicit session-level `workspace_root` (auto-generated sandbox or
//! user-imported), so cwd ≈ project_root and walk-up is unnecessary.
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

/// Default per-doc byte cap (#58). Mirrors codex `project_doc_max_bytes` default of 8 KiB.
pub const DEFAULT_PROJECT_DOC_MAX_BYTES: usize = 8 * 1024;

/// Truncate `content` to at most `max_bytes`, preserving complete lines.
///
/// If the content fits within the cap it is returned as-is. Otherwise the
/// longest prefix of complete lines (terminated by `\n`) that fits in
/// `max_bytes` is kept. If even the first line is longer than `max_bytes`
/// the prefix is cut at the largest valid UTF-8 char boundary `<= max_bytes`,
/// guaranteeing a valid `String` without panicking on multi-byte chars.
fn truncate_to_max_bytes(content: String, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content;
    }
    let mut out = String::with_capacity(max_bytes);
    for line in content.split_inclusive('\n') {
        if out.len() + line.len() > max_bytes {
            break;
        }
        out.push_str(line);
    }
    if !out.is_empty() {
        return out;
    }
    // Single line wider than cap: trim back to a UTF-8 boundary.
    let mut end = max_bytes.min(content.len());
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    content[..end].to_string()
}

/// Read a single doc, skipping unreadable / whitespace-only files.
///
/// Centralises the “best-effort, never panic” I/O policy mandated by the
/// [`ProjectDocLoader`] trait contract. Whitespace-only files are skipped to
/// match codex `agents_md.rs` behaviour. `max_bytes` (#58) caps the doc body
/// at line boundaries before whitespace check, mirroring codex
/// `project_doc_max_bytes`.
fn read_doc_at(path: PathBuf, layer: DocLayer, max_bytes: usize) -> Option<ProjectDoc> {
    let raw = std::fs::read_to_string(&path).ok()?;
    let content = truncate_to_max_bytes(raw, max_bytes);
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
fn first_priority_doc(
    base: &Path,
    chain: &[&str],
    layer: DocLayer,
    max_bytes: usize,
) -> Option<ProjectDoc> {
    chain
        .iter()
        .find_map(|relative| read_doc_at(base.join(relative), layer, max_bytes))
}

/// Single-directory priority loader (#55).
///
/// Walks [`DOC_FILENAME_PRIORITY`] in order under `cwd` and returns the first
/// readable, non-empty doc. I/O errors fall through silently per the
/// [`ProjectDocLoader`] contract. Doc bodies are truncated at `max_bytes`
/// line-boundaries (#58, default [`DEFAULT_PROJECT_DOC_MAX_BYTES`]).
///
/// Multi-layer merge is provided by [`LayeredProjectDocLoader`] (#56).
#[derive(Debug, Clone)]
pub struct PriorityProjectDocLoader {
    max_bytes: usize,
}

impl Default for PriorityProjectDocLoader {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_PROJECT_DOC_MAX_BYTES,
        }
    }
}

impl PriorityProjectDocLoader {
    /// Construct a loader with a custom per-doc byte cap (#58).
    pub fn with_max_bytes(max_bytes: usize) -> Self {
        Self { max_bytes }
    }
}

impl ProjectDocLoader for PriorityProjectDocLoader {
    fn load(&self, cwd: &Path) -> Vec<ProjectDoc> {
        first_priority_doc(cwd, DOC_FILENAME_PRIORITY, DocLayer::Cwd, self.max_bytes)
            .map(|doc| vec![doc])
            .unwrap_or_default()
    }
}

/// 3-layer merge loader (#56) with per-doc byte cap (#58).
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
/// non-repo `cwd`s in tests).
#[derive(Debug, Clone)]
pub struct LayeredProjectDocLoader {
    user_home: Option<PathBuf>,
    project_root: Option<PathBuf>,
    max_bytes: usize,
}

impl Default for LayeredProjectDocLoader {
    fn default() -> Self {
        Self {
            user_home: None,
            project_root: None,
            max_bytes: DEFAULT_PROJECT_DOC_MAX_BYTES,
        }
    }
}

impl LayeredProjectDocLoader {
    /// Construct a layered loader with explicit user-home and project-root paths.
    ///
    /// Uses [`DEFAULT_PROJECT_DOC_MAX_BYTES`] as the per-doc byte cap; call
    /// [`Self::with_max_bytes`] to override.
    pub fn new(user_home: Option<PathBuf>, project_root: Option<PathBuf>) -> Self {
        Self {
            user_home,
            project_root,
            max_bytes: DEFAULT_PROJECT_DOC_MAX_BYTES,
        }
    }

    /// Override the per-doc byte cap (#58).
    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }
}

impl ProjectDocLoader for LayeredProjectDocLoader {
    fn load(&self, cwd: &Path) -> Vec<ProjectDoc> {
        let mut docs = Vec::with_capacity(3);

        let user_doc = self.user_home.as_deref().and_then(|home| {
            first_priority_doc(
                home,
                USER_GLOBAL_FILENAME_PRIORITY,
                DocLayer::UserGlobal,
                self.max_bytes,
            )
        });
        if let Some(doc) = user_doc {
            docs.push(doc);
        }

        let project_doc = self.project_root.as_deref().and_then(|root| {
            first_priority_doc(
                root,
                DOC_FILENAME_PRIORITY,
                DocLayer::Project,
                self.max_bytes,
            )
        });
        if let Some(doc) = project_doc {
            docs.push(doc);
        }

        let cwd_is_project_root = self.project_root.as_deref() == Some(cwd);
        if !cwd_is_project_root {
            if let Some(doc) =
                first_priority_doc(cwd, DOC_FILENAME_PRIORITY, DocLayer::Cwd, self.max_bytes)
            {
                docs.push(doc);
            }
        }

        docs
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

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

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

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "claude-body");
        assert_eq!(docs[0].source_path, tmp.path().join("CLAUDE.md"));
    }

    #[test]
    fn req_project_docs_55_falls_back_to_dasclaw_namespace() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".dasclaw/AGENTS.md", "dasclaw-body");

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "dasclaw-body");
        assert_eq!(docs[0].source_path, tmp.path().join(".dasclaw/AGENTS.md"));
    }

    #[test]
    fn req_project_docs_55_falls_back_to_codex_compat_namespace() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".codex/AGENTS.md", "codex-body");

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "codex-body");
        assert_eq!(docs[0].source_path, tmp.path().join(".codex/AGENTS.md"));
    }

    #[test]
    fn req_project_docs_55_dasclaw_wins_over_codex_compat() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".dasclaw/AGENTS.md", "dasclaw-body");
        write(tmp.path(), ".codex/AGENTS.md", "codex-body");

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "dasclaw-body");
    }

    #[test]
    fn req_project_docs_55_dasclaw_namespace_wins_over_claude_md() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), ".dasclaw/AGENTS.md", "dasclaw-body");
        write(tmp.path(), "CLAUDE.md", "claude-body");

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "dasclaw-body");
        assert_eq!(docs[0].source_path, tmp.path().join(".dasclaw/AGENTS.md"));
    }

    #[test]
    fn req_project_docs_55_returns_empty_when_no_docs_present() {
        let tmp = tempdir().unwrap();

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert!(docs.is_empty());
    }

    #[test]
    fn req_project_docs_55_skips_whitespace_only_file_and_falls_through() {
        let tmp = tempdir().unwrap();
        write(tmp.path(), "AGENTS.md", "   \n\t\n");
        write(tmp.path(), "CLAUDE.md", "claude-body");

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "claude-body");
        assert_eq!(docs[0].source_path, tmp.path().join("CLAUDE.md"));
    }

    #[test]
    fn req_project_docs_55_missing_directory_returns_empty_without_panic() {
        let docs =
            PriorityProjectDocLoader::default().load(Path::new("/nonexistent/x-claw-test-path"));
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

    // -------------------- #58 max_bytes truncation --------------------

    fn make_lined_body(line_count: usize, line_body: &str) -> String {
        let mut s = String::new();
        for _ in 0..line_count {
            s.push_str(line_body);
            s.push('\n');
        }
        s
    }

    #[test]
    fn req_project_docs_58_default_cap_is_8_kib() {
        assert_eq!(DEFAULT_PROJECT_DOC_MAX_BYTES, 8 * 1024);
    }

    #[test]
    fn req_project_docs_58_under_cap_returned_verbatim() {
        let tmp = tempdir().unwrap();
        // ~5 KiB body of 64-byte lines (64 * 80 = 5120).
        let body = make_lined_body(80, &"a".repeat(63));
        assert!(body.len() < DEFAULT_PROJECT_DOC_MAX_BYTES);
        write(tmp.path(), "AGENTS.md", &body);

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, body);
        assert_eq!(docs[0].bytes, body.len());
    }

    #[test]
    fn req_project_docs_58_over_cap_truncated_at_line_boundary() {
        let tmp = tempdir().unwrap();
        // ~10 KiB body of 64-byte lines: 160 lines * 64 = 10240.
        let body = make_lined_body(160, &"b".repeat(63));
        assert!(body.len() > DEFAULT_PROJECT_DOC_MAX_BYTES);
        write(tmp.path(), "AGENTS.md", &body);

        let docs = PriorityProjectDocLoader::default().load(tmp.path());

        assert_eq!(docs.len(), 1);
        let content = &docs[0].content;
        assert!(content.len() <= DEFAULT_PROJECT_DOC_MAX_BYTES);
        // Last char must be '\n' — i.e. the last surviving line is complete.
        assert_eq!(content.as_bytes().last().copied(), Some(b'\n'));
        assert_eq!(docs[0].bytes, content.len());
    }

    #[test]
    fn req_project_docs_58_with_max_bytes_override_applied() {
        let tmp = tempdir().unwrap();
        // 4 lines of 100 bytes each = 400 bytes; cap to 250 -> exactly 2 lines.
        let body = make_lined_body(4, &"c".repeat(99));
        write(tmp.path(), "AGENTS.md", &body);

        let docs = PriorityProjectDocLoader::with_max_bytes(250).load(tmp.path());

        assert_eq!(docs.len(), 1);
        let content = &docs[0].content;
        assert!(content.len() <= 250);
        assert_eq!(content.matches('\n').count(), 2);
    }

    #[test]
    fn req_project_docs_58_layered_loader_truncates_each_layer() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let body = make_lined_body(4, &"d".repeat(99)); // 400 bytes
        write(home.path(), ".dasclaw/AGENTS.md", &body);
        write(project.path(), "AGENTS.md", &body);

        let loader = LayeredProjectDocLoader::new(
            Some(home.path().to_path_buf()),
            Some(project.path().to_path_buf()),
        )
        .with_max_bytes(150); // 1 line of 100 bytes fits, 2nd would overflow

        let docs = loader.load(project.path());

        assert_eq!(docs.len(), 2);
        for doc in &docs {
            assert!(doc.content.len() <= 150);
            assert_eq!(doc.content.matches('\n').count(), 1);
        }
    }

    #[test]
    fn req_project_docs_58_single_line_over_cap_keeps_utf8_boundary() {
        let tmp = tempdir().unwrap();
        // 4-byte UTF-8 char repeated -> 100 chars * 4 bytes = 400 bytes, no '\n'.
        let body = "🦀".repeat(100);
        assert_eq!(body.len(), 400);
        write(tmp.path(), "AGENTS.md", &body);

        let docs = PriorityProjectDocLoader::with_max_bytes(150).load(tmp.path());

        assert_eq!(docs.len(), 1);
        let content = &docs[0].content;
        assert!(content.len() <= 150);
        // Must remain valid UTF-8 (round-trip without panic).
        let _: &str = content.as_str();
        // 150 / 4 = 37.5 → 37 crabs → 148 bytes.
        assert_eq!(content.chars().count(), 37);
        assert_eq!(content.len(), 148);
    }
}
