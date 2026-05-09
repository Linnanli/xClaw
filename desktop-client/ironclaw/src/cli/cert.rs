//! `dasclaw cert` CLI — MITM CA trust-chain management.
//!
//! ADR-139 §4.3 — five subcommands dispatching to `dasclaw_cert_trust`.
//! PR1 wires the subcommand surface; the underlying lib stubs return
//! `NotImplemented` until PR2/PR3.

use clap::Subcommand;
use dasclaw_cert_trust::Error as TrustError;

/// `dasclaw cert <subcommand>` operations (ADR-139 §4.3).
#[derive(Subcommand, Debug, Clone)]
pub enum CertCommand {
    /// Install the dasclaw MITM CA into the per-user trust store.
    Install,
    /// Remove the dasclaw MITM CA and clear `$CODEX_HOME/proxy/`.
    Revoke,
    /// Regenerate the CA and re-inject it.
    Rotate,
    /// Show CA fingerprint, expiry, and trust-store state.
    Status {
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Print the installed CA in PEM format on stdout.
    Export,
}

/// Run a `dasclaw cert` subcommand.
///
/// PR1: every operation routes to the `dasclaw_cert_trust` stub layer and
/// surfaces `NotImplemented` as a friendly error. The `Status` subcommand
/// is the one path that succeeds end-to-end (returns
/// `installed: false`).
pub fn run_cert_command(cmd: CertCommand) -> anyhow::Result<()> {
    match cmd {
        CertCommand::Install => {
            // PR1 has no CA bytes to install yet; a placeholder is fine
            // because the stub never inspects the input.
            map_trust_result(dasclaw_cert_trust::install_ca(b""))
        }
        CertCommand::Revoke => map_trust_result(dasclaw_cert_trust::uninstall_ca()),
        CertCommand::Rotate => map_trust_result(dasclaw_cert_trust::rotate_ca(b"")),
        CertCommand::Status { json } => print_status(json),
        CertCommand::Export => match dasclaw_cert_trust::export_ca_pem() {
            Ok(pem) => {
                use std::io::Write as _;
                std::io::stdout().write_all(&pem)?;
                Ok(())
            }
            Err(err) => Err(format_trust_error(err)),
        },
    }
}

fn map_trust_result(result: Result<(), TrustError>) -> anyhow::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(err) => Err(format_trust_error(err)),
    }
}

fn format_trust_error(err: TrustError) -> anyhow::Error {
    // PR2/PR3 will replace `NotImplemented` with real backend errors;
    // surface a stable user-facing string until then.
    anyhow::anyhow!("{err}")
}

fn print_status(json: bool) -> anyhow::Result<()> {
    let status = dasclaw_cert_trust::status().map_err(format_trust_error)?;
    if json {
        let payload = serde_json::to_string_pretty(&status)?;
        println!("{payload}");
    } else {
        println!("Platform:           {}", status.platform);
        println!("Installed:          {}", status.installed);
        println!(
            "Fingerprint SHA-256: {}",
            status.fingerprint_sha256.as_deref().unwrap_or("(none)")
        );
        println!(
            "Expires at:         {}",
            status.expires_at.as_deref().unwrap_or("(unknown)")
        );
        println!("Env-var fallback:   {}", status.env_fallback_active);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(clap::Parser, Debug)]
    struct Wrapper {
        #[command(subcommand)]
        cmd: CertCommand,
    }

    #[test]
    fn parses_each_subcommand() {
        for argv in [
            vec!["test", "install"],
            vec!["test", "revoke"],
            vec!["test", "rotate"],
            vec!["test", "status"],
            vec!["test", "status", "--json"],
            vec!["test", "export"],
        ] {
            Wrapper::try_parse_from(&argv).unwrap_or_else(|e| panic!("parse {argv:?}: {e}"));
        }
    }

    #[test]
    fn install_routes_to_trust_lib_stub() {
        let err = run_cert_command(CertCommand::Install).expect_err("PR1 install must error");
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[test]
    fn status_text_succeeds_on_stub() {
        // `status` is the only subcommand that does not return NotImplemented
        // in PR1 — the stub backends report `installed: false`.
        run_cert_command(CertCommand::Status { json: true }).expect("status must succeed");
    }
}
