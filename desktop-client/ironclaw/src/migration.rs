//! `~/.ironclaw/` → `~/.dasclaw/` data migration helper.
//!
//! Implements ADR-114 §2.2 类 B-Ⅱ (issue #107). Conservative scope for the
//! first cut:
//!
//! * **Copy semantics, never move** — the legacy `~/.ironclaw/` directory is
//!   preserved as an implicit backup. Users decide when to delete it.
//! * **Skip-existing on the destination** — files that already exist under
//!   `~/.dasclaw/` are *never* overwritten; the migrator only fills in gaps.
//! * **Idempotent** — a `.migrated_from_ironclaw` marker is written into the
//!   destination once a successful pass completes; re-runs are no-ops unless
//!   `force` is set.
//! * **Fail-safe** — any I/O error aborts before the marker is written, so a
//!   partial copy will be retried on the next invocation.
//!
//! Out of scope for this PR (tracked by ADR-114 B-Ⅲ–Ⅴ red-line tickets):
//!
//! * macOS Keychain rename (`com.ironclaw.*` → `com.dasclaw.*`) — B-Ⅳ #109.
//! * Rewriting absolute paths embedded inside config files (e.g. an explicit
//!   `/Users/x/.ironclaw/...` string in `settings.json`) — separate issue.
//! * `--symlink` and move modes mentioned in the issue body — deferred until
//!   the copy path has soaked.

use std::path::{Path, PathBuf};

/// Marker file written to the destination once a successful migration pass
/// completes. Its presence makes subsequent runs a no-op (unless `force` is
/// set), which is what makes the helper safe to call from bootstrap.
pub const MIGRATED_MARKER: &str = ".migrated_from_ironclaw";

/// Caller-controlled options for [`migrate_with_paths`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MigrateOptions {
    /// Compute the report without touching the filesystem.
    pub dry_run: bool,
    /// Re-run even when the destination already carries the marker. Still
    /// **does not** overwrite existing files at the destination — only
    /// resumes filling gaps.
    pub force: bool,
}

/// Outcome of a migration call. Suitable for both human-friendly CLI output
/// and assertions in tests.
#[derive(Debug, Clone)]
pub struct MigrationReport {
    pub source: PathBuf,
    pub destination: PathBuf,
    /// Files copied (or that *would have been* copied in `dry_run`). Paths
    /// are relative to the source root.
    pub copied: Vec<PathBuf>,
    /// Files skipped because the destination already had them. Paths are
    /// relative to the source root.
    pub skipped_existing: Vec<PathBuf>,
    /// True when the source did not exist — treated as success because the
    /// helper is safe to call unconditionally from bootstrap.
    pub source_missing: bool,
    /// True when the destination already carried the marker and `force` was
    /// not set; the migrator returned without copying anything.
    pub already_migrated: bool,
    /// Mirrors [`MigrateOptions::dry_run`] for callers inspecting the report.
    pub dry_run: bool,
}

impl MigrationReport {
    fn new(source: &Path, destination: &Path, dry_run: bool) -> Self {
        Self {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            copied: Vec::new(),
            skipped_existing: Vec::new(),
            source_missing: false,
            already_migrated: false,
            dry_run,
        }
    }
}

/// Errors surfaced from the migration helper. Wraps the offending path so the
/// CLI can render an actionable message.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn io<P: Into<PathBuf>>(path: P) -> impl FnOnce(std::io::Error) -> MigrationError {
    let path = path.into();
    move |source| MigrationError::Io { path, source }
}

/// Migrate `~/.ironclaw/` under `home` into `~/.dasclaw/`.
///
/// Convenience wrapper around [`migrate_with_paths`] that derives the legacy
/// and rebranded directory names from a home directory. Tests prefer the
/// explicit-path variant so they can target a `tempdir`.
pub fn migrate(home: &Path, opts: MigrateOptions) -> Result<MigrationReport, MigrationError> {
    let source = home.join(".ironclaw");
    let destination = home.join(".dasclaw");
    migrate_with_paths(&source, &destination, opts)
}

