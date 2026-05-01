//! Project-doc injection into [`ReasoningContext::system_prompt`].
//!
//! Implements W3 issue **#59b** under [ADR-115] (`ProjectDoc` 不进 hook 系统):
//! at session-construction time the agent runtime calls a
//! [`ProjectDocLoader`] + [`assemble_section`] and **appends** the result to
//! the existing `system_prompt`, after the dynamic boundary set by upstream
//! prompt builders.
//!
//! Path C2 (entry-point injection) is chosen over C1 (SessionManager field)
//! because:
//!
//! 1. `cwd` is naturally available at the agentic-loop entry, but not at
//!    [`SessionManager::get_or_create_session`]; threading `cwd` through the
//!    user-keyed session API would force every consumer to pick a single
//!    workspace per user, contradicting the `auto-generated sandbox /
//!    user-imported workspace_root` model documented on
//!    [`crate::session::Session`].
//! 2. [`ReasoningContext`] is the type the prompt is actually read from —
//!    injecting at the same site eliminates a layer of indirection and keeps
//!    the operation observable in tests.
//! 3. ADR-115 forbids both the `dasclaw_hooks` system and any hook handler
//!    mutating the prompt; an explicit constructor-DI helper satisfies that
//!    contract by construction.
//!
//! Re-export `ProjectDocLoader` so consumers can implement custom loaders
//! without depending on `dasclaw_project_docs` directly.
//!
//! [ADR-115]: ../../../../docs/plans/architecture-refactor/adr-115-project-docs-not-in-hook.md

use std::path::Path;

pub use dasclaw_project_docs::{
    DocLayer, EmptyProjectDocLoader, LayeredProjectDocLoader, PriorityProjectDocLoader, ProjectDoc,
    ProjectDocLoader, assemble_section,
};

use crate::reasoning_ctx::ReasoningContext;

/// Boundary marker placed between an existing `system_prompt` (the "dynamic
/// boundary" assembled upstream) and the appended project-doc section.
///
/// Two newlines mirror the inter-doc separator inside [`assemble_section`]
/// so the prompt reads as a single contiguous Markdown document. The
/// constant is exposed for tests asserting boundary behaviour.
pub const PROJECT_DOCS_BOUNDARY: &str = "\n\n";

