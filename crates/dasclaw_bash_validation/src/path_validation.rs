//! Path-aware command validation (Phase 3.1.A).
//!
//! Ports the data-tables and workspace-boundary core of upstream
//! `claude-code-main/src/tools/BashTool/pathValidation.ts` so a parsed
//! `(PathCommand, args)` tuple can be checked against the user's
//! allowed working directories.
//!
//! Scope of this slice
//! -------------------
//! - [`PathCommand`] enum covering the 36 path-restricted commands
//! - [`PATH_EXTRACTORS`][extract_paths] dispatch via [`extract_paths`]
//! - [`COMMAND_OPERATION_TYPE`][command_operation_type] via
//!   [`PathCommand::operation_type`]
//! - [`expand_tilde_and_home`] (literal `~` / `$HOME` only — NOT a full
//!   shell expansion)
//! - [`is_dangerous_removal_path`] + [`check_dangerous_removal_paths`]
//!   (SECURITY PIN S4)
//! - [`validate_command_paths`] — the workspace-boundary judgment
//!
//! Out of scope (handled by later slices)
//! --------------------------------------
//! - Output-redirection validation (3.1.B, builds on
//!   [`crate::redirects`])
//! - AST argv splitting / `stripSafeWrappers` integration (3.1.B)
//! - Hook wireup + audit-log contract pin (3.1.C)
//! - Symlink-escape detection (Phase 3.1 follow-up)
//!
//! Security contract (Fail-Closed)
//! -------------------------------
//! Every reason a target cannot be reliably validated maps to an
//! [`PathValidationOutcome::Ask`] (or `Block` for explicit deny):
//! - Path contains `$` / `%` / leading `=` (shell expansion)
//! - Path starts with unsupported tilde variant (`~user`, `~+`, `~-`)
//! - Resolved path falls outside every workspace directory
//! - `rm` / `rmdir` targets a known-dangerous path (S4)
//!
//! Empty path lists are treated as `Passthrough` — the caller (3.1.B)
//! is responsible for treating an empty parse as Fail-Closed at the
//! orchestration layer.

use std::path::{Component, Path, PathBuf};

/// Path-restricted command (subset of all shell commands that
/// `pathValidation.ts` knows how to inspect).
///
/// Variants mirror the upstream `PathCommand` union exactly so the
/// drift surface stays small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)] // upstream names are lowercase short verbs
pub enum PathCommand {
    Cd,
    Ls,
    Find,
    Mkdir,
    Touch,
    Rm,
    Rmdir,
    Mv,
    Cp,
    Cat,
    Head,
    Tail,
    Sort,
    Uniq,
    Wc,
    Cut,
    Paste,
    Column,
    Tr,
    File,
    Stat,
    Diff,
    Awk,
    Strings,
    Hexdump,
    Od,
    Base64,
    Nl,
    Grep,
    Rg,
    Sed,
    Git,
    Jq,
    Sha256sum,
    Sha1sum,
    Md5sum,
}

/// File operation classification mirrored from upstream
/// `FileOperationType`.
///
/// The `Write` / `Create` split matters for downstream slices that
/// gate auto-approval differently for new files vs. modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperationType {
    Read,
    Write,
    Create,
}

/// Decision returned by [`validate_command_paths`].
///
/// Designed to feed directly into Phase 3.1.C's `DecisionReason`
/// without lossy conversion: `Block` for explicit deny-rule hits,
/// `Ask` for everything else that fails the boundary check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathValidationOutcome {
    /// All extracted paths fall inside an allowed workspace dir.
    Passthrough,
    /// Hard deny (today only returned by the dangerous-removal gate;
    /// reserved for explicit deny-rule matching once 3.1.C wires the
    /// rule engine).
    Block { reason: String },
    /// Fall back to user approval. Carries the offending path string
    /// for richer prompts.
    Ask {
        reason: String,
        blocked_path: Option<String>,
    },
}

impl PathCommand {
    /// Parse a bare command word into a [`PathCommand`].
    ///
    /// Returns `None` for any command outside the path-restricted set
    /// — the caller should treat unknown commands as
    /// [`PathValidationOutcome::Passthrough`] (no path inspection
    /// required) at the orchestration layer.
    #[must_use]
    pub fn parse(base: &str) -> Option<Self> {
        use PathCommand::*;
        Some(match base {
            "cd" => Cd,
            "ls" => Ls,
            "find" => Find,
            "mkdir" => Mkdir,
            "touch" => Touch,
            "rm" => Rm,
            "rmdir" => Rmdir,
            "mv" => Mv,
            "cp" => Cp,
            "cat" => Cat,
            "head" => Head,
            "tail" => Tail,
            "sort" => Sort,
            "uniq" => Uniq,
            "wc" => Wc,
            "cut" => Cut,
            "paste" => Paste,
            "column" => Column,
            "tr" => Tr,
            "file" => File,
            "stat" => Stat,
            "diff" => Diff,
            "awk" => Awk,
            "strings" => Strings,
            "hexdump" => Hexdump,
            "od" => Od,
            "base64" => Base64,
            "nl" => Nl,
            "grep" => Grep,
            "rg" => Rg,
            "sed" => Sed,
            "git" => Git,
            "jq" => Jq,
            "sha256sum" => Sha256sum,
            "sha1sum" => Sha1sum,
            "md5sum" => Md5sum,
            _ => return None,
        })
    }

