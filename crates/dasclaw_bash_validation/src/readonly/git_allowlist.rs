//! Phase 3.2.C.rest — verbatim port of `GIT_READ_ONLY_COMMANDS` and the
//! 8 shared GIT flag groups from
//! `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts` L44-924.
//!
//! 24 git subcommands in upstream order:
//!   git diff, git log, git show, git shortlog, git reflog,
//!   git stash list, git ls-remote, git status, git blame, git ls-files,
//!   git config --get, git remote show, git remote, git merge-base,
//!   git rev-parse, git rev-list, git describe, git cat-file,
//!   git for-each-ref, git grep, git stash show, git worktree list,
//!   git tag, git branch.
//!
//! Five commands carry an `additional_dangerous_callback`:
//!   - `git reflog`        : block `expire`/`delete`/`exists` subcommands.
//!   - `git remote show`   : require exactly one alphanumeric remote name.
//!   - `git remote`        : reject any positional argument.
//!   - `git tag`           : reject positional tag creation without `-l/--list`.
//!   - `git branch`        : reject positional branch creation without `-l/--list`
//!     (or optional-arg `--merged`/`--no-merged`).
//!
//! All `// SECURITY:` comments from upstream are preserved verbatim.

use std::sync::LazyLock;

use super::flag_parser::{CommandConfig, FlagArgType};

// ---------------------------------------------------------------------------
// Shared git flag groups (upstream L44-103)
// ---------------------------------------------------------------------------

const GIT_REF_SELECTION_FLAGS: &[(&str, FlagArgType)] = &[
    ("--all", FlagArgType::None),
    ("--branches", FlagArgType::None),
    ("--tags", FlagArgType::None),
    ("--remotes", FlagArgType::None),
];

const GIT_DATE_FILTER_FLAGS: &[(&str, FlagArgType)] = &[
    ("--since", FlagArgType::String),
    ("--after", FlagArgType::String),
    ("--until", FlagArgType::String),
    ("--before", FlagArgType::String),
];

const GIT_LOG_DISPLAY_FLAGS: &[(&str, FlagArgType)] = &[
    ("--oneline", FlagArgType::None),
    ("--graph", FlagArgType::None),
    ("--decorate", FlagArgType::None),
    ("--no-decorate", FlagArgType::None),
    ("--date", FlagArgType::String),
    ("--relative-date", FlagArgType::None),
];

const GIT_COUNT_FLAGS: &[(&str, FlagArgType)] = &[
    ("--max-count", FlagArgType::Number),
    ("-n", FlagArgType::Number),
];

// Stat output flags - used in git log, show, diff
const GIT_STAT_FLAGS: &[(&str, FlagArgType)] = &[
    ("--stat", FlagArgType::None),
    ("--numstat", FlagArgType::None),
    ("--shortstat", FlagArgType::None),
    ("--name-only", FlagArgType::None),
    ("--name-status", FlagArgType::None),
];

// Color output flags - used in git log, show, diff
const GIT_COLOR_FLAGS: &[(&str, FlagArgType)] = &[
    ("--color", FlagArgType::None),
    ("--no-color", FlagArgType::None),
];

// Patch display flags - used in git log, show
const GIT_PATCH_FLAGS: &[(&str, FlagArgType)] = &[
    ("--patch", FlagArgType::None),
    ("-p", FlagArgType::None),
    ("--no-patch", FlagArgType::None),
    ("--no-ext-diff", FlagArgType::None),
    ("-s", FlagArgType::None),
];

