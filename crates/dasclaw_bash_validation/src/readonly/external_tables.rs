//! Phase 3.2.C.2.a — verbatim port of `RIPGREP_READ_ONLY_COMMANDS`,
//! `DOCKER_READ_ONLY_COMMANDS`, `PYRIGHT_READ_ONLY_COMMANDS` from
//! `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts`
//! L1386–L1538.
//!
//! GIT and GH tables ship in 3.2.C.1 / 3.2.C.2.b respectively
//! (separate PRs). ANT_ONLY ships with GH because upstream ANT_ONLY
//! spreads `...GH_READ_ONLY_COMMANDS`.
//!
//! Dispatch is unchanged: entries are appended to the same
//! `COMMAND_ALLOWLIST` static in `bash_allowlist.rs`.

use super::flag_parser::{CommandConfig, FlagArgType};

// ---------------------------------------------------------------------------
// DOCKER_READ_ONLY_COMMANDS — docker read-only subcommands
// ---------------------------------------------------------------------------

// ---- docker logs -----------------------------------------------------------
const DOCKER_LOGS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--follow", FlagArgType::None),
    ("-f", FlagArgType::None),
    ("--tail", FlagArgType::String),
    ("-n", FlagArgType::String),
    ("--timestamps", FlagArgType::None),
    ("-t", FlagArgType::None),
    ("--since", FlagArgType::String),
    ("--until", FlagArgType::String),
    ("--details", FlagArgType::None),
];
const DOCKER_LOGS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: DOCKER_LOGS_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---- docker inspect --------------------------------------------------------
const DOCKER_INSPECT_FLAGS: &[(&str, FlagArgType)] = &[
    ("--format", FlagArgType::String),
    ("-f", FlagArgType::String),
    ("--type", FlagArgType::String),
    ("--size", FlagArgType::None),
    ("-s", FlagArgType::None),
];
const DOCKER_INSPECT_CONFIG: CommandConfig = CommandConfig {
    safe_flags: DOCKER_INSPECT_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---------------------------------------------------------------------------
// RIPGREP_READ_ONLY_COMMANDS — rg (ripgrep) read-only search
// ---------------------------------------------------------------------------

// ---- rg --------------------------------------------------------------------
const RG_FLAGS: &[(&str, FlagArgType)] = &[
    // Pattern flags
    ("-e", FlagArgType::String), // Pattern to search for
    ("--regexp", FlagArgType::String),
    ("-f", FlagArgType::String), // Read patterns from file
    // Common search options
    ("-i", FlagArgType::None), // Case insensitive
    ("--ignore-case", FlagArgType::None),
    ("-S", FlagArgType::None), // Smart case
    ("--smart-case", FlagArgType::None),
    ("-F", FlagArgType::None), // Fixed strings
    ("--fixed-strings", FlagArgType::None),
    ("-w", FlagArgType::None), // Word regexp
    ("--word-regexp", FlagArgType::None),
    ("-v", FlagArgType::None), // Invert match
    ("--invert-match", FlagArgType::None),
    // Output options
    ("-c", FlagArgType::None), // Count matches
    ("--count", FlagArgType::None),
    ("-l", FlagArgType::None), // Files with matches
    ("--files-with-matches", FlagArgType::None),
    ("--files-without-match", FlagArgType::None),
    ("-n", FlagArgType::None), // Line number
    ("--line-number", FlagArgType::None),
    ("-o", FlagArgType::None), // Only matching
    ("--only-matching", FlagArgType::None),
    ("-A", FlagArgType::Number), // After context
    ("--after-context", FlagArgType::Number),
    ("-B", FlagArgType::Number), // Before context
    ("--before-context", FlagArgType::Number),
    ("-C", FlagArgType::Number), // Context
    ("--context", FlagArgType::Number),
    ("-H", FlagArgType::None), // With filename
    ("-h", FlagArgType::None), // No filename
    ("--heading", FlagArgType::None),
    ("--no-heading", FlagArgType::None),
    ("-q", FlagArgType::None), // Quiet
    ("--quiet", FlagArgType::None),
    ("--column", FlagArgType::None),
    // File filtering
    ("-g", FlagArgType::String), // Glob
    ("--glob", FlagArgType::String),
    ("-t", FlagArgType::String), // Type
    ("--type", FlagArgType::String),
    ("-T", FlagArgType::String), // Type not
    ("--type-not", FlagArgType::String),
    ("--type-list", FlagArgType::None),
    ("--hidden", FlagArgType::None),
    ("--no-ignore", FlagArgType::None),
    ("-u", FlagArgType::None), // Unrestricted
    // Common options
    ("-m", FlagArgType::Number), // Max count per file
    ("--max-count", FlagArgType::Number),
    ("-d", FlagArgType::Number), // Max depth
    ("--max-depth", FlagArgType::Number),
    ("-a", FlagArgType::None), // Text (search binary files)
    ("--text", FlagArgType::None),
    ("-z", FlagArgType::None), // Search zip
    ("-L", FlagArgType::None), // Follow symlinks
    ("--follow", FlagArgType::None),
    // Display options
    ("--color", FlagArgType::String),
    ("--json", FlagArgType::None),
    ("--stats", FlagArgType::None),
    // Help and version
    ("--help", FlagArgType::None),
    ("--version", FlagArgType::None),
    ("--debug", FlagArgType::None),
    // Special argument separator
    ("--", FlagArgType::None),
];
const RG_CONFIG: CommandConfig = CommandConfig {
    safe_flags: RG_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---------------------------------------------------------------------------
// PYRIGHT_READ_ONLY_COMMANDS — pyright static type checker
// ---------------------------------------------------------------------------

// ---- pyright ---------------------------------------------------------------
const PYRIGHT_FLAGS: &[(&str, FlagArgType)] = &[
    ("--outputjson", FlagArgType::None),
    ("--project", FlagArgType::String),
    ("-p", FlagArgType::String),
    ("--pythonversion", FlagArgType::String),
    ("--pythonplatform", FlagArgType::String),
    ("--typeshedpath", FlagArgType::String),
    ("--venvpath", FlagArgType::String),
    ("--level", FlagArgType::String),
    ("--stats", FlagArgType::None),
    ("--verbose", FlagArgType::None),
    ("--version", FlagArgType::None),
    ("--dependencies", FlagArgType::None),
    ("--warnings", FlagArgType::None),
];

// SECURITY: upstream `additionalCommandIsDangerousCallback` for pyright.
// Check if `--watch` or `-w` appears as a standalone token (flag).
// `--watch` runs pyright as a long-lived process re-checking files on
// change — outside the read-only execution model.
/// Pyright dangerous-flag callback — verbatim port of upstream
/// `additionalCommandIsDangerousCallback` from `readOnlyCommandValidation.ts`.
///
/// Defense-in-depth: `--watch` / `-w` are also absent from `PYRIGHT_FLAGS`,
/// so `validate_flags` already rejects them via the flag-table check. This
/// callback is retained verbatim to mirror the upstream defense layer; if a
/// future PR (or shadowing rebase) accidentally adds `--watch` to the safe
/// flags, this callback is the second line of defense.
fn pyright_watch_check(_raw: &str, args: &[&str]) -> bool {
    args.iter().any(|t| *t == "--watch" || *t == "-w")
}

const PYRIGHT_CONFIG: CommandConfig = CommandConfig {
    safe_flags: PYRIGHT_FLAGS,
    additional_dangerous_callback: Some(pyright_watch_check),
    // pyright treats -- as a file path, not end-of-options
    respects_double_dash: false,
};

// ---------------------------------------------------------------------------
// Public exports — preserve upstream entry order within each table.
// ---------------------------------------------------------------------------

pub static DOCKER_READ_ONLY_COMMANDS: &[(&str, &CommandConfig)] = &[
    ("docker logs", &DOCKER_LOGS_CONFIG),
    ("docker inspect", &DOCKER_INSPECT_CONFIG),
];

pub static RIPGREP_READ_ONLY_COMMANDS: &[(&str, &CommandConfig)] = &[("rg", &RG_CONFIG)];

pub static PYRIGHT_READ_ONLY_COMMANDS: &[(&str, &CommandConfig)] = &[("pyright", &PYRIGHT_CONFIG)];
