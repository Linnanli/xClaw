//! `trust_resolver` — worktree / cwd trust evaluation.
//!
//! Ported from `claw-code/rust/crates/runtime/src/trust_resolver.rs`.
//!
//! # Surface
//!
//! - [`detect_trust_prompt`] — substring scan for known TUI trust prompts.
//! - [`TrustConfig`] — builder over allowlist + denylist of `PathBuf` roots.
//! - [`TrustResolver::resolve`] — fold a screen capture + cwd into a
//!   [`TrustDecision`] with an event trail.
//! - [`TrustResolver::trusts`] — `bool` shortcut: `cwd ∈ allowlist ∧ cwd ∉ denylist`.
//! - [`path_matches_trusted_root`] — pure-string matcher reused by callers
//!   that already have a normalised root string.
//!
//! Decision precedence: **denylist > allowlist > require-approval**. The
//! denylist is checked first so an explicit deny always wins, even when an
//! ancestor allowlist root would otherwise match.
//!
//! `normalize_path` falls back to the original path when `canonicalize` fails
//! (path not yet on disk, permission errors, …) — mandatory for tests that
//! work with synthetic paths under `/tmp/worktrees/...`.

use std::path::{Path, PathBuf};

const TRUST_PROMPT_CUES: &[&str] = &[
    "do you trust the files in this folder",
    "trust the files in this folder",
    "trust this folder",
    "allow and continue",
    "yes, proceed",
];

/// Outcome handed back to the caller after a trust evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustPolicy {
    /// Cwd matched the allowlist — auto-trust the worktree.
    AutoTrust,
    /// Cwd did not match any list — caller must surface to the operator.
    RequireApproval,
    /// Cwd matched the denylist — refuse to trust.
    Deny,
}

/// Audit event emitted during resolution. Order in the returned slice
/// matches emission order; downstream pipelines forward them verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustEvent {
    TrustRequired { cwd: String },
    TrustResolved { cwd: String, policy: TrustPolicy },
    TrustDenied { cwd: String, reason: String },
}

/// Allowlist / denylist of trust roots. Empty by default — every cwd then
/// resolves to `RequireApproval` whenever a prompt is detected.
#[derive(Debug, Clone, Default)]
pub struct TrustConfig {
    allowlisted: Vec<PathBuf>,
    denied: Vec<PathBuf>,
}

impl TrustConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a directory to the allowlist (cwd or any descendant matches).
    #[must_use]
    pub fn with_allowlisted(mut self, path: impl Into<PathBuf>) -> Self {
        self.allowlisted.push(path.into());
        self
    }

    /// Add a directory to the denylist (cwd or any descendant matches).
    /// Denylist beats allowlist.
    #[must_use]
    pub fn with_denied(mut self, path: impl Into<PathBuf>) -> Self {
        self.denied.push(path.into());
        self
    }
}

/// Top-level decision surfaced by [`TrustResolver::resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustDecision {
    /// No trust prompt was detected — caller can keep streaming.
    NotRequired,
    /// A trust prompt was detected; act according to `policy` and emit
    /// `events`.
    Required {
        policy: TrustPolicy,
        events: Vec<TrustEvent>,
    },
}

impl TrustDecision {
    #[must_use]
    pub fn policy(&self) -> Option<TrustPolicy> {
        match self {
            Self::NotRequired => None,
            Self::Required { policy, .. } => Some(*policy),
        }
    }

    #[must_use]
    pub fn events(&self) -> &[TrustEvent] {
        match self {
            Self::NotRequired => &[],
            Self::Required { events, .. } => events,
        }
    }
}

/// Stateless evaluator. Cheap to clone — holds only the config.
#[derive(Debug, Clone)]
pub struct TrustResolver {
    config: TrustConfig,
}

impl TrustResolver {
    #[must_use]
    pub fn new(config: TrustConfig) -> Self {
        Self { config }
    }

