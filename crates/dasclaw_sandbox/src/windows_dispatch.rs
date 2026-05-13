//! Windows sandbox backend dispatch helpers.
//!
//! Cross-platform pure functions used by the Windows backend adapter to
//! decide which upstream `dasclaw_sandbox_windows` entry point to invoke.
//! Lives outside `windows/mod.rs` (which is `#[cfg(target_os = "windows")]`
//! only) so contract tests can exercise the decision matrix on any host.
//!
//! Source of truth: verbatim ported from
//! `codex-cli-main/codex-rs/core/src/exec.rs::windows_sandbox_uses_elevated_backend`
//! (codex upstream). Keep behaviour bit-identical so the dispatch story
//! stays consistent with codex's Windows sandbox model.
//!
//! See `docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md`
//! and issue #462 (B4-7).

use dasclaw_protocol::config_types::WindowsSandboxLevel;

/// Decide whether the Windows sandbox should run on the **elevated**
/// backend (`run_windows_sandbox_capture_elevated`) versus the default
/// **restricted-token** backend (`run_windows_sandbox_capture_with_extra_deny_write_paths`).
///
/// Verbatim from codex `core/src/exec.rs:113-121`:
/// > Windows firewall enforcement is tied to the logon-user sandbox
/// > identities, so proxy-enforced sessions must use that backend even
/// > when the configured mode is the default restricted-token sandbox.
///
/// Returns `true` to use the elevated backend in either of:
/// - `proxy_enforced == true` (an LLM/HTTP proxy is in front of the
///   sandboxed command — needs per-user firewall ⇒ elevated backend), or
/// - `sandbox_level == Elevated` (explicit operator/admin opt-in).
///
/// Otherwise returns `false`: use the restricted-token backend (default
/// in xClaw today; weaker isolation but no admin setup required at run
/// time).
pub fn windows_sandbox_uses_elevated_backend(
    sandbox_level: WindowsSandboxLevel,
    proxy_enforced: bool,
) -> bool {
    proxy_enforced || matches!(sandbox_level, WindowsSandboxLevel::Elevated)
}