    /// Stringify back to the lowercase bare-command form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        use PathCommand::*;
        match self {
            Cd => "cd",
            Ls => "ls",
            Find => "find",
            Mkdir => "mkdir",
            Touch => "touch",
            Rm => "rm",
            Rmdir => "rmdir",
            Mv => "mv",
            Cp => "cp",
            Cat => "cat",
            Head => "head",
            Tail => "tail",
            Sort => "sort",
            Uniq => "uniq",
            Wc => "wc",
            Cut => "cut",
            Paste => "paste",
            Column => "column",
            Tr => "tr",
            File => "file",
            Stat => "stat",
            Diff => "diff",
            Awk => "awk",
            Strings => "strings",
            Hexdump => "hexdump",
            Od => "od",
            Base64 => "base64",
            Nl => "nl",
            Grep => "grep",
            Rg => "rg",
            Sed => "sed",
            Git => "git",
            Jq => "jq",
            Sha256sum => "sha256sum",
            Sha1sum => "sha1sum",
            Md5sum => "md5sum",
        }
    }

    /// Map a command to its [`FileOperationType`] (mirrors upstream
    /// `COMMAND_OPERATION_TYPE`).
    #[must_use]
    pub fn operation_type(self) -> FileOperationType {
        use FileOperationType::{Create, Read, Write};
        use PathCommand::*;
        match self {
            Mkdir | Touch => Create,
            Rm | Rmdir | Mv | Cp | Sed => Write,
            _ => Read,
        }
    }
}

// -- helpers --------------------------------------------------------------

/// `filterOutFlags` — return positional args, honoring POSIX `--`
/// end-of-options delimiter.
///
/// Without this, attack payloads like `rm -- -/../.claude/x` would be
/// silently dropped by a naive `!starts_with('-')` filter (see
/// upstream doc comment in `pathValidation.ts`).
fn filter_out_flags(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut after_dd = false;
    for a in args {
        if after_dd {
            out.push(a.clone());
        } else if a == "--" {
            after_dd = true;
        } else if !a.starts_with('-') {
            out.push(a.clone());
        }
    }
    out
}

/// `parsePatternCommand` — generic "pattern then paths" extractor used
/// by `grep` / `rg` / `jq`.
fn parse_pattern_command(
    args: &[String],
    flags_with_args: &[&str],
    defaults: &[&str],
) -> Vec<String> {
    let mut paths = Vec::new();
    let mut pattern_found = false;
    let mut after_dd = false;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !after_dd && arg == "--" {
            after_dd = true;
            i += 1;
            continue;
        }
        if !after_dd && arg.starts_with('-') {
            let flag = arg.split('=').next().unwrap_or(arg);
            if matches!(flag, "-e" | "--regexp" | "-f" | "--file" | "--expression") {
                pattern_found = true;
            }
            if flags_with_args.contains(&flag) && !arg.contains('=') {
                i += 1; // skip flag value
            }
            i += 1;
            continue;
        }
        if !pattern_found {
            pattern_found = true;
            i += 1;
            continue;
        }
        paths.push(arg.clone());
        i += 1;
    }
    if paths.is_empty() {
        defaults.iter().map(|s| (*s).to_string()).collect()
    } else {
        paths
    }
}

fn find_extractor(args: &[String]) -> Vec<String> {
    const PATH_FLAGS: &[&str] = &[
        "-newer",
        "-anewer",
        "-cnewer",
        "-mnewer",
        "-samefile",
        "-path",
        "-wholename",
        "-ilname",
        "-lname",
        "-ipath",
        "-iwholename",
    ];
    const GLOBAL_FLAGS: &[&str] = &["-H", "-L", "-P"];
    let mut paths: Vec<String> = Vec::new();
    let mut found_non_global = false;
    let mut after_dd = false;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if after_dd {
            paths.push(arg.clone());
            i += 1;
            continue;
        }
        if arg == "--" {
            after_dd = true;
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            if GLOBAL_FLAGS.contains(&arg.as_str()) {
                i += 1;
                continue;
            }
            found_non_global = true;
            // `-newer[acmBt][acmtB]` variants
            let is_newer_variant = arg.len() == 7 && arg.starts_with("-newer") && {
                let c = arg.as_bytes()[6];
                matches!(c, b'a' | b'c' | b'm' | b'B' | b't')
            };
            if PATH_FLAGS.contains(&arg.as_str()) || is_newer_variant {
                if let Some(next) = args.get(i + 1) {
                    paths.push(next.clone());
                    i += 1;
                }
            }
            i += 1;
            continue;
        }
        if !found_non_global {
            paths.push(arg.clone());
        }
        i += 1;
    }
    if paths.is_empty() {
        vec![".".to_string()]
    } else {
        paths
    }
}

fn tr_extractor(args: &[String]) -> Vec<String> {
    let has_delete = args
        .iter()
        .any(|a| a == "-d" || a == "--delete" || (a.starts_with('-') && a.contains('d')));
    let non_flags = filter_out_flags(args);
    let skip = if has_delete { 1 } else { 2 };
    non_flags.into_iter().skip(skip).collect()
}

fn sed_extractor(args: &[String]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut skip_next = false;
    let mut script_found = false;
    let mut after_dd = false;
    let mut i = 0;
    while i < args.len() {
        if skip_next {
            skip_next = false;
            i += 1;
            continue;
        }
        let arg = &args[i];
        if !after_dd && arg == "--" {
            after_dd = true;
            i += 1;
            continue;
        }
        if !after_dd && arg.starts_with('-') {
            if arg == "-f" || arg == "--file" {
                if let Some(next) = args.get(i + 1) {
                    paths.push(next.clone());
                    skip_next = true;
                }
                script_found = true;
            } else if arg == "-e" || arg == "--expression" {
                skip_next = true;
                script_found = true;
            } else if arg.contains('e') || arg.contains('f') {
                script_found = true;
            }
            i += 1;
            continue;
        }
        if !script_found {
            script_found = true;
            i += 1;
            continue;
        }
        paths.push(arg.clone());
        i += 1;
    }
    paths
}

fn git_extractor(args: &[String]) -> Vec<String> {
    // Only `git diff --no-index <a> <b>` is a path-bearing case for our gate.
    if args.first().map(String::as_str) == Some("diff") && args.iter().any(|a| a == "--no-index") {
        let rest: Vec<String> = args.iter().skip(1).cloned().collect();
        filter_out_flags(&rest).into_iter().take(2).collect()
    } else {
        Vec::new()
    }
}

