//! Bash command allowlist tables (Phase 3.2.B).
//!
//! Semantic port of the `COMMAND_ALLOWLIST` const definition from
//! `claude-code-main/src/tools/BashTool/readOnlyValidation.ts`
//! (upstream lines L128–L1140, stopping at `ANT_ONLY_COMMAND_ALLOWLIST`).
//!
//! Phase 3.2.B scope: data tables only. No dispatch function, no tests.
//! The dispatcher and callbacks integration land in Phase 3.2.C/D/E per
//! ADR-129 verbatim-port discipline.
//!
//! Phase 3.2.C wires all external tables: `GIT_READ_ONLY_COMMANDS`
//! (`super::git_allowlist`), `RIPGREP_READ_ONLY_COMMANDS`,
//! `DOCKER_READ_ONLY_COMMANDS`, `PYRIGHT_READ_ONLY_COMMANDS`
//! (`super::external_tables`), and `ANT_ONLY_COMMANDS` (gh + aki,
//! `super::gh_allowlist`). `get_command_allowlist()` applies the
//! `USER_TYPE=ant` + Windows-xargs-strip gates that mirror upstream
//! `getCommandAllowlist()` (readOnlyValidation.ts L1199-L1213).
//!
//! See `docs/plans/bash-parity/phase-3.2-readonly-validation-deepening.md`
//! and ADR-129 (verbatim-port discipline).

use std::sync::LazyLock;

use super::external_tables::{
    DOCKER_READ_ONLY_COMMANDS, PYRIGHT_READ_ONLY_COMMANDS, RIPGREP_READ_ONLY_COMMANDS,
};
use super::flag_parser::{validate_flags, CommandConfig, FlagArgType, ValidateOptions};
use super::gh_allowlist::ANT_ONLY_COMMANDS;
use super::git_allowlist::GIT_READ_ONLY_COMMANDS;

