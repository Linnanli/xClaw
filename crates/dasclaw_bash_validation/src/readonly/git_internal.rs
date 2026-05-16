//! Git-internal write-path protection — verbatim port of upstream
//! `claude-code-main/src/tools/BashTool/readOnlyValidation.ts` L1760–1865.
//!
//! ## Why this exists
//!
//! Git resolves `$PWD` as a *bare repository* root when it finds the four
//! magic items at the top level: `HEAD`, `objects/`, `refs/`, `hooks/`.
//! An attacker who can write any of those items inside a CWD that the
//! agent later passes to git triggers arbitrary code execution via the
//! `hooks/pre-commit`-style hook script — without ever touching the
//! real `.git/` directory.
//!
//! Sandbox-escape proof-of-concept (upstream L1834-L1836):
//!
//! ```bash
//! mkdir -p hooks && echo '#!/bin/bash\nmalicious' > hooks/pre-commit && git status
//! ```
//!
//! [`command_writes_to_git_internal_paths`] detects writes to any of
//! those four roots, in either of the two attack vectors:
//!
//! 1. **Output redirection** — `echo m > hooks/pre-commit`
//! 2. **Write-class commands with explicit paths** — `mkdir hooks`,
//!    `touch HEAD`, `cp src refs/heads/master`, `mv x objects/aa/bb`
//!
//! [`command_has_any_git`] gates whether this check should even run —
//! upstream only triggers git-internal protection when the compound also
//! contains a git invocation (defense-in-depth, no false positives on
//! non-git workflows that happen to write `hooks/`).
//!
//! ## Slice boundary (Phase 3.2.E.1)
//!
//! This slice is **pure addition**:
//!
//! - `pub fn`s exposed via `lib.rs`
//! - **No** wiring into [`crate::readonly::main_entry::is_command_read_only`]
//!   or a new compound dispatcher — that is Phase 3.2.E.2
//! - **No** Hook接线 / `DecisionReason` widening — Phase 3.2.E.3
//!
//! ## Upstream parity refs
//!
//! - `GIT_INTERNAL_PATTERNS` / `isGitInternalPath` — L1771-L1786
//! - `NON_CREATING_WRITE_COMMANDS` — L1789
//! - `extractWritePathsFromSubcommand` — L1795-L1822
//! - `commandWritesToGitInternalPaths` — L1840-L1865
//! - `commandHasAnyGit` — L1760-L1764
//! - `isNormalizedGitCommand` — `bashPermissions.ts` L2567-L2594

use std::path::Path;

use dasclaw_bash_permissions::strip_safe_wrappers;
use dasclaw_shell_command::bash::{try_parse_shell, try_parse_word_only_commands_sequence};
use once_cell::sync::Lazy;
use regex::Regex;
use tree_sitter::{Node, Tree};

use crate::path_validation::{extract_paths, FileOperationType, PathCommand};
use crate::redirects::extract_output_redirections;

// ---------------------------------------------------------------------------
// 1. is_git_internal_path — upstream L1771-L1786
// ---------------------------------------------------------------------------

/// Path prefixes that, when present at the top level of $PWD, make git
/// treat $PWD as a bare repository root. Mirrors upstream
/// `GIT_INTERNAL_PATTERNS` (`readOnlyValidation.ts:1771`).
///
/// The four entries are anchored at start-of-string and require either
/// end-of-string or a trailing `/`, so we only match `hooks`, `hooks/`,
/// `hooks/pre-commit`, etc. — never `myhooks/` or `repo-hooks`.
static GIT_INTERNAL_PATTERNS: Lazy<[Regex; 4]> = Lazy::new(|| {
    [
        Regex::new(r"^HEAD$").expect("static regex"), // safety: literal pattern
        Regex::new(r"^objects(?:/|$)").expect("static regex"), // safety: literal pattern
        Regex::new(r"^refs(?:/|$)").expect("static regex"), // safety: literal pattern
        Regex::new(r"^hooks(?:/|$)").expect("static regex"), // safety: literal pattern
    ]
});

/// Returns `true` when `path` resolves to one of the four git-internal
/// roots (HEAD / objects/ / refs/ / hooks/). Strips a single leading
/// `./` or `/` before matching, mirroring upstream
/// `path.replace(/^\.?\//, '')` semantics.
///
/// **Scope**: only flags *CWD-relative* writes that masquerade as a
/// bare repo. Absolute writes like `/tmp/x/hooks/y` are intentionally
/// not flagged here — they are an entirely different attack surface
/// guarded by `path_constraints`.
#[must_use]
pub fn is_git_internal_path(path: &str) -> bool {
    let normalized = strip_leading_dot_slash(path);
    GIT_INTERNAL_PATTERNS
        .iter()
        .any(|re| re.is_match(normalized))
}

