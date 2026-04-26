#!/usr/bin/env python3
"""Generate W1 dasclaw_* crate skeletons (Cargo.toml + lib.rs).
See docs/plans/architecture-refactor/32-execution-plan.md §W1.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATES = {
    "dasclaw_sandbox": (
        "Cross-platform sandbox (Linux landlock / macOS seatbelt / Windows job-object).",
        "Sandbox",
        "fn execute(&self, cmd: std::process::Command) -> Result<std::process::Output, SkeletonError>;",
        "",
    ),
    "dasclaw_pty": (
        "portable-pty wrapper with signal forwarding + resize.",
        "Pty",
        "fn spawn(&self, cmd: std::process::Command) -> Result<(), SkeletonError>;",
        "",
    ),
    "dasclaw_hooks": (
        "Schema + Registry + Engine three-layer hook system (codex-style).",
        "HookEngine",
        "fn dispatch(&self, event: &str) -> Result<(), SkeletonError>;",
        "",
    ),
    "dasclaw_apply_patch": (
        "Lark-grammar apply_patch protocol (codex port).",
        "ApplyPatch",
        "fn apply(&self, patch: &str) -> Result<(), SkeletonError>;",
        "",
    ),
    "dasclaw_project_docs": (
        "AGENTS.md / CLAUDE.md multi-layer project doc loader.",
        "ProjectDocLoader",
        "fn load(&self, cwd: &std::path::Path) -> Result<String, SkeletonError>;",
        "",
    ),
    "dasclaw_bash_validation": (
        "Bash injection detection (6 validation modules from claw-code).",
        "BashValidator",
        "fn validate(&self, cmd: &str) -> Result<(), SkeletonError>;",
        "",
    ),
    "dasclaw_governance": (
        "Self-governance 6-pack: policy / recovery / trust / branch_lock / stale / green.",
        "GovernanceEngine",
        "fn evaluate(&self, ctx: &GovernanceContext) -> GovernanceDecision;",
        "pub struct GovernanceContext;\npub enum GovernanceDecision { Allow, Deny, Escalate }\n",
    ),
    "dasclaw_mcp": (
        "MCP client with 6 transports (Stdio / SSE / HTTP / WebSocket / SDK / ManagedProxy).",
        "McpTransport",
        "fn connect(&self) -> Result<(), SkeletonError>;",
        "",
    ),
    "dasclaw_execpolicy": (
        "Starlark-based exec permission rules (codex port).",
        "ExecPolicy",
        "fn evaluate(&self, cmd: &str) -> PolicyDecision;",
        "pub enum PolicyDecision { Allow, Deny, Ask }\n",
    ),
    "dasclaw_features": (
        "4-stage feature flag lifecycle (codex port).",
        "FeatureFlags",
        "fn is_enabled(&self, flag: &str) -> bool;",
        "",
    ),
    "dasclaw_observability": (
        "Unified observability + rollout-trace 4 sub-tables (codex + ironclaw).",
        "Observer",
        "fn record(&self, event: ObservabilityEvent);",
        "pub struct ObservabilityEvent;\n",
    ),
    "dasclaw_identity": (
        "Device-key (P256 ECDSA) + agent-identity (codex port).",
        "Identity",
        "fn sign(&self, data: &[u8]) -> Result<Vec<u8>, SkeletonError>;",
        "",
    ),
    "dasclaw_crash": (
        "panic_hook + sentry adapter (cross-cutting gap fill).",
        "CrashReporter",
        "fn install(&self);",
        "",
    ),
    "dasclaw_net_proxy": (
        "rama-based MITM proxy with self-signed CA (codex port).",
        "NetProxy",
        "fn start(&self) -> Result<(), SkeletonError>;",
        "",
    ),
    "dasclaw_lsp": (
        "LSP client wrapper (textDocument/* tools, fork private cargo, ~1,694 LOC).",
        "LspClient",
        "fn request(&self, method: &str, params_json: &str) -> Result<String, SkeletonError>;",
        "",
    ),
    "dasclaw_git_tools": (
        "Git operation toolset: branch/commit/diff/log/push/runner/stale/status (fork private cargo, ~1,331 LOC).",
        "GitTool",
        "fn run(&self, args: &[&str]) -> Result<String, SkeletonError>;",
        "",
    ),
    "dasclaw_routines": (
        "Routine orchestration (fork ironclaw 0.24 routines module promotion).",
        "RoutineRunner",
        "fn execute(&self, routine_id: &str) -> Result<(), SkeletonError>;",
        "",
    ),
}

CARGO_TMPL = """[package]
name = "{name}"
version = "0.0.0-w1"
edition = "2021"
description = "{desc}"
license = "Apache-2.0 OR MIT"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
thiserror = "1"
"""

LIB_TMPL = '''//! {desc}
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("{name} skeleton error: {{0}}")]
pub struct SkeletonError(pub String);

{extras}
/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait {trait_name} {{
    /// {desc}
    {sig}
}}

#[cfg(test)]
mod tests {{
    #[test]
    fn skeleton_compiles() {{ /* W1 placeholder */ }}
}}
'''


def main() -> None:
    for name, (desc, trait_name, sig, extras) in CRATES.items():
        crate_dir = ROOT / "crates" / name
        (crate_dir / "src").mkdir(parents=True, exist_ok=True)
        (crate_dir / "Cargo.toml").write_text(
            CARGO_TMPL.format(name=name, desc=desc)
        )
        (crate_dir / "src" / "lib.rs").write_text(
            LIB_TMPL.format(name=name, desc=desc, trait_name=trait_name, sig=sig, extras=extras)
        )
        print(f"  {name}")
    print(f"generated {len(CRATES)} crate skeletons")


if __name__ == "__main__":
    main()
