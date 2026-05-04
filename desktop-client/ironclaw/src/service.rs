//! OS service management for running the daemon.
//!
//! Generates and manages platform-native service definitions:
//! - **macOS**: launchd plist at `~/Library/LaunchAgents/com.dasclaw.daemon.plist`
//! - **Linux**: systemd user unit at `~/.config/systemd/user/dasclaw.service`
//!
//! The installed service runs `ironclaw run` (the default agent mode) and is
//! configured to restart automatically on failure.
//!
//! ## ADR-114 Ⅳ rename + migration
//!
//! The current names are the rebranded `dasclaw` family. The previous
//! `com.ironclaw.daemon` / `ironclaw.service` names are still recognised so
//! that `service uninstall` and `service migrate` can clean up legacy
//! installations without leaving orphan units behind. See
//! `docs/SERVICE_MIGRATION.md` for the user-facing flow.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::bootstrap::dasclaw_base_dir;

const SERVICE_LABEL: &str = "com.dasclaw.daemon";
const SYSTEMD_UNIT: &str = "dasclaw.service";

/// Pre-rename launchd label, kept for `service uninstall` / `service migrate`.
const LEGACY_SERVICE_LABEL: &str = "com.ironclaw.daemon";
/// Pre-rename systemd unit, kept for `service uninstall` / `service migrate`.
const LEGACY_SYSTEMD_UNIT: &str = "ironclaw.service";

// ── Public dispatch ─────────────────────────────────────────────

/// Route a service subcommand to the appropriate handler.
pub fn handle_command(command: &ServiceAction) -> Result<()> {
    match command {
        ServiceAction::Install => install(),
        ServiceAction::Start => start(),
        ServiceAction::Stop => stop(),
        ServiceAction::Status => status(),
        ServiceAction::Uninstall => uninstall(),
        ServiceAction::Migrate => migrate(),
    }
}

/// Service lifecycle actions.
#[derive(Debug, Clone)]
pub enum ServiceAction {
    Install,
    Start,
    Stop,
    Status,
    Uninstall,
    /// ADR-114 Ⅳ — detect legacy `ironclaw.*` service, stop+uninstall it,
    /// then install+start the new `dasclaw.*` service.
    Migrate,
}

// ── Install ─────────────────────────────────────────────────────

fn install() -> Result<()> {
    if cfg!(target_os = "macos") {
        install_macos()
    } else if cfg!(target_os = "linux") {
        install_linux()
    } else {
        bail!("Service management is only supported on macOS and Linux");
    }
}

fn install_macos() -> Result<()> {
    let file = macos_plist_path()?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    let logs_dir = ironclaw_logs_dir();
    std::fs::create_dir_all(&logs_dir)?;

    let stdout = logs_dir.join("daemon.stdout.log");
    let stderr = logs_dir.join("daemon.stderr.log");

    let plist = macos_plist_content(
        &exe.display().to_string(),
        &stdout.display().to_string(),
        &stderr.display().to_string(),
    );

    std::fs::write(&file, plist)?;
    println!("Installed launchd service: {}", file.display());
    println!("  Start with: ironclaw service start");
    Ok(())
}

fn macos_plist_content(exe: &str, stdout: &str, stderr: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <!-- Disable interactive CLI/REPL in daemon mode to prevent blocking on stdin -->
  <key>EnvironmentVariables</key>
  <dict>
    <key>CLI_ENABLED</key>
    <string>false</string>
  </dict>
  <key>StandardOutPath</key>
  <string>{stdout}</string>
  <key>StandardErrorPath</key>
  <string>{stderr}</string>
</dict>
</plist>
"#,
        label = SERVICE_LABEL,
        exe = xml_escape(exe),
        stdout = xml_escape(stdout),
        stderr = xml_escape(stderr),
    )
}

fn install_linux() -> Result<()> {
    let file = linux_unit_path()?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    let unit = format!(
        "[Unit]\n\
         Description=IronClaw daemon\n\
         After=network.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         # Disable interactive CLI/REPL in daemon mode to prevent blocking on stdin\n\
         Environment=\"CLI_ENABLED=false\"\n\
         ExecStart=\"{exe}\" run\n\
         Restart=always\n\
         RestartSec=3\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        exe = exe.display(),
    );

    std::fs::write(&file, unit)?;
    run_checked(Command::new("systemctl").args(["--user", "daemon-reload"])).ok();
    run_checked(Command::new("systemctl").args(["--user", "enable", SYSTEMD_UNIT])).ok();
    println!("Installed systemd user service: {}", file.display());
    println!("  Start with: ironclaw service start");
    Ok(())
}