/// Mirror of upstream's `path.replace(/^\.?\//, '')`: strip a single
/// leading `/` or `./` (and nothing more — `..` stays, `./foo/./bar`
/// keeps the inner `./`).
fn strip_leading_dot_slash(path: &str) -> &str {
    if let Some(rest) = path.strip_prefix("./") {
        return rest;
    }
    if let Some(rest) = path.strip_prefix('/') {
        return rest;
    }
    path
}

// ---------------------------------------------------------------------------
// 2. NON_CREATING_WRITE_COMMANDS — upstream L1789
// ---------------------------------------------------------------------------

/// Commands that only delete or modify in-place — they cannot *create*
/// new files at new paths, so they cannot stand up a bare-repo
/// masquerade. Mirrors upstream `NON_CREATING_WRITE_COMMANDS` Set.
///
/// `sed` is included because `sed -i file` modifies `file` in place; it
/// cannot point at a path that does not already exist.
const NON_CREATING_WRITE_COMMANDS: &[&str] = &["rm", "rmdir", "sed"];

fn is_non_creating_write_command(base: &str) -> bool {
    NON_CREATING_WRITE_COMMANDS.contains(&base)
}

// ---------------------------------------------------------------------------
// 3. is_normalized_git_command / command_has_any_git
//    — upstream bashPermissions.ts:2567 + readOnlyValidation.ts:1760
// ---------------------------------------------------------------------------

/// Returns `true` when `command` (a *single* subcommand string, no
/// `&&` / `||` / `;` / `|`) is a git invocation after peeling safe
/// wrappers (env var prefix, `timeout`, `time`, `command`, `builtin`,
/// shell-quote wrappers). Mirrors upstream `isNormalizedGitCommand`.
///
/// SECURITY: bypass-resistant against
/// - `'git' status` — shell quotes (handled by tree-sitter parse)
/// - `NO_COLOR=1 git status` — env-var prefix (handled by
///   [`strip_safe_wrappers`])
/// - `xargs git fetch` — xargs runs git in CWD, so it counts as git
///   for the cd+git / git-internal guards
#[must_use]
pub fn is_normalized_git_command(command: &str) -> bool {
    // Fast path — catches the most common case before any parsing.
    // Mirrors upstream L2575-L2577.
    if command.starts_with("git ") || command == "git" {
        return true;
    }

    let stripped = strip_safe_wrappers(command);
    let stripped_trim = stripped.trim_start();

    // AST tokenize — handles quoted forms like `'git' status` correctly.
    if let Some(tokens) = parse_command_tokens(stripped_trim) {
        if tokens.is_empty() {
            return false;
        }
        // Direct git command.
        if tokens[0] == "git" {
            return true;
        }
        // `xargs git ...` — xargs runs git in CWD, so it must count as
        // a git command for cd+git / git-internal security checks.
        // Mirrors upstream L2585-L2588.
        if tokens[0] == "xargs" && tokens.iter().any(|t| t == "git") {
            return true;
        }
        return false;
    }

    // Fallback — AST parse failed. Mirrors upstream L2591 regex test.
    static FALLBACK: Lazy<Regex> = Lazy::new(|| Regex::new(r"^git(?:\s|$)").expect("static regex")); // safety: literal pattern
    FALLBACK.is_match(stripped_trim)
}

