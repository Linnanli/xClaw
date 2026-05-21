//! Prompt-injection scanning helpers for system-prompt-injected workspace files.
//!
//! Extracted verbatim from `ironclaw::workspace::mod` (ADR-152 §3 F3.4 slice 7
//! preparatory). See issue #674 / #704.
//!
//! Writes to any file listed in [`SYSTEM_PROMPT_FILES`] are scanned for
//! prompt-injection patterns; high-severity hits are rejected, lower-severity
//! ones are logged.

use ironclaw_safety::{Sanitizer, Severity};

use crate::document::paths;
use crate::error::WorkspaceError;

/// Files injected into the system prompt. Writes to these are scanned for
/// prompt injection patterns and rejected if high-severity matches are found.
pub const SYSTEM_PROMPT_FILES: &[&str] = &[
    paths::SOUL,
    paths::AGENTS,
    paths::USER,
    paths::IDENTITY,
    paths::MEMORY,
    paths::TOOLS,
    paths::HEARTBEAT,
    paths::BOOTSTRAP,
    paths::ASSISTANT_DIRECTIVES,
    paths::PROFILE,
];

/// Returns true if `path` (already normalized) is a system-prompt-injected file.
pub fn is_system_prompt_file(path: &str) -> bool {
    SYSTEM_PROMPT_FILES
        .iter()
        .any(|p| path.eq_ignore_ascii_case(p))
}

/// Shared sanitizer instance — avoids rebuilding Aho-Corasick + regexes on every write.
static SANITIZER: std::sync::LazyLock<Sanitizer> = std::sync::LazyLock::new(Sanitizer::new);

/// Scan content for prompt injection. Returns `Err` if high-severity patterns
/// are detected, otherwise logs warnings and returns `Ok(())`.
pub fn reject_if_injected(path: &str, content: &str) -> Result<(), WorkspaceError> {
    let sanitizer = &*SANITIZER;
    let warnings = sanitizer.detect(content);
    let dominated = warnings.iter().any(|w| w.severity >= Severity::High);
    if dominated {
        let descriptions: Vec<&str> = warnings
            .iter()
            .filter(|w| w.severity >= Severity::High)
            .map(|w| w.description.as_str())
            .collect();
        tracing::warn!(
            target: "ironclaw::safety",
            file = %path,
            "workspace write rejected: prompt injection detected ({})",
            descriptions.join("; "),
        );
        return Err(WorkspaceError::InjectionRejected {
            path: path.to_string(),
            reason: descriptions.join("; "),
        });
    }
    for w in &warnings {
        tracing::warn!(
            target: "ironclaw::safety",
            file = %path, severity = ?w.severity, pattern = %w.pattern,
            "workspace write warning: {}", w.description,
        );
    }
    Ok(())
}
