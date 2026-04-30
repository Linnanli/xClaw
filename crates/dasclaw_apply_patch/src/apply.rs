//! Filesystem apply layer for parsed `apply_patch` hunks (W3 #53).
//!
//! This is the minimal apply contract consumed by ironclaw's `ApplyPatchTool`
//! wrapper. Higher-level concerns — sandbox enforcement, approval gating,
//! audit, UI diff preview — live in the wrapper, not here.
//!
//! ## Scope
//!
//! Supported hunks:
//! - `*** Add File:` — creates parent dirs as needed; rejects existing target.
//! - `*** Delete File:` — removes target; rejects missing target.
//! - `*** Update File:` — locates `old_lines` via three fallback levels
//!   (exact / `trim_end` / `trim`) mirroring the first three tiers of codex
//!   `seek_sequence`. Multi-hunk updates apply in patch order against the
//!   original line indices and splice in reverse.
//!
//! ## Out of scope (deferred to follow-up issues)
//!
//! - `*** Move to:` rename — currently returns [`ApplyError::MoveNotSupported`].
//! - Unicode punctuation normalization (codex `seek_sequence` tier 4).
//! - Approval / sandbox / proxy enforcement.
//!
//! Path policy: when [`ApplyOptions::base_dir`] is set, absolute paths and
//! `..` escapes are rejected via lexical normalization (no symlink follow).

use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::parser::{ApplyPatchArgs, Hunk, UpdateFileChunk};

/// Per-invocation policy passed to [`apply`].
#[derive(Debug, Default, Clone)]
pub struct ApplyOptions {
    /// If set, all hunk paths must be relative and resolve inside this
    /// directory (after lexical `..` collapse). When `None`, paths are
    /// resolved relative to the process's current working directory and the
    /// caller is responsible for sandboxing.
    pub base_dir: Option<PathBuf>,
}

/// Per-call summary of filesystem effects.
///
/// Paths are recorded as the original (unresolved) hunk paths so the wrapper
/// can show users the same path that appeared in the patch.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct ApplyReport {
    pub files_added: Vec<PathBuf>,
    pub files_deleted: Vec<PathBuf>,
    pub files_updated: Vec<PathBuf>,
    pub hunks_applied: usize,
}

/// Errors returned by the apply layer.
#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("path '{0}' is absolute; only relative paths are accepted under base_dir")]
    AbsolutePath(PathBuf),
    #[error("path '{0}' escapes base directory")]
    PathEscapesBase(PathBuf),
    #[error("file '{0}' already exists")]
    FileAlreadyExists(PathBuf),
    #[error("file '{0}' does not exist")]
    FileNotFound(PathBuf),
    #[error("could not locate context for update of '{0}'")]
    ContextNotFound(PathBuf),
    #[error("io error on '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("'*** Move to:' is not yet supported by the apply layer")]
    MoveNotSupported,
}

/// Apply parsed hunks to the filesystem.
///
/// Hunks execute in document order. The first error short-circuits and leaves
/// any prior hunks already on disk; callers that need transactional semantics
/// should snapshot before invocation.
pub fn apply(args: &ApplyPatchArgs, opts: &ApplyOptions) -> Result<ApplyReport, ApplyError> {
    let mut report = ApplyReport::default();
    for hunk in &args.hunks {
        match hunk {
            Hunk::AddFile { path, contents } => {
                apply_add(path, contents, opts, &mut report)?;
            }
            Hunk::DeleteFile { path } => {
                apply_delete(path, opts, &mut report)?;
            }
            Hunk::UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                if move_path.is_some() {
                    return Err(ApplyError::MoveNotSupported);
                }
                apply_update(path, chunks, opts, &mut report)?;
            }
        }
    }
    Ok(report)
}

fn apply_add(
    path: &Path,
    contents: &str,
    opts: &ApplyOptions,
    report: &mut ApplyReport,
) -> Result<(), ApplyError> {
    let resolved = resolve_path(path, opts)?;
    if resolved.exists() {
        return Err(ApplyError::FileAlreadyExists(path.to_path_buf()));
    }
    if let Some(parent) = resolved.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| ApplyError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }
    fs::write(&resolved, contents).map_err(|source| ApplyError::Io {
        path: resolved.clone(),
        source,
    })?;
    report.files_added.push(path.to_path_buf());
    report.hunks_applied += 1;
    Ok(())
}

fn apply_delete(
    path: &Path,
    opts: &ApplyOptions,
    report: &mut ApplyReport,
) -> Result<(), ApplyError> {
    let resolved = resolve_path(path, opts)?;
    if !resolved.exists() {
        return Err(ApplyError::FileNotFound(path.to_path_buf()));
    }
    fs::remove_file(&resolved).map_err(|source| ApplyError::Io {
        path: resolved.clone(),
        source,
    })?;
    report.files_deleted.push(path.to_path_buf());
    report.hunks_applied += 1;
    Ok(())
}

