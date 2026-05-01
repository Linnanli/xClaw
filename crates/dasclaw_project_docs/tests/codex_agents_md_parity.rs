//! W3 issue #63 — codex `agents_md.rs` parity contract.
//!
//! Locks `dasclaw_project_docs` loader output against the reference
//! implementation at
//! [`codex-cli-main/codex-rs/core/src/agents_md.rs`][codex_src] and its test
//! suite at
//! [`codex-cli-main/codex-rs/core/src/agents_md_tests.rs`][codex_tests].
//!
//! Each scenario sets up identical fixtures (tempdirs to mirror codex
//! `tempfile::tempdir()` usage) and asserts BYTE-EQUAL parity for:
//!
//! 1. The discovered doc list (order + per-doc content).
//! 2. The codex-style join of those contents (`parts.join("\n\n")` —
//!    `agents_md.rs` `read_agents_md` line ~187), against an oracle string.
//!
//! Path strings are intentionally NOT byte-compared because both systems use
//! tempdir prefixes that differ at runtime; the loader's `source_path` shape
//! is locked separately by `loader_trait_contract.rs`.
//!
//! ## Documented divergences (intentional, NOT bugs)
//!
//! - **Truncation strategy** (codex byte-cap mid-line vs dasclaw line
//!   boundary). Pinned by `req_project_docs_58_*` in `src/lib.rs`. Single-line
//!   bodies still match codex byte-for-byte because dasclaw falls through to a
//!   UTF-8-boundary cut when no `\n` is present.
//! - **Project-root discovery** (codex auto walk-up via `.git` markers vs
//!   dasclaw explicit `project_root: Option<PathBuf>`). #57 closed-as-obsolete:
//!   desktop-client passes a session-level `workspace_root` so cwd ≈ root.
//! - **`AGENTS.override.md` precedence** (codex prefers per-directory override
//!   file, dasclaw does not yet). Real gap, separate follow-up issue.
//! - **Configurable fallback filenames** (codex `project_doc_fallback_filenames`
//!   runtime configurable; dasclaw fixed
//!   `[AGENTS.md, .dasclaw/AGENTS.md, CLAUDE.md, .codex/AGENTS.md]`). By design
//!   per ADR-106.
//! - **Layer-join separator at `assemble_section` level** (codex
//!   `parts.join("\n\n")` vs dasclaw `PROJECT_DOC_SEPARATOR`). Tracked
//!   separately; this file asserts parity at the LOADER level using
//!   codex-style `\n\n` join, so the discrepancy at the assembly layer does
//!   not leak into these tests.
//!
//! [codex_src]: ../../../../codex-cli-main/codex-rs/core/src/agents_md.rs
//! [codex_tests]: ../../../../codex-cli-main/codex-rs/core/src/agents_md_tests.rs

use std::fs;
use std::path::Path;

use dasclaw_project_docs::{
    DocLayer, LayeredProjectDocLoader, PriorityProjectDocLoader, ProjectDocLoader,
};
use tempfile::tempdir;

/// Codex `read_agents_md` joins layer contents with plain `\n\n` (see
/// `agents_md.rs::read_agents_md` final `parts.join("\n\n")`). Tests use this
/// as the parity oracle so the loader-layer contract stays independent of
/// `assemble_section`'s in-prompt separator choice.
const CODEX_LAYER_JOIN: &str = "\n\n";

fn write_doc(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(&path, body).expect("write fixture doc");
}

fn join_codex_style(docs: &[dasclaw_project_docs::ProjectDoc]) -> String {
    docs.iter()
        .map(|d| d.content.as_str())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>()
        .join(CODEX_LAYER_JOIN)
}

// ───────────────────────── 1. 单层 (single layer) ─────────────────────────

/// Codex `doc_smaller_than_limit_is_returned`: under-cap doc returned verbatim.
///
/// Codex returns the body as-is when below `project_doc_max_bytes`; dasclaw
/// must produce the same content with a single `Cwd`-layer `ProjectDoc`.
#[test]
fn req_project_docs_63_codex_parity_single_doc_under_cap_verbatim() {
    let tmp = tempdir().expect("tempdir");
    write_doc(tmp.path(), "AGENTS.md", "hello world");

    let docs = PriorityProjectDocLoader::default().load(tmp.path());

    assert_eq!(docs.len(), 1, "single doc → exactly one ProjectDoc");
    assert_eq!(docs[0].content, "hello world");
    assert_eq!(docs[0].layer, DocLayer::Cwd);
    assert_eq!(
        join_codex_style(&docs),
        "hello world",
        "codex `read_agents_md` returns the body verbatim under cap",
    );
}

// ───────────────────────── 2. 优先级 (priority) ─────────────────────────

/// Codex prefers `AGENTS.md` over fallback names. dasclaw extends the chain
/// with `.dasclaw/AGENTS.md`, `CLAUDE.md`, `.codex/AGENTS.md`, but `AGENTS.md`
/// at the same directory must still win — matching codex's primary preference.
#[test]
fn req_project_docs_63_codex_parity_agents_md_wins_over_claude_at_same_dir() {
    let tmp = tempdir().expect("tempdir");
    write_doc(tmp.path(), "AGENTS.md", "agents wins");
    write_doc(tmp.path(), "CLAUDE.md", "claude loses");

    let docs = PriorityProjectDocLoader::default().load(tmp.path());

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, "agents wins");
    assert_eq!(
        join_codex_style(&docs),
        "agents wins",
        "codex prefers AGENTS.md; dasclaw must too at the same directory",
    );
}

// ─────────────────────── 3. 嵌套 (nested cwd) ───────────────────────

