//! Sandbox IPC commands (W2 ADR-110 接线 — first wire-up of `dasclaw_sandbox`).
//!
//! Currently exposes a single smoke-test command so the front-end can
//! verify the cross-platform sandbox abstraction is reachable from Tauri
//! without disturbing existing command-execution paths.
//!
//! W6+ (per 32 v2.4): existing IPC commands that spawn processes
//! (workspace git, jobs, mcp tools, etc.) will be migrated to call
//! `dasclaw_sandbox::select_backend(...).execute(...)` in place of bare
//! `Command::output()`.

use dasclaw_sandbox::{
    select_backend, SandboxExecRequest, SandboxPolicy, SandboxablePreference, SandboxType,
};
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Serialize)]
pub struct SandboxSmokeReport {
    pub backend: String,
    pub stdout: String,
    pub success: bool,
    /// Optional human-readable explanation when `success == false`,
    /// e.g. "backend not yet implemented (W2.3 Linux landlock pending)".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Run a fixed harmless echo through the platform sandbox and report which
/// backend served it.
///
/// Used by `desktop-client/ui` settings → "Diagnostics" → "Sandbox health"
/// (UI hook lands later). For now usable from devtools:
/// `await window.__TAURI__.invoke('ic_sandbox_smoke_test')`.
#[tauri::command]
pub async fn ic_sandbox_smoke_test() -> Result<SandboxSmokeReport, String> {
    // P1 fix (review #1): `Sandbox::execute()` is synchronous + spawns a
    // child process. Calling it from a `#[tauri::command] async fn` would
    // block a tokio worker thread, so push the whole synchronous body to
    // `spawn_blocking`.
    tokio::task::spawn_blocking(run_smoke_blocking)
        .await
        .map_err(|e| format!("smoke task panicked: {e}"))?
}

fn run_smoke_blocking() -> Result<SandboxSmokeReport, String> {
    let sb = select_backend(SandboxablePreference::Auto, false)
        .map_err(|e| format!("select_backend failed: {e}"))?;

    let backend_label = backend_label(sb.kind()).to_string();

    // Linux/Windows are still W1.5 stubs that return NotImplemented; surface
    // that gracefully rather than bubbling a 500 to the front-end.
    if sb.kind() == SandboxType::LinuxSeccomp || sb.kind() == SandboxType::WindowsRestrictedToken {
        return Ok(SandboxSmokeReport {
            backend: backend_label,
            stdout: String::new(),
            success: false,
            note: Some(format!(
                "backend not yet implemented; tracked in W2.{} (see docs/plans/architecture-refactor/32-execution-plan.md)",
                if sb.kind() == SandboxType::LinuxSeccomp { 3 } else { 4 }
            )),
        });
    }

    // P1 fix (review #2): do NOT inject parent PATH — `/bin/echo` /
    // `cmd.exe` are absolute paths, and `SeatbeltSandbox::execute()` relies
    // on env_clear() being effective. The previous `cmd.env("PATH", …)` was
    // a direct end-run around W2.2c env scrubbing.
    let cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", "echo dasclaw"]);
        c
    } else {
        let mut c = Command::new("/bin/echo");
        c.arg("dasclaw");
        c
    };

    let req = SandboxExecRequest {
        command: cmd,
        policy: SandboxPolicy::read_only_defaults(),
        preference: SandboxablePreference::Auto,
        windows_sandbox_enabled: false,
    };

    let out = sb
        .execute(req)
        .map_err(|e| format!("sandbox execute failed: {e}"))?;

    Ok(SandboxSmokeReport {
        backend: backend_label,
        stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        success: out.status.success(),
        note: None,
    })
}

fn backend_label(t: SandboxType) -> &'static str {
    match t {
        SandboxType::None => "none",
        SandboxType::MacosSeatbelt => "macos_seatbelt",
        SandboxType::LinuxSeccomp => "linux_seccomp",
        SandboxType::WindowsRestrictedToken => "windows_restricted_token",
        // `SandboxType` is `#[non_exhaustive]`; future variants land as
        // "unknown" until this match is updated.
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn smoke_test_reports_a_backend_label() {
        let report = ic_sandbox_smoke_test().await.expect("smoke must not error");
        assert!(
            ["none", "macos_seatbelt", "linux_seccomp", "windows_restricted_token"]
                .contains(&report.backend.as_str()),
            "unexpected backend label: {}",
            report.backend
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn smoke_test_runs_under_macos_seatbelt() {
        let report = ic_sandbox_smoke_test().await.expect("smoke");
        assert_eq!(report.backend, "macos_seatbelt");
        assert!(report.success);
        assert_eq!(report.stdout, "dasclaw");
    }
}