// Author/committer filter flags - used in git log, reflog
const GIT_AUTHOR_FILTER_FLAGS: &[(&str, FlagArgType)] = &[
    ("--author", FlagArgType::String),
    ("--committer", FlagArgType::String),
    ("--grep", FlagArgType::String),
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Leak a vector of static-strized flag tuples to obtain a `&'static [...]`
/// usable inside a `CommandConfig`. Called once per command at first access.
fn leak_flags(v: Vec<(&'static str, FlagArgType)>) -> &'static [(&'static str, FlagArgType)] {
    Box::leak(v.into_boxed_slice())
}

// ---------------------------------------------------------------------------
// Callbacks (5)
// ---------------------------------------------------------------------------

// SECURITY: Block `git reflog expire`/`delete`/`exists` — they write to
// .git/logs/**. Bare `git reflog` and `git reflog show` are safe.
fn git_reflog_callback(_raw: &str, args: &[&str]) -> bool {
    const DANGEROUS_SUBCOMMANDS: &[&str] = &["expire", "delete", "exists"];
    for token in args {
        if token.is_empty() || token.starts_with('-') {
            continue;
        }
        if DANGEROUS_SUBCOMMANDS.contains(token) {
            return true;
        }
        return false;
    }
    false
}

// Only allow optional -n, then one alphanumeric remote name.
fn git_remote_show_callback(_raw: &str, args: &[&str]) -> bool {
    let positional: Vec<&&str> = args.iter().filter(|a| **a != "-n").collect();
    if positional.len() != 1 {
        return true;
    }
    let name = positional[0];
    if name.is_empty() {
        return true;
    }
    !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

// Only allow bare `git remote` or `git remote -v/--verbose`.
fn git_remote_callback(_raw: &str, args: &[&str]) -> bool {
    args.iter().any(|a| *a != "-v" && *a != "--verbose")
}

// Helper for git tag / git branch: returns true if a short-flag bundle
// (e.g. `-li`) contains `l`. Mirrors upstream `token.slice(1).includes('l')`.
fn short_bundle_contains_l(token: &str) -> bool {
    let b = token.as_bytes();
    b.len() > 2 && b[0] == b'-' && b[1] != b'-' && !token.contains('=') && token[1..].contains('l')
}

// SECURITY: Block tag creation via positional args. `git tag foo` writes
// .git/refs/tags/foo. Safe uses: bare `git tag`, `git tag -l <pattern>`,
// `git tag --contains <ref>`.
fn git_tag_callback(_raw: &str, args: &[&str]) -> bool {
    const FLAGS_WITH_ARGS: &[&str] = &[
        "--contains",
        "--no-contains",
        "--merged",
        "--no-merged",
        "--points-at",
        "--sort",
        "--format",
        "-n",
    ];
    let mut i = 0;
    let mut seen_list_flag = false;
    let mut seen_dash_dash = false;
    while i < args.len() {
        let token = args[i];
        if token.is_empty() {
            i += 1;
            continue;
        }
        if token == "--" && !seen_dash_dash {
            seen_dash_dash = true;
            i += 1;
            continue;
        }
        if !seen_dash_dash && token.starts_with('-') {
            if token == "--list" || token == "-l" || short_bundle_contains_l(token) {
                seen_list_flag = true;
            }
            if token.contains('=') {
                i += 1;
            } else if FLAGS_WITH_ARGS.contains(&token) {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            // Non-flag positional (or post-`--` positional). Safe only if
            // preceded by -l/--list (then it's a pattern, not a tag name).
            if !seen_list_flag {
                return true;
            }
            i += 1;
        }
    }
    false
}

// SECURITY: Block branch creation via positional args. Safe uses:
// `git branch`, `git branch -flags` (list), or filtering via
// --contains/--merged/etc.
fn git_branch_callback(_raw: &str, args: &[&str]) -> bool {
    // Flags that require an argument
    const FLAGS_WITH_ARGS: &[&str] = &[
        "--contains",
        "--no-contains",
        "--points-at",
        "--sort",
        // --abbrev REMOVED: git does NOT consume detached arg (PARSE_OPT_OPTARG)
    ];
    // Flags with optional arguments (don't require, but can take one)
    const FLAGS_WITH_OPTIONAL_ARGS: &[&str] = &["--merged", "--no-merged"];
    let mut i = 0;
    let mut last_flag: &str = "";
    let mut seen_list_flag = false;
    let mut seen_dash_dash = false;
    while i < args.len() {
        let token = args[i];
        if token.is_empty() {
            i += 1;
            continue;
        }
        // `--` ends flag parsing. `git branch -- -l` CREATES a branch named `-l`.
        if token == "--" && !seen_dash_dash {
            seen_dash_dash = true;
            last_flag = "";
            i += 1;
            continue;
        }
        if !seen_dash_dash && token.starts_with('-') {
            if token == "--list" || token == "-l" || short_bundle_contains_l(token) {
                seen_list_flag = true;
            }
            if let Some(eq_idx) = token.find('=') {
                last_flag = &token[..eq_idx];
                i += 1;
            } else if FLAGS_WITH_ARGS.contains(&token) {
                last_flag = token;
                i += 2;
            } else {
                last_flag = token;
                i += 1;
            }
        } else {
            let last_flag_has_optional_arg = FLAGS_WITH_OPTIONAL_ARGS.contains(&last_flag);
            if !seen_list_flag && !last_flag_has_optional_arg {
                return true; // Positional without --list = branch creation
            }
            i += 1;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Per-command flags + configs (block A: diff, log, show, shortlog, reflog,
// stash list, ls-remote, status)
// ---------------------------------------------------------------------------

// ---- git diff (upstream L108-179) ------------------------------------------
static GIT_DIFF_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> = LazyLock::new(|| {
    let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
    v.extend_from_slice(GIT_STAT_FLAGS);
    v.extend_from_slice(GIT_COLOR_FLAGS);
    v.extend_from_slice(&[
        ("--dirstat", FlagArgType::None),
        ("--summary", FlagArgType::None),
        ("--patch-with-stat", FlagArgType::None),
        ("--word-diff", FlagArgType::None),
        ("--word-diff-regex", FlagArgType::String),
        ("--color-words", FlagArgType::None),
        ("--no-renames", FlagArgType::None),
        ("--no-ext-diff", FlagArgType::None),
        ("--check", FlagArgType::None),
        ("--ws-error-highlight", FlagArgType::String),
        ("--full-index", FlagArgType::None),
        ("--binary", FlagArgType::None),
        ("--abbrev", FlagArgType::Number),
        ("--break-rewrites", FlagArgType::None),
        ("--find-renames", FlagArgType::None),
        ("--find-copies", FlagArgType::None),
        ("--find-copies-harder", FlagArgType::None),
        ("--irreversible-delete", FlagArgType::None),
        ("--diff-algorithm", FlagArgType::String),
        ("--histogram", FlagArgType::None),
        ("--patience", FlagArgType::None),
        ("--minimal", FlagArgType::None),
        ("--ignore-space-at-eol", FlagArgType::None),
        ("--ignore-space-change", FlagArgType::None),
        ("--ignore-all-space", FlagArgType::None),
        ("--ignore-blank-lines", FlagArgType::None),
        ("--inter-hunk-context", FlagArgType::Number),
        ("--function-context", FlagArgType::None),
        ("--exit-code", FlagArgType::None),
        ("--quiet", FlagArgType::None),
        ("--cached", FlagArgType::None),
        ("--staged", FlagArgType::None),
        ("--pickaxe-regex", FlagArgType::None),
        ("--pickaxe-all", FlagArgType::None),
        ("--no-index", FlagArgType::None),
        ("--relative", FlagArgType::String),
        ("--diff-filter", FlagArgType::String),
        ("-p", FlagArgType::None),
        ("-u", FlagArgType::None),
        ("-s", FlagArgType::None),
        ("-M", FlagArgType::None),
        ("-C", FlagArgType::None),
        ("-B", FlagArgType::None),
        ("-D", FlagArgType::None),
        ("-l", FlagArgType::None),
        // SECURITY: -S/-G/-O take REQUIRED string arguments (pickaxe search,
        // pickaxe regex, orderfile). Previously 'none' caused a parser
        // differential with git: `git diff -S -- --output=/tmp/pwned` —
        // validator sees -S as no-arg → advances 1 token → breaks on `--` →
        // --output unchecked. git sees -S requires arg → consumes `--` as the
        // pickaxe string (standard getopt: required-arg options consume next
        // argv unconditionally, BEFORE the top-level `--` check) → cursor at
        // --output=... → parses as long option → ARBITRARY FILE WRITE.
        // git log config correctly has -S/-G as 'string'.
        ("-S", FlagArgType::String),
        ("-G", FlagArgType::String),
        ("-O", FlagArgType::String),
        ("-R", FlagArgType::None),
    ]);
    leak_flags(v)
});
static GIT_DIFF_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_DIFF_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git log (upstream L180-237) -------------------------------------------
static GIT_LOG_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> = LazyLock::new(|| {
    let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
    v.extend_from_slice(GIT_LOG_DISPLAY_FLAGS);
    v.extend_from_slice(GIT_REF_SELECTION_FLAGS);
    v.extend_from_slice(GIT_DATE_FILTER_FLAGS);
    v.extend_from_slice(GIT_COUNT_FLAGS);
    v.extend_from_slice(GIT_STAT_FLAGS);
    v.extend_from_slice(GIT_COLOR_FLAGS);
    v.extend_from_slice(GIT_PATCH_FLAGS);
    v.extend_from_slice(GIT_AUTHOR_FILTER_FLAGS);
    v.extend_from_slice(&[
        ("--abbrev-commit", FlagArgType::None),
        ("--full-history", FlagArgType::None),
        ("--dense", FlagArgType::None),
        ("--sparse", FlagArgType::None),
        ("--simplify-merges", FlagArgType::None),
        ("--ancestry-path", FlagArgType::None),
        ("--source", FlagArgType::None),
        ("--first-parent", FlagArgType::None),
        ("--merges", FlagArgType::None),
        ("--no-merges", FlagArgType::None),
        ("--reverse", FlagArgType::None),
        ("--walk-reflogs", FlagArgType::None),
        ("--skip", FlagArgType::Number),
        ("--max-age", FlagArgType::Number),
        ("--min-age", FlagArgType::Number),
        ("--no-min-parents", FlagArgType::None),
        ("--no-max-parents", FlagArgType::None),
        ("--follow", FlagArgType::None),
        ("--no-walk", FlagArgType::None),
        ("--left-right", FlagArgType::None),
        ("--cherry-mark", FlagArgType::None),
        ("--cherry-pick", FlagArgType::None),
        ("--boundary", FlagArgType::None),
        ("--topo-order", FlagArgType::None),
        ("--date-order", FlagArgType::None),
        ("--author-date-order", FlagArgType::None),
        ("--pretty", FlagArgType::String),
        ("--format", FlagArgType::String),
        ("--diff-filter", FlagArgType::String),
        ("-S", FlagArgType::String),
        ("-G", FlagArgType::String),
        ("--pickaxe-regex", FlagArgType::None),
        ("--pickaxe-all", FlagArgType::None),
    ]);
    leak_flags(v)
});
static GIT_LOG_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_LOG_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git show (upstream L238-258) ------------------------------------------
static GIT_SHOW_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> = LazyLock::new(|| {
    let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
    v.extend_from_slice(GIT_LOG_DISPLAY_FLAGS);
    v.extend_from_slice(GIT_STAT_FLAGS);
    v.extend_from_slice(GIT_COLOR_FLAGS);
    v.extend_from_slice(GIT_PATCH_FLAGS);
    v.extend_from_slice(&[
        ("--abbrev-commit", FlagArgType::None),
        ("--word-diff", FlagArgType::None),
        ("--word-diff-regex", FlagArgType::String),
        ("--color-words", FlagArgType::None),
        ("--pretty", FlagArgType::String),
        ("--format", FlagArgType::String),
        ("--first-parent", FlagArgType::None),
        ("--raw", FlagArgType::None),
        ("--diff-filter", FlagArgType::String),
        ("-m", FlagArgType::None),
        ("--quiet", FlagArgType::None),
    ]);
    leak_flags(v)
});
static GIT_SHOW_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_SHOW_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git shortlog (upstream L259-279) --------------------------------------
static GIT_SHORTLOG_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> = LazyLock::new(|| {
    let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
    v.extend_from_slice(GIT_REF_SELECTION_FLAGS);
    v.extend_from_slice(GIT_DATE_FILTER_FLAGS);
    v.extend_from_slice(&[
        ("-s", FlagArgType::None),
        ("--summary", FlagArgType::None),
        ("-n", FlagArgType::None),
        ("--numbered", FlagArgType::None),
        ("-e", FlagArgType::None),
        ("--email", FlagArgType::None),
        ("-c", FlagArgType::None),
        ("--committer", FlagArgType::None),
        ("--group", FlagArgType::String),
        ("--format", FlagArgType::String),
        ("--no-merges", FlagArgType::None),
        ("--author", FlagArgType::String),
    ]);
    leak_flags(v)
});
static GIT_SHORTLOG_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_SHORTLOG_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git reflog (upstream L280-322) ----------------------------------------
// SECURITY: Block `git reflog expire` (positional subcommand) — it writes
// to .git/logs/** by expiring reflog entries. `git reflog delete` similarly
// writes. Only `git reflog` (bare = show) and `git reflog show` are safe.
// The positional-arg fallthrough would otherwise accept `expire` as a
// non-flag arg, and `--all` is in GIT_REF_SELECTION_FLAGS → passes.
static GIT_REFLOG_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> = LazyLock::new(|| {
    let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
    v.extend_from_slice(GIT_LOG_DISPLAY_FLAGS);
    v.extend_from_slice(GIT_REF_SELECTION_FLAGS);
    v.extend_from_slice(GIT_DATE_FILTER_FLAGS);
    v.extend_from_slice(GIT_COUNT_FLAGS);
    v.extend_from_slice(GIT_AUTHOR_FILTER_FLAGS);
    leak_flags(v)
});
static GIT_REFLOG_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_REFLOG_FLAGS,
    additional_dangerous_callback: Some(git_reflog_callback),
    respects_double_dash: true,
});

// ---- git stash list (upstream L323-329) ------------------------------------
static GIT_STASH_LIST_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> =
    LazyLock::new(|| {
        let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
        v.extend_from_slice(GIT_LOG_DISPLAY_FLAGS);
        v.extend_from_slice(GIT_REF_SELECTION_FLAGS);
        v.extend_from_slice(GIT_COUNT_FLAGS);
        leak_flags(v)
    });
static GIT_STASH_LIST_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_STASH_LIST_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git ls-remote (upstream L330-358) -------------------------------------
// SECURITY: --server-option and -o are INTENTIONALLY EXCLUDED. They transmit
// an arbitrary attacker-controlled string to the remote git server in the
// protocol v2 capability advertisement. This is a network WRITE primitive
// (sending data to remote) on what is supposed to be a read-only command.
// Even without command substitution (which is caught elsewhere),
// `--server-option="sensitive-data"` exfiltrates the value to whatever
// `origin` points to. The read-only path should never enable network writes.
const GIT_LS_REMOTE_FLAGS: &[(&str, FlagArgType)] = &[
    ("--branches", FlagArgType::None),
    ("-b", FlagArgType::None),
    ("--tags", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("--heads", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--refs", FlagArgType::None),
    ("--quiet", FlagArgType::None),
    ("-q", FlagArgType::None),
    ("--exit-code", FlagArgType::None),
    ("--get-url", FlagArgType::None),
    ("--symref", FlagArgType::None),
    ("--sort", FlagArgType::String),
];
const GIT_LS_REMOTE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_LS_REMOTE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git status (upstream L359-393) ----------------------------------------
const GIT_STATUS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--short", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--branch", FlagArgType::None),
    ("-b", FlagArgType::None),
    ("--porcelain", FlagArgType::None),
    ("--long", FlagArgType::None),
    ("--verbose", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--untracked-files", FlagArgType::String),
    ("-u", FlagArgType::String),
    ("--ignored", FlagArgType::None),
    ("--ignore-submodules", FlagArgType::String),
    ("--column", FlagArgType::None),
    ("--no-column", FlagArgType::None),
    ("--ahead-behind", FlagArgType::None),
    ("--no-ahead-behind", FlagArgType::None),
    ("--renames", FlagArgType::None),
    ("--no-renames", FlagArgType::None),
    ("--find-renames", FlagArgType::String),
    ("-M", FlagArgType::String),
];
const GIT_STATUS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_STATUS_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git blame (upstream L394-422) -----------------------------------------
// Verbatim port of upstream `git blame` safeFlags (readOnlyCommandValidation.ts L370-409).
static GIT_BLAME_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> = LazyLock::new(|| {
    let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
    v.extend_from_slice(GIT_COLOR_FLAGS);
    v.extend_from_slice(&[
        // Line range
        ("-L", FlagArgType::String),
        // Output format
        ("--porcelain", FlagArgType::None),
        ("-p", FlagArgType::None),
        ("--line-porcelain", FlagArgType::None),
        ("--incremental", FlagArgType::None),
        ("--root", FlagArgType::None),
        ("--show-stats", FlagArgType::None),
        ("--show-name", FlagArgType::None),
        ("--show-number", FlagArgType::None),
        ("-n", FlagArgType::None),
        ("--show-email", FlagArgType::None),
        ("-e", FlagArgType::None),
        ("-f", FlagArgType::None),
        // Date formatting
        ("--date", FlagArgType::String),
        // Ignore whitespace
        ("-w", FlagArgType::None),
        // Ignore revisions
        ("--ignore-rev", FlagArgType::String),
        ("--ignore-revs-file", FlagArgType::String),
        // Move/copy detection
        ("-M", FlagArgType::None),
        ("-C", FlagArgType::None),
        ("--score-debug", FlagArgType::None),
        // Abbreviation
        ("--abbrev", FlagArgType::Number),
        // Other options
        ("-s", FlagArgType::None),
        ("-l", FlagArgType::None),
        ("-t", FlagArgType::None),
    ]);
    leak_flags(v)
});
static GIT_BLAME_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_BLAME_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git ls-files (upstream L423-462) --------------------------------------
// Verbatim port — added `-f` (was missing); flag order matches upstream.
const GIT_LS_FILES_FLAGS: &[(&str, FlagArgType)] = &[
    // File selection
    ("--cached", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--deleted", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("--modified", FlagArgType::None),
    ("-m", FlagArgType::None),
    ("--others", FlagArgType::None),
    ("-o", FlagArgType::None),
    ("--ignored", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--stage", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--killed", FlagArgType::None),
    ("-k", FlagArgType::None),
    ("--unmerged", FlagArgType::None),
    ("-u", FlagArgType::None),
    // Output format
    ("--directory", FlagArgType::None),
    ("--no-empty-directory", FlagArgType::None),
    ("--eol", FlagArgType::None),
    ("--full-name", FlagArgType::None),
    ("--abbrev", FlagArgType::Number),
    ("--debug", FlagArgType::None),
    ("-z", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("-f", FlagArgType::None),
    // Exclude patterns
    ("--exclude", FlagArgType::String),
    ("-x", FlagArgType::String),
    ("--exclude-from", FlagArgType::String),
    ("-X", FlagArgType::String),
    ("--exclude-per-directory", FlagArgType::String),
    ("--exclude-standard", FlagArgType::None),
    // Error handling
    ("--error-unmatch", FlagArgType::None),
    // Recursion
    ("--recurse-submodules", FlagArgType::None),
];
const GIT_LS_FILES_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_LS_FILES_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git config --get (upstream L463-484) ----------------------------------
// Verbatim port — upstream allows only read-side flags (no --get/--get-all
// flags; the `git config --get` prefix itself signals read-mode).
const GIT_CONFIG_GET_FLAGS: &[(&str, FlagArgType)] = &[
    // No additional flags needed - just reading config values
    ("--local", FlagArgType::None),
    ("--global", FlagArgType::None),
    ("--system", FlagArgType::None),
    ("--worktree", FlagArgType::None),
    ("--default", FlagArgType::String),
    ("--type", FlagArgType::String),
    ("--bool", FlagArgType::None),
    ("--int", FlagArgType::None),
    ("--bool-or-int", FlagArgType::None),
    ("--path", FlagArgType::None),
    ("--expiry-date", FlagArgType::None),
    ("-z", FlagArgType::None),
    ("--null", FlagArgType::None),
    ("--name-only", FlagArgType::None),
    ("--show-origin", FlagArgType::None),
    ("--show-scope", FlagArgType::None),
];
const GIT_CONFIG_GET_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_CONFIG_GET_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git remote show (upstream L476-489) -----------------------------------
// Only allow `git remote show [-n] <name>` where <name> is alphanumeric.
// The callback enforces this; safe_flags only includes `-n`.
const GIT_REMOTE_SHOW_FLAGS: &[(&str, FlagArgType)] = &[("-n", FlagArgType::None)];
const GIT_REMOTE_SHOW_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_REMOTE_SHOW_FLAGS,
    additional_dangerous_callback: Some(git_remote_show_callback),
    respects_double_dash: true,
};

// ---- git remote (upstream L490-505) ----------------------------------------
// Only allow bare `git remote` or `git remote -v`/`--verbose`.
// IMPORTANT: This entry must be matched AFTER `git remote show` because
// longest-prefix lookup is required.
const GIT_REMOTE_FLAGS: &[(&str, FlagArgType)] =
    &[("-v", FlagArgType::None), ("--verbose", FlagArgType::None)];
const GIT_REMOTE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_REMOTE_FLAGS,
    additional_dangerous_callback: Some(git_remote_callback),
    respects_double_dash: true,
};

// ---- git merge-base (upstream L506-518) ------------------------------------
const GIT_MERGE_BASE_FLAGS: &[(&str, FlagArgType)] = &[
    ("--all", FlagArgType::None),
    ("--octopus", FlagArgType::None),
    ("--independent", FlagArgType::None),
    ("--is-ancestor", FlagArgType::None),
    ("--fork-point", FlagArgType::None),
];
const GIT_MERGE_BASE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_MERGE_BASE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git rev-parse (upstream L519-555) -------------------------------------
// Verbatim port — `--short` is `string` upstream (optional length via =N).
const GIT_REV_PARSE_FLAGS: &[(&str, FlagArgType)] = &[
    // SHA resolution and verification
    ("--verify", FlagArgType::None),
    ("--short", FlagArgType::String),
    ("--abbrev-ref", FlagArgType::None),
    ("--symbolic", FlagArgType::None),
    ("--symbolic-full-name", FlagArgType::None),
    // Repository path queries (all read-only)
    ("--show-toplevel", FlagArgType::None),
    ("--show-cdup", FlagArgType::None),
    ("--show-prefix", FlagArgType::None),
    ("--git-dir", FlagArgType::None),
    ("--git-common-dir", FlagArgType::None),
    ("--absolute-git-dir", FlagArgType::None),
    ("--show-superproject-working-tree", FlagArgType::None),
    // Boolean queries
    ("--is-inside-work-tree", FlagArgType::None),
    ("--is-inside-git-dir", FlagArgType::None),
    ("--is-bare-repository", FlagArgType::None),
    ("--is-shallow-repository", FlagArgType::None),
    ("--is-shallow-update", FlagArgType::None),
    ("--path-prefix", FlagArgType::None),
];
const GIT_REV_PARSE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_REV_PARSE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git rev-list (upstream L556-606) --------------------------------------
static GIT_REV_LIST_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> = LazyLock::new(|| {
    let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
    v.extend_from_slice(GIT_REF_SELECTION_FLAGS);
    v.extend_from_slice(GIT_DATE_FILTER_FLAGS);
    v.extend_from_slice(GIT_COUNT_FLAGS);
    v.extend_from_slice(GIT_AUTHOR_FILTER_FLAGS);
    v.extend_from_slice(&[
        // Counting
        ("--count", FlagArgType::None),
        // Traversal control
        ("--reverse", FlagArgType::None),
        ("--first-parent", FlagArgType::None),
        ("--ancestry-path", FlagArgType::None),
        ("--merges", FlagArgType::None),
        ("--no-merges", FlagArgType::None),
        ("--min-parents", FlagArgType::Number),
        ("--max-parents", FlagArgType::Number),
        ("--no-min-parents", FlagArgType::None),
        ("--no-max-parents", FlagArgType::None),
        ("--skip", FlagArgType::Number),
        ("--max-age", FlagArgType::Number),
        ("--min-age", FlagArgType::Number),
        ("--walk-reflogs", FlagArgType::None),
        // Output formatting
        ("--oneline", FlagArgType::None),
        ("--abbrev-commit", FlagArgType::None),
        ("--pretty", FlagArgType::String),
        ("--format", FlagArgType::String),
        ("--abbrev", FlagArgType::Number),
        ("--full-history", FlagArgType::None),
        ("--dense", FlagArgType::None),
        ("--sparse", FlagArgType::None),
        ("--source", FlagArgType::None),
        ("--graph", FlagArgType::None),
    ]);
    leak_flags(v)
});
static GIT_REV_LIST_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_REV_LIST_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git describe (upstream L607-628) --------------------------------------
// Verbatim port — added --first-match; dropped --all/--debug/--first-parent
// (none present upstream).
const GIT_DESCRIBE_FLAGS: &[(&str, FlagArgType)] = &[
    // Tag selection
    ("--tags", FlagArgType::None),
    ("--match", FlagArgType::String),
    ("--exclude", FlagArgType::String),
    // Output control
    ("--long", FlagArgType::None),
    ("--abbrev", FlagArgType::Number),
    ("--always", FlagArgType::None),
    ("--contains", FlagArgType::None),
    ("--first-match", FlagArgType::None),
    ("--exact-match", FlagArgType::None),
    ("--candidates", FlagArgType::Number),
    // Suffix/dirty markers
    ("--dirty", FlagArgType::None),
    ("--broken", FlagArgType::None),
];
const GIT_DESCRIBE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_DESCRIBE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git cat-file (upstream L629-651) --------------------------------------
// Verbatim port. NOTE: --batch (without --check) is intentionally excluded —
// it reads arbitrary objects from stdin which could be exploited in piped
// commands to dump sensitive objects. --batch-check is the safe variant.
const GIT_CAT_FILE_FLAGS: &[(&str, FlagArgType)] = &[
    // Object query modes (all purely read-only)
    ("-t", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("-p", FlagArgType::None),
    ("-e", FlagArgType::None),
    // Batch mode — read-only check variant only
    ("--batch-check", FlagArgType::None),
    // Output control
    ("--allow-undetermined-type", FlagArgType::None),
];
const GIT_CAT_FILE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_CAT_FILE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git for-each-ref (upstream L652-672) ----------------------------------
// Verbatim port — dropped --shell/--perl/--python/--tcl/--color/--no-color/
// --ignore-case (none present upstream).
const GIT_FOR_EACH_REF_FLAGS: &[(&str, FlagArgType)] = &[
    // Output formatting
    ("--format", FlagArgType::String),
    // Sorting
    ("--sort", FlagArgType::String),
    // Limiting
    ("--count", FlagArgType::Number),
    // Filtering
    ("--contains", FlagArgType::String),
    ("--no-contains", FlagArgType::String),
    ("--merged", FlagArgType::String),
    ("--no-merged", FlagArgType::String),
    ("--points-at", FlagArgType::String),
];
const GIT_FOR_EACH_REF_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_FOR_EACH_REF_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git grep (upstream L673-720) ------------------------------------------
// Verbatim port. Dropped non-upstream flags (-a/--text/-I/--column/-z/--null/
// -p/--show-function/--max-count/--all-match/-f file/--exclude-standard).
const GIT_GREP_FLAGS: &[(&str, FlagArgType)] = &[
    // Pattern matching modes
    ("-e", FlagArgType::String),
    ("-E", FlagArgType::None),
    ("--extended-regexp", FlagArgType::None),
    ("-G", FlagArgType::None),
    ("--basic-regexp", FlagArgType::None),
    ("-F", FlagArgType::None),
    ("--fixed-strings", FlagArgType::None),
    ("-P", FlagArgType::None),
    ("--perl-regexp", FlagArgType::None),
    // Match control
    ("-i", FlagArgType::None),
    ("--ignore-case", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--invert-match", FlagArgType::None),
    ("-w", FlagArgType::None),
    ("--word-regexp", FlagArgType::None),
    // Output control
    ("-n", FlagArgType::None),
    ("--line-number", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--count", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("--files-with-matches", FlagArgType::None),
    ("-L", FlagArgType::None),
    ("--files-without-match", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("-H", FlagArgType::None),
    ("--heading", FlagArgType::None),
    ("--break", FlagArgType::None),
    ("--full-name", FlagArgType::None),
    ("--color", FlagArgType::None),
    ("--no-color", FlagArgType::None),
    ("-o", FlagArgType::None),
    ("--only-matching", FlagArgType::None),
    // Context
    ("-A", FlagArgType::Number),
    ("--after-context", FlagArgType::Number),
    ("-B", FlagArgType::Number),
    ("--before-context", FlagArgType::Number),
    ("-C", FlagArgType::Number),
    ("--context", FlagArgType::Number),
    // Boolean operators for multi-pattern
    ("--and", FlagArgType::None),
    ("--or", FlagArgType::None),
    ("--not", FlagArgType::None),
    // Scope control
    ("--max-depth", FlagArgType::Number),
    ("--untracked", FlagArgType::None),
    ("--no-index", FlagArgType::None),
    ("--recurse-submodules", FlagArgType::None),
    ("--cached", FlagArgType::None),
    // Threads
    ("--threads", FlagArgType::Number),
    // Quiet
    ("-q", FlagArgType::None),
    ("--quiet", FlagArgType::None),
];
const GIT_GREP_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_GREP_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git stash show (upstream L721-735) ------------------------------------
// Verbatim port of upstream `git stash show` safeFlags (L721-735) —
// includes COLOR spread; replaces untracked options with diff-mode flags.
static GIT_STASH_SHOW_FLAGS: LazyLock<&'static [(&'static str, FlagArgType)]> =
    LazyLock::new(|| {
        let mut v: Vec<(&'static str, FlagArgType)> = Vec::new();
        v.extend_from_slice(GIT_STAT_FLAGS);
        v.extend_from_slice(GIT_COLOR_FLAGS);
        v.extend_from_slice(GIT_PATCH_FLAGS);
        v.extend_from_slice(&[
            // Diff options
            ("--word-diff", FlagArgType::None),
            ("--word-diff-regex", FlagArgType::String),
            ("--diff-filter", FlagArgType::String),
            ("--abbrev", FlagArgType::Number),
        ]);
        leak_flags(v)
    });
static GIT_STASH_SHOW_CONFIG: LazyLock<CommandConfig> = LazyLock::new(|| CommandConfig {
    safe_flags: *GIT_STASH_SHOW_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
});

// ---- git worktree list (upstream L736-746) ---------------------------------
// Verbatim port — dropped -z (not present upstream).
const GIT_WORKTREE_LIST_FLAGS: &[(&str, FlagArgType)] = &[
    ("--porcelain", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--verbose", FlagArgType::None),
    ("--expire", FlagArgType::String),
];
const GIT_WORKTREE_LIST_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_WORKTREE_LIST_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- git tag (upstream L747-839) -------------------------------------------
// SECURITY: `git tag` is dual-purpose. With `-l`/`--list` (or filter flags
// like --contains) it merely reads. Without those, ANY positional argument
// creates a tag (writes .git/refs/tags/<name>). The callback enforces
// "must have -l/--list before any positional appears (excluding pattern
// arguments to filter flags)".
// Verbatim port of upstream `git tag` safeFlags (L747-839) — dropped
// --color/--no-color/--omit-empty/--create-reflog (not present upstream).
const GIT_TAG_FLAGS: &[(&str, FlagArgType)] = &[
    // List mode flags
    ("-l", FlagArgType::None),
    ("--list", FlagArgType::None),
    ("-n", FlagArgType::Number),
    ("--contains", FlagArgType::String),
    ("--no-contains", FlagArgType::String),
    ("--merged", FlagArgType::String),
    ("--no-merged", FlagArgType::String),
    ("--sort", FlagArgType::String),
    ("--format", FlagArgType::String),
    ("--points-at", FlagArgType::String),
    ("--column", FlagArgType::None),
    ("--no-column", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--ignore-case", FlagArgType::None),
];
const GIT_TAG_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_TAG_FLAGS,
    additional_dangerous_callback: Some(git_tag_callback),
    respects_double_dash: true,
};

// ---- git branch (upstream L840-924) ----------------------------------------
// SECURITY: Same dual-purpose pattern as `git tag`. Positional argument
// without -l/--list creates a branch (writes .git/refs/heads/<name>).
// Callback enforces this; additionally `--merged`/`--no-merged` have
// OPTIONAL args, so a positional immediately after them is permitted.
// Verbatim port of upstream `git branch` safeFlags (L840-867).
// SECURITY: --format is INTENTIONALLY EXCLUDED — see upstream L865 comment.
// --format with %(refname) etc. can be exploited; upstream blocks it.
// --merged/--no-merged are 'none' (optional commit arg handled by callback).
const GIT_BRANCH_FLAGS: &[(&str, FlagArgType)] = &[
    // List mode flags
    ("-l", FlagArgType::None),
    ("--list", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--all", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("--remotes", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("-vv", FlagArgType::None),
    ("--verbose", FlagArgType::None),
    // Display options
    ("--color", FlagArgType::None),
    ("--no-color", FlagArgType::None),
    ("--column", FlagArgType::None),
    ("--no-column", FlagArgType::None),
    // SECURITY: --abbrev stays 'number' so validateFlags accepts --abbrev=N
    // (attached form, safe). DETACHED `--abbrev N` is caught by callback:
    // --abbrev is NOT in FLAGS_WITH_ARGS, so callback treats N as positional
    // without --list → dangerous. Two-layer defense matches upstream.
    ("--abbrev", FlagArgType::Number),
    ("--no-abbrev", FlagArgType::None),
    // Filtering - these take commit/ref arguments
    ("--contains", FlagArgType::String),
    ("--no-contains", FlagArgType::String),
    ("--merged", FlagArgType::None), // Optional commit argument - handled in callback
    ("--no-merged", FlagArgType::None), // Optional commit argument - handled in callback
    ("--points-at", FlagArgType::String),
    // Sorting
    ("--sort", FlagArgType::String),
    // Note: --format is intentionally excluded as it could pose security risks
    // Show current
    ("--show-current", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--ignore-case", FlagArgType::None),
];
const GIT_BRANCH_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GIT_BRANCH_FLAGS,
    additional_dangerous_callback: Some(git_branch_callback),
    respects_double_dash: true,
};

// ---------------------------------------------------------------------------
// Public table (longest-prefix order matters)
// ---------------------------------------------------------------------------

/// 24 read-only git command prefixes, ordered so longer prefixes appear
/// before shorter ones (e.g. `git remote show` precedes `git remote`).
/// Consumers should iterate in order and accept the first match.
pub(crate) static GIT_READ_ONLY_COMMANDS: LazyLock<Vec<(&'static str, &'static CommandConfig)>> =
    LazyLock::new(|| {
        vec![
            ("git diff", &*GIT_DIFF_CONFIG),
            ("git log", &*GIT_LOG_CONFIG),
            ("git show", &*GIT_SHOW_CONFIG),
            ("git shortlog", &*GIT_SHORTLOG_CONFIG),
            ("git reflog", &*GIT_REFLOG_CONFIG),
            ("git stash list", &*GIT_STASH_LIST_CONFIG),
            ("git ls-remote", &GIT_LS_REMOTE_CONFIG),
            ("git status", &GIT_STATUS_CONFIG),
            ("git blame", &GIT_BLAME_CONFIG),
            ("git ls-files", &GIT_LS_FILES_CONFIG),
            ("git config --get", &GIT_CONFIG_GET_CONFIG),
            // longest-prefix: `git remote show` MUST come before `git remote`
            ("git remote show", &GIT_REMOTE_SHOW_CONFIG),
            ("git remote", &GIT_REMOTE_CONFIG),
            ("git merge-base", &GIT_MERGE_BASE_CONFIG),
            ("git rev-parse", &GIT_REV_PARSE_CONFIG),
            ("git rev-list", &*GIT_REV_LIST_CONFIG),
            ("git describe", &GIT_DESCRIBE_CONFIG),
            ("git cat-file", &GIT_CAT_FILE_CONFIG),
            ("git for-each-ref", &GIT_FOR_EACH_REF_CONFIG),
            ("git grep", &GIT_GREP_CONFIG),
            ("git stash show", &*GIT_STASH_SHOW_CONFIG),
            ("git worktree list", &GIT_WORKTREE_LIST_CONFIG),
            ("git tag", &GIT_TAG_CONFIG),
            ("git branch", &GIT_BRANCH_CONFIG),
        ]
    });
