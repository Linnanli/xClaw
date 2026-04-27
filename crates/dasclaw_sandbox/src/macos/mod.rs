//! macOS Seatbelt sandbox backend (W2.2a — minimum viable).
//!
//! Wraps `/usr/bin/sandbox-exec -p <policy> -- <cmd>`. The policy is composed
//! by concatenating the three SBPL files lifted from
//! `codex-cli-main/codex-rs/sandboxing/src/`:
//!
//! - `seatbelt_base_policy.sbpl`   — base deny-by-default
//! - `restricted_read_only_platform_defaults.sbpl` — RO platform paths
//! - `seatbelt_network_policy.sbpl` — network allow rules (only when
//!   `policy.allow_network` is true)
//!
//! W2.2a scope: literal sbpl include + writable_roots inline append. W2.2b
//! ports codex's full proxy-loopback-port + UDS allow-list logic
//! (`seatbelt.rs` 723 LOC) once `dasclaw_net_proxy` lands in W7.

use std::process::Command;

use crate::{Sandbox, SandboxError, SandboxExecRequest, SandboxType};

const SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

/// Pre-allocation hint for sbpl assembly. Tuned to fit base + RO + network
/// + a handful of writable_roots without realloc.
const POLICY_BUFFER_HINT: usize = 256;

const BASE_POLICY: &str = include_str!("policies/seatbelt_base_policy.sbpl");
const RO_DEFAULTS: &str = include_str!("policies/restricted_read_only_platform_defaults.sbpl");
const NETWORK_POLICY: &str = include_str!("policies/seatbelt_network_policy.sbpl");

/// Escape a path for inclusion in an sbpl `"..."` string literal.
///
/// sbpl is S-expression based; the only characters that can break out of a
/// string literal are `\` and `"`. Both must be backslash-prefixed.
fn escape_sbpl_path(p: &std::path::Path) -> String {
    let raw = p.display().to_string();
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

pub struct SeatbeltSandbox;

impl SeatbeltSandbox {
    pub fn new() -> Self {
        Self
    }

    /// Compose the full sbpl text for `req.policy`.
    fn compose_policy(&self, req: &SandboxExecRequest) -> String {
        let mut policy = String::with_capacity(
            BASE_POLICY.len() + RO_DEFAULTS.len() + NETWORK_POLICY.len() + POLICY_BUFFER_HINT,
        );
        policy.push_str(BASE_POLICY);
        policy.push('\n');
        policy.push_str(RO_DEFAULTS);
        policy.push('\n');

        if req.policy.allow_network {
            policy.push_str(NETWORK_POLICY);
            policy.push('\n');
        } else if !req.policy.proxy_loopback_ports.is_empty() {
            // Network is denied wholesale, but punch holes for the local
            // proxy listener(s). Mirrors codex `seatbelt.rs` proxy-routing
            // path. Each port allowed for outbound TCP only.
            for port in &req.policy.proxy_loopback_ports {
                policy.push_str(&format!(
                    "(allow network-outbound (remote tcp \"localhost:{port}\"))\n"
                ));
            }
        }

        for root in &req.policy.writable_roots {
            policy.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                escape_sbpl_path(root)
            ));
        }
        for root in &req.policy.readable_roots {
            policy.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                escape_sbpl_path(root)
            ));
        }
        policy
    }
}

impl Default for SeatbeltSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox for SeatbeltSandbox {
    fn kind(&self) -> SandboxType {
        SandboxType::MacosSeatbelt
    }