fn apply_update(
    path: &Path,
    chunks: &[UpdateFileChunk],
    opts: &ApplyOptions,
    report: &mut ApplyReport,
) -> Result<(), ApplyError> {
    let resolved = resolve_path(path, opts)?;
    if !resolved.exists() {
        return Err(ApplyError::FileNotFound(path.to_path_buf()));
    }
    let original = fs::read_to_string(&resolved).map_err(|source| ApplyError::Io {
        path: resolved.clone(),
        source,
    })?;
    let updated = splice_chunks(path, &original, chunks)?;
    fs::write(&resolved, &updated).map_err(|source| ApplyError::Io {
        path: resolved.clone(),
        source,
    })?;
    report.files_updated.push(path.to_path_buf());
    report.hunks_applied += chunks.len();
    Ok(())
}

/// Apply update chunks against the original buffer, preserving trailing newline.
fn splice_chunks(
    path: &Path,
    original: &str,
    chunks: &[UpdateFileChunk],
) -> Result<String, ApplyError> {
    let trailing_newline = original.ends_with('\n');
    let mut lines: Vec<String> = if original.is_empty() {
        Vec::new()
    } else {
        let stripped = if trailing_newline {
            &original[..original.len() - 1]
        } else {
            original
        };
        stripped.split('\n').map(String::from).collect()
    };

    // Resolve every edit on the original line indices first; chunks are
    // documented as "in order", so we walk forward and bump the search cursor
    // past each match. Splicing in reverse keeps the indices stable.
    let mut edits: Vec<(usize, usize, Vec<String>)> = Vec::with_capacity(chunks.len());
    let mut cursor = 0usize;
    for chunk in chunks {
        let start = locate_chunk(&lines, chunk, cursor)
            .ok_or_else(|| ApplyError::ContextNotFound(path.to_path_buf()))?;
        let end = start + chunk.old_lines.len();
        edits.push((start, end, chunk.new_lines.clone()));
        cursor = end;
    }
    for (start, end, replacement) in edits.into_iter().rev() {
        lines.splice(start..end, replacement);
    }

    let mut joined = lines.join("\n");
    if trailing_newline {
        joined.push('\n');
    }
    Ok(joined)
}

fn locate_chunk(lines: &[String], chunk: &UpdateFileChunk, cursor: usize) -> Option<usize> {
    let search_from = match chunk.change_context.as_deref() {
        Some(ctx) => find_context_line(lines, ctx, cursor)?,
        None => cursor,
    };
    seek_pattern(lines, &chunk.old_lines, search_from, chunk.is_end_of_file)
}

fn find_context_line(lines: &[String], ctx: &str, cursor: usize) -> Option<usize> {
    let needle = ctx.trim();
    lines
        .iter()
        .enumerate()
        .skip(cursor)
        .find(|(_, line)| line.trim() == needle)
        .map(|(idx, _)| idx + 1)
}

/// Mirror codex `seek_sequence` tiers 1–3: exact → rstrip → trim. Unicode
/// punctuation normalization (tier 4) is intentionally deferred.
fn seek_pattern(lines: &[String], pattern: &[String], cursor: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(cursor);
    }
    if pattern.len() > lines.len() {
        return None;
    }
    let last = lines.len() - pattern.len();
    let start = if eof { last } else { cursor.min(last) };

    let exact = (start..=last).find(|&i| lines[i..i + pattern.len()] == *pattern);
    if exact.is_some() {
        return exact;
    }
    let rstrip = (start..=last)
        .find(|&i| (0..pattern.len()).all(|p| lines[i + p].trim_end() == pattern[p].trim_end()));
    if rstrip.is_some() {
        return rstrip;
    }
    (start..=last).find(|&i| (0..pattern.len()).all(|p| lines[i + p].trim() == pattern[p].trim()))
}

fn resolve_path(path: &Path, opts: &ApplyOptions) -> Result<PathBuf, ApplyError> {
    let Some(base) = opts.base_dir.as_deref() else {
        return Ok(path.to_path_buf());
    };
    if path.is_absolute() {
        return Err(ApplyError::AbsolutePath(path.to_path_buf()));
    }
    let joined = base.join(path);
    let normalized = lexical_normalize(&joined);
    if !normalized.starts_with(base) {
        return Err(ApplyError::PathEscapesBase(path.to_path_buf()));
    }
    Ok(normalized)
}

/// Collapse `.` / `..` lexically without touching the filesystem (no symlink
/// follow). `..` at the root is dropped so it cannot rebase outside `base`.
fn lexical_normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
