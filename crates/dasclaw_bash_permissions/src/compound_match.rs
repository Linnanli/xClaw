//! Compound-command splitting + per-subcommand prefix pipeline — port of
//! the deny-no-degrade red line enforced by upstream
//! `checkSandboxAutoAllow` (`bashPermissions.ts` L1280+) and
//! `checkPathConstraints` (L2151-L2167), restricted to the subcommand
//! fanout guard `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK` (upstream L103).
//!
//! ## Scope (Slice 2.2.d)
//!
//! Adds compound-aware permission matching on top of
//! [`crate::prefix_match::check_prefix_match`]:
//!
//! - parse the trimmed command via tree-sitter
//!   (`dasclaw_shell_command::bash::try_parse_word_only_commands_sequence`)
//!   into a `Vec<Vec<String>>` of subcommand argvs joined by safe
//!   operators (`&&`, `||`, `;`, `|`)
//! - cap fanout at [`MAX_SUBCOMMANDS_FOR_SECURITY_CHECK`] = 50 — exceed
//!   the cap and we return [`PermissionBehavior::Ask`] (upstream
//!   L2164-L2167 same exact cap value and same downgrade target)
//! - per-subcommand call into prefix-match; merge per upstream
//!   precedence: **any** subcommand Deny → compound Deny (red line);
//!   else **any** Ask → compound Ask; else Passthrough.
//!
//! ## What is NOT in this slice
//!
//! - `Allow` is **never** granted on a multi-subcommand path. This is
//!   deliberate: upstream only grants compound-Allow inside the sandbox
//!   auto-allow code path (`checkSandboxAutoAllow`) which requires extra
//!   context (sandbox enabled, no path constraints) that we don't have
//!   yet. Default-to-Passthrough is the Fail-Safe shape.
//! - No env-var stripping, no safe-wrapper stripping (Slice 2.2.e).
//! - No `RuleSuggestion` generation (Slice 2.2.g).
//! - No hook wiring — library-only (Slice 2.2.f).
//! - When tree-sitter cannot word-only-parse the script (e.g. it
//!   contains a substitution, a redirect, or any non-safe operator), we
//!   delegate to single-command [`crate::check_prefix_match`] on the
//!   raw trimmed string. This matches upstream's
//!   `skipCompoundCheck = true` fallback path (`bashPermissions.ts`
//!   L1079: `astCommand !== undefined`) — the AST has already vouched
//!   for the structure so we do not need a second compound-aware pass.
//!
//! ## Upstream parity refs
//!
//! - `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK = 50` — L103
//! - cap behavior `ask` — L2164-L2167
//! - per-subcommand deny precedence — L1303-L1335 / L2151-L2167
//! - red line "deny 不降级" — plan `phase-2.2-bash-permissions-rule-engine.md` §4

use dasclaw_shell_command::bash::{try_parse_shell, try_parse_word_only_commands_sequence};
use dasclaw_shell_command::parse_command::shlex_join;

use crate::prefix_match::check_prefix_match;
use crate::types::{PermissionDecisionReason, PermissionResult, ToolPermissionContext};

pub const BASH_TOOL_NAME: &str = "Bash";

/// Upstream `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK` (`bashPermissions.ts`
/// L103). Compounds with more than this many subcommands collapse to
/// [`PermissionResult::Ask`] so the rule engine is not used as a DoS
/// surface by adversarial loops.
pub const MAX_SUBCOMMANDS_FOR_SECURITY_CHECK: usize = 50;

/// Run the compound-aware permission pipeline against a bash command.
///
/// See module docs for the precedence rules and the AST-fallback
/// contract. The result's `reason` always carries either the
/// rule that triggered the decision (per-subcommand) or an `Other`
/// reason explaining the fanout cap / passthrough.
pub fn check_compound_match(command: &str, context: &ToolPermissionContext) -> PermissionResult {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return PermissionResult::Passthrough {
            message: "Empty command requires approval".to_string(),
            reason: PermissionDecisionReason::Other {
                reason: "Empty command".to_string(),
            },
        };
    }

    let argvs = try_parse_shell(trimmed)
        .and_then(|tree| try_parse_word_only_commands_sequence(&tree, trimmed));

    // AST could not word-only-parse → upstream L1079 fallback: trust
    // the tree-sitter verdict and skip compound check. We hand the raw
    // string to the single-command prefix pipeline.
    let Some(argvs) = argvs else {
        return check_prefix_match(trimmed, context);
    };

    // Single-command — no compound semantics needed.
    if argvs.len() <= 1 {
        return check_prefix_match(trimmed, context);
    }

    // Fanout cap (upstream L2164-L2167).
    if argvs.len() > MAX_SUBCOMMANDS_FOR_SECURITY_CHECK {
        return PermissionResult::Ask {
            message: format!(
                "Claude requested permissions to use {BASH_TOOL_NAME}, but you haven't granted it yet."
            ),
            reason: PermissionDecisionReason::Other {
                reason: format!(
                    "bashPermissions: {n} subcommands exceeds cap ({cap}) — returning ask",
                    n = argvs.len(),
                    cap = MAX_SUBCOMMANDS_FOR_SECURITY_CHECK,
                ),
            },
        };
    }

    // Per-subcommand iteration. Deny short-circuits; Ask is buffered.
    let mut first_ask: Option<PermissionResult> = None;
    for argv in &argvs {
        let sub_cmd = shlex_join(argv);
        let result = check_prefix_match(&sub_cmd, context);
        match &result {
            PermissionResult::Deny { .. } => return result,
            PermissionResult::Ask { .. } if first_ask.is_none() => {
                first_ask = Some(result);
            }
            _ => {}
        }
    }

    if let Some(ask) = first_ask {
        return ask;
    }

    // No deny, no ask — default to Passthrough on compound (see module
    // docs; we deliberately do NOT grant Allow here).
    PermissionResult::Passthrough {
        message: "This command requires approval".to_string(),
        reason: PermissionDecisionReason::Other {
            reason: "Compound command — no matching deny / ask rule on any subcommand".to_string(),
        },
    }
}