/// Migrate `source` into `destination`. See module-level docs for the full
/// contract. Both paths are taken explicitly so tests can drive the helper
/// against a `tempdir` without mutating `$HOME`.
pub fn migrate_with_paths(
    source: &Path,
    destination: &Path,
    opts: MigrateOptions,
) -> Result<MigrationReport, MigrationError> {
    let mut report = MigrationReport::new(source, destination, opts.dry_run);

    if !source.exists() {
        report.source_missing = true;
        return Ok(report);
    }

    let marker = destination.join(MIGRATED_MARKER);
    if marker.exists() && !opts.force {
        report.already_migrated = true;
        return Ok(report);
    }

    if !opts.dry_run {
        std::fs::create_dir_all(destination).map_err(io(destination))?;
    }

    copy_dir_recursive(source, destination, source, &mut report, opts.dry_run)?;

    if !opts.dry_run {
        write_marker(&marker, source)?;
    }

    Ok(report)
}

fn copy_dir_recursive(
    src_root: &Path,
    dst_root: &Path,
    cur: &Path,
    report: &mut MigrationReport,
    dry_run: bool,
) -> Result<(), MigrationError> {
    let entries = std::fs::read_dir(cur).map_err(io(cur))?;
    for entry in entries {
        let entry = entry.map_err(io(cur))?;
        let path = entry.path();
        let dst = remap_path(src_root, dst_root, &path);
        let rel = relative_to(src_root, &path);

        let file_type = entry.file_type().map_err(io(&path))?;
        if file_type.is_dir() {
            if !dry_run {
                std::fs::create_dir_all(&dst).map_err(io(&dst))?;
            }
            copy_dir_recursive(src_root, dst_root, &path, report, dry_run)?;
        } else if dst.exists() {
            report.skipped_existing.push(rel);
        } else {
            if !dry_run {
                std::fs::copy(&path, &dst).map_err(io(&dst))?;
            }
            report.copied.push(rel);
        }
    }
    Ok(())
}

fn remap_path(src_root: &Path, dst_root: &Path, path: &Path) -> PathBuf {
    let rel = path.strip_prefix(src_root).unwrap_or(path);
    dst_root.join(rel)
}