// ── Start ───────────────────────────────────────────────────────

fn start() -> Result<()> {
    if cfg!(target_os = "macos") {
        let plist = macos_plist_path()?;
        if !plist.exists() {
            bail!("Service not installed. Run `ironclaw service install` first.");
        }
        run_checked(Command::new("launchctl").arg("load").arg("-w").arg(&plist))?;
        run_checked(Command::new("launchctl").arg("start").arg(SERVICE_LABEL))?;
        println!("Service started");
        Ok(())
    } else if cfg!(target_os = "linux") {
        run_checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
        run_checked(Command::new("systemctl").args(["--user", "start", SYSTEMD_UNIT]))?;
        println!("Service started");
        Ok(())
    } else {
        bail!("Service management is only supported on macOS and Linux");
    }
}

// ── Stop ────────────────────────────────────────────────────────

fn stop() -> Result<()> {
    if cfg!(target_os = "macos") {
        let plist = macos_plist_path()?;
        run_checked(Command::new("launchctl").arg("stop").arg(SERVICE_LABEL)).ok();
        run_checked(
            Command::new("launchctl")
                .arg("unload")
                .arg("-w")
                .arg(&plist),
        )
        .ok();
        println!("Service stopped");
        Ok(())
    } else if cfg!(target_os = "linux") {
        run_checked(Command::new("systemctl").args(["--user", "stop", SYSTEMD_UNIT])).ok();
        println!("Service stopped");
        Ok(())
    } else {
        bail!("Service management is only supported on macOS and Linux");
    }
}

// ── Status ──────────────────────────────────────────────────────

fn status() -> Result<()> {
    if cfg!(target_os = "macos") {
        let out = run_capture(Command::new("launchctl").arg("list"))?;
        let running = out.lines().any(|line| line.contains(SERVICE_LABEL));
        println!(
            "Service: {}",
            if running {
                "running/loaded"
            } else {
                "not loaded"
            }
        );
        println!("Unit: {}", macos_plist_path()?.display());
        Ok(())
    } else if cfg!(target_os = "linux") {
        let state =
            run_capture(Command::new("systemctl").args(["--user", "is-active", SYSTEMD_UNIT]))
                .unwrap_or_else(|_| "unknown".into());
        println!("Service state: {}", state.trim());
        println!("Unit: {}", linux_unit_path()?.display());
        Ok(())
    } else {
        bail!("Service management is only supported on macOS and Linux");
    }
}

// ── Uninstall ───────────────────────────────────────────────────

fn uninstall() -> Result<()> {
    // Stop first (ignore errors, service might not be running). We attempt
    // to stop both current and legacy units so that a stale ironclaw.* unit
    // does not survive an `uninstall` issued after a partial migration.
    stop().ok();
    stop_legacy().ok();

    if cfg!(target_os = "macos") {
        let file = macos_plist_path()?;
        let legacy = macos_legacy_plist_path()?;
        let mut removed = Vec::new();
        for p in [&file, &legacy] {
            if p.exists() {
                std::fs::remove_file(p)
                    .with_context(|| format!("failed to remove {}", p.display()))?;
                removed.push(p.display().to_string());
            }
        }
        if removed.is_empty() {
            println!(
                "Service uninstalled (nothing to remove at {})",
                file.display()
            );
        } else {
            println!("Service uninstalled ({})", removed.join(", "));
        }
        Ok(())
    } else if cfg!(target_os = "linux") {
        let file = linux_unit_path()?;
        let legacy = linux_legacy_unit_path()?;
        let mut removed = Vec::new();
        for p in [&file, &legacy] {
            if p.exists() {
                std::fs::remove_file(p)
                    .with_context(|| format!("failed to remove {}", p.display()))?;
                removed.push(p.display().to_string());
            }
        }
        run_checked(Command::new("systemctl").args(["--user", "daemon-reload"])).ok();
        if removed.is_empty() {
            println!(
                "Service uninstalled (nothing to remove at {})",
                file.display()
            );
        } else {
            println!("Service uninstalled ({})", removed.join(", "));
        }
        Ok(())
    } else {
        bail!("Service management is only supported on macOS and Linux");
    }
}