    /// Inspect `screen_text`; if it contains a known trust prompt, classify
    /// `cwd` against the configured lists and emit the audit trail.
    #[must_use]
    pub fn resolve(&self, cwd: &str, screen_text: &str) -> TrustDecision {
        if !detect_trust_prompt(screen_text) {
            return TrustDecision::NotRequired;
        }

        let mut events = vec![TrustEvent::TrustRequired {
            cwd: cwd.to_owned(),
        }];

        if let Some(matched_root) = self
            .config
            .denied
            .iter()
            .find(|root| path_matches(cwd, root))
        {
            let reason = format!("cwd matches denied trust root: {}", matched_root.display());
            events.push(TrustEvent::TrustDenied {
                cwd: cwd.to_owned(),
                reason,
            });
            return TrustDecision::Required {
                policy: TrustPolicy::Deny,
                events,
            };
        }

        if self
            .config
            .allowlisted
            .iter()
            .any(|root| path_matches(cwd, root))
        {
            events.push(TrustEvent::TrustResolved {
                cwd: cwd.to_owned(),
                policy: TrustPolicy::AutoTrust,
            });
            return TrustDecision::Required {
                policy: TrustPolicy::AutoTrust,
                events,
            };
        }

        TrustDecision::Required {
            policy: TrustPolicy::RequireApproval,
            events,
        }
    }

    /// Bool shortcut without the audit trail: cwd is on the allowlist and
    /// not on the denylist.
    #[must_use]
    pub fn trusts(&self, cwd: &str) -> bool {
        !self
            .config
            .denied
            .iter()
            .any(|root| path_matches(cwd, root))
            && self
                .config
                .allowlisted
                .iter()
                .any(|root| path_matches(cwd, root))
    }
}

/// Returns `true` iff `screen_text` matches one of the known TUI trust
/// prompts (case-insensitive substring match).
#[must_use]
pub fn detect_trust_prompt(screen_text: &str) -> bool {
    let lowered = screen_text.to_ascii_lowercase();
    TRUST_PROMPT_CUES
        .iter()
        .any(|needle| lowered.contains(needle))
}

/// Pure-string equivalent of the in-resolver path comparison. Intended for
/// callers that already have a canonicalised trusted root string.
#[must_use]
pub fn path_matches_trusted_root(cwd: &str, trusted_root: &str) -> bool {
    path_matches(cwd, &normalize_path(Path::new(trusted_root)))
}

fn path_matches(candidate: &str, root: &Path) -> bool {
    let candidate = normalize_path(Path::new(candidate));
    let root = normalize_path(root);
    candidate == root || candidate.starts_with(&root)
}

