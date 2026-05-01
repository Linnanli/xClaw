//! `dasclaw migrate` CLI subcommand — ADR-114 B-Ⅱ (issue #107).
//!
//! Thin wrapper over [`crate::migration`] that resolves the user's home
//! directory and prints a human-readable summary.

use clap::Args;

use crate::migration::{self, MigrateOptions, MigrationReport};

#[derive(Args, Debug, Clone)]
pub struct MigrateCommand {
    /// Print the migration plan without writing anything to disk.
    #[arg(long)]
    pub dry_run: bool,

    /// Re-run even when the destination already carries the migration marker.
    /// Existing destination files are *still* never overwritten — `--force`
    /// only resumes filling in gaps from the legacy directory.
    #[arg(long)]
    pub force: bool,
}

pub fn run_migrate_command(cmd: MigrateCommand) -> anyhow::Result<()> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;

    let opts = MigrateOptions {
        dry_run: cmd.dry_run,
        force: cmd.force,
    };
    let report = migration::migrate(&home, opts)?;
    print_report(&report);
    Ok(())
}

fn print_report(report: &MigrationReport) {
    let mode = if report.dry_run { "[dry-run] " } else { "" };
    println!(
        "{mode}migrate: {} -> {}",
        report.source.display(),
        report.destination.display()
    );

    if report.source_missing {
        println!("  source does not exist; nothing to migrate");
        return;
    }
    if report.already_migrated {
        println!(
            "  destination already carries `{}`; pass --force to resume",
            migration::MIGRATED_MARKER
        );
        return;
    }

    println!("  copied: {} file(s)", report.copied.len());
    for path in &report.copied {
        println!("    + {}", path.display());
    }
    if !report.skipped_existing.is_empty() {
        println!(
            "  skipped (destination already has them): {} file(s)",
            report.skipped_existing.len()
        );
        for path in &report.skipped_existing {
            println!("    = {}", path.display());
        }
    }
    if report.dry_run {
        println!("  (dry-run: no files were written)");
    } else {
        println!(
            "  marker written to {}",
            report
                .destination
                .join(migration::MIGRATED_MARKER)
                .display()
        );
        println!(
            "  legacy directory preserved at {} — delete manually when ready",
            report.source.display()
        );
    }
}