fn grep_flags() -> &'static [&'static str] {
    &[
        "-e",
        "--regexp",
        "-f",
        "--file",
        "--exclude",
        "--include",
        "--exclude-dir",
        "--include-dir",
        "-m",
        "--max-count",
        "-A",
        "--after-context",
        "-B",
        "--before-context",
        "-C",
        "--context",
    ]
}

fn rg_flags() -> &'static [&'static str] {
    &[
        "-e",
        "--regexp",
        "-f",
        "--file",
        "-t",
        "--type",
        "-T",
        "--type-not",
        "-g",
        "--glob",
        "-m",
        "--max-count",
        "--max-depth",
        "-r",
        "--replace",
        "-A",
        "--after-context",
        "-B",
        "--before-context",
        "-C",
        "--context",
    ]
}

fn jq_flags() -> &'static [&'static str] {
    &[
        "-e",
        "--expression",
        "-f",
        "--from-file",
        "--arg",
        "--argjson",
        "--slurpfile",
        "--rawfile",
        "--args",
        "--jsonargs",
        "-L",
        "--library-path",
        "--indent",
        "--tab",
    ]
}

// -- public API -----------------------------------------------------------

/// `expandTilde` + literal `$HOME` substitution.
///
/// Only the safe head-anchored forms are expanded:
/// - `~` → `$HOME`
/// - `~/...` → `$HOME/...`
/// - `$HOME` / `$HOME/...` → `$HOME/...`
///
/// All other tilde forms (`~user`, `~+`, `~-`, `~N`) and any other
/// shell variable reference are left untouched; the caller's
/// boundary check will reject them via [`has_unsafe_expansion`].
#[must_use]
pub fn expand_tilde_and_home(path: &str, home: &Path) -> String {
    let home_str = home.to_string_lossy();
    if path == "~" {
        return home_str.into_owned();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return format!("{}/{}", home_str, rest);
    }
    if path == "$HOME" {
        return home_str.into_owned();
    }
    if let Some(rest) = path.strip_prefix("$HOME/") {
        return format!("{}/{}", home_str, rest);
    }
    path.to_string()
}

/// Return `true` if the path contains any shell expansion or
/// unsupported tilde variant. Mirrors the TOCTOU rejections in
/// upstream `validatePath` and is a Fail-Closed signal.
#[must_use]
pub fn has_unsafe_expansion(path: &str) -> bool {
    if path.starts_with('=') {
        return true; // zsh equals-expansion
    }
    if path.starts_with('~') && path != "~" && !path.starts_with("~/") {
        return true; // ~user / ~+ / ~- / ~N
    }
    // `$HOME` head-form is already expanded by `expand_tilde_and_home`;
    // any remaining `$` or `%` indicates further expansion we don't model.
    path.contains('$') || path.contains('%')
}

/// Path-extractor dispatcher (mirrors upstream `PATH_EXTRACTORS`).
///
/// `args` is expected to be the already-tokenized argv tail (without
/// the base command). `home` is consulted for `cd` with no args
/// (upstream uses `homedir()` directly).
#[must_use]
pub fn extract_paths(cmd: PathCommand, args: &[String], home: &Path) -> Vec<String> {
    use PathCommand::*;
    match cmd {
        Cd => {
            if args.is_empty() {
                vec![home.to_string_lossy().into_owned()]
            } else {
                vec![args.join(" ")]
            }
        }
        Ls => {
            let p = filter_out_flags(args);
            if p.is_empty() {
                vec![".".to_string()]
            } else {
                p
            }
        }
        Find => find_extractor(args),
        Tr => tr_extractor(args),
        Grep => {
            let mut paths = parse_pattern_command(args, grep_flags(), &[]);
            // `-r`/`-R`/`--recursive` with no paths → current dir
            if paths.is_empty()
                && args
                    .iter()
                    .any(|a| a == "-r" || a == "-R" || a == "--recursive")
            {
                paths.push(".".to_string());
            }
            paths
        }
        Rg => parse_pattern_command(args, rg_flags(), &["."]),
        Sed => sed_extractor(args),
        Jq => parse_pattern_command(args, jq_flags(), &[]),
        Git => git_extractor(args),
        // Simple commands — just filter flags.
        Mkdir | Touch | Rm | Rmdir | Mv | Cp | Cat | Head | Tail | Sort | Uniq | Wc | Cut
        | Paste | Column | File | Stat | Diff | Awk | Strings | Hexdump | Od | Base64 | Nl
        | Sha256sum | Sha1sum | Md5sum => filter_out_flags(args),
    }
}

/// Return `true` if `resolved` is a known-dangerous removal target.
///
/// Mirrors upstream `isDangerousRemovalPath`: bare wildcard, root,
/// home, direct root children. The `home` arg lets tests inject a
/// deterministic value rather than rely on `dirs::home_dir`.
#[must_use]
pub fn is_dangerous_removal_path(resolved: &str, home: &Path) -> bool {
    let forward = resolved.replace('\\', "/");
    if forward == "*" || forward.ends_with("/*") {
        return true;
    }
    let normalized: &str = if forward == "/" {
        "/"
    } else {
        forward.trim_end_matches('/')
    };
    if normalized == "/" {
        return true;
    }
    let home_norm = home.to_string_lossy().replace('\\', "/");
    let home_norm = home_norm.trim_end_matches('/');
    if !home_norm.is_empty() && normalized == home_norm {
        return true;
    }
    // direct child of root: parent is "/"
    if let Some(parent) = Path::new(normalized).parent() {
        if parent == Path::new("/") {
            return true;
        }
    }
    false
}

