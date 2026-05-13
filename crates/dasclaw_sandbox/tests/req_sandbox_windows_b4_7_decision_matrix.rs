//! Issue #462 (B4-7) — Windows sandbox backend dispatch decision matrix.
//!
//! Verifies the pure helper
//! [`dasclaw_sandbox::windows_dispatch::windows_sandbox_uses_elevated_backend`]
//! matches codex `core/src/exec.rs::windows_sandbox_uses_elevated_backend`
//! across the full 3×2 matrix of `(WindowsSandboxLevel × proxy_enforced)`.
//!
//! Also covers the [`dasclaw_sandbox::launcher_ipc::LauncherRequest::use_elevated_backend`]
//! serde back-compat contract: `false` is omitted from the wire (kept
//! legacy launchers happy) while `true` survives a JSON round-trip.

use dasclaw_sandbox::launcher_ipc::{LauncherRequest, OuterJobLimitsWire, PROTOCOL_VERSION};
use dasclaw_sandbox::windows_dispatch::windows_sandbox_uses_elevated_backend;
use dasclaw_sandbox::WindowsSandboxLevel;
use std::collections::HashMap;
use std::path::PathBuf;

// -------------------- decision matrix (6 cases) --------------------

#[test]
fn req_sandbox_windows_b4_7_disabled_no_proxy_uses_restricted() {
    assert!(!windows_sandbox_uses_elevated_backend(
        WindowsSandboxLevel::Disabled,
        false,
    ));
}

#[test]
fn req_sandbox_windows_b4_7_disabled_with_proxy_uses_elevated() {
    assert!(windows_sandbox_uses_elevated_backend(
        WindowsSandboxLevel::Disabled,
        true,
    ));
}

#[test]
fn req_sandbox_windows_b4_7_restricted_no_proxy_uses_restricted() {
    assert!(!windows_sandbox_uses_elevated_backend(
        WindowsSandboxLevel::RestrictedToken,
        false,
    ));
}

#[test]
fn req_sandbox_windows_b4_7_restricted_with_proxy_uses_elevated() {
    assert!(windows_sandbox_uses_elevated_backend(
        WindowsSandboxLevel::RestrictedToken,
        true,
    ));
}

#[test]
fn req_sandbox_windows_b4_7_elevated_no_proxy_uses_elevated() {
    assert!(windows_sandbox_uses_elevated_backend(
        WindowsSandboxLevel::Elevated,
        false,
    ));
}

#[test]
fn req_sandbox_windows_b4_7_elevated_with_proxy_uses_elevated() {
    assert!(windows_sandbox_uses_elevated_backend(
        WindowsSandboxLevel::Elevated,
        true,
    ));
}

// -------------------- wire back-compat round-trip --------------------

fn sample_request(use_elevated: bool) -> LauncherRequest {
    LauncherRequest {
        protocol_version: PROTOCOL_VERSION,
        argv: vec!["cmd.exe".into()],
        cwd: PathBuf::from("C:/work"),
        env: HashMap::new(),
        dasclaw_home: PathBuf::from("C:/dasclaw"),
        policy_json: "{}".into(),
        outer_limits: None::<OuterJobLimitsWire>,
        use_private_desktop: false,
        additional_deny_write_paths: vec![],
        use_elevated_backend: use_elevated,
    }
}

#[test]
fn req_sandbox_windows_b4_7_wire_default_false_is_omitted() {
    // `#[serde(skip_serializing_if = "Not::not")]` keeps the wire clean
    // for legacy launcher binaries that decoded request without the
    // field.
    let req = sample_request(false);
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(
        !json.contains("use_elevated_backend"),
        "default `false` must not appear on wire: {json}"
    );

    // Forward-compat: old launcher decoding new (still-false) JSON
    // produces `false` via `#[serde(default)]`.
    let back: LauncherRequest = serde_json::from_str(&json).expect("deserialize");
    assert!(!back.use_elevated_backend);
}

#[test]
fn req_sandbox_windows_b4_7_wire_true_round_trips() {
    let req = sample_request(true);
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(
        json.contains("\"use_elevated_backend\":true"),
        "elevated request must emit field on wire: {json}"
    );
    let back: LauncherRequest = serde_json::from_str(&json).expect("deserialize");
    assert!(back.use_elevated_backend);
    assert_eq!(req, back);
}