// ── Migrate (ADR-114 Ⅳ) ────────────────────────────────────────

/// Detect a legacy `ironclaw.*` service installation and migrate it to the
/// new `dasclaw.*` names: stop legacy → remove legacy unit file → install
/// new unit → start it. Idempotent: if no legacy install is found, this is
/// equivalent to `install` followed by `start` (skipping start if a current
/// install already exists).
fn migrate() -> Result<()> {
    if !cfg!(target_os = "macos") && !cfg!(target_os = "linux") {
        bail!("Service management is only supported on macOS and Linux");
    }

    let legacy_present = legacy_unit_path_if_present()?;
    match &legacy_present {
        Some(p) => println!("Detected legacy service at {}", p.display()),
        None => println!("No legacy ironclaw service detected"),
    }

    // Step 1: stop the legacy service (best-effort, ignore errors when not
    // loaded). Skipped when no legacy unit exists.
    if legacy_present.is_some() {
        stop_legacy().ok();
    }

    // Step 2: remove the legacy unit file so the new install does not
    // collide with stale auto-load metadata.
    if let Some(path) = legacy_present.as_ref()
        && path.exists()
    {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove legacy {}", path.display()))?;
        println!("Removed legacy unit {}", path.display());
        if cfg!(target_os = "linux") {
            run_checked(Command::new("systemctl").args(["--user", "daemon-reload"])).ok();
        }
    }

    // Step 3: install the new unit (idempotent — overwrites if present).
    install()?;

    // Step 4: start the new service. Best-effort stop first so re-running
    // `migrate` against an already-loaded current install does not trip
    // launchctl's "service already loaded" error.
    stop().ok();
    start()?;
    println!("Migration to {SERVICE_LABEL} / {SYSTEMD_UNIT} complete");
    Ok(())
}

/// Stop the legacy service if any. Best-effort; never bails.
fn stop_legacy() -> Result<()> {
    if cfg!(target_os = "macos") {
        let plist = macos_legacy_plist_path()?;
        run_checked(
            Command::new("launchctl")
                .arg("stop")
                .arg(LEGACY_SERVICE_LABEL),
        )
        .ok();
        if plist.exists() {
            run_checked(
                Command::new("launchctl")
                    .arg("unload")
                    .arg("-w")
                    .arg(&plist),
            )
            .ok();
        }
        Ok(())
    } else if cfg!(target_os = "linux") {
        run_checked(Command::new("systemctl").args(["--user", "stop", LEGACY_SYSTEMD_UNIT])).ok();
        run_checked(Command::new("systemctl").args(["--user", "disable", LEGACY_SYSTEMD_UNIT]))
            .ok();
        Ok(())
    } else {
        Ok(())
    }
}

/// Return the path to the legacy unit file if it exists on disk, else `None`.
fn legacy_unit_path_if_present() -> Result<Option<PathBuf>> {
    if cfg!(target_os = "macos") {
        let p = macos_legacy_plist_path()?;
        Ok(if p.exists() { Some(p) } else { None })
    } else if cfg!(target_os = "linux") {
        let p = linux_legacy_unit_path()?;
        Ok(if p.exists() { Some(p) } else { None })
    } else {
        Ok(None)
    }
}

// ── Path helpers ────────────────────────────────────────────────

fn macos_plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not find home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{SERVICE_LABEL}.plist")))
}

fn macos_legacy_plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not find home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LEGACY_SERVICE_LABEL}.plist")))
}

fn linux_unit_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not find home directory")?;
    Ok(home
        .join(".config")
        .join("systemd")
        .join("user")
        .join(SYSTEMD_UNIT))
}

fn linux_legacy_unit_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not find home directory")?;
    Ok(home
        .join(".config")
        .join("systemd")
        .join("user")
        .join(LEGACY_SYSTEMD_UNIT))
}

fn ironclaw_logs_dir() -> PathBuf {
    dasclaw_base_dir().join("logs")
}

