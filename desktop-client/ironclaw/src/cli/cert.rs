//! `dasclaw cert` CLI — MITM CA trust-chain management.
//!
//! ADR-139 §4.3 — five subcommands dispatching to `dasclaw_cert_trust`.
//! PR1 wired the subcommand surface; PR2 (this commit) plumbs real PEM
//! input through `--from <path>` for `install` / `rotate`.
//!
//! Generation of the CA itself is `dasclaw_net_proxy`'s job (ADR-139
//! §3.6). Until that PR lands, callers point `--from` at a manually
//! generated CA (`openssl req -x509 …`).

use std::path::PathBuf;

use clap::Subcommand;
use dasclaw_cert_trust::Error as TrustError;

/// `dasclaw cert <subcommand>` operations (ADR-139 §4.3).
#[derive(Subcommand, Debug, Clone)]
pub enum CertCommand {
    /// Install the dasclaw MITM CA into the per-user trust store.
    Install {
        /// Path to a PEM-encoded CA certificate. Once `dasclaw_net_proxy`
        /// auto-generates the CA (ADR-139 §3.6 follow-up) this flag will
        /// default to `$CODEX_HOME/proxy/ca.pem`.
        #[arg(long, value_name = "PATH")]
        from: PathBuf,
    },
    /// Remove the dasclaw MITM CA and clear `$CODEX_HOME/proxy/`.
    Revoke,
    /// Regenerate the CA and re-inject it.
    Rotate {
        /// Path to the new PEM-encoded CA certificate.
        #[arg(long, value_name = "PATH")]
        from: PathBuf,
    },
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
pub fn run_cert_command(cmd: CertCommand) -> anyhow::Result<()> {
    match cmd {
        CertCommand::Install { from } => {
            let pem = read_pem_file(&from)?;
            map_trust_result(dasclaw_cert_trust::install_ca(&pem))
        }
        CertCommand::Revoke => map_trust_result(dasclaw_cert_trust::uninstall_ca()),
        CertCommand::Rotate { from } => {
            let pem = read_pem_file(&from)?;
            map_trust_result(dasclaw_cert_trust::rotate_ca(&pem))
        }
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

fn read_pem_file(path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    std::fs::read(path).map_err(|err| anyhow::anyhow!("failed to read CA PEM at {path:?}: {err}"))
}

fn map_trust_result(result: Result<(), TrustError>) -> anyhow::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(err) => Err(format_trust_error(err)),
    }
}

fn format_trust_error(err: TrustError) -> anyhow::Error {
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
            vec!["test", "install", "--from", "/tmp/ca.pem"],
            vec!["test", "revoke"],
            vec!["test", "rotate", "--from", "/tmp/ca.pem"],
            vec!["test", "status"],
            vec!["test", "status", "--json"],
            vec!["test", "export"],
        ] {
            Wrapper::try_parse_from(&argv).unwrap_or_else(|e| panic!("parse {argv:?}: {e}"));
        }
    }

    #[test]
    fn install_without_from_flag_is_rejected() {
        let err = Wrapper::try_parse_from(["test", "install"])
            .expect_err("missing --from must be a clap error");
        assert!(err.to_string().contains("--from"), "unexpected: {err}");
    }

    #[test]
    fn install_with_missing_file_returns_io_error() {
        // Path that cannot exist.
        let err = run_cert_command(CertCommand::Install {
            from: "/dev/null/nonexistent/ca.pem".into(),
        })
        .expect_err("missing file must error");
        assert!(err.to_string().contains("failed to read CA PEM"));
    }

    #[test]
    fn status_text_succeeds() {
        run_cert_command(CertCommand::Status { json: true }).expect("status must succeed");
    }
}
