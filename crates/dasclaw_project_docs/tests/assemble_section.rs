//! W3 issue #59a — `assemble_section` helper contract.
//!
//! Locks the assembly behaviour of multi-layer project docs into a single
//! string suitable for injection into the LLM `system_prompt` after the
//! dynamic boundary. See [ADR-115](../../docs/plans/architecture-refactor/adr-115-project-docs-not-in-hook.md):
//! this helper is invoked at session-construction time via constructor DI;
//! it never enters the `dasclaw_hooks` system.
//!
//! The separator and ordering match codex `agents_md.rs::AGENTS_MD_SEPARATOR`
//! (`"\n\n--- project-doc ---\n\n"`) so cross-tool prompt diffs stay readable.

use std::path::PathBuf;

use dasclaw_project_docs::{assemble_section, DocLayer, ProjectDoc, PROJECT_DOC_SEPARATOR};

fn doc(layer: DocLayer, path: &str, content: &str) -> ProjectDoc {
    ProjectDoc {
        content: content.to_string(),
        source_path: PathBuf::from(path),
        layer,
        bytes: content.len(),
    }
}

#[test]
fn req_project_docs_59a_separator_matches_codex() {
    // Locked to codex `AGENTS_MD_SEPARATOR` for cross-tool diff readability.
    assert_eq!(PROJECT_DOC_SEPARATOR, "\n\n--- project-doc ---\n\n");
}

#[test]
fn req_project_docs_59a_empty_slice_returns_none() {
    // Avoid emitting a stray separator or empty section into the prompt.
    assert!(assemble_section(&[]).is_none());
}

#[test]
fn req_project_docs_59a_only_blank_content_returns_none() {
    // Blank-content docs (e.g. file existed but was empty after truncation)
    // must not produce orphan separators.
    let docs = [doc(DocLayer::UserGlobal, "/u/AGENTS.md", "")];
    assert!(assemble_section(&docs).is_none());
}

#[test]
fn req_project_docs_59a_single_doc_returns_content_verbatim() {
    // No separator should be added when there is exactly one fragment.
    let docs = [doc(DocLayer::Project, "/p/AGENTS.md", "be helpful")];
    assert_eq!(assemble_section(&docs).as_deref(), Some("be helpful"));
}

#[test]
fn req_project_docs_59a_three_layer_merge_preserves_input_order() {
    // The trait contract states `load()` returns docs in load-priority order;
    // assembly must preserve that order rather than re-sorting by `DocLayer`,
    // since callers may have already trimmed/reordered (e.g. cwd overrides).
    let docs = [
        doc(DocLayer::UserGlobal, "/u/AGENTS.md", "global"),
        doc(DocLayer::Project, "/p/AGENTS.md", "project"),
        doc(DocLayer::Cwd, "/p/sub/AGENTS.md", "cwd"),
    ];
    let expected = format!("global{sep}project{sep}cwd", sep = PROJECT_DOC_SEPARATOR);
    assert_eq!(assemble_section(&docs).as_deref(), Some(expected.as_str()));
}

#[test]
fn req_project_docs_59a_blank_fragments_are_skipped_not_dropped_around() {
    // A blank middle fragment must not produce two adjacent separators
    // (which would confuse the model with empty section headers).
    let docs = [
        doc(DocLayer::UserGlobal, "/u/AGENTS.md", "global"),
        doc(DocLayer::Project, "/p/AGENTS.md", ""),
        doc(DocLayer::Cwd, "/p/sub/AGENTS.md", "cwd"),
    ];
    let expected = format!("global{sep}cwd", sep = PROJECT_DOC_SEPARATOR);
    assert_eq!(assemble_section(&docs).as_deref(), Some(expected.as_str()));
}

#[test]
fn req_project_docs_59a_truncated_content_still_assembles() {
    // After #58 truncation the `content` may end mid-sentence; `assemble_section`
    // must not assert on byte-shape and must not re-truncate.
    let truncated = "a".repeat(8 * 1024);
    let docs = [doc(DocLayer::Project, "/p/AGENTS.md", &truncated)];
    let out = assemble_section(&docs).expect("non-empty input must assemble");
    assert_eq!(out.len(), 8 * 1024);
}

#[test]
fn req_project_docs_59a_only_cwd_layer_returns_content_verbatim() {
    // When only the cwd layer is loaded (e.g. user has no global AGENTS.md
    // and the loader skipped project layer), output must equal that content
    // without any separator prefix/suffix.
    let docs = [doc(DocLayer::Cwd, "/work/AGENTS.md", "cwd-only")];
    assert_eq!(assemble_section(&docs).as_deref(), Some("cwd-only"));
}