    fn execute(&self, req: SandboxExecRequest) -> Result<std::process::Output, SandboxError> {
        let policy = self.compose_policy(&req);

        let program = req.command.get_program().to_os_string();
        let args: Vec<_> = req.command.get_args().map(|a| a.to_os_string()).collect();

        let mut wrapped = Command::new(SEATBELT_EXECUTABLE);
        wrapped.arg("-p").arg(&policy).arg("--").arg(&program);
        for a in &args {
            wrapped.arg(a);
        }

        // Env scrubbing (W2.2c P1 fix): clear inherited parent env, then
        // pass *only* what the caller explicitly set on `req.command`. This
        // prevents accidental secret leakage (AWS_*, OPENAI_API_KEY, etc.)
        // into the sandboxed child.
        wrapped.env_clear();
        for (k, v) in req.command.get_envs() {
            if let Some(val) = v {
                wrapped.env(k, val);
            }
            // Note: `None` means "remove" on the original command, but since
            // env_clear() already wiped everything we just skip.
        }
        if let Some(d) = req.command.get_current_dir() {
            wrapped.current_dir(d);
        }

        Ok(wrapped.output()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SandboxPolicy, SandboxablePreference};

    #[test]
    fn compose_policy_includes_base_and_ro() {
        let sb = SeatbeltSandbox::new();
        let req = SandboxExecRequest {
            command: Command::new("/bin/echo"),
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
        };
        let policy = sb.compose_policy(&req);
        // Sanity: base policy must mention `(version 1)` opener.
        assert!(policy.contains("(version 1)"));
        // No network policy when allow_network=false.
        assert!(!policy.contains("(allow network*"));
    }

    #[test]
    fn compose_policy_appends_writable_root() {
        let sb = SeatbeltSandbox::new();
        let req = SandboxExecRequest {
            command: Command::new("/bin/echo"),
            policy: SandboxPolicy::read_only_defaults().with_writable("/tmp/work"),
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
        };
        let policy = sb.compose_policy(&req);
        assert!(policy.contains("(allow file-write* (subpath \"/tmp/work\"))"));
    }

    #[test]
    fn compose_policy_punches_proxy_loopback_holes_when_network_denied() {
        let sb = SeatbeltSandbox::new();
        let req = SandboxExecRequest {
            command: Command::new("/bin/echo"),
            policy: SandboxPolicy::read_only_defaults().with_proxy_ports(vec![8888]),
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
        };
        let policy = sb.compose_policy(&req);
        assert!(policy.contains("(allow network-outbound (remote tcp \"localhost:8888\"))"));
    }

    #[test]
    fn escape_sbpl_path_handles_quotes_and_backslashes() {
        use std::path::Path;
        assert_eq!(escape_sbpl_path(Path::new("/normal/path")), "/normal/path");
        assert_eq!(escape_sbpl_path(Path::new("/has\"quote")), "/has\\\"quote");
        assert_eq!(
            escape_sbpl_path(Path::new("/has\\backslash")),
            "/has\\\\backslash"
        );
    }

    #[test]
    fn compose_policy_escapes_path_with_quote() {
        let sb = SeatbeltSandbox::new();
        let req = SandboxExecRequest {
            command: Command::new("/bin/echo"),
            policy: SandboxPolicy::read_only_defaults().with_writable("/tmp/has\"quote"),
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
        };
        let policy = sb.compose_policy(&req);
        // Must NOT contain unescaped `"quote` mid-string (which would break sbpl)
        assert!(policy.contains("(allow file-write* (subpath \"/tmp/has\\\"quote\"))"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_scrubs_parent_env() {
        // Set a sentinel in parent env, run echo $SENTINEL under sandbox,
        // and verify it is empty (env_clear scrubbed it).
        std::env::set_var("DASCLAW_SBX_SENTINEL", "leaked");
        let sb = SeatbeltSandbox::new();
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg("echo \"v=${DASCLAW_SBX_SENTINEL:-empty}\"");
        let req = SandboxExecRequest {
            command: cmd,
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Require,
            windows_sandbox_enabled: false,
        };
        let out = sb.execute(req).expect("seatbelt should run sh");
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(stdout.trim(), "v=empty", "parent env leaked into sandbox");
        std::env::remove_var("DASCLAW_SBX_SENTINEL");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_runs_echo_under_real_sandbox() {
        // Real integration: must actually invoke /usr/bin/sandbox-exec.
        let sb = SeatbeltSandbox::new();
        let mut cmd = Command::new("/bin/echo");
        cmd.arg("hello");
        let req = SandboxExecRequest {
            command: cmd,
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Require,
            windows_sandbox_enabled: false,
        };
        let out = sb.execute(req).expect("seatbelt should run echo");
        assert!(out.status.success(), "exit={:?}", out.status);
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
    }
}