/// Codex `finds_doc_in_repo_root`: nested cwd inside repo with doc only at the
/// repo root → emits the root doc.
///
/// dasclaw mirrors this via explicit `project_root` (per #57 closed). The
/// emitted layer is `Project` (not `Cwd`), since cwd has no doc of its own.
#[test]
fn req_project_docs_63_codex_parity_finds_doc_in_repo_root_only() {
    let project = tempdir().expect("tempdir");
    write_doc(project.path(), "AGENTS.md", "root level doc");
    let nested = project.path().join("workspace/crate_a");
    fs::create_dir_all(&nested).expect("create nested dir");
    // No doc at nested.

    let docs = LayeredProjectDocLoader::new(None, Some(project.path().to_path_buf())).load(&nested);

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].layer, DocLayer::Project);
    assert_eq!(docs[0].content, "root level doc");
    assert_eq!(
        join_codex_style(&docs),
        "root level doc",
        "codex emits the single doc found between project_root and cwd",
    );
}

/// Codex `concatenates_root_and_cwd_docs`: docs at both repo root and nested
/// cwd → concatenated root-to-cwd with `\n\n`.
#[test]
fn req_project_docs_63_codex_parity_concatenates_root_and_cwd_with_newline_join() {
    let project = tempdir().expect("tempdir");
    write_doc(project.path(), "AGENTS.md", "root doc");
    let nested = project.path().join("workspace/crate_a");
    fs::create_dir_all(&nested).expect("create nested dir");
    write_doc(&nested, "AGENTS.md", "crate doc");

    let docs = LayeredProjectDocLoader::new(None, Some(project.path().to_path_buf())).load(&nested);

    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].layer, DocLayer::Project);
    assert_eq!(docs[1].layer, DocLayer::Cwd);
    assert_eq!(
        join_codex_style(&docs),
        "root doc\n\ncrate doc",
        "codex joins root → cwd doc bodies with plain `\\n\\n`",
    );
}

// ───────────────────────── 4. 三层 (three layers) ─────────────────────────

/// User-global + project-root + nested-cwd → three docs in load order.
/// Extends codex `concatenates_root_and_cwd_docs` with the user-global layer
/// emitted by `instruction_sources` ahead of the doc bundle.
#[test]
fn req_project_docs_63_codex_parity_three_layer_user_project_cwd_join() {
    let home = tempdir().expect("tempdir");
    let project = tempdir().expect("tempdir");
    let cwd = project.path().join("subdir");
    fs::create_dir_all(&cwd).expect("create cwd dir");

    write_doc(home.path(), ".dasclaw/AGENTS.md", "user-doc");
    write_doc(project.path(), "AGENTS.md", "project-doc");
    write_doc(&cwd, "AGENTS.md", "cwd-doc");

    let docs = LayeredProjectDocLoader::new(
        Some(home.path().to_path_buf()),
        Some(project.path().to_path_buf()),
    )
    .load(&cwd);

    assert_eq!(docs.len(), 3);
    assert_eq!(
        docs.iter()
            .map(|d| (d.layer, d.content.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (DocLayer::UserGlobal, "user-doc"),
            (DocLayer::Project, "project-doc"),
            (DocLayer::Cwd, "cwd-doc"),
        ],
    );
    assert_eq!(
        join_codex_style(&docs),
        "user-doc\n\nproject-doc\n\ncwd-doc",
        "codex emits user-global first, then project root, then cwd",
    );
}

// ───────────────────────── 5. 截断 (truncation) ─────────────────────────

/// Codex `doc_larger_than_limit_is_truncated`: oversize doc cut to LIMIT bytes.
///
/// Single-line bodies (no `\n`) are byte-equal between codex and dasclaw because
/// dasclaw's line-boundary truncator falls through to a UTF-8-boundary cut.
/// This is the byte-equal slice of the codex-truncated parity oracle.
#[test]
fn req_project_docs_63_codex_parity_oversize_single_line_truncates_to_limit() {
    const LIMIT: usize = 1024;
    let tmp = tempdir().expect("tempdir");
    let body = "A".repeat(LIMIT * 2); // 2 KiB single line.
    write_doc(tmp.path(), "AGENTS.md", &body);

    let docs = PriorityProjectDocLoader::with_max_bytes(LIMIT).load(tmp.path());

    assert_eq!(docs.len(), 1);
    let codex_oracle = &body[..LIMIT];
    assert_eq!(
        docs[0].content, codex_oracle,
        "single-line truncation must be byte-equal to codex's mid-line cut",
    );
    assert_eq!(docs[0].bytes, LIMIT);
}

// ─────────────────── Edge cases shared with codex ───────────────────

/// Codex `no_doc_file_returns_none`: empty discovery returns nothing.
#[test]
fn req_project_docs_63_codex_parity_no_doc_returns_empty() {
    let tmp = tempdir().expect("tempdir");
    let docs = PriorityProjectDocLoader::default().load(tmp.path());
    assert!(docs.is_empty());
    assert_eq!(
        join_codex_style(&docs),
        "",
        "codex returns Ok(None); the codex-style join of an empty list is empty",
    );
}

/// Codex `zero_byte_limit_disables_docs`: `project_doc_max_bytes == 0` skips
/// loading entirely. dasclaw mirrors via `with_max_bytes(0)`: truncate-to-zero
/// produces empty content which is whitespace-only, so `read_doc_at` skips it.
#[test]
fn req_project_docs_63_codex_parity_zero_max_bytes_disables_loading() {
    let tmp = tempdir().expect("tempdir");
    write_doc(tmp.path(), "AGENTS.md", "should be skipped");

    let docs = PriorityProjectDocLoader::with_max_bytes(0).load(tmp.path());

    assert!(docs.is_empty(), "max_bytes == 0 must disable doc loading");
}