/// `checkDangerousRemovalPaths` — only applicable to `rm` / `rmdir`.
///
/// Returns `Some(Ask { … })` if any extracted target hits the
/// dangerous-path list. Always allow downstream tools to compose this
/// with the workspace-boundary check (upstream runs both).
#[must_use]
pub fn check_dangerous_removal_paths(
    cmd: PathCommand,
    args: &[String],
    cwd: &Path,
    home: &Path,
) -> Option<PathValidationOutcome> {
    if cmd != PathCommand::Rm && cmd != PathCommand::Rmdir {
        return None;
    }
    let paths = extract_paths(cmd, args, home);
    for raw in &paths {
        let stripped = strip_outer_quotes(raw);
        let expanded = expand_tilde_and_home(stripped, home);
        let absolute = resolve_logical(&expanded, cwd);
        if is_dangerous_removal_path(&absolute, home) {
            return Some(PathValidationOutcome::Ask {
                reason: format!(
                    "Dangerous {} operation detected: '{}' — \
                     critical system path requires explicit approval",
                    cmd.as_str(),
                    absolute
                ),
                blocked_path: Some(absolute),
            });
        }
    }
    None
}

/// Workspace-boundary judgment for the given command.
///
/// Returns the first non-`Passthrough` outcome encountered while
/// iterating extracted paths. Order:
/// 1. Dangerous-removal check (for `rm` / `rmdir`)
/// 2. Per-path: expansion guard → `~`/`$HOME` expand → absolute path
///    → check against `workspace_dirs`
///
/// `workspace_dirs` is the union of "allowed working directories"
/// (cwd + extra `--add-dir` paths). The first match wins.
///
/// **Lexical-only**: this entry point performs zero IO; symlinks
/// are not followed. For Fail-Safe symlink-escape detection use
/// [`validate_command_paths_with_fs`] with a real
/// [`FsResolver`][crate::fs_resolver::FsResolver].
#[must_use]
pub fn validate_command_paths(
    cmd: PathCommand,
    args: &[String],
    cwd: &Path,
    workspace_dirs: &[PathBuf],
    home: &Path,
) -> PathValidationOutcome {
    validate_command_paths_with_fs(
        cmd,
        args,
        cwd,
        workspace_dirs,
        home,
        &crate::fs_resolver::NoopFsResolver,
    )
}

/// Same contract as [`validate_command_paths`] but additionally walks
/// the symlink chain (via `fs`) for every extracted path. Any chain
/// step landing outside `workspace_dirs` flips the outcome to
/// [`PathValidationOutcome::Ask`] regardless of whether the original
/// lexical path was inside.
///
/// Fail-Safe contract (ADR-150 §5):
/// - Walker termination "unclean" (loop, max-depth, IO failure) →
///   Ask with `SymlinkResolveFailed` reason.
/// - Any resolved chain step outside `workspace_dirs` → Ask with
///   `SymlinkEscape` reason (carries both original and escaping
///   path).
///
/// Pass [`crate::fs_resolver::NoopFsResolver`] to disable symlink
/// resolution and recover the pre-3.1.g lexical behaviour exactly —
/// the walker returns `terminated_cleanly = false` with a one-step
/// chain, but the outer function treats Noop specifically as
/// "no IO requested" rather than Fail-Safe (otherwise every Noop
/// caller would Ask on every path). See the matching test
/// [`req_safety_490_3_1_g_noop_caller_preserves_3_1_a_behaviour`].
///
/// **Note**: `workspace_dirs` should be canonicalized by the caller.
/// The real POSIX resolver canonicalizes resolved chain steps (e.g.
/// `/tmp/...` → `/private/tmp/...` on macOS); without matching
/// canonical workspace entries the comparison would Ask spuriously.
#[must_use]
pub fn validate_command_paths_with_fs(
    cmd: PathCommand,
    args: &[String],
    cwd: &Path,
    workspace_dirs: &[PathBuf],
    home: &Path,
    fs: &dyn crate::fs_resolver::FsResolver,
) -> PathValidationOutcome {
    if let Some(dangerous) = check_dangerous_removal_paths(cmd, args, cwd, home) {
        return dangerous;
    }
    let paths = extract_paths(cmd, args, home);
    for raw in &paths {
        let stripped = strip_outer_quotes(raw);
        if has_unsafe_expansion(stripped) {
            return PathValidationOutcome::Ask {
                reason: format!(
                    "{} target '{}' uses shell expansion — Claude Code \
                     cannot safely resolve it without manual review",
                    cmd.as_str(),
                    stripped
                ),
                blocked_path: Some(stripped.to_string()),
            };
        }
        let expanded = expand_tilde_and_home(stripped, home);
        let absolute = resolve_logical(&expanded, cwd);
        if !path_in_workspace(&absolute, workspace_dirs) {
            return PathValidationOutcome::Ask {
                reason: format!(
                    "{} target '{}' is outside the allowed workspace \
                     directories — requires explicit approval",
                    cmd.as_str(),
                    absolute
                ),
                blocked_path: Some(absolute),
            };
        }
        // Symlink-escape pass: walk the on-disk chain. Skipped
        // entirely when the resolver is the Noop sentinel (we
        // detect this by probing one method on a known-absent
        // path — see test
        // `req_safety_490_3_1_g_noop_caller_preserves_3_1_a_behaviour`).
        let chain = crate::fs_resolver::resolve_path_chain(Path::new(&absolute), fs);
        if chain.steps.len() == 1 && !chain.terminated_cleanly {
            // Noop resolver (or any resolver that knows nothing
            // about this path AND has no realpath fallback)
            // produces this exact shape. Preserve 3.1.A behaviour.
            continue;
        }
        if !chain.terminated_cleanly {
            return PathValidationOutcome::Ask {
                reason: format!(
                    "{} target '{}' could not be safely resolved — \
                     symlink chain truncated (loop, depth limit, or IO \
                     error) requires explicit approval",
                    cmd.as_str(),
                    absolute
                ),
                blocked_path: Some(absolute),
            };
        }
        for step in &chain.steps {
            let step_str = step.to_string_lossy();
            if !path_in_workspace(&step_str, workspace_dirs) {
                return PathValidationOutcome::Ask {
                    reason: format!(
                        "{} target '{}' resolves through symlink to \
                         '{}' which is outside the allowed workspace \
                         directories — requires explicit approval",
                        cmd.as_str(),
                        absolute,
                        step_str
                    ),
                    blocked_path: Some(step_str.into_owned()),
                };
            }
        }
    }
    PathValidationOutcome::Passthrough
}