fn normalize_path(path: &Path) -> PathBuf {
    // Tests work with synthetic /tmp/worktrees/... paths that don't exist on
    // disk; canonicalize() will fail with ENOENT — fall back to the literal
    // path so allowlist matching stays meaningful.
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{
        detect_trust_prompt, path_matches_trusted_root, TrustConfig, TrustDecision, TrustEvent,
        TrustPolicy, TrustResolver,
    };

    #[test]
    fn req_trust_resolver_67_detects_known_trust_prompt_copy() {
        let screen_text = "Do you trust the files in this folder?\n1. Yes, proceed\n2. No";
        assert!(detect_trust_prompt(screen_text));
    }

    #[test]
    fn req_trust_resolver_67_detects_alternate_cues_case_insensitive() {
        // "Allow and continue" cue + uppercase variants should still match.
        assert!(detect_trust_prompt("ALLOW AND CONTINUE"));
        assert!(detect_trust_prompt("Trust this folder?"));
        assert!(detect_trust_prompt("Trust the files in this folder"));
        // Negative: no cue → not detected.
        assert!(!detect_trust_prompt("Ready for your input\n>"));
    }

    #[test]
    fn req_trust_resolver_67_does_not_emit_events_when_prompt_is_absent() {
        let resolver = TrustResolver::new(TrustConfig::new().with_allowlisted("/tmp/worktrees"));

        let decision = resolver.resolve("/tmp/worktrees/repo-a", "Ready for your input\n>");

        assert_eq!(decision, TrustDecision::NotRequired);
        assert_eq!(decision.events(), &[]);
        assert_eq!(decision.policy(), None);
    }

    #[test]
    fn req_trust_resolver_67_auto_trusts_allowlisted_cwd_after_prompt_detection() {
        let resolver = TrustResolver::new(TrustConfig::new().with_allowlisted("/tmp/worktrees"));

        let decision = resolver.resolve(
            "/tmp/worktrees/repo-a",
            "Do you trust the files in this folder?\n1. Yes, proceed\n2. No",
        );

        assert_eq!(decision.policy(), Some(TrustPolicy::AutoTrust));
        assert_eq!(
            decision.events(),
            &[
                TrustEvent::TrustRequired {
                    cwd: "/tmp/worktrees/repo-a".to_string(),
                },
                TrustEvent::TrustResolved {
                    cwd: "/tmp/worktrees/repo-a".to_string(),
                    policy: TrustPolicy::AutoTrust,
                },
            ]
        );
    }

    #[test]
    fn req_trust_resolver_67_requires_approval_for_unknown_cwd_after_prompt() {
        let resolver = TrustResolver::new(TrustConfig::new().with_allowlisted("/tmp/worktrees"));

        let decision = resolver.resolve(
            "/tmp/other/repo-b",
            "Do you trust the files in this folder?\n1. Yes, proceed\n2. No",
        );

        assert_eq!(decision.policy(), Some(TrustPolicy::RequireApproval));
        assert_eq!(
            decision.events(),
            &[TrustEvent::TrustRequired {
                cwd: "/tmp/other/repo-b".to_string(),
            }]
        );
    }

    #[test]
    fn req_trust_resolver_67_denied_root_takes_precedence_over_allowlist() {
        let resolver = TrustResolver::new(
            TrustConfig::new()
                .with_allowlisted("/tmp/worktrees")
                .with_denied("/tmp/worktrees/repo-c"),
        );

        let decision = resolver.resolve(
            "/tmp/worktrees/repo-c",
            "Do you trust the files in this folder?\n1. Yes, proceed\n2. No",
        );

        assert_eq!(decision.policy(), Some(TrustPolicy::Deny));
        assert_eq!(
            decision.events(),
            &[
                TrustEvent::TrustRequired {
                    cwd: "/tmp/worktrees/repo-c".to_string(),
                },
                TrustEvent::TrustDenied {
                    cwd: "/tmp/worktrees/repo-c".to_string(),
                    reason: "cwd matches denied trust root: /tmp/worktrees/repo-c".to_string(),
                },
            ]
        );
    }

    #[test]
    fn req_trust_resolver_67_sibling_prefix_does_not_match_trusted_root() {
        // "/tmp/worktrees-other/..." must NOT match "/tmp/worktrees".
        assert!(!path_matches_trusted_root(
            "/tmp/worktrees-other/repo-d",
            "/tmp/worktrees"
        ));
    }

    #[test]
    fn req_trust_resolver_67_trusts_helper_matches_resolve_outcome() {
        let resolver = TrustResolver::new(
            TrustConfig::new()
                .with_allowlisted("/tmp/worktrees")
                .with_denied("/tmp/worktrees/repo-c"),
        );

        // allowlisted → trusted
        assert!(resolver.trusts("/tmp/worktrees/repo-a"));
        // denied wins → not trusted
        assert!(!resolver.trusts("/tmp/worktrees/repo-c"));
        // unknown → not trusted
        assert!(!resolver.trusts("/tmp/other/repo-b"));
    }

    #[test]
    fn req_trust_resolver_67_empty_config_requires_approval() {
        // No allowlist, no denylist, prompt detected → RequireApproval (fail safe).
        let resolver = TrustResolver::new(TrustConfig::new());

        let decision = resolver.resolve(
            "/tmp/anywhere",
            "Do you trust the files in this folder?\n1. Yes, proceed\n2. No",
        );

        assert_eq!(decision.policy(), Some(TrustPolicy::RequireApproval));
        // No prompt → NotRequired (fail-closed when no signal).
        let no_prompt = resolver.resolve("/tmp/anywhere", "ready");
        assert_eq!(no_prompt, TrustDecision::NotRequired);
    }
}