/// Load project docs for `cwd` and append the assembled section to
/// `ctx.system_prompt`.
///
/// Returns the byte length of the appended section, or `0` when nothing was
/// injected (so callers can record a metric without re-doing the work).
///
/// # Behaviour
///
/// * When the loader returns no docs (or every fragment is empty), the
///   context is left untouched — no orphan separator is emitted, mirroring
///   the [`assemble_section`] contract.
/// * When `ctx.system_prompt` is already populated, the assembled section is
///   appended after a [`PROJECT_DOCS_BOUNDARY`].
/// * When `ctx.system_prompt` is empty / unset, the assembled section
///   becomes the entire prompt (no leading boundary).
/// * The function never panics on missing files or permission errors —
///   that's the [`ProjectDocLoader`] trait's contract.
pub fn inject_project_docs_section(
    ctx: &mut ReasoningContext,
    loader: &dyn ProjectDocLoader,
    cwd: &Path,
) -> usize {
    let docs = loader.load(cwd);
    let Some(section) = assemble_section(&docs) else {
        return 0;
    };
    let appended = section.len();
    match ctx.system_prompt.as_mut() {
        Some(existing) if !existing.is_empty() => {
            existing.push_str(PROJECT_DOCS_BOUNDARY);
            existing.push_str(&section);
        }
        _ => {
            ctx.system_prompt = Some(section);
        }
    }
    appended
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Build a [`LayeredProjectDocLoader`] whose three layers all resolve
    /// inside the supplied tempdir, so tests don't touch `$HOME` or escape
    /// the sandbox.
    fn loader_for(tmp: &TempDir) -> LayeredProjectDocLoader {
        LayeredProjectDocLoader::new(
            Some(tmp.path().join("home")),
            Some(tmp.path().join("project")),
        )
    }

    fn write(path: PathBuf, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, body).expect("write fixture");
    }

    #[test]
    fn req_x_claw_agent_59b_injects_section_when_agents_md_exists() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("project");
        write(cwd.join("AGENTS.md"), "## test instructions");

        let mut ctx = ReasoningContext::new();
        let appended = inject_project_docs_section(&mut ctx, &loader_for(&tmp), &cwd);

        assert!(appended > 0);
        let prompt = ctx.system_prompt.expect("prompt populated");
        assert!(
            prompt.contains("## test instructions"),
            "AGENTS.md body must end up in the prompt, got: {prompt}"
        );
    }

    #[test]
    fn req_x_claw_agent_59b_no_injection_when_no_docs() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("project");
        fs::create_dir_all(&cwd).unwrap();

        let mut ctx = ReasoningContext::new();
        let appended = inject_project_docs_section(&mut ctx, &loader_for(&tmp), &cwd);

        assert_eq!(appended, 0);
        assert!(
            ctx.system_prompt.is_none(),
            "no orphan separator may be emitted when docs are absent"
        );
    }

    #[test]
    fn req_x_claw_agent_59b_appends_after_existing_dynamic_boundary() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("project");
        write(cwd.join("AGENTS.md"), "rule-1");

        let mut ctx = ReasoningContext::new();
        ctx.system_prompt = Some("base-template".to_string());

        inject_project_docs_section(&mut ctx, &loader_for(&tmp), &cwd);

        let prompt = ctx.system_prompt.unwrap();
        let expected = format!("base-template{PROJECT_DOCS_BOUNDARY}rule-1");
        assert_eq!(prompt, expected);
        // Sanity: project-doc text comes *after* the existing template.
        let base_idx = prompt.find("base-template").unwrap();
        let doc_idx = prompt.find("rule-1").unwrap();
        assert!(
            base_idx < doc_idx,
            "project docs must follow dynamic boundary"
        );
    }

    #[test]
    fn req_x_claw_agent_59b_handles_nested_layered_docs() {
        let tmp = TempDir::new().unwrap();
        // Layered loader walks `home`, `project`, `cwd` independently.
        write(tmp.path().join("home/.dasclaw/AGENTS.md"), "user-global");
        write(tmp.path().join("project/AGENTS.md"), "repo-rule");
        let cwd = tmp.path().join("project/sub");
        write(cwd.join("AGENTS.md"), "cwd-rule");

        let mut ctx = ReasoningContext::new();
        inject_project_docs_section(&mut ctx, &loader_for(&tmp), &cwd);

        let prompt = ctx.system_prompt.expect("prompt populated");
        for needle in ["user-global", "repo-rule", "cwd-rule"] {
            assert!(prompt.contains(needle), "missing `{needle}` in: {prompt}");
        }
    }

    #[test]
    fn req_x_claw_agent_59b_empty_existing_prompt_replaced_without_boundary() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("project");
        write(cwd.join("AGENTS.md"), "only");

        let mut ctx = ReasoningContext::new();
        ctx.system_prompt = Some(String::new());

        inject_project_docs_section(&mut ctx, &loader_for(&tmp), &cwd);

        let prompt = ctx.system_prompt.unwrap();
        assert_eq!(
            prompt, "only",
            "empty existing prompt must not produce a leading boundary"
        );
    }

    #[test]
    fn req_x_claw_agent_59b_empty_loader_is_noop_with_existing_prompt() {
        let mut ctx = ReasoningContext::new();
        ctx.system_prompt = Some("base".to_string());

        let appended = inject_project_docs_section(
            &mut ctx,
            &EmptyProjectDocLoader,
            Path::new("/nonexistent"),
        );

        assert_eq!(appended, 0);
        assert_eq!(ctx.system_prompt.as_deref(), Some("base"));
    }

    #[test]
    fn req_x_claw_agent_59b_reports_appended_byte_count() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("project");
        write(cwd.join("AGENTS.md"), "abcdef");

        let mut ctx = ReasoningContext::new();
        let appended = inject_project_docs_section(&mut ctx, &loader_for(&tmp), &cwd);

        assert_eq!(appended, "abcdef".len());
    }
}