// -- internal path helpers ------------------------------------------------

pub(crate) fn strip_outer_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Logical (no-IO) absolute-path resolution.
///
/// - If `path` is absolute, normalize `.`/`..` segments
/// - Otherwise prepend `cwd` and normalize
///
/// Does NOT touch the filesystem; symlink escape is handled by a
/// later slice (see Phase 3.1 follow-up §6 OQ).
pub(crate) fn resolve_logical(path: &str, cwd: &Path) -> String {
    let p = Path::new(path);
    let combined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };
    normalize_path(&combined).to_string_lossy().into_owned()
}

pub(crate) fn normalize_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Prefix(pre) => out.push(pre.as_os_str()),
            Component::RootDir => out.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = out.pop();
                if !popped {
                    out.push("..");
                }
            }
            Component::Normal(seg) => out.push(seg),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

pub(crate) fn path_in_workspace(absolute: &str, workspace_dirs: &[PathBuf]) -> bool {
    if workspace_dirs.is_empty() {
        return false; // Fail-Closed: empty workspace = no allowed paths
    }
    let abs_path = Path::new(absolute);
    for dir in workspace_dirs {
        let dir_norm = normalize_path(dir);
        if abs_path.starts_with(&dir_norm) {
            return true;
        }
    }
    false
}

// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/user")
    }
    fn ws() -> Vec<PathBuf> {
        vec![PathBuf::from("/home/user/proj")]
    }
    fn cwd() -> PathBuf {
        PathBuf::from("/home/user/proj")
    }
    fn args(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_string()).collect()
    }

    // -- PathCommand enum ------------------------------------------------

    #[test]
    fn parse_recognizes_all_36_commands() {
        let names = [
            "cd",
            "ls",
            "find",
            "mkdir",
            "touch",
            "rm",
            "rmdir",
            "mv",
            "cp",
            "cat",
            "head",
            "tail",
            "sort",
            "uniq",
            "wc",
            "cut",
            "paste",
            "column",
            "tr",
            "file",
            "stat",
            "diff",
            "awk",
            "strings",
            "hexdump",
            "od",
            "base64",
            "nl",
            "grep",
            "rg",
            "sed",
            "git",
            "jq",
            "sha256sum",
            "sha1sum",
            "md5sum",
        ];
        for n in names {
            let parsed = PathCommand::parse(n).expect("known command");
            assert_eq!(parsed.as_str(), n, "round-trip name mismatch");
        }
    }

    #[test]
    fn parse_rejects_unknown_commands() {
        assert!(PathCommand::parse("python").is_none());
        assert!(PathCommand::parse("docker").is_none());
        assert!(PathCommand::parse("").is_none());
    }

    #[test]
    fn operation_type_classifies_writes_and_reads() {
        use FileOperationType::*;
        assert_eq!(PathCommand::Rm.operation_type(), Write);
        assert_eq!(PathCommand::Sed.operation_type(), Write);
        assert_eq!(PathCommand::Mkdir.operation_type(), Create);
        assert_eq!(PathCommand::Touch.operation_type(), Create);
        assert_eq!(PathCommand::Cat.operation_type(), Read);
        assert_eq!(PathCommand::Ls.operation_type(), Read);
        assert_eq!(PathCommand::Git.operation_type(), Read);
    }

    // -- extract_paths simple commands -----------------------------------

    #[test]
    fn extract_simple_filters_flags() {
        let out = extract_paths(
            PathCommand::Cat,
            &args(&["-n", "/etc/hosts", "--", "-weird"]),
            &home(),
        );
        assert_eq!(out, vec!["/etc/hosts".to_string(), "-weird".to_string()]);
    }

    #[test]
    fn extract_rm_honors_double_dash_security_pin() {
        // Without `--` handling the attack arg below would be silently dropped.
        let out = extract_paths(
            PathCommand::Rm,
            &args(&["--", "-/../.claude/settings.local.json"]),
            &home(),
        );
        assert_eq!(out, vec!["-/../.claude/settings.local.json".to_string()]);
    }

    #[test]
    fn extract_cd_no_args_returns_home() {
        let out = extract_paths(PathCommand::Cd, &args(&[]), &home());
        assert_eq!(out, vec!["/home/user".to_string()]);
    }

    #[test]
    fn extract_cd_joins_args_as_single_path() {
        // `cd path with spaces` upstream behavior: join everything.
        let out = extract_paths(PathCommand::Cd, &args(&["my", "dir"]), &home());
        assert_eq!(out, vec!["my dir".to_string()]);
    }

    #[test]
    fn extract_ls_defaults_to_current_dir() {
        let out = extract_paths(PathCommand::Ls, &args(&["-la"]), &home());
        assert_eq!(out, vec![".".to_string()]);
    }

    // -- extract_paths find ----------------------------------------------

    #[test]
    fn extract_find_collects_roots_before_first_predicate() {
        let out = extract_paths(
            PathCommand::Find,
            &args(&["src", "tests", "-name", "*.rs"]),
            &home(),
        );
        assert_eq!(out, vec!["src".to_string(), "tests".to_string()]);
    }

    #[test]
    fn extract_find_picks_up_path_flag_values() {
        let out = extract_paths(
            PathCommand::Find,
            &args(&[".", "-newer", "stamp", "-name", "*.txt"]),
            &home(),
        );
        assert_eq!(out, vec![".".to_string(), "stamp".to_string()]);
    }

    #[test]
    fn extract_find_double_dash_security_pin() {
        let out = extract_paths(PathCommand::Find, &args(&["--", "-/../etc"]), &home());
        assert_eq!(out, vec!["-/../etc".to_string()]);
    }

    // -- extract_paths pattern commands ----------------------------------

    #[test]
    fn extract_grep_separates_pattern_from_paths() {
        let out = extract_paths(
            PathCommand::Grep,
            &args(&["-i", "needle", "a.txt", "b.txt"]),
            &home(),
        );
        assert_eq!(out, vec!["a.txt".to_string(), "b.txt".to_string()]);
    }

    #[test]
    fn extract_grep_recursive_no_paths_defaults_to_cwd() {
        let out = extract_paths(PathCommand::Grep, &args(&["-r", "todo"]), &home());
        assert_eq!(out, vec![".".to_string()]);
    }

    #[test]
    fn extract_rg_defaults_to_cwd() {
        let out = extract_paths(PathCommand::Rg, &args(&["needle"]), &home());
        assert_eq!(out, vec![".".to_string()]);
    }

    // -- extract_paths sed / jq / git -----------------------------------

    #[test]
    fn extract_sed_picks_up_script_file_via_dash_f() {
        let out = extract_paths(
            PathCommand::Sed,
            &args(&["-f", "edits.sed", "input.txt"]),
            &home(),
        );
        assert_eq!(out, vec!["edits.sed".to_string(), "input.txt".to_string()]);
    }

    #[test]
    fn extract_sed_inline_expression_then_file() {
        let out = extract_paths(
            PathCommand::Sed,
            &args(&["-e", "s/a/b/", "input.txt"]),
            &home(),
        );
        assert_eq!(out, vec!["input.txt".to_string()]);
    }

    #[test]
    fn extract_jq_filter_and_files() {
        let out = extract_paths(PathCommand::Jq, &args(&[".", "a.json", "b.json"]), &home());
        assert_eq!(out, vec!["a.json".to_string(), "b.json".to_string()]);
    }

    #[test]
    fn extract_git_diff_no_index_returns_two_paths() {
        let out = extract_paths(
            PathCommand::Git,
            &args(&["diff", "--no-index", "a.txt", "b.txt"]),
            &home(),
        );
        assert_eq!(out, vec!["a.txt".to_string(), "b.txt".to_string()]);
    }

    #[test]
    fn extract_git_status_returns_no_paths() {
        let out = extract_paths(PathCommand::Git, &args(&["status"]), &home());
        assert!(out.is_empty());
    }

    // -- extract_paths tr ------------------------------------------------

    #[test]
    fn extract_tr_with_delete_flag_skips_one_set() {
        // `tr -d 'a-z' < file` → after filter_out_flags = ["a-z"], skip 1 = []
        let out = extract_paths(PathCommand::Tr, &args(&["-d", "a-z"]), &home());
        assert!(out.is_empty());
    }

    // -- expand_tilde_and_home ------------------------------------------

    #[test]
    fn expand_tilde_alone() {
        assert_eq!(expand_tilde_and_home("~", &home()), "/home/user");
    }

    #[test]
    fn expand_tilde_with_subpath() {
        assert_eq!(
            expand_tilde_and_home("~/.ssh/id_rsa", &home()),
            "/home/user/.ssh/id_rsa"
        );
    }

    #[test]
    fn expand_home_literal_alone() {
        assert_eq!(expand_tilde_and_home("$HOME", &home()), "/home/user");
    }

    #[test]
    fn expand_home_literal_with_subpath() {
        assert_eq!(
            expand_tilde_and_home("$HOME/.aws/credentials", &home()),
            "/home/user/.aws/credentials"
        );
    }

    #[test]
    fn expand_leaves_user_variants_untouched() {
        // Caller's `has_unsafe_expansion` is responsible for rejecting these.
        assert_eq!(expand_tilde_and_home("~root", &home()), "~root");
        assert_eq!(expand_tilde_and_home("~+", &home()), "~+");
    }

    // -- has_unsafe_expansion -------------------------------------------

    #[test]
    fn rejects_dollar_expansion() {
        assert!(has_unsafe_expansion("$EVIL"));
        assert!(has_unsafe_expansion("/etc/$(echo passwd)"));
    }

    #[test]
    fn rejects_percent_expansion() {
        assert!(has_unsafe_expansion("%USERPROFILE%"));
    }

    #[test]
    fn rejects_zsh_equals_expansion() {
        assert!(has_unsafe_expansion("=rg"));
    }

    #[test]
    fn rejects_tilde_user_variants() {
        assert!(has_unsafe_expansion("~root"));
        assert!(has_unsafe_expansion("~+"));
        assert!(has_unsafe_expansion("~-"));
        assert!(has_unsafe_expansion("~1"));
    }

    #[test]
    fn accepts_safe_paths() {
        assert!(!has_unsafe_expansion("/etc/passwd"));
        assert!(!has_unsafe_expansion("./relative/path"));
        assert!(!has_unsafe_expansion("~/already-head-form"));
        assert!(!has_unsafe_expansion("~"));
    }

    // -- is_dangerous_removal_path --------------------------------------

    #[test]
    fn dangerous_root_and_wildcards() {
        assert!(is_dangerous_removal_path("/", &home()));
        assert!(is_dangerous_removal_path("*", &home()));
        assert!(is_dangerous_removal_path("/tmp/*", &home()));
    }

    #[test]
    fn dangerous_home_match() {
        assert!(is_dangerous_removal_path("/home/user", &home()));
        assert!(is_dangerous_removal_path("/home/user/", &home()));
    }

    #[test]
    fn dangerous_direct_root_children() {
        assert!(is_dangerous_removal_path("/etc", &home()));
        assert!(is_dangerous_removal_path("/usr", &home()));
        assert!(is_dangerous_removal_path("/tmp", &home()));
    }

    #[test]
    fn safe_deep_paths() {
        assert!(!is_dangerous_removal_path("/usr/local/bin", &home()));
        assert!(!is_dangerous_removal_path("/home/user/proj/build", &home()));
    }

    // -- check_dangerous_removal_paths -----------------------------------

    #[test]
    fn rm_rf_root_blocked_security_pin_s4() {
        let out =
            check_dangerous_removal_paths(PathCommand::Rm, &args(&["-rf", "/"]), &cwd(), &home());
        assert!(matches!(out, Some(PathValidationOutcome::Ask { .. })));
    }

    #[test]
    fn rm_rf_home_tilde_blocked_security_pin_s4() {
        let out =
            check_dangerous_removal_paths(PathCommand::Rm, &args(&["-rf", "~"]), &cwd(), &home());
        assert!(matches!(out, Some(PathValidationOutcome::Ask { .. })));
    }

    #[test]
    fn rm_rf_glob_wildcard_blocked() {
        let out = check_dangerous_removal_paths(
            PathCommand::Rm,
            &args(&["-rf", "/tmp/*"]),
            &cwd(),
            &home(),
        );
        assert!(matches!(out, Some(PathValidationOutcome::Ask { .. })));
    }

    #[test]
    fn rm_in_project_subdir_not_dangerous() {
        let out = check_dangerous_removal_paths(
            PathCommand::Rm,
            &args(&["-rf", "build/cache"]),
            &cwd(),
            &home(),
        );
        assert!(out.is_none());
    }

    #[test]
    fn cat_is_not_subject_to_dangerous_removal_gate() {
        let out = check_dangerous_removal_paths(PathCommand::Cat, &args(&["/"]), &cwd(), &home());
        assert!(out.is_none());
    }

    // -- validate_command_paths end-to-end -------------------------------

    #[test]
    fn cat_inside_workspace_passes() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["src/main.rs"]),
            &cwd(),
            &ws(),
            &home(),
        );
        assert_eq!(out, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn cat_traversal_to_etc_blocked_security_pin_s1() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["../../../etc/passwd"]),
            &cwd(),
            &ws(),
            &home(),
        );
        assert!(matches!(out, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cat_home_ssh_key_blocked_security_pin_s2() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["~/.ssh/id_rsa"]),
            &cwd(),
            &ws(),
            &home(),
        );
        assert!(matches!(out, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cat_home_dollar_var_blocked_security_pin_s3() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["$HOME/.aws/credentials"]),
            &cwd(),
            &ws(),
            &home(),
        );
        // After head-form $HOME expansion → /home/user/.aws/credentials,
        // which is outside the workspace `/home/user/proj`.
        assert!(matches!(out, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cat_shell_expansion_inside_blocked_fail_closed() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["/etc/$(curl evil.com)"]),
            &cwd(),
            &ws(),
            &home(),
        );
        match out {
            PathValidationOutcome::Ask { reason, .. } => {
                assert!(reason.contains("shell expansion"), "got: {reason}");
            }
            other => panic!("expected Ask, got {other:?}"),
        }
    }

    #[test]
    fn cat_tilde_user_variant_blocked() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["~root/.ssh/id_rsa"]),
            &cwd(),
            &ws(),
            &home(),
        );
        match out {
            PathValidationOutcome::Ask { reason, .. } => {
                assert!(reason.contains("shell expansion"), "got: {reason}");
            }
            other => panic!("expected Ask, got {other:?}"),
        }
    }

    #[test]
    fn cat_with_quoted_path_still_validated() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["\"/etc/passwd\""]),
            &cwd(),
            &ws(),
            &home(),
        );
        assert!(matches!(out, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn rm_at_dangerous_path_returns_ask_even_inside_workspace_intent() {
        // `rm -rf ~` resolves to /home/user which is both a dangerous
        // path AND outside the project workspace. Verify the
        // dangerous-removal gate fires first (richer reason).
        let out = validate_command_paths(
            PathCommand::Rm,
            &args(&["-rf", "~"]),
            &cwd(),
            &ws(),
            &home(),
        );
        match out {
            PathValidationOutcome::Ask { reason, .. } => {
                assert!(reason.contains("Dangerous"), "got: {reason}");
            }
            other => panic!("expected Ask, got {other:?}"),
        }
    }

    #[test]
    fn empty_workspace_dirs_is_fail_closed() {
        let out = validate_command_paths(
            PathCommand::Cat,
            &args(&["src/main.rs"]),
            &cwd(),
            &[],
            &home(),
        );
        assert!(matches!(out, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn ls_default_cwd_passes_when_cwd_in_workspace() {
        let out = validate_command_paths(PathCommand::Ls, &args(&["-la"]), &cwd(), &ws(), &home());
        assert_eq!(out, PathValidationOutcome::Passthrough);
    }

    // -- normalize_path / resolve_logical sanity ------------------------

    #[test]
    fn normalize_collapses_dot_and_parent() {
        let out = normalize_path(Path::new("/a/./b/../c"));
        assert_eq!(out, PathBuf::from("/a/c"));
    }

    #[test]
    fn resolve_logical_relative_against_cwd() {
        let out = resolve_logical("src/main.rs", &PathBuf::from("/home/user/proj"));
        assert_eq!(out, "/home/user/proj/src/main.rs");
    }

    #[test]
    fn resolve_logical_traversal_escapes_cwd() {
        let out = resolve_logical("../../../etc/passwd", &PathBuf::from("/home/user/proj"));
        assert_eq!(out, "/etc/passwd");
    }

    // -- ADR-150 §6.1: validate_command_paths_with_fs integration -------
    //
    // These tests exercise the symlink-escape pass that wraps the
    // 3.1.A lexical check. We use the real on-disk POSIX resolver
    // against a tempdir-rooted workspace so the assertions cover the
    // full IO path (lstat → readlink → realpath fallback).

    #[cfg(unix)]
    mod symlink_escape {
        use super::*;
        use crate::fs_resolver::{real_posix::RealFsResolver, NoopFsResolver};
        use std::os::unix::fs::symlink;
        use tempfile::TempDir;

        fn setup() -> (TempDir, PathBuf) {
            let dir = TempDir::new().expect("tempdir");
            // Canonicalize to dodge macOS /var → /private/var redirection;
            // production callers are expected to pass canonical workspace
            // roots so the symlink walker's canonicalized output matches.
            let ws_root = std::fs::canonicalize(dir.path()).expect("canonicalize tempdir");
            (dir, ws_root)
        }

        fn args(xs: &[&str]) -> Vec<String> {
            xs.iter().map(|s| (*s).to_string()).collect()
        }

        #[test]
        fn req_safety_490_3_1_g_1_noop_resolver_matches_lexical_passthrough() {
            // Noop produces unclean+singleton → preserved as Passthrough
            // (i.e. 3.1.A behaviour). Path is lexically inside workspace.
            let (_dir, ws_root) = setup();
            let file = ws_root.join("a.txt");
            std::fs::write(&file, b"x").unwrap();
            let out = validate_command_paths_with_fs(
                PathCommand::Cat,
                &args(&[file.to_str().unwrap()]),
                &ws_root,
                std::slice::from_ref(&ws_root),
                &PathBuf::from("/home/user"),
                &NoopFsResolver,
            );
            assert_eq!(out, PathValidationOutcome::Passthrough);
        }

        #[test]
        fn req_safety_490_3_1_g_1_real_symlink_escape_to_etc_asks() {
            // T-SYM-1: ws/link → /etc/passwd. Lexical check passes
            // (link is inside ws), but symlink chain step lands at
            // /etc/passwd which is outside ws → Ask.
            let (_dir, ws_root) = setup();
            let link = ws_root.join("leak");
            symlink("/etc/passwd", &link).unwrap();
            let out = validate_command_paths_with_fs(
                PathCommand::Cat,
                &args(&[link.to_str().unwrap()]),
                &ws_root,
                std::slice::from_ref(&ws_root),
                &PathBuf::from("/home/user"),
                &RealFsResolver,
            );
            assert!(
                matches!(out, PathValidationOutcome::Ask { ref reason, .. }
                    if reason.contains("symlink") && reason.contains("/etc/passwd")),
                "expected Ask citing symlink → /etc/passwd, got {out:?}",
            );
        }

        #[test]
        fn req_safety_490_3_1_g_1_real_symlink_inside_workspace_passes() {
            // Symlink target is inside workspace → no escape → Passthrough.
            let (_dir, ws_root) = setup();
            let target = ws_root.join("real.txt");
            std::fs::write(&target, b"ok").unwrap();
            let link = ws_root.join("alias");
            symlink(&target, &link).unwrap();
            let out = validate_command_paths_with_fs(
                PathCommand::Cat,
                &args(&[link.to_str().unwrap()]),
                &ws_root,
                std::slice::from_ref(&ws_root),
                &PathBuf::from("/home/user"),
                &RealFsResolver,
            );
            assert_eq!(out, PathValidationOutcome::Passthrough);
        }

        #[test]
        fn req_safety_490_3_1_g_1_real_symlink_loop_asks() {
            // T-SYM-4: A → B → A. Walker exits unclean → Ask.
            let (_dir, ws_root) = setup();
            let a = ws_root.join("A");
            let b = ws_root.join("B");
            symlink(&b, &a).unwrap();
            symlink(&a, &b).unwrap();
            let out = validate_command_paths_with_fs(
                PathCommand::Cat,
                &args(&[a.to_str().unwrap()]),
                &ws_root,
                std::slice::from_ref(&ws_root),
                &PathBuf::from("/home/user"),
                &RealFsResolver,
            );
            assert!(
                matches!(out, PathValidationOutcome::Ask { ref reason, .. }
                    if reason.contains("symlink chain truncated")),
                "expected Ask citing chain truncation, got {out:?}",
            );
        }

        #[test]
        fn req_safety_490_3_1_g_1_real_dangling_symlink_via_realpath_fallback() {
            // T-SYM-2: dangling symlink. lstat succeeds (Symlink),
            // readlink succeeds, next-hop lstat fails (no fallback
            // → walker terminates unclean) → Ask.
            let (_dir, ws_root) = setup();
            let link = ws_root.join("dangle");
            symlink("/nonexistent/target/file", &link).unwrap();
            let out = validate_command_paths_with_fs(
                PathCommand::Cat,
                &args(&[link.to_str().unwrap()]),
                &ws_root,
                std::slice::from_ref(&ws_root),
                &PathBuf::from("/home/user"),
                &RealFsResolver,
            );
            assert!(
                matches!(out, PathValidationOutcome::Ask { ref reason, .. }
                    if reason.contains("/nonexistent")
                        || reason.contains("symlink chain truncated")),
                "expected Ask, got {out:?}",
            );
        }

        #[test]
        fn req_safety_490_3_1_g_1_nonexistent_path_in_workspace_passes() {
            // Lexically inside ws, no lstat hit, no chain to walk.
            // Walker returns singleton unclean → preserve 3.1.A
            // Passthrough (consistent with Noop semantics for paths
            // the resolver cannot resolve).
            let (_dir, ws_root) = setup();
            let out = validate_command_paths_with_fs(
                PathCommand::Cat,
                &args(&[ws_root.join("future.txt").to_str().unwrap()]),
                &ws_root,
                std::slice::from_ref(&ws_root),
                &PathBuf::from("/home/user"),
                &RealFsResolver,
            );
            assert_eq!(out, PathValidationOutcome::Passthrough);
        }
    }
}