// ── Shell helpers ───────────────────────────────────────────────

fn run_checked(command: &mut Command) -> Result<()> {
    let output = command.output().context("failed to spawn command")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("command failed: {}", stderr.trim());
    }
    Ok(())
}

fn run_capture(command: &mut Command) -> Result<String> {
    let output = command.output().context("failed to spawn command")?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    Ok(text)
}

fn xml_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::service::*;

    #[test]
    fn xml_escape_handles_reserved_chars() {
        let escaped = xml_escape("<&>\"' and text");
        assert_eq!(escaped, "&lt;&amp;&gt;&quot;&apos; and text");
    }

    #[test]
    fn xml_escape_passes_through_plain_text() {
        assert_eq!(xml_escape("hello world"), "hello world");
    }

    #[test]
    fn run_capture_reads_stdout() {
        let out = run_capture(Command::new("sh").args(["-c", "echo hello"]))
            .expect("stdout capture should succeed");
        assert_eq!(out.trim(), "hello");
    }

    #[test]
    fn run_capture_falls_back_to_stderr() {
        let out = run_capture(Command::new("sh").args(["-c", "echo warn 1>&2"]))
            .expect("stderr capture should succeed");
        assert_eq!(out.trim(), "warn");
    }

    #[test]
    fn run_checked_errors_on_non_zero_exit() {
        let err = run_checked(Command::new("sh").args(["-c", "exit 17"]))
            .expect_err("non-zero exit should error");
        assert!(err.to_string().contains("command failed"));
    }

    #[test]
    fn run_checked_succeeds_on_zero_exit() {
        assert!(run_checked(Command::new("sh").args(["-c", "exit 0"])).is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn req_109_macos_plist_path_uses_dasclaw_label() {
        let path = macos_plist_path().unwrap();
        let s = path.to_string_lossy();
        assert!(
            s.ends_with("Library/LaunchAgents/com.dasclaw.daemon.plist"),
            "unexpected path: {s}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn req_109_macos_legacy_plist_path_points_at_ironclaw() {
        let path = macos_legacy_plist_path().unwrap();
        let s = path.to_string_lossy();
        assert!(
            s.ends_with("Library/LaunchAgents/com.ironclaw.daemon.plist"),
            "legacy path must still resolve to the pre-rename plist: {s}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn req_109_linux_unit_path_uses_dasclaw_unit() {
        let path = linux_unit_path().unwrap();
        let s = path.to_string_lossy();
        assert!(
            s.ends_with(".config/systemd/user/dasclaw.service"),
            "unexpected path: {s}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn req_109_linux_legacy_unit_path_points_at_ironclaw() {
        let path = linux_legacy_unit_path().unwrap();
        let s = path.to_string_lossy();
        assert!(
            s.ends_with(".config/systemd/user/ironclaw.service"),
            "legacy path must still resolve to the pre-rename unit: {s}"
        );
    }

    #[test]
    fn req_109_legacy_constants_remain_pinned_to_ironclaw_names() {
        // Hard guard: if anyone refactors the constants away the rebrand
        // window leaves users with no migration path. These two lines must
        // stay literal.
        assert_eq!(LEGACY_SERVICE_LABEL, "com.ironclaw.daemon");
        assert_eq!(LEGACY_SYSTEMD_UNIT, "ironclaw.service");
    }

    #[test]
    fn req_109_current_constants_use_dasclaw_namespace() {
        assert_eq!(SERVICE_LABEL, "com.dasclaw.daemon");
        assert_eq!(SYSTEMD_UNIT, "dasclaw.service");
    }

    #[test]
    fn logs_dir_under_ironclaw() {
        let path = ironclaw_logs_dir();
        let s = path.to_string_lossy();
        assert!(s.ends_with(".dasclaw/logs"), "unexpected path: {s}");
    }

    #[test]
    fn macos_plist_sets_cli_enabled_false() {
        let plist = macos_plist_content("/tmp/ironclaw", "/tmp/stdout.log", "/tmp/stderr.log");
        assert!(plist.contains("<key>EnvironmentVariables</key>"));
        assert!(plist.contains("    <key>CLI_ENABLED</key>\n    <string>false</string>"));
    }
}