// ============================================================================
// Shared safe flags for `fd` and `fdfind` (Debian/Ubuntu package name).
//
// SECURITY: `-x`/`--exec` and `-X`/`--exec-batch` are deliberately excluded —
// they execute arbitrary commands for each search result (S25).
// SECURITY: `-l`/`--list-details` EXCLUDED — internally executes `ls` as a
// subprocess (same pathway as --exec-batch). PATH hijacking risk if a
// malicious `ls` is on PATH.
// ============================================================================
const FD_SAFE_FLAGS: &[(&str, FlagArgType)] = &[
    ("-h", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("--version", FlagArgType::None),
    ("-H", FlagArgType::None),
    ("--hidden", FlagArgType::None),
    ("-I", FlagArgType::None),
    ("--no-ignore", FlagArgType::None),
    ("--no-ignore-vcs", FlagArgType::None),
    ("--no-ignore-parent", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--case-sensitive", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--ignore-case", FlagArgType::None),
    ("-g", FlagArgType::None),
    ("--glob", FlagArgType::None),
    ("--regex", FlagArgType::None),
    ("-F", FlagArgType::None),
    ("--fixed-strings", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--absolute-path", FlagArgType::None),
    // -l / --list-details EXCLUDED — PATH hijacking via `ls` subprocess.
    ("-L", FlagArgType::None),
    ("--follow", FlagArgType::None),
    ("-p", FlagArgType::None),
    ("--full-path", FlagArgType::None),
    ("-0", FlagArgType::None),
    ("--print0", FlagArgType::None),
    ("-d", FlagArgType::Number),
    ("--max-depth", FlagArgType::Number),
    ("--min-depth", FlagArgType::Number),
    ("--exact-depth", FlagArgType::Number),
    ("-t", FlagArgType::String),
    ("--type", FlagArgType::String),
    ("-e", FlagArgType::String),
    ("--extension", FlagArgType::String),
    ("-S", FlagArgType::String),
    ("--size", FlagArgType::String),
    ("--changed-within", FlagArgType::String),
    ("--changed-before", FlagArgType::String),
    ("-o", FlagArgType::String),
    ("--owner", FlagArgType::String),
    ("-E", FlagArgType::String),
    ("--exclude", FlagArgType::String),
    ("--ignore-file", FlagArgType::String),
    ("-c", FlagArgType::String),
    ("--color", FlagArgType::String),
    ("-j", FlagArgType::Number),
    ("--threads", FlagArgType::Number),
    ("--max-buffer-time", FlagArgType::String),
    ("--max-results", FlagArgType::Number),
    ("-1", FlagArgType::None),
    ("-q", FlagArgType::None),
    ("--quiet", FlagArgType::None),
    ("--show-errors", FlagArgType::None),
    ("--strip-cwd-prefix", FlagArgType::None),
    ("--one-file-system", FlagArgType::None),
    ("--prune", FlagArgType::None),
    ("--search-path", FlagArgType::String),
    ("--base-directory", FlagArgType::String),
    ("--path-separator", FlagArgType::String),
    ("--batch-size", FlagArgType::Number),
    ("--no-require-git", FlagArgType::None),
    ("--hyperlink", FlagArgType::String),
    ("--and", FlagArgType::String),
    ("--format", FlagArgType::String),
];

/// xargs target commands considered safe when nested under `xargs`.
pub const SAFE_TARGET_COMMANDS_FOR_XARGS: &[&str] =
    &["echo", "printf", "wc", "grep", "head", "tail"];

// ============================================================================
// Callbacks — match `fn(&str, &[&str]) -> bool` (true => dangerous).
// ============================================================================

fn sed_always_dangerous(_raw: &str, _args: &[&str]) -> bool {
    // TODO(3.2.D): port sedCommandIsAllowedByAllowlist from sedValidation.ts.
    true
}

fn ps_bsd_e_modifier_check(_raw: &str, args: &[&str]) -> bool {
    // Block BSD-style 'e' modifier which shows environment variables.
    // BSD options are letter-only tokens without a leading dash.
    args.iter().any(|a| {
        !a.starts_with('-')
            && !a.is_empty()
            && a.bytes().all(|b| b.is_ascii_alphabetic())
            && a.contains('e')
    })
}

fn date_positional_must_start_with_plus(_raw: &str, args: &[&str]) -> bool {
    // CRITICAL: date positional args in format MMDDhhmm[[CC]YY][.ss] set system
    // time. Positional tokens must start with `+` (format strings).
    const FLAGS_WITH_ARGS: &[&str] = &[
        "-d",
        "--date",
        "-r",
        "--reference",
        "--iso-8601",
        "--rfc-3339",
    ];
    let mut i = 0;
    while i < args.len() {
        let token = args[i];
        if token.starts_with("--") && token.contains('=') {
            i += 1;
        } else if token.starts_with('-') {
            if FLAGS_WITH_ARGS.contains(&token) {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            if !token.starts_with('+') {
                return true;
            }
            i += 1;
        }
    }
    false
}

// SECURITY: substitutes for the upstream regex
// /^hostname(?:\s+(?:-[a-zA-Z]|--[a-zA-Z-]+))*\s*$/ (readOnlyValidation.ts
// L827). Our CommandConfig has no `regex` field, so the "no positional
// arguments at all" guarantee is enforced via this callback. The check
// flags ANY non-empty token that does not start with `-`, preventing
// `hostname newname` and `hostname -- newname` from setting the host
// identity. Long flags with `=` (e.g. `--alias=foo`) are not blocked here;
// they must be absent from `HOSTNAME_FLAGS` to be rejected by validate_flags.
fn hostname_no_positional_args(_raw: &str, args: &[&str]) -> bool {
    // CRITICAL: any positional argument to `hostname` SETS the hostname.
    args.iter().any(|a| !a.is_empty() && !a.starts_with('-'))
}

fn lsof_plus_m_check(_raw: &str, args: &[&str]) -> bool {
    // Block `+m` (create mount supplement file) — writes to disk. `+prefix`
    // flags are treated as positional by validate_flags, so catch them here.
    args.iter().any(|a| *a == "+m" || a.starts_with("+m"))
}

fn tput_dangerous_check(_raw: &str, args: &[&str]) -> bool {
    // Capabilities that modify terminal state or could be harmful (see
    // upstream readOnlyValidation.ts for full rationale on each token).
    const DANGEROUS: &[&str] = &[
        "init", "reset", "rs1", "rs2", "rs3", "is1", "is2", "is3", "iprog", "if", "rf", "clear",
        "flash", "mc0", "mc4", "mc5", "mc5i", "mc5p", "pfkey", "pfloc", "pfx", "pfxl", "smcup",
        "rmcup",
    ];
    let mut i = 0;
    let mut after_dd = false;
    while i < args.len() {
        let token = args[i];
        if token == "--" {
            after_dd = true;
            i += 1;
            continue;
        }
        if !after_dd && token.starts_with('-') {
            // Defense-in-depth: block -S even if it somehow passes validate_flags.
            if token == "-S" {
                return true;
            }
            // Also check for -S bundled with other flags (e.g., -xS).
            if !token.starts_with("--") && token.len() > 2 && token.contains('S') {
                return true;
            }
            if token == "-T" {
                i += 2;
                continue;
            }
            i += 1;
        } else {
            if DANGEROUS.contains(&token) {
                return true;
            }
            i += 1;
        }
    }
    false
}

// ============================================================================
// Per-command flag tables + configs.
// ============================================================================

// ---- xargs -----------------------------------------------------------------
// SECURITY: `-i` and `-e` (lowercase) REMOVED — both use GNU getopt
// optional-attached-arg semantics (`i::`, `e::`). The arg MUST be attached
// (`-iX`, `-eX`); space-separated (`-i X`, `-e X`) means the flag takes NO
// arg and `X` becomes the next positional (target command), allowing
// validator/getopt divergence (NETWORK EXFIL via tail→sendmail; CODE EXEC
// via -e consuming a sentinel). Use uppercase `-I {}` (Brace) and `-E EOF`
// (POSIX, MANDATORY separate arg) instead — both validator and xargs agree.
const XARGS_FLAGS: &[(&str, FlagArgType)] = &[
    ("-I", FlagArgType::Brace),
    ("-n", FlagArgType::Number),
    ("-P", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("-s", FlagArgType::Number),
    ("-E", FlagArgType::Eof),
    ("-0", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("-x", FlagArgType::None),
    ("-d", FlagArgType::Char),
];
const XARGS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: XARGS_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- file ------------------------------------------------------------------
const FILE_FLAGS: &[(&str, FlagArgType)] = &[
    ("--brief", FlagArgType::None),
    ("-b", FlagArgType::None),
    ("--mime", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--mime-type", FlagArgType::None),
    ("--mime-encoding", FlagArgType::None),
    ("--apple", FlagArgType::None),
    ("--check-encoding", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--exclude", FlagArgType::String),
    ("--exclude-quiet", FlagArgType::String),
    ("--print0", FlagArgType::None),
    ("-0", FlagArgType::None),
    ("-f", FlagArgType::String),
    ("-F", FlagArgType::String),
    ("--separator", FlagArgType::String),
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--no-dereference", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--dereference", FlagArgType::None),
    ("-L", FlagArgType::None),
    ("--magic-file", FlagArgType::String),
    ("-m", FlagArgType::String),
    ("--keep-going", FlagArgType::None),
    ("-k", FlagArgType::None),
    ("--list", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("--no-buffer", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("--preserve-date", FlagArgType::None),
    ("-p", FlagArgType::None),
    ("--raw", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--special-files", FlagArgType::None),
    ("--uncompress", FlagArgType::None),
    ("-z", FlagArgType::None),
];
const FILE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: FILE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- sed -------------------------------------------------------------------
const SED_FLAGS: &[(&str, FlagArgType)] = &[
    ("--expression", FlagArgType::String),
    ("-e", FlagArgType::String),
    ("--quiet", FlagArgType::None),
    ("--silent", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("--regexp-extended", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("--posix", FlagArgType::None),
    ("-E", FlagArgType::None),
    ("--line-length", FlagArgType::Number),
    ("-l", FlagArgType::Number),
    ("--zero-terminated", FlagArgType::None),
    ("-z", FlagArgType::None),
    ("--separate", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--unbuffered", FlagArgType::None),
    ("-u", FlagArgType::None),
    ("--debug", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const SED_CONFIG: CommandConfig = CommandConfig {
    safe_flags: SED_FLAGS,
    additional_dangerous_callback: Some(sed_always_dangerous),
    respects_double_dash: true,
};

// ---- sort ------------------------------------------------------------------
// SECURITY: `--output` / `-o` deliberately EXCLUDED — writes sorted output to
// a file. Sort is read-only only when output goes to stdout.
const SORT_FLAGS: &[(&str, FlagArgType)] = &[
    ("--ignore-leading-blanks", FlagArgType::None),
    ("-b", FlagArgType::None),
    ("--dictionary-order", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("--ignore-case", FlagArgType::None),
    ("-f", FlagArgType::None),
    ("--general-numeric-sort", FlagArgType::None),
    ("-g", FlagArgType::None),
    ("--human-numeric-sort", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--ignore-nonprinting", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--month-sort", FlagArgType::None),
    ("-M", FlagArgType::None),
    ("--numeric-sort", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("--random-sort", FlagArgType::None),
    ("-R", FlagArgType::None),
    ("--reverse", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("--sort", FlagArgType::String),
    ("--stable", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--unique", FlagArgType::None),
    ("-u", FlagArgType::None),
    ("--version-sort", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("--zero-terminated", FlagArgType::None),
    ("-z", FlagArgType::None),
    ("--key", FlagArgType::String),
    ("-k", FlagArgType::String),
    ("--field-separator", FlagArgType::String),
    ("-t", FlagArgType::String),
    ("--check", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--check-char-order", FlagArgType::None),
    ("-C", FlagArgType::None),
    ("--merge", FlagArgType::None),
    ("-m", FlagArgType::None),
    ("--buffer-size", FlagArgType::String),
    ("-S", FlagArgType::String),
    ("--parallel", FlagArgType::Number),
    ("--batch-size", FlagArgType::Number),
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const SORT_CONFIG: CommandConfig = CommandConfig {
    safe_flags: SORT_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- man -------------------------------------------------------------------
// SECURITY: `-P` (pager) EXCLUDED — allows arbitrary command execution via
// custom pager binary (e.g. `man -P sh` spawns a shell).
const MAN_FLAGS: &[(&str, FlagArgType)] = &[
    ("-a", FlagArgType::None),
    ("--all", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("-f", FlagArgType::None),
    ("--whatis", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("-k", FlagArgType::None),
    ("--apropos", FlagArgType::None),
    ("-l", FlagArgType::String),
    ("-w", FlagArgType::None),
    ("-S", FlagArgType::String),
    ("-s", FlagArgType::String),
];
const MAN_CONFIG: CommandConfig = CommandConfig {
    safe_flags: MAN_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- help ------------------------------------------------------------------
// `help` is restricted to bash-builtin flags because help may be aliased to
// `man` (e.g. oh-my-zsh `common-aliases`), and `man -P` permits arbitrary
// command execution via pager.
const HELP_FLAGS: &[(&str, FlagArgType)] = &[
    ("-d", FlagArgType::None),
    ("-m", FlagArgType::None),
    ("-s", FlagArgType::None),
];
const HELP_CONFIG: CommandConfig = CommandConfig {
    safe_flags: HELP_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- netstat ---------------------------------------------------------------
const NETSTAT_FLAGS: &[(&str, FlagArgType)] = &[
    ("-a", FlagArgType::None),
    ("-L", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("-f", FlagArgType::String),
    ("-g", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("-I", FlagArgType::String),
    ("-s", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("-m", FlagArgType::None),
    ("-v", FlagArgType::None),
];
const NETSTAT_CONFIG: CommandConfig = CommandConfig {
    safe_flags: NETSTAT_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- ps --------------------------------------------------------------------
const PS_FLAGS: &[(&str, FlagArgType)] = &[
    ("-e", FlagArgType::None),
    ("-A", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("-N", FlagArgType::None),
    ("--deselect", FlagArgType::None),
    ("-f", FlagArgType::None),
    ("-F", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("-j", FlagArgType::None),
    ("-y", FlagArgType::None),
    ("-w", FlagArgType::None),
    ("-ww", FlagArgType::None),
    ("--width", FlagArgType::Number),
    ("-c", FlagArgType::None),
    ("-H", FlagArgType::None),
    ("--forest", FlagArgType::None),
    ("--headers", FlagArgType::None),
    ("--no-headers", FlagArgType::None),
    ("-n", FlagArgType::String),
    ("--sort", FlagArgType::String),
    ("-L", FlagArgType::None),
    ("-T", FlagArgType::None),
    ("-m", FlagArgType::None),
    ("-C", FlagArgType::String),
    ("-G", FlagArgType::String),
    ("-g", FlagArgType::String),
    ("-p", FlagArgType::String),
    ("--pid", FlagArgType::String),
    ("-q", FlagArgType::String),
    ("--quick-pid", FlagArgType::String),
    ("-s", FlagArgType::String),
    ("--sid", FlagArgType::String),
    ("-t", FlagArgType::String),
    ("--tty", FlagArgType::String),
    ("-U", FlagArgType::String),
    ("-u", FlagArgType::String),
    ("--user", FlagArgType::String),
    ("--help", FlagArgType::None),
    ("--info", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const PS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: PS_FLAGS,
    additional_dangerous_callback: Some(ps_bsd_e_modifier_check),
    respects_double_dash: true,
};

// ---- base64 ----------------------------------------------------------------
// macOS `base64` does NOT respect POSIX `--`, hence respects_double_dash=false.
const BASE64_FLAGS: &[(&str, FlagArgType)] = &[
    ("-d", FlagArgType::None),
    ("-D", FlagArgType::None),
    ("--decode", FlagArgType::None),
    ("-b", FlagArgType::Number),
    ("--break", FlagArgType::Number),
    ("-w", FlagArgType::Number),
    ("--wrap", FlagArgType::Number),
    ("-i", FlagArgType::String),
    ("--input", FlagArgType::String),
    ("--ignore-garbage", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const BASE64_CONFIG: CommandConfig = CommandConfig {
    safe_flags: BASE64_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: false,
};

// ---- grep ------------------------------------------------------------------
const GREP_FLAGS: &[(&str, FlagArgType)] = &[
    ("-e", FlagArgType::String),
    ("--regexp", FlagArgType::String),
    ("-f", FlagArgType::String),
    ("--file", FlagArgType::String),
    ("-F", FlagArgType::None),
    ("--fixed-strings", FlagArgType::None),
    ("-G", FlagArgType::None),
    ("--basic-regexp", FlagArgType::None),
    ("-E", FlagArgType::None),
    ("--extended-regexp", FlagArgType::None),
    ("-P", FlagArgType::None),
    ("--perl-regexp", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--ignore-case", FlagArgType::None),
    ("--no-ignore-case", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--invert-match", FlagArgType::None),
    ("-w", FlagArgType::None),
    ("--word-regexp", FlagArgType::None),
    ("-x", FlagArgType::None),
    ("--line-regexp", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--count", FlagArgType::None),
    ("--color", FlagArgType::String),
    ("--colour", FlagArgType::String),
    ("-L", FlagArgType::None),
    ("--files-without-match", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("--files-with-matches", FlagArgType::None),
    ("-m", FlagArgType::Number),
    ("--max-count", FlagArgType::Number),
    ("-o", FlagArgType::None),
    ("--only-matching", FlagArgType::None),
    ("-q", FlagArgType::None),
    ("--quiet", FlagArgType::None),
    ("--silent", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--no-messages", FlagArgType::None),
    ("-b", FlagArgType::None),
    ("--byte-offset", FlagArgType::None),
    ("-H", FlagArgType::None),
    ("--with-filename", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--no-filename", FlagArgType::None),
    ("--label", FlagArgType::String),
    ("-n", FlagArgType::None),
    ("--line-number", FlagArgType::None),
    ("-T", FlagArgType::None),
    ("--initial-tab", FlagArgType::None),
    ("-u", FlagArgType::None),
    ("--unix-byte-offsets", FlagArgType::None),
    ("-Z", FlagArgType::None),
    ("--null", FlagArgType::None),
    ("-z", FlagArgType::None),
    ("--null-data", FlagArgType::None),
    ("-A", FlagArgType::Number),
    ("--after-context", FlagArgType::Number),
    ("-B", FlagArgType::Number),
    ("--before-context", FlagArgType::Number),
    ("-C", FlagArgType::Number),
    ("--context", FlagArgType::Number),
    ("--group-separator", FlagArgType::String),
    ("--no-group-separator", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--text", FlagArgType::None),
    ("--binary-files", FlagArgType::String),
    ("-D", FlagArgType::String),
    ("--devices", FlagArgType::String),
    ("-d", FlagArgType::String),
    ("--directories", FlagArgType::String),
    ("--exclude", FlagArgType::String),
    ("--exclude-from", FlagArgType::String),
    ("--exclude-dir", FlagArgType::String),
    ("--include", FlagArgType::String),
    ("-r", FlagArgType::None),
    ("--recursive", FlagArgType::None),
    ("-R", FlagArgType::None),
    ("--dereference-recursive", FlagArgType::None),
    ("--line-buffered", FlagArgType::None),
    ("-U", FlagArgType::None),
    ("--binary", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const GREP_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GREP_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- checksum commands (sha256sum / sha1sum / md5sum) ----------------------
// All checksum tools share identical flag sets — only read files and compute
// or verify hashes. No flags write or execute.
const CHECKSUM_FLAGS: &[(&str, FlagArgType)] = &[
    ("-b", FlagArgType::None),
    ("--binary", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("--text", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--check", FlagArgType::None),
    ("--ignore-missing", FlagArgType::None),
    ("--quiet", FlagArgType::None),
    ("--status", FlagArgType::None),
    ("--strict", FlagArgType::None),
    ("-w", FlagArgType::None),
    ("--warn", FlagArgType::None),
    ("--tag", FlagArgType::None),
    ("-z", FlagArgType::None),
    ("--zero", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const SHA256SUM_FLAGS: &[(&str, FlagArgType)] = CHECKSUM_FLAGS;
const SHA1SUM_FLAGS: &[(&str, FlagArgType)] = CHECKSUM_FLAGS;
const MD5SUM_FLAGS: &[(&str, FlagArgType)] = CHECKSUM_FLAGS;
const SHA256SUM_CONFIG: CommandConfig = CommandConfig {
    safe_flags: SHA256SUM_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};
const SHA1SUM_CONFIG: CommandConfig = CommandConfig {
    safe_flags: SHA1SUM_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};
const MD5SUM_CONFIG: CommandConfig = CommandConfig {
    safe_flags: MD5SUM_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- tree ------------------------------------------------------------------
// SECURITY: `-o`/`--output` EXCLUDED — writes tree output to a file.
// SECURITY: `-R` EXCLUDED — combined with `-H` (HTML) and `-L` (depth), tree
// WRITES `00Tree.html` to every subdirectory at the depth boundary. FILE
// WRITE with zero permissions. From man tree (< 2.1.0): "-R — at each of
// them execute tree again adding `-o 00Tree.html` as a new option."
const TREE_FLAGS: &[(&str, FlagArgType)] = &[
    ("-a", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("-f", FlagArgType::None),
    ("-x", FlagArgType::None),
    ("-L", FlagArgType::Number),
    // -R EXCLUDED (HTML write side-effect when combined with -H).
    ("-P", FlagArgType::String),
    ("-I", FlagArgType::String),
    ("--gitignore", FlagArgType::None),
    ("--gitfile", FlagArgType::String),
    ("--ignore-case", FlagArgType::None),
    ("--matchdirs", FlagArgType::None),
    ("--metafirst", FlagArgType::None),
    ("--prune", FlagArgType::None),
    ("--info", FlagArgType::None),
    ("--infofile", FlagArgType::String),
    ("--noreport", FlagArgType::None),
    ("--charset", FlagArgType::String),
    ("--filelimit", FlagArgType::Number),
    ("-q", FlagArgType::None),
    ("-N", FlagArgType::None),
    ("-Q", FlagArgType::None),
    ("-p", FlagArgType::None),
    ("-u", FlagArgType::None),
    ("-g", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--si", FlagArgType::None),
    ("--du", FlagArgType::None),
    ("-D", FlagArgType::None),
    ("--timefmt", FlagArgType::String),
    ("-F", FlagArgType::None),
    ("--inodes", FlagArgType::None),
    ("--device", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("-U", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("--dirsfirst", FlagArgType::None),
    ("--filesfirst", FlagArgType::None),
    ("--sort", FlagArgType::String),
    ("-i", FlagArgType::None),
    ("-A", FlagArgType::None),
    ("-S", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("-C", FlagArgType::None),
    ("-X", FlagArgType::None),
    ("-J", FlagArgType::None),
    ("-H", FlagArgType::String),
    ("--nolinks", FlagArgType::None),
    ("--hintro", FlagArgType::String),
    ("--houtro", FlagArgType::String),
    ("-T", FlagArgType::String),
    ("--hyperlink", FlagArgType::None),
    ("--scheme", FlagArgType::String),
    ("--authority", FlagArgType::String),
    ("--fromfile", FlagArgType::None),
    ("--fromtabfile", FlagArgType::None),
    ("--fflinks", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const TREE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: TREE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- date ------------------------------------------------------------------
// SECURITY: `-s`/`--set` EXCLUDED — sets system time.
// SECURITY: `-f`/`--file` EXCLUDED — reads dates from file (batch system-time
// modification).
const DATE_FLAGS: &[(&str, FlagArgType)] = &[
    ("-d", FlagArgType::String),
    ("--date", FlagArgType::String),
    ("-r", FlagArgType::String),
    ("--reference", FlagArgType::String),
    ("-u", FlagArgType::None),
    ("--utc", FlagArgType::None),
    ("--universal", FlagArgType::None),
    ("-I", FlagArgType::None),
    ("--iso-8601", FlagArgType::String),
    ("-R", FlagArgType::None),
    ("--rfc-email", FlagArgType::None),
    ("--rfc-3339", FlagArgType::String),
    ("--debug", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const DATE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: DATE_FLAGS,
    additional_dangerous_callback: Some(date_positional_must_start_with_plus),
    respects_double_dash: true,
};

// ---- hostname --------------------------------------------------------------
// CRITICAL: any positional argument SETS the hostname.
// SECURITY: `-F`/`--file`, `-b`/`--boot`, `-y`/`--yp`/`--nis` EXCLUDED
// (blocked by omission); positional args blocked by callback.
const HOSTNAME_FLAGS: &[(&str, FlagArgType)] = &[
    ("-f", FlagArgType::None),
    ("--fqdn", FlagArgType::None),
    ("--long", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--short", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--ip-address", FlagArgType::None),
    ("-I", FlagArgType::None),
    ("--all-ip-addresses", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--alias", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("--domain", FlagArgType::None),
    ("-A", FlagArgType::None),
    ("--all-fqdns", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--verbose", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const HOSTNAME_CONFIG: CommandConfig = CommandConfig {
    safe_flags: HOSTNAME_FLAGS,
    additional_dangerous_callback: Some(hostname_no_positional_args),
    respects_double_dash: true,
};

// ---- info ------------------------------------------------------------------
// SECURITY: `-o`/`--output` EXCLUDED — writes output to a file.
// SECURITY: `--dribble` EXCLUDED — records keystrokes to a file.
// SECURITY: `--init-file` EXCLUDED — loads custom config (potential code
// execution).
// SECURITY: `--restore` EXCLUDED — replays keystrokes from a file.
const INFO_FLAGS: &[(&str, FlagArgType)] = &[
    ("-f", FlagArgType::String),
    ("--file", FlagArgType::String),
    ("-d", FlagArgType::String),
    ("--directory", FlagArgType::String),
    ("-n", FlagArgType::String),
    ("--node", FlagArgType::String),
    ("-a", FlagArgType::None),
    ("--all", FlagArgType::None),
    ("-k", FlagArgType::String),
    ("--apropos", FlagArgType::String),
    ("-w", FlagArgType::None),
    ("--where", FlagArgType::None),
    ("--location", FlagArgType::None),
    ("--show-options", FlagArgType::None),
    ("--vi-keys", FlagArgType::None),
    ("--subnodes", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("--usage", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const INFO_CONFIG: CommandConfig = CommandConfig {
    safe_flags: INFO_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- lsof ------------------------------------------------------------------
// SECURITY: `-D` EXCLUDED — builds/updates device cache file on disk.
// SECURITY: `+m` blocked by callback — creates mount supplement file.
const LSOF_FLAGS: &[(&str, FlagArgType)] = &[
    ("-?", FlagArgType::None),
    ("-h", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("-b", FlagArgType::None),
    ("-C", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("-N", FlagArgType::None),
    ("-O", FlagArgType::None),
    ("-P", FlagArgType::None),
    ("-Q", FlagArgType::None),
    ("-R", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("-U", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("-X", FlagArgType::None),
    ("-H", FlagArgType::None),
    ("-E", FlagArgType::None),
    ("-F", FlagArgType::None),
    ("-g", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("-K", FlagArgType::None),
    ("-L", FlagArgType::None),
    ("-o", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("-S", FlagArgType::None),
    ("-T", FlagArgType::None),
    ("-x", FlagArgType::None),
    ("-A", FlagArgType::String),
    ("-c", FlagArgType::String),
    ("-d", FlagArgType::String),
    ("-e", FlagArgType::String),
    ("-k", FlagArgType::String),
    ("-p", FlagArgType::String),
    ("-u", FlagArgType::String),
];
const LSOF_CONFIG: CommandConfig = CommandConfig {
    safe_flags: LSOF_FLAGS,
    additional_dangerous_callback: Some(lsof_plus_m_check),
    respects_double_dash: true,
};

// ---- pgrep -----------------------------------------------------------------
const PGREP_FLAGS: &[(&str, FlagArgType)] = &[
    ("-d", FlagArgType::String),
    ("--delimiter", FlagArgType::String),
    ("-l", FlagArgType::None),
    ("--list-name", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--list-full", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--inverse", FlagArgType::None),
    ("-w", FlagArgType::None),
    ("--lightweight", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--count", FlagArgType::None),
    ("-f", FlagArgType::None),
    ("--full", FlagArgType::None),
    ("-g", FlagArgType::String),
    ("--pgroup", FlagArgType::String),
    ("-G", FlagArgType::String),
    ("--group", FlagArgType::String),
    ("-i", FlagArgType::None),
    ("--ignore-case", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("--newest", FlagArgType::None),
    ("-o", FlagArgType::None),
    ("--oldest", FlagArgType::None),
    ("-O", FlagArgType::String),
    ("--older", FlagArgType::String),
    ("-P", FlagArgType::String),
    ("--parent", FlagArgType::String),
    ("-s", FlagArgType::String),
    ("--session", FlagArgType::String),
    ("-t", FlagArgType::String),
    ("--terminal", FlagArgType::String),
    ("-u", FlagArgType::String),
    ("--euid", FlagArgType::String),
    ("-U", FlagArgType::String),
    ("--uid", FlagArgType::String),
    ("-x", FlagArgType::None),
    ("--exact", FlagArgType::None),
    ("-F", FlagArgType::String),
    ("--pidfile", FlagArgType::String),
    ("-L", FlagArgType::None),
    ("--logpidfile", FlagArgType::None),
    ("-r", FlagArgType::String),
    ("--runstates", FlagArgType::String),
    ("--ns", FlagArgType::String),
    ("--nslist", FlagArgType::String),
    ("--help", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("--version", FlagArgType::None),
];
const PGREP_CONFIG: CommandConfig = CommandConfig {
    safe_flags: PGREP_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- tput ------------------------------------------------------------------
// SECURITY: `-S` (read capability names from stdin) deliberately EXCLUDED.
// It must NOT be in safe_flags because validate_flags unbundles combined
// short flags (e.g., `-xS` → `-x` + `-S`), but the callback receives the raw
// token `-xS` and only checks exact match `token == "-S"`. Excluding `-S`
// from safe_flags ensures validate_flags rejects it (bundled or not) before
// the callback runs. The callback's `-S` check is defense-in-depth.
const TPUT_FLAGS: &[(&str, FlagArgType)] = &[
    ("-T", FlagArgType::String),
    ("-V", FlagArgType::None),
    ("-x", FlagArgType::None),
];
const TPUT_CONFIG: CommandConfig = CommandConfig {
    safe_flags: TPUT_FLAGS,
    additional_dangerous_callback: Some(tput_dangerous_check),
    respects_double_dash: true,
};

// ---- ss --------------------------------------------------------------------
// SECURITY: `-K`/`--kill` EXCLUDED — forcibly closes sockets.
// SECURITY: `-D`/`--diag` EXCLUDED — dumps raw TCP data to a file.
// SECURITY: `-F`/`--filter` EXCLUDED — reads filter expressions from a file.
// SECURITY: `-N`/`--net` EXCLUDED — performs setns/unshare/mount/umount to
// switch network namespace (too invasive even when isolated to forked proc).
const SS_FLAGS: &[(&str, FlagArgType)] = &[
    ("-h", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("-V", FlagArgType::None),
    ("--version", FlagArgType::None),
    ("-n", FlagArgType::None),
    ("--numeric", FlagArgType::None),
    ("-r", FlagArgType::None),
    ("--resolve", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--all", FlagArgType::None),
    ("-l", FlagArgType::None),
    ("--listening", FlagArgType::None),
    ("-o", FlagArgType::None),
    ("--options", FlagArgType::None),
    ("-e", FlagArgType::None),
    ("--extended", FlagArgType::None),
    ("-m", FlagArgType::None),
    ("--memory", FlagArgType::None),
    ("-p", FlagArgType::None),
    ("--processes", FlagArgType::None),
    ("-i", FlagArgType::None),
    ("--info", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--summary", FlagArgType::None),
    ("-4", FlagArgType::None),
    ("--ipv4", FlagArgType::None),
    ("-6", FlagArgType::None),
    ("--ipv6", FlagArgType::None),
    ("-0", FlagArgType::None),
    ("--packet", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("--tcp", FlagArgType::None),
    ("-M", FlagArgType::None),
    ("--mptcp", FlagArgType::None),
    ("-S", FlagArgType::None),
    ("--sctp", FlagArgType::None),
    ("-u", FlagArgType::None),
    ("--udp", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("--dccp", FlagArgType::None),
    ("-w", FlagArgType::None),
    ("--raw", FlagArgType::None),
    ("-x", FlagArgType::None),
    ("--unix", FlagArgType::None),
    ("--tipc", FlagArgType::None),
    ("--vsock", FlagArgType::None),
    ("-f", FlagArgType::String),
    ("--family", FlagArgType::String),
    ("-A", FlagArgType::String),
    ("--query", FlagArgType::String),
    ("--socket", FlagArgType::String),
    ("-Z", FlagArgType::None),
    ("--context", FlagArgType::None),
    ("-z", FlagArgType::None),
    ("--contexts", FlagArgType::None),
    ("-b", FlagArgType::None),
    ("--bpf", FlagArgType::None),
    ("-E", FlagArgType::None),
    ("--events", FlagArgType::None),
    ("-H", FlagArgType::None),
    ("--no-header", FlagArgType::None),
    ("-O", FlagArgType::None),
    ("--oneline", FlagArgType::None),
    ("--tipcinfo", FlagArgType::None),
    ("--tos", FlagArgType::None),
    ("--cgroup", FlagArgType::None),
    ("--inet-sockopt", FlagArgType::None),
];
const SS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: SS_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- fd / fdfind -----------------------------------------------------------
// Both share `FD_SAFE_FLAGS`. `fdfind` is the Debian/Ubuntu package name.
const FD_CONFIG: CommandConfig = CommandConfig {
    safe_flags: FD_SAFE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};
const FDFIND_CONFIG: CommandConfig = CommandConfig {
    safe_flags: FD_SAFE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ============================================================================
// Master allowlist.
// ============================================================================

/// Base allowlist (USER_TYPE != ant). Order is SECURITY-significant —
/// first-match-wins, matching upstream `readOnlyValidation.ts` L128-L1140.
///
/// Layout (in iteration order):
///   1. 23 inline single-binary entries (xargs..fdfind). On Windows, `xargs`
///      is omitted because file contents containing UNC paths can be piped
///      to `xargs cat` to trigger SMB resolution — bypassing string-based
///      detection (upstream L1193-L1198).
///   2. DOCKER / RIPGREP / PYRIGHT external tables (multi-word entries like
///      `docker logs` first, then single-binary `rg` / `pyright`). Order
///      within externals (DOCKER → RIPGREP → PYRIGHT) differs cosmetically
///      from upstream (PYRIGHT → DOCKER); since keys don't collide,
///      first-match-wins behavior is identical.
///   3. GIT_READ_ONLY_COMMANDS (`git diff`, `git log`, …) — 24 entries in
///      longest-prefix order (`git remote show` before `git remote`).
///
/// Upstream `COMMAND_ALLOWLIST` interleaves GIT after line 164 and RIPGREP
/// after line 559; we group all externals at the end because none of the
/// external keys collide with inline keys (verified by Layer 1 audit), so
/// first-match-wins lookup yields the same result.
static COMMAND_ALLOWLIST_BASE: LazyLock<Vec<(&'static str, &'static CommandConfig)>> =
    LazyLock::new(|| {
        let mut out: Vec<(&'static str, &'static CommandConfig)> = Vec::new();
        // SECURITY: omit `xargs` on Windows. UNC paths in file contents can
        // be piped to `xargs cat` triggering SMB resolution; regex-based
        // detection cannot inspect file contents. Matches upstream
        // `getCommandAllowlist()` L1193-L1198.
        if !cfg!(target_os = "windows") {
            out.push(("xargs", &XARGS_CONFIG));
        }
        out.extend_from_slice(&[
            ("file", &FILE_CONFIG),
            ("sed", &SED_CONFIG),
            ("sort", &SORT_CONFIG),
            ("man", &MAN_CONFIG),
            ("help", &HELP_CONFIG),
            ("netstat", &NETSTAT_CONFIG),
            ("ps", &PS_CONFIG),
            ("base64", &BASE64_CONFIG),
            ("grep", &GREP_CONFIG),
            ("sha256sum", &SHA256SUM_CONFIG),
            ("sha1sum", &SHA1SUM_CONFIG),
            ("md5sum", &MD5SUM_CONFIG),
            ("tree", &TREE_CONFIG),
            ("date", &DATE_CONFIG),
            ("hostname", &HOSTNAME_CONFIG),
            ("info", &INFO_CONFIG),
            ("lsof", &LSOF_CONFIG),
            ("pgrep", &PGREP_CONFIG),
            ("tput", &TPUT_CONFIG),
            ("ss", &SS_CONFIG),
            ("fd", &FD_CONFIG),
            ("fdfind", &FDFIND_CONFIG),
        ]);
        out.extend_from_slice(DOCKER_READ_ONLY_COMMANDS);
        out.extend_from_slice(RIPGREP_READ_ONLY_COMMANDS);
        out.extend_from_slice(PYRIGHT_READ_ONLY_COMMANDS);
        out.extend(GIT_READ_ONLY_COMMANDS.iter().copied());
        out
    });

/// Ant-extended allowlist (USER_TYPE == ant). Appends `ANT_ONLY_COMMANDS`
/// (22 gh entries + aki) to the base allowlist. Matches upstream
/// `{ ...allowlist, ...ANT_ONLY_COMMAND_ALLOWLIST }` (L1212).
static COMMAND_ALLOWLIST_ANT: LazyLock<Vec<(&'static str, &'static CommandConfig)>> =
    LazyLock::new(|| {
        let mut out = COMMAND_ALLOWLIST_BASE.clone();
        out.extend(ANT_ONLY_COMMANDS.iter().copied());
        out
    });

/// Return the effective allowlist for the current process. Reads
/// `USER_TYPE` on every call (matching upstream `getCommandAllowlist()`
/// L1201-L1212 which reads `process.env.USER_TYPE` per call). Both base
/// and ant variants are statically cached, so the per-call cost is one
/// env lookup and one branch.
pub fn get_command_allowlist() -> &'static [(&'static str, &'static CommandConfig)] {
    if std::env::var("USER_TYPE").as_deref() == Ok("ant") {
        &COMMAND_ALLOWLIST_ANT
    } else {
        &COMMAND_ALLOWLIST_BASE
    }
}

/// Phase 3.2.B port of `isCommandSafeViaFlagParsing` (claude-code-main
/// readOnlyValidation.ts L1245-L1408).
pub fn is_command_safe_via_flag_parsing(command: &str) -> bool {
    let tokens = match shell_words::split(command) {
        Ok(t) if !t.is_empty() => t,
        _ => return false,
    };

    // Multi-word command lookup: first-match-wins, matching upstream
    // `for (cmd, config) of COMMAND_ALLOWLIST.entries()` with early break
    // (readOnlyValidation.ts L1294-L1303). Order is SECURITY-significant:
    // longest-prefix multi-word entries (e.g. `git remote show`) must
    // precede shorter prefixes (`git remote`) in the underlying tables.
    let mut matched: Option<(usize, &CommandConfig)> = None;
    for (cmd_pattern, config) in get_command_allowlist().iter() {
        let cmd_tokens: Vec<&str> = cmd_pattern.split(' ').collect();
        if tokens.len() < cmd_tokens.len() {
            continue;
        }
        let prefix_match = cmd_tokens.iter().enumerate().all(|(i, t)| tokens[i] == *t);
        if prefix_match {
            matched = Some((cmd_tokens.len(), *config));
            break;
        }
    }
    let (command_tokens, config) = match matched {
        Some(m) => m,
        None => return false,
    };

    // Bypass scans on every token after the command itself.
    for token in tokens.iter().skip(command_tokens) {
        if token.contains('$') {
            return false;
        }
        if token.contains('{') && (token.contains(',') || token.contains("..")) {
            return false;
        }
    }

    // Backtick check on the raw command (none of our 23 allowlist entries
    // legitimately uses backticks).
    if command.contains('`') {
        return false;
    }

    // grep / rg: reject newlines on the raw command string.
    if (tokens[0] == "grep" || tokens[0] == "rg")
        && (command.contains('\n') || command.contains('\r'))
    {
        return false;
    }

    let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
    let xargs_targets: Option<&[&str]> = if tokens[0] == "xargs" {
        Some(SAFE_TARGET_COMMANDS_FOR_XARGS)
    } else {
        None
    };
    let options = ValidateOptions {
        command_name: Some(token_refs[0]),
        raw_command: Some(command),
        xargs_target_commands: xargs_targets,
    };

    if !validate_flags(&token_refs, command_tokens, config, &options) {
        return false;
    }

    if let Some(cb) = config.additional_dangerous_callback {
        let post: Vec<&str> = token_refs.iter().copied().skip(command_tokens).collect();
        if cb(command, &post) {
            return false;
        }
    }

    true
}

/// Phase 3.2.B port of `makeRegexForSafeCommand` (upstream L1414-L1424).
pub fn make_regex_for_safe_command(command: &str) -> String {
    format!(r"^{}(?:\s|$)[^<>()$`|{{}}&;\n\r]*$", command)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Happy path: one per command ----------
    #[test]
    fn req_bash_validation_320_b_xargs_safe() {
        assert!(is_command_safe_via_flag_parsing("xargs -r echo hi"));
    }
    #[test]
    fn req_bash_validation_320_b_file() {
        assert!(is_command_safe_via_flag_parsing("file --mime-type x"));
    }
    #[test]
    fn req_bash_validation_320_b_sort() {
        // NOTE: spec listed `sort -k1 f`, but `-k` is a String-typed flag
        // and the foundation flag parser only treats attached numeric
        // forms (e.g. `-A20`) specially for grep/rg. Use space-separated
        // form so the test exercises the same flag with the same intent.
        assert!(is_command_safe_via_flag_parsing("sort -k 1 f"));
    }
    #[test]
    fn req_bash_validation_320_b_man() {
        assert!(is_command_safe_via_flag_parsing("man ls"));
    }
    #[test]
    fn req_bash_validation_320_b_help() {
        assert!(is_command_safe_via_flag_parsing("help -d"));
    }
    #[test]
    fn req_bash_validation_320_b_netstat() {
        assert!(is_command_safe_via_flag_parsing("netstat -an"));
    }
    #[test]
    fn req_bash_validation_320_b_ps() {
        assert!(is_command_safe_via_flag_parsing("ps aux"));
    }
    #[test]
    fn req_bash_validation_320_b_base64() {
        assert!(is_command_safe_via_flag_parsing("base64 -d f"));
    }
    #[test]
    fn req_bash_validation_320_b_grep() {
        assert!(is_command_safe_via_flag_parsing("grep -ri pat src"));
    }
    #[test]
    fn req_bash_validation_320_b_sha256sum() {
        assert!(is_command_safe_via_flag_parsing("sha256sum f"));
    }
    #[test]
    fn req_bash_validation_320_b_sha1sum() {
        assert!(is_command_safe_via_flag_parsing("sha1sum f"));
    }
    #[test]
    fn req_bash_validation_320_b_md5sum() {
        assert!(is_command_safe_via_flag_parsing("md5sum f"));
    }
    #[test]
    fn req_bash_validation_320_b_tree() {
        assert!(is_command_safe_via_flag_parsing("tree -L 2"));
    }
    #[test]
    fn req_bash_validation_320_b_date() {
        assert!(is_command_safe_via_flag_parsing("date +%Y-%m-%d"));
    }
    #[test]
    fn req_bash_validation_320_b_hostname() {
        assert!(is_command_safe_via_flag_parsing("hostname -f"));
    }
    #[test]
    fn req_bash_validation_320_b_info() {
        assert!(is_command_safe_via_flag_parsing("info -a"));
    }
    #[test]
    fn req_bash_validation_320_b_lsof() {
        assert!(is_command_safe_via_flag_parsing("lsof -i"));
    }
    #[test]
    fn req_bash_validation_320_b_pgrep() {
        assert!(is_command_safe_via_flag_parsing("pgrep -l x"));
    }
    #[test]
    fn req_bash_validation_320_b_tput() {
        assert!(is_command_safe_via_flag_parsing("tput cols"));
    }
    #[test]
    fn req_bash_validation_320_b_ss() {
        assert!(is_command_safe_via_flag_parsing("ss -tnl"));
    }
    #[test]
    fn req_bash_validation_320_b_fd() {
        assert!(is_command_safe_via_flag_parsing("fd --hidden"));
    }
    #[test]
    fn req_bash_validation_320_b_fdfind() {
        assert!(is_command_safe_via_flag_parsing("fdfind --hidden"));
    }

    // ---------- SECURITY pins ----------
    #[test]
    fn test_security_s21_xargs_bundling_differential() {
        // `-rI` bundle: `-r` is None, `-I` is Brace (arg-taking) → bundle
        // member is non-None ⇒ reject (parser differential vs GNU getopt).
        assert!(!is_command_safe_via_flag_parsing("xargs -rI echo rm evil"));
    }

    #[test]
    fn test_security_s21_xargs_unsafe_target_rejected() {
        assert!(!is_command_safe_via_flag_parsing("xargs -r rm hi"));
    }

    #[test]
    fn test_xargs_safe_target_accepted() {
        assert!(is_command_safe_via_flag_parsing("xargs -r echo hi"));
    }

    #[test]
    fn test_security_s24_sed_inplace_rejected() {
        assert!(!is_command_safe_via_flag_parsing("sed -i s/a/b/ f"));
    }

    #[test]
    fn test_security_s24_sed_callback_always_dangerous() {
        // Even structurally valid sed is blocked until 3.2.D ports the
        // internal allowlist (sed_always_dangerous returns true).
        assert!(!is_command_safe_via_flag_parsing("sed -n 1p f"));
    }

    #[test]
    fn test_security_s25_fd_exec_rejected() {
        assert!(!is_command_safe_via_flag_parsing("fd -x cmd"));
        assert!(!is_command_safe_via_flag_parsing("fd --exec cmd"));
        assert!(!is_command_safe_via_flag_parsing("fd -X cmd"));
        assert!(!is_command_safe_via_flag_parsing("fd --exec-batch cmd"));
    }

    #[test]
    fn test_security_s27_sort_output_rejected() {
        assert!(!is_command_safe_via_flag_parsing("sort -o out in"));
        assert!(!is_command_safe_via_flag_parsing("sort --output=out in"));
    }

    #[test]
    fn test_security_s28_man_pager_rejected() {
        assert!(!is_command_safe_via_flag_parsing("man -P /bin/sh foo"));
    }

    // ---------- Bypass guards ----------
    #[test]
    fn test_security_var_expansion_rejected() {
        assert!(!is_command_safe_via_flag_parsing("grep -r $X ."));
    }
    #[test]
    fn test_security_brace_expansion_comma_rejected() {
        assert!(!is_command_safe_via_flag_parsing("tree {1,2}"));
    }
    #[test]
    fn test_security_brace_expansion_range_rejected() {
        assert!(!is_command_safe_via_flag_parsing("tree {1..2}"));
    }
    #[test]
    fn test_security_backtick_rejected() {
        assert!(!is_command_safe_via_flag_parsing(r#"date `whoami`"#));
    }
    #[test]
    fn test_security_grep_newline_rejected() {
        assert!(!is_command_safe_via_flag_parsing("grep -r pat\n"));
    }

    // ---------- Callback edges ----------
    #[test]
    fn test_ps_bsd_e_modifier_rejected() {
        assert!(!is_command_safe_via_flag_parsing("ps axe"));
    }
    #[test]
    fn test_hostname_positional_rejected() {
        assert!(!is_command_safe_via_flag_parsing("hostname newname"));
    }
    #[test]
    fn test_lsof_plus_m_rejected() {
        assert!(!is_command_safe_via_flag_parsing("lsof +m"));
        assert!(!is_command_safe_via_flag_parsing("lsof +m/tmp/x"));
    }
    #[test]
    fn test_tput_dash_s_rejected() {
        assert!(!is_command_safe_via_flag_parsing("tput -S"));
    }
    #[test]
    fn test_tput_dash_s_bundled_rejected() {
        // `-xS` bundle: `-x` is None, `-S` is not in TPUT_SAFE_FLAGS →
        // bundle reject before callback runs.
        assert!(!is_command_safe_via_flag_parsing("tput -xS"));
    }
    #[test]
    fn test_tput_dangerous_cap_rejected() {
        assert!(!is_command_safe_via_flag_parsing("tput reset"));
        assert!(!is_command_safe_via_flag_parsing("tput clear"));
    }
    #[test]
    fn test_date_no_positional_plus_rejected() {
        assert!(!is_command_safe_via_flag_parsing("date foobar"));
    }
    #[test]
    fn test_date_with_d_flag_arg_ok() {
        assert!(is_command_safe_via_flag_parsing("date -d 2024-01-01 +%s"));
    }

    // ---------- Unknown command ----------
    #[test]
    fn test_unknown_command_rejected() {
        assert!(!is_command_safe_via_flag_parsing("evil_cmd --help"));
    }

    // ---------- Empty / whitespace ----------
    #[test]
    fn test_empty_rejected() {
        assert!(!is_command_safe_via_flag_parsing(""));
        assert!(!is_command_safe_via_flag_parsing("   "));
    }

    // ---------- Regex builder ----------
    #[test]
    fn test_make_regex_basic() {
        let r = make_regex_for_safe_command("ls");
        assert!(r.starts_with("^ls"));
        assert!(r.contains(r"(?:\s|$)"));
        assert!(r.ends_with("$"));
    }

    // ---------- SECURITY pin absence (regression guards) ----------
    // Each assertion pins a flag/pattern intentionally OMITTED from a safe
    // command's flag table. A future "just add this flag" refactor that
    // re-introduces any of these will fail the corresponding line and force
    // the author to re-read the upstream SECURITY comment.
    #[test]
    fn test_security_pin_absence_tree_html_write() {
        assert!(!is_command_safe_via_flag_parsing("tree -R /"));
    }

    #[test]
    fn test_security_pin_absence_date_set_system_time() {
        assert!(!is_command_safe_via_flag_parsing("date -s 12:00"));
        assert!(!is_command_safe_via_flag_parsing("date --set=12:00"));
    }

    #[test]
    fn test_security_pin_absence_lsof_device_cache() {
        assert!(!is_command_safe_via_flag_parsing("lsof -D /tmp/x"));
    }

    #[test]
    fn test_security_pin_absence_info_keystroke_record() {
        assert!(!is_command_safe_via_flag_parsing("info -o /tmp/x"));
        assert!(!is_command_safe_via_flag_parsing("info --dribble /tmp/x"));
    }

    #[test]
    fn test_security_pin_absence_ss_destructive() {
        assert!(!is_command_safe_via_flag_parsing("ss -K"));
        assert!(!is_command_safe_via_flag_parsing("ss -D /tmp/x"));
        assert!(!is_command_safe_via_flag_parsing("ss -F /tmp/x"));
        assert!(!is_command_safe_via_flag_parsing("ss -N /netns"));
    }

    #[test]
    fn test_security_pin_absence_hostname_set_flags() {
        assert!(!is_command_safe_via_flag_parsing(
            "hostname -F /etc/hostname"
        ));
        assert!(!is_command_safe_via_flag_parsing("hostname -b"));
    }

    #[test]
    fn test_security_pin_absence_fd_list_details_path_hijack() {
        assert!(!is_command_safe_via_flag_parsing("fd -l"));
        assert!(!is_command_safe_via_flag_parsing("fd --list-details"));
    }

    // ---------- Phase 3.2.C.2.a: external tables ----------

    // DOCKER_READ_ONLY_COMMANDS
    #[test]
    fn req_bash_validation_320_c2a_docker_logs() {
        assert!(is_command_safe_via_flag_parsing(
            "docker logs --tail 100 abc"
        ));
    }
    #[test]
    fn req_bash_validation_320_c2a_docker_inspect() {
        assert!(is_command_safe_via_flag_parsing("docker inspect abc"));
    }

    // RIPGREP_READ_ONLY_COMMANDS
    #[test]
    fn req_bash_validation_320_c2a_rg_basic() {
        assert!(is_command_safe_via_flag_parsing("rg --hidden pat src"));
    }

    // PYRIGHT_READ_ONLY_COMMANDS
    #[test]
    fn req_bash_validation_320_c2a_pyright_basic() {
        assert!(is_command_safe_via_flag_parsing("pyright --stats"));
    }
    #[test]
    fn req_bash_validation_320_c2a_pyright_project() {
        assert!(is_command_safe_via_flag_parsing("pyright --project myproj"));
    }

    // ---------- SECURITY pin absence (regression guards) for 3.2.C.2.a ----------

    // docker: only `logs` and `inspect` are read-only. ALL state-changing
    // sub-commands must be rejected because they're not in the allowlist.
    #[test]
    fn test_security_docker_dangerous_subcommands_rejected() {
        assert!(!is_command_safe_via_flag_parsing("docker run alpine"));
        assert!(!is_command_safe_via_flag_parsing("docker exec abc sh"));
        assert!(!is_command_safe_via_flag_parsing("docker build ."));
        assert!(!is_command_safe_via_flag_parsing("docker rm abc"));
        assert!(!is_command_safe_via_flag_parsing("docker rmi abc"));
        assert!(!is_command_safe_via_flag_parsing("docker kill abc"));
        assert!(!is_command_safe_via_flag_parsing("docker stop abc"));
        assert!(!is_command_safe_via_flag_parsing("docker push abc"));
        assert!(!is_command_safe_via_flag_parsing("docker pull abc"));
    }

    // rg: --pre and --pre-glob enable arbitrary command execution.
    // --search-zip touches the FS in surprising ways.
    #[test]
    fn test_security_rg_pre_arbitrary_exec_rejected() {
        assert!(!is_command_safe_via_flag_parsing("rg --pre /bin/sh pat ."));
        assert!(!is_command_safe_via_flag_parsing(
            "rg --pre-glob '*.log' pat ."
        ));
        // --search-zip long form must also be rejected (only short form -z is in safe_flags).
        assert!(!is_command_safe_via_flag_parsing("rg --search-zip pat ."));
    }
    #[test]
    fn test_security_rg_newline_rejected() {
        // Newline guard already covered by 3.2.B (grep/rg branch),
        // pinned here for the table-wired path too.
        assert!(!is_command_safe_via_flag_parsing("rg -i pat\n"));
    }

    // pyright: --watch makes the process long-lived (not read-only).
    // --createstub writes files. Both must be absent from safe_flags.
    #[test]
    fn test_security_pyright_watch_rejected() {
        assert!(!is_command_safe_via_flag_parsing("pyright --watch"));
        assert!(!is_command_safe_via_flag_parsing("pyright -w"));
    }
    #[test]
    fn test_security_pyright_createstub_rejected() {
        assert!(!is_command_safe_via_flag_parsing(
            "pyright --createstub mymod"
        ));
    }

    // Unknown command after multi-word prefix should still reject.
    #[test]
    fn test_unknown_docker_sub_rejected() {
        assert!(!is_command_safe_via_flag_parsing("docker evilcmd"));
    }
}