/// Returns `true` if any subcommand of the compound `command` is a git
/// invocation. Mirrors upstream `commandHasAnyGit`
/// (`readOnlyValidation.ts:1760`).
///
/// Compound splitting uses [`walk_top_level_command_argvs`] so commands
/// containing redirections (which the strict AST splitter rejects) are
/// still scanned — important because the canonical attack
/// (`mkdir hooks && echo m > hooks/pre-commit && git status`) contains
/// a redirection.
#[must_use]
pub fn command_has_any_git(command: &str) -> bool {
    let Some(tree) = try_parse_shell(command) else {
        // Parse failed — fall back to whole-string check.
        return is_normalized_git_command(command.trim());
    };
    for argv in walk_top_level_command_argvs(&tree, command) {
        if argv.is_empty() {
            continue;
        }
        // Reconstruct the subcommand text from argv for the wrapper /
        // env-prefix normalization to apply. argv from the AST already
        // skips env-prefix variable_assignments, so argv[0] is the
        // actual command — but we still feed `argv.join(" ")` through
        // `is_normalized_git_command` so the xargs-git branch sees the
        // full argv.
        let subcmd = argv.join(" ");
        if is_normalized_git_command(&subcmd) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// 4. extract_write_paths_from_subcommand — upstream L1795-L1822
// ---------------------------------------------------------------------------

/// Extract the *write-creation* path arguments of a single subcommand.
///
/// Returns the subset of paths that the command would *create* (mkdir,
/// touch, cp, mv) — `rm` / `rmdir` / `sed` are excluded because they
/// cannot create new files at new paths (and therefore cannot stand up
/// a bare-repo masquerade).
///
/// Mirrors upstream `extractWritePathsFromSubcommand`.
///
/// Returns an empty vector when:
/// - the subcommand cannot be parsed
/// - the base command is not in [`PathCommand`]
/// - the base command's [`FileOperationType`] is not `Write` or `Create`
/// - the base command is in [`NON_CREATING_WRITE_COMMANDS`]
#[must_use]
pub fn extract_write_paths_from_subcommand(subcommand: &str) -> Vec<String> {
    let Some(tree) = try_parse_shell(subcommand) else {
        return Vec::new();
    };
    // Use the strict single-command argv extraction — the input here is
    // assumed to be one subcommand (no `&&`/`||`/`;`/`|`).
    let argvs = walk_top_level_command_argvs(&tree, subcommand);
    if argvs.len() != 1 {
        return Vec::new();
    }
    let Some(argv) = argvs.into_iter().next() else {
        return Vec::new();
    };
    extract_write_paths_from_argv(&argv)
}

/// Inner: apply the upstream "write/create operation, not in-place"
/// filter to an already-tokenized argv. Returns the path strings the
/// command would create.
fn extract_write_paths_from_argv(argv: &[String]) -> Vec<String> {
    let Some(base) = argv.first() else {
        return Vec::new();
    };
    if is_non_creating_write_command(base) {
        return Vec::new();
    }
    let Some(cmd) = PathCommand::parse(base) else {
        return Vec::new();
    };
    match cmd.operation_type() {
        FileOperationType::Write | FileOperationType::Create => {}
        FileOperationType::Read => return Vec::new(),
    }
    // The `home` argument is only consulted by `extract_paths` for
    // bare `cd` (no args) — irrelevant here because `cd`'s
    // operation_type is `Read`, so we never reach the cd branch. Pass
    // a placeholder root path that will never be inspected.
    let placeholder_home = Path::new("/");
    extract_paths(cmd, &argv[1..], placeholder_home)
}

// ---------------------------------------------------------------------------
// 5. command_writes_to_git_internal_paths — upstream L1840-L1865
// ---------------------------------------------------------------------------

/// Returns `true` if the compound `command` contains any subcommand
/// that creates or redirects into a git-internal path
/// (HEAD / objects/ / refs/ / hooks/), exploitable for the bare-repo
/// masquerade attack documented in [`is_git_internal_path`].
///
/// Mirrors upstream `commandWritesToGitInternalPaths`.
///
/// The function scans **two attack vectors**:
///
/// 1. **Output redirection targets** — via
///    [`crate::redirects::extract_output_redirections`]. Catches
///    `echo m > hooks/pre-commit` and similar.
/// 2. **Write-class commands with explicit path arguments** — via
///    [`walk_top_level_command_argvs`] + [`extract_write_paths_from_argv`].
///    Catches `mkdir hooks` / `touch HEAD` / `cp src refs/heads/main`.
///
/// Either match → `true`.
#[must_use]
pub fn command_writes_to_git_internal_paths(command: &str) -> bool {
    // Vector 1 — output redirections (compound-aware: scans the entire
    // parse tree for `file_redirect` nodes).
    let redirs = extract_output_redirections(command);
    for r in &redirs.redirections {
        if is_git_internal_path(&r.target) {
            return true;
        }
    }

    // Vector 2 — write-class commands with explicit path args. Walk
    // every top-level `command` node (including those wrapped under
    // `redirected_statement`) and probe via the write-path extractor.
    let Some(tree) = try_parse_shell(command) else {
        return false;
    };
    for argv in walk_top_level_command_argvs(&tree, command) {
        for p in extract_write_paths_from_argv(&argv) {
            if is_git_internal_path(&p) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// 6. AST walker — collect every top-level `command`'s argv even when
//    redirections are present (which would otherwise reject the strict
//    word-only splitter). Replaces the call sites of upstream
//    `splitCommand_DEPRECATED` for this slice.
// ---------------------------------------------------------------------------

/// Walk `tree` and return the argv of every `command` node reachable
/// from the root, skipping into command/process substitutions (those
/// have separate execution semantics that the readonly gates don't
/// model here — they are guarded elsewhere by
/// [`crate::contains_unquoted_expansion`] equivalents in the main
/// entry).
///
/// Argv extraction is intentionally lenient: it skips
/// `variable_assignment` (env-prefix), strips outer quotes on string /
/// raw_string literals, and ignores other unknown child kinds. This is
/// fine for the path-extractor use case — words like `mkdir`, `hooks`,
/// `refs/heads/main` are simple literals; complex expansions would not
/// be classified as write paths anyway.
fn walk_top_level_command_argvs(tree: &Tree, src: &str) -> Vec<Vec<String>> {
    let src_bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "command" => {
                if let Some(argv) = parse_command_argv_lenient(node, src_bytes) {
                    out.push(argv);
                }
                // Don't recurse into `command` children — argv parsing
                // above already consumed them.
                continue;
            }
            // Don't descend into substitutions / heredocs — their
            // contents execute under a different context that
            // git-internal protection doesn't model.
            "command_substitution"
            | "process_substitution"
            | "subshell"
            | "heredoc_body"
            | "heredoc_redirect" => continue,
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    out
}

/// Lenient single-command argv parse. Returns `None` only when the
/// argv is empty (no usable words).
fn parse_command_argv_lenient(cmd: Node<'_>, src: &[u8]) -> Option<Vec<String>> {
    if cmd.kind() != "command" {
        return None;
    }
    let mut words = Vec::new();
    let mut cursor = cmd.walk();
    for child in cmd.named_children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "command_name" => {
                if let Some(inner) = child.named_child(0) {
                    if let Ok(txt) = inner.utf8_text(src) {
                        words.push(unquote_word(inner.kind(), txt));
                    }
                }
            }
            "word" | "number" => {
                if let Ok(txt) = child.utf8_text(src) {
                    words.push(txt.to_owned());
                }
            }
            "string" | "raw_string" => {
                if let Ok(txt) = child.utf8_text(src) {
                    words.push(unquote_word(kind, txt));
                }
            }
            "concatenation" => {
                // Reassemble concatenated args like `-g"*.py"` by
                // unquoting each piece.
                let mut buf = String::new();
                let mut inner = child.walk();
                for part in child.named_children(&mut inner) {
                    let pk = part.kind();
                    if let Ok(txt) = part.utf8_text(src) {
                        match pk {
                            "word" | "number" => buf.push_str(txt),
                            "string" | "raw_string" => buf.push_str(&unquote_word(pk, txt)),
                            _ => {}
                        }
                    }
                }
                if !buf.is_empty() {
                    words.push(buf);
                }
            }
            // Skip env-var prefixes / comments / anything else: argv[0]
            // becomes the actual command, matching `strip_safe_wrappers`
            // semantics for the env-prefix case.
            _ => {}
        }
    }
    if words.is_empty() {
        None
    } else {
        Some(words)
    }
}

/// Strip a *single* matching outer quote pair from a tree-sitter
/// `string` / `raw_string` literal text. Inner escapes are not
/// decoded (the path extractors operate on simple literals).
fn unquote_word(kind: &str, txt: &str) -> String {
    match kind {
        "raw_string" => {
            if txt.len() >= 2 && txt.starts_with('\'') && txt.ends_with('\'') {
                txt[1..txt.len() - 1].to_owned()
            } else {
                txt.to_owned()
            }
        }
        "string" => {
            if txt.len() >= 2 && txt.starts_with('"') && txt.ends_with('"') {
                txt[1..txt.len() - 1].to_owned()
            } else {
                txt.to_owned()
            }
        }
        _ => txt.to_owned(),
    }
}

/// Tokenize a *single* subcommand string into a flat `Vec<String>` for
/// [`is_normalized_git_command`]. Returns `None` if tree-sitter cannot
/// parse the string at all; otherwise returns the lenient argv of the
/// first `command` node (and an empty `Vec` if no `command` node
/// exists — e.g. the string contains only a redirection).
fn parse_command_tokens(subcmd: &str) -> Option<Vec<String>> {
    let tree = try_parse_shell(subcmd)?;
    let argvs = walk_top_level_command_argvs(&tree, subcmd);
    Some(argvs.into_iter().next().unwrap_or_default())
}

// Sanity check — `try_parse_word_only_commands_sequence` is intentionally
// not used here even though it's imported elsewhere, because it rejects
// any compound containing a redirection (which is the dominant attack
// vector). The walker above is the right primitive.
#[allow(dead_code)]
fn _sequence_check(tree: &Tree, src: &str) -> Option<Vec<Vec<String>>> {
    try_parse_word_only_commands_sequence(tree, src)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_git_internal_path ----

    #[test]
    fn git_internal_head_exact() {
        assert!(is_git_internal_path("HEAD"));
        assert!(is_git_internal_path("./HEAD"));
        assert!(is_git_internal_path("/HEAD"));
    }

    #[test]
    fn git_internal_head_not_prefix() {
        // HEAD$ must not match `HEADER` or `HEAD.bak`.
        assert!(!is_git_internal_path("HEADER"));
        assert!(!is_git_internal_path("HEAD.bak"));
        assert!(!is_git_internal_path("HEAD/something"));
    }

    #[test]
    fn git_internal_objects_dir_and_file() {
        assert!(is_git_internal_path("objects"));
        assert!(is_git_internal_path("objects/"));
        assert!(is_git_internal_path("objects/aa/bb"));
        assert!(is_git_internal_path("./objects/aa"));
        assert!(is_git_internal_path("/objects/aa"));
    }

    #[test]
    fn git_internal_objects_word_boundary() {
        // `objects-backup` is not a git-internal write target.
        assert!(!is_git_internal_path("objects-backup"));
        assert!(!is_git_internal_path("myobjects/aa"));
    }

    #[test]
    fn git_internal_refs_dir_and_file() {
        assert!(is_git_internal_path("refs"));
        assert!(is_git_internal_path("refs/heads"));
        assert!(is_git_internal_path("refs/heads/main"));
        assert!(is_git_internal_path("./refs/heads/main"));
        assert!(is_git_internal_path("/refs/heads/main"));
    }

    #[test]
    fn git_internal_refs_word_boundary() {
        assert!(!is_git_internal_path("refspec"));
        assert!(!is_git_internal_path("myrefs/heads"));
    }

    #[test]
    fn git_internal_hooks_dir_and_file() {
        assert!(is_git_internal_path("hooks"));
        assert!(is_git_internal_path("hooks/pre-commit"));
        assert!(is_git_internal_path("./hooks/post-merge"));
        assert!(is_git_internal_path("/hooks/pre-push"));
    }

    #[test]
    fn git_internal_hooks_word_boundary() {
        assert!(!is_git_internal_path("hookscript"));
        assert!(!is_git_internal_path("myhooks/x"));
    }

    #[test]
    fn git_internal_dotgit_path_is_not_flagged_here() {
        // Upstream semantics: `.git/hooks/pre-commit` is NOT flagged by
        // this function. It's a different attack surface (writing into
        // an existing repo's `.git/`), guarded elsewhere via
        // `isCurrentDirectoryBareGitRepo` (not ported). The `^\.?\/`
        // strip only removes a leading `./` or `/`, not `.git/`.
        assert!(!is_git_internal_path(".git/hooks/pre-commit"));
        assert!(!is_git_internal_path(".git/HEAD"));
    }

    #[test]
    fn git_internal_absolute_paths_outside_cwd_not_flagged() {
        // Absolute paths in /tmp / /var / etc. are not bare-repo
        // masquerade targets — they'd be a different attack surface
        // (writing into an existing repo's hooks dir).
        assert!(!is_git_internal_path("/tmp/x/hooks/y"));
        assert!(!is_git_internal_path("/var/cache/objects/aa"));
    }

    #[test]
    fn git_internal_trailing_slash_only() {
        // `objects/` (just the dir with trailing slash) is flagged.
        assert!(is_git_internal_path("hooks/"));
        assert!(is_git_internal_path("refs/"));
        assert!(is_git_internal_path("objects/"));
    }

    // ---- NON_CREATING_WRITE_COMMANDS ----

    #[test]
    fn non_creating_write_commands_set() {
        assert!(is_non_creating_write_command("rm"));
        assert!(is_non_creating_write_command("rmdir"));
        assert!(is_non_creating_write_command("sed"));
        assert!(!is_non_creating_write_command("mkdir"));
        assert!(!is_non_creating_write_command("touch"));
        assert!(!is_non_creating_write_command("cp"));
        assert!(!is_non_creating_write_command("mv"));
    }

    // ---- is_normalized_git_command ----

    #[test]
    fn normalized_git_fast_path_bare() {
        assert!(is_normalized_git_command("git"));
        assert!(is_normalized_git_command("git status"));
        assert!(is_normalized_git_command("git fetch --all"));
    }

    #[test]
    fn normalized_git_env_prefix() {
        // `strip_safe_wrappers` peels env-var prefixes (NO_COLOR,
        // GIT_PAGER, etc.) so `NO_COLOR=1 git status` is detected.
        assert!(is_normalized_git_command("NO_COLOR=1 git status"));
        assert!(is_normalized_git_command("GIT_PAGER=cat git log"));
    }

    #[test]
    fn normalized_git_timeout_prefix() {
        // `timeout 30s git status` — `timeout` is in
        // `strip_safe_wrappers`'s safe wrapper set.
        assert!(is_normalized_git_command("timeout 30s git status"));
    }

    #[test]
    fn normalized_git_quoted_form() {
        // `'git' status` — tree-sitter unwraps the raw_string.
        assert!(is_normalized_git_command("'git' status"));
        assert!(is_normalized_git_command("\"git\" status"));
    }

    #[test]
    fn normalized_git_xargs_form() {
        // `xargs git ...` — xargs runs git in CWD, counts as git.
        assert!(is_normalized_git_command("xargs git status"));
        assert!(is_normalized_git_command("xargs -I {} git checkout {}"));
    }

    #[test]
    fn normalized_git_xargs_without_git_is_not_git() {
        // `xargs ls` — not a git command.
        assert!(!is_normalized_git_command("xargs ls"));
        assert!(!is_normalized_git_command("xargs -I {} echo {}"));
    }

    #[test]
    fn normalized_git_non_git_rejected() {
        assert!(!is_normalized_git_command("ls"));
        assert!(!is_normalized_git_command("cat HEAD"));
        assert!(!is_normalized_git_command("github-cli pr list"));
        assert!(!is_normalized_git_command("gitlab-runner exec"));
    }

    #[test]
    fn normalized_git_empty_string() {
        assert!(!is_normalized_git_command(""));
        assert!(!is_normalized_git_command("   "));
    }

    // ---- command_has_any_git ----

    #[test]
    fn has_any_git_simple_compound() {
        assert!(command_has_any_git("ls && git status"));
        assert!(command_has_any_git("git status || true"));
        assert!(command_has_any_git("echo x; git fetch"));
    }

    #[test]
    fn has_any_git_with_redirection_in_other_subcommand() {
        // Canonical attack — the redirection is what
        // `try_parse_word_only_commands_sequence` would reject, but
        // our AST walker still finds the `git status` command node.
        assert!(command_has_any_git(
            "mkdir -p hooks && echo m > hooks/pre-commit && git status"
        ));
    }

    #[test]
    fn has_any_git_env_prefix_in_compound() {
        assert!(command_has_any_git("mkdir hooks && NO_COLOR=1 git status"));
    }

    #[test]
    fn has_any_git_xargs_in_compound() {
        assert!(command_has_any_git("ls && xargs git status"));
    }

    #[test]
    fn has_any_git_none() {
        assert!(!command_has_any_git("ls && cat /etc/hosts"));
        assert!(!command_has_any_git("mkdir hooks && echo m > hooks/x"));
    }

    // ---- extract_write_paths_from_subcommand ----

    #[test]
    fn write_paths_mkdir() {
        let paths = extract_write_paths_from_subcommand("mkdir hooks");
        assert_eq!(paths, vec!["hooks".to_string()]);
    }

    #[test]
    fn write_paths_mkdir_with_flag() {
        let paths = extract_write_paths_from_subcommand("mkdir -p refs/heads");
        assert_eq!(paths, vec!["refs/heads".to_string()]);
    }

    #[test]
    fn write_paths_touch() {
        let paths = extract_write_paths_from_subcommand("touch HEAD");
        assert_eq!(paths, vec!["HEAD".to_string()]);
    }

    #[test]
    fn write_paths_cp() {
        let paths = extract_write_paths_from_subcommand("cp src refs/heads/main");
        assert!(paths.contains(&"refs/heads/main".to_string()));
    }

    #[test]
    fn write_paths_mv() {
        let paths = extract_write_paths_from_subcommand("mv src objects/aa/bb");
        assert!(paths.contains(&"objects/aa/bb".to_string()));
    }

    #[test]
    fn write_paths_rm_excluded() {
        // rm cannot create new files at new paths → empty.
        let paths = extract_write_paths_from_subcommand("rm hooks/pre-commit");
        assert!(paths.is_empty());
    }

    #[test]
    fn write_paths_rmdir_excluded() {
        let paths = extract_write_paths_from_subcommand("rmdir hooks");
        assert!(paths.is_empty());
    }

    #[test]
    fn write_paths_sed_excluded() {
        // sed -i modifies in-place → empty.
        let paths = extract_write_paths_from_subcommand("sed -i s/a/b/ HEAD");
        assert!(paths.is_empty());
    }

    #[test]
    fn write_paths_read_only_commands_empty() {
        // ls/cat/grep are not write/create → empty.
        assert!(extract_write_paths_from_subcommand("ls hooks").is_empty());
        assert!(extract_write_paths_from_subcommand("cat HEAD").is_empty());
        assert!(extract_write_paths_from_subcommand("grep foo HEAD").is_empty());
    }

    #[test]
    fn write_paths_unknown_command_empty() {
        // `myscript hooks` — not in PathCommand → empty.
        assert!(extract_write_paths_from_subcommand("myscript hooks").is_empty());
    }

    #[test]
    fn write_paths_compound_not_supported_returns_empty() {
        // This function takes a SINGLE subcommand. Feeding it a
        // compound returns empty (the dispatcher in
        // `command_writes_to_git_internal_paths` handles splitting).
        assert!(extract_write_paths_from_subcommand("mkdir hooks && touch HEAD").is_empty());
    }

    // ---- command_writes_to_git_internal_paths — vector 1 (redirection) ----

    #[test]
    fn writes_redirection_to_hooks() {
        assert!(command_writes_to_git_internal_paths(
            "echo 'malicious' > hooks/pre-commit"
        ));
    }

    #[test]
    fn writes_redirection_to_head() {
        assert!(command_writes_to_git_internal_paths("echo ref: > HEAD"));
    }

    #[test]
    fn writes_redirection_to_refs_heads() {
        assert!(command_writes_to_git_internal_paths(
            "echo sha > refs/heads/main"
        ));
    }

    #[test]
    fn writes_redirection_to_objects() {
        assert!(command_writes_to_git_internal_paths(
            "echo blob > objects/aa/bb"
        ));
    }

    #[test]
    fn writes_redirection_append_form() {
        assert!(command_writes_to_git_internal_paths(
            "echo more >> hooks/pre-commit"
        ));
    }

    #[test]
    fn writes_redirection_with_dot_prefix() {
        assert!(command_writes_to_git_internal_paths("echo x > ./hooks/y"));
    }

    #[test]
    fn writes_redirection_compound_attack() {
        // Canonical sandbox-escape PoC.
        assert!(command_writes_to_git_internal_paths(
            "mkdir -p hooks && echo '#!/bin/bash\nmalicious' > hooks/pre-commit && git status"
        ));
    }

    #[test]
    fn writes_redirection_to_safe_target() {
        assert!(!command_writes_to_git_internal_paths(
            "echo log > /tmp/output.txt"
        ));
        assert!(!command_writes_to_git_internal_paths(
            "echo data > ./my-output.log"
        ));
    }

    // ---- command_writes_to_git_internal_paths — vector 2 (write commands) ----

    #[test]
    fn writes_mkdir_hooks() {
        assert!(command_writes_to_git_internal_paths("mkdir hooks"));
        assert!(command_writes_to_git_internal_paths("mkdir -p hooks"));
    }

    #[test]
    fn writes_mkdir_refs_heads() {
        assert!(command_writes_to_git_internal_paths("mkdir -p refs/heads"));
    }

    #[test]
    fn writes_touch_head() {
        assert!(command_writes_to_git_internal_paths("touch HEAD"));
    }

    #[test]
    fn writes_cp_to_refs() {
        assert!(command_writes_to_git_internal_paths(
            "cp src refs/heads/main"
        ));
    }

    #[test]
    fn writes_mv_to_objects() {
        assert!(command_writes_to_git_internal_paths("mv src objects/aa/bb"));
    }

    #[test]
    fn writes_no_redirect_compound_attack() {
        // Variant attack without redirections — uses mkdir to set up
        // refs/heads before git runs.
        assert!(command_writes_to_git_internal_paths(
            "mkdir -p refs/heads && git status"
        ));
    }

    #[test]
    fn writes_mixed_attack_redirect_and_mkdir() {
        // Both vectors fire — redirection AND mkdir, just to confirm
        // either branch alone returns true (short-circuit on the first
        // hit).
        assert!(command_writes_to_git_internal_paths(
            "mkdir -p hooks && touch HEAD && echo m > hooks/pre-commit"
        ));
    }

    // ---- command_writes_to_git_internal_paths — negatives ----

    #[test]
    fn writes_nothing_safe_compound() {
        assert!(!command_writes_to_git_internal_paths(
            "ls -la && cat /etc/hosts"
        ));
    }

    #[test]
    fn writes_rm_to_hooks_not_flagged() {
        // `rm hooks/pre-commit` — rm is non-creating, doesn't stand
        // up the bare-repo masquerade.
        assert!(!command_writes_to_git_internal_paths("rm hooks/pre-commit"));
        assert!(!command_writes_to_git_internal_paths("rmdir hooks"));
    }

    #[test]
    fn writes_sed_inplace_to_head_not_flagged() {
        // `sed -i ... HEAD` — sed is non-creating (-i modifies in
        // place, can't create new file at new path).
        assert!(!command_writes_to_git_internal_paths(
            "sed -i 's/a/b/' HEAD"
        ));
    }

    #[test]
    fn writes_to_similarly_named_dir_not_flagged() {
        // `mkdir hookscript` / `mkdir myhooks` — word-boundary check.
        assert!(!command_writes_to_git_internal_paths("mkdir hookscript"));
        assert!(!command_writes_to_git_internal_paths("mkdir myhooks"));
        assert!(!command_writes_to_git_internal_paths("touch HEADER"));
    }

    #[test]
    fn writes_empty_command() {
        assert!(!command_writes_to_git_internal_paths(""));
        assert!(!command_writes_to_git_internal_paths("   "));
    }

    #[test]
    fn writes_garbage_command_not_flagged() {
        // Parse failures fall through both branches to `false`.
        assert!(!command_writes_to_git_internal_paths(")))"));
    }

    #[test]
    fn writes_absolute_path_to_hooks_not_flagged() {
        // /tmp/x/hooks/y is a different attack surface — not the
        // bare-repo masquerade.
        assert!(!command_writes_to_git_internal_paths(
            "mkdir -p /tmp/x/hooks"
        ));
        assert!(!command_writes_to_git_internal_paths(
            "echo m > /tmp/repo/hooks/x"
        ));
    }

    // ---- AST walker edge cases ----

    #[test]
    fn writes_with_quoted_paths() {
        // `mkdir "hooks"` — string node, outer quotes stripped.
        assert!(command_writes_to_git_internal_paths("mkdir \"hooks\""));
        // `mkdir 'hooks'` — raw_string node.
        assert!(command_writes_to_git_internal_paths("mkdir 'hooks'"));
    }

    #[test]
    fn writes_with_env_prefix_on_write_cmd() {
        // `NO_COLOR=1 mkdir hooks` — env prefix doesn't change the
        // write target.
        assert!(command_writes_to_git_internal_paths(
            "NO_COLOR=1 mkdir hooks"
        ));
    }

    #[test]
    fn writes_pipeline_form_tee_not_flagged_verbatim_upstream() {
        // Upstream `extractWritePathsFromSubcommand` only extracts paths for
        // commands in `COMMAND_OPERATION_TYPE` / `PATH_EXTRACTORS`. `tee` is
        // **not** in that set, so `tee hooks/pre-commit` is NOT flagged here.
        // This is intentional verbatim parity — defense-in-depth covers `tee`
        // via separate output-redirection path constraints elsewhere
        // (`path_constraints::check_path_constraints` on the tee argv).
        //
        // Inclusion of this test is a regression pin: if `tee` is ever
        // added to `PathCommand::parse`, this test reminds us to revisit
        // git-internal coverage.
        assert!(!command_writes_to_git_internal_paths(
            "echo m | tee hooks/pre-commit"
        ));
        // Pipelines without write-class commands stay clean.
        assert!(!command_writes_to_git_internal_paths(
            "echo log | grep refs"
        ));
    }
}