fn relative_to(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn write_marker(marker: &Path, source: &Path) -> Result<(), MigrationError> {
    let body = format!(
        "Migrated from {} on {}\n",
        source.display(),
        chrono::Utc::now().to_rfc3339()
    );
    std::fs::write(marker, body).map_err(io(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, body).expect("write file");
    }

    fn run(src: &Path, dst: &Path, opts: MigrateOptions) -> MigrationReport {
        migrate_with_paths(src, dst, opts).expect("migrate")
    }

    #[test]
    fn req_adr_114_bii_source_missing_is_noop() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");

        let report = run(&src, &dst, MigrateOptions::default());

        assert!(report.source_missing);
        assert!(report.copied.is_empty());
        assert!(!dst.exists(), "destination should not be created");
    }

    #[test]
    fn req_adr_114_bii_copies_files_when_destination_empty() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "DATABASE_URL=foo");
        write(&src.join("settings.json"), "{}");
        write(&src.join("projects/a/state.json"), "{}");

        let report = run(&src, &dst, MigrateOptions::default());

        assert!(!report.source_missing);
        assert_eq!(report.copied.len(), 3);
        assert!(report.skipped_existing.is_empty());
        assert_eq!(
            std::fs::read_to_string(dst.join(".env")).unwrap(),
            "DATABASE_URL=foo"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("settings.json")).unwrap(),
            "{}"
        );
        assert!(dst.join("projects/a/state.json").exists());
    }

    #[test]
    fn req_adr_114_bii_writes_marker_after_success() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "x=1");

        run(&src, &dst, MigrateOptions::default());

        let marker = dst.join(MIGRATED_MARKER);
        assert!(marker.exists());
        let body = std::fs::read_to_string(&marker).unwrap();
        assert!(body.contains(&src.display().to_string()));
    }

    #[test]
    fn req_adr_114_bii_idempotent_when_marker_present() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "x=1");
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join(MIGRATED_MARKER), "stamp").unwrap();

        let report = run(&src, &dst, MigrateOptions::default());

        assert!(report.already_migrated);
        assert!(report.copied.is_empty());
        assert!(!dst.join(".env").exists(), "second run must not copy");
    }

    #[test]
    fn req_adr_114_bii_force_resumes_after_marker() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "x=1");
        write(&src.join("new.json"), "{}");
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join(MIGRATED_MARKER), "stamp").unwrap();
        // Pretend a previous run already brought over `.env`.
        write(&dst.join(".env"), "x=1");

        let report = run(
            &src,
            &dst,
            MigrateOptions {
                force: true,
                ..MigrateOptions::default()
            },
        );

        assert!(!report.already_migrated);
        let copied: Vec<_> = report
            .copied
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(copied.contains(&"new.json".to_string()));
        let skipped: Vec<_> = report
            .skipped_existing
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(
            skipped.contains(&".env".to_string()),
            "existing .env must be left alone"
        );
    }

    #[test]
    fn req_adr_114_bii_dry_run_does_not_touch_disk() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "x=1");
        write(&src.join("projects/a.txt"), "hi");

        let report = run(
            &src,
            &dst,
            MigrateOptions {
                dry_run: true,
                ..MigrateOptions::default()
            },
        );

        assert!(report.dry_run);
        assert_eq!(report.copied.len(), 2);
        assert!(!dst.exists(), "dry-run must not create destination");
    }

    #[test]
    fn req_adr_114_bii_skips_existing_files() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "old=1");
        write(&src.join("only-source.json"), "{}");
        write(&dst.join(".env"), "new=2");

        let report = run(&src, &dst, MigrateOptions::default());

        let copied: Vec<_> = report
            .copied
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let skipped: Vec<_> = report
            .skipped_existing
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(copied, vec!["only-source.json".to_string()]);
        assert_eq!(skipped, vec![".env".to_string()]);
        assert_eq!(std::fs::read_to_string(dst.join(".env")).unwrap(), "new=2");
    }

    #[test]
    fn req_adr_114_bii_preserves_source() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "x=1");

        run(&src, &dst, MigrateOptions::default());

        assert!(
            src.join(".env").exists(),
            "source must remain intact (copy semantics)"
        );
    }

    #[test]
    fn req_adr_114_bii_recursive_subdirs() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join("channels/telegram/cfg.json"), "{}");
        write(&src.join("tools/wasm/foo.wasm"), "WASM");
        write(&src.join("projects/p/sub/deep.txt"), "deep");

        let report = run(&src, &dst, MigrateOptions::default());

        assert_eq!(report.copied.len(), 3);
        assert!(dst.join("channels/telegram/cfg.json").exists());
        assert!(dst.join("tools/wasm/foo.wasm").exists());
        assert_eq!(
            std::fs::read_to_string(dst.join("projects/p/sub/deep.txt")).unwrap(),
            "deep"
        );
    }

    #[test]
    fn req_adr_114_bii_dry_run_does_not_write_marker() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "x=1");

        run(
            &src,
            &dst,
            MigrateOptions {
                dry_run: true,
                ..MigrateOptions::default()
            },
        );

        assert!(!dst.join(MIGRATED_MARKER).exists());
    }

    #[test]
    fn req_adr_114_bii_empty_source_still_marks_destination() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        let dst = tmp.path().join(".dasclaw");
        std::fs::create_dir_all(&src).unwrap();

        let report = run(&src, &dst, MigrateOptions::default());

        assert!(report.copied.is_empty());
        assert!(report.skipped_existing.is_empty());
        assert!(dst.join(MIGRATED_MARKER).exists());
    }

    #[test]
    fn req_adr_114_bii_marker_not_written_on_io_error() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join(".ironclaw");
        // Create destination as a *file* to force `create_dir_all` to fail.
        let dst = tmp.path().join(".dasclaw");
        write(&src.join(".env"), "x=1");
        std::fs::write(&dst, "blocking-file").unwrap();

        let err = migrate_with_paths(&src, &dst, MigrateOptions::default()).unwrap_err();
        let MigrationError::Io { .. } = err;
        // Marker file lives under the (would-be) destination dir; confirm we
        // never rewrote it.
        assert!(
            !dst.is_dir(),
            "destination should not be coerced into a dir"
        );
    }

    #[test]
    fn req_adr_114_bii_migrate_helper_uses_home_subdirs() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        write(&home.join(".ironclaw/.env"), "x=1");

        let report = migrate(home, MigrateOptions::default()).unwrap();

        assert_eq!(report.source, home.join(".ironclaw"));
        assert_eq!(report.destination, home.join(".dasclaw"));
        assert!(home.join(".dasclaw/.env").exists());
        assert!(home.join(".dasclaw").join(MIGRATED_MARKER).exists());
    }
}
