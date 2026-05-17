//! sedValidation — AST-aware safety check for `sed` invocations.
//!
//! Ports the danger-detection half of upstream
//! [`claude-code-main/src/tools/BashTool/sedValidation.ts`][upstream] into
//! Rust, replacing the previous "first-word == sed && contains(' -i')" stub.
//!
//! [upstream]: https://github.com/anthropics/claude-code/blob/main/src/tools/BashTool/sedValidation.ts
//!
//! # Threat model
//!
//! `sed` has two opt-in commands that escape the read-only contract even
//! without `-i`:
//!
//! * `w <file>` / `W <file>` writes (a slice of) the pattern space to an
//!   arbitrary path.
//! * `e [cmd]` / flag `e` on `s///` executes a shell sub-command.
//!
//! Both can also appear inside substitution flags (`s/old/new/we`, `s/old/new/w out.txt`)
//! and inside multi-`-e` expression chains. The previous validator missed
//! every one of these.
//!
//! # Layers
//!
//! 1. Tokenise the command line with [`shell_words`] and split into
//!    pipeline segments on top-level operators (`|`, `&&`, `||`, `;`,
//!    redirection, …).
//! 2. For each segment whose first non-env-prefix token is `sed`,
//!    collect the expression strings (`-e`, `--expression=`, or the
//!    first positional argument).
//! 3. Run a denylist scan ([`contains_dangerous_operations`]) over each
//!    expression. The denylist mirrors upstream's `containsDangerousOperations`
//!    and rejects `w` / `W` / `e` / `E` commands, block groups, comments,
//!    negation, GNU step/offset addresses, escape-tricks, and non-ASCII
//!    glyph spoofing.
//! 4. Preserve existing behaviour: in [`PermissionMode::ReadOnly`], a
//!    sed call that uses `-i` / `--in-place` is Block.
//!
//! # Fail-Safe contract
//!
//! If the shell tokenizer cannot parse the command line but the source
//! text literally contains the word `sed`, the validator returns
//! [`ValidationResult::Block`] rather than risk allowing an
//! unparsable command through. This mirrors the pipeline policy used
//! by [`crate::security::ast`].
//!
//! # Optional strict allowlist
//!
//! [`sed_command_is_allowed_by_allowlist`] re-exports the upstream
//! Pattern 1 (`-n` line printing) and Pattern 2 (single `s/…/…/`
//! substitution) allowlist. Callers that want the full upstream
//! "allowlist-or-ask" semantics — for example a future
//! `BashPermissionHook` integration that maps "not allowlisted" to a
//! user-approval prompt — can use it directly. The default
//! [`validate_sed`] entry only Block-s on positively detected danger
//! to preserve the historical `Allow` for benign substitution like
//! `sed 's/old/new/' file.txt`.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::{PermissionMode, ValidationResult};

/// Run the sed danger gate over `command`.
///
/// Returns [`ValidationResult::Block`] if any sed invocation contains a
/// `w`/`W`/`e`/`E` command, escape tricks, or — when `mode` is
/// [`PermissionMode::ReadOnly`] — uses `-i` / `--in-place`. Returns
/// [`ValidationResult::Allow`] when no sed call is present, or all sed
/// calls are denylist-clean.
#[must_use]
pub fn validate_sed(command: &str, mode: PermissionMode) -> ValidationResult {
    let calls = match extract_sed_invocations(command) {
        Extracted::Calls(v) => v,
        Extracted::FailClosed => return block(BLOCK_PARSE),
        Extracted::NoSed => return ValidationResult::Allow,
    };

    for argv in &calls {
        if let Some(reason) = inspect_sed_call(argv, mode) {
            return block(reason);
        }
    }

    ValidationResult::Allow
}

/// Strict upstream allowlist: returns `true` iff `command` is a single
/// `sed ...` invocation matching Pattern 1 (line printing with `-n`) or
/// Pattern 2 (single `s/…/…/` substitution) and is denylist-clean.
///
/// `allow_file_writes` corresponds to upstream `acceptEdits` mode: when
/// `true`, only the substitution pattern is considered and `-i` /
/// `--in-place` plus trailing file arguments are tolerated.
#[must_use]
pub fn sed_command_is_allowed_by_allowlist(command: &str, allow_file_writes: bool) -> bool {
    let Some(argv) = tokenize_single_sed_call(command) else {
        return false;
    };
    let parsed = split_sed_args(&argv);

    let pattern1 = !allow_file_writes && is_line_printing_pattern(&parsed);
    let pattern2 = is_substitution_pattern(&parsed, allow_file_writes);

    if !pattern1 && !pattern2 {
        return false;
    }

    // Pattern 2 must not contain `;` (command separator). Pattern 1 allows
    // semicolons between print commands and validates them itself.
    if pattern2 && !pattern1 {
        for expr in &parsed.expressions {
            if expr.contains(';') {
                return false;
            }
        }
    }

    for expr in &parsed.expressions {
        if contains_dangerous_operations(expr) {
            return false;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Internal: extraction
// ---------------------------------------------------------------------------

enum Extracted {
    /// One argv vector per `sed ...` simple command in the pipeline.
    Calls(Vec<Vec<String>>),
    /// The command mentions `sed` but tokenization failed → Fail-Closed.
    FailClosed,
    /// The command does not invoke `sed` at all.
    NoSed,
}

/// Top-level shell operators that always terminate a pipeline segment.
///
/// `shell_words::split` does not classify operators — it just returns them
/// as plain string tokens. We split on them ourselves so each segment
/// becomes a candidate command. Redirections are included so a trailing
/// `> out.txt` does not get folded into the sed argv as a file argument.
const SEGMENT_OPERATORS: &[&str] = &[
    "|", "||", "&&", ";", "&", "|&", ">", ">>", "<", "<<", "<<<", "2>", "2>>", "&>", "&>>", "<>",
];

fn extract_sed_invocations(command: &str) -> Extracted {
    let mentions = mentions_sed_token(command);
    let Ok(tokens) = shell_words::split(command) else {
        return if mentions {
            Extracted::FailClosed
        } else {
            Extracted::NoSed
        };
    };

    let segments = split_by_operators(&tokens);
    let mut calls: Vec<Vec<String>> = Vec::new();
    for segment in segments {
        let stripped = strip_env_and_sudo_prefix(&segment);
        if stripped.first().map(String::as_str) == Some("sed") {
            calls.push(stripped[1..].to_vec());
        }
    }

    if !calls.is_empty() {
        return Extracted::Calls(calls);
    }
    if mentions {
        Extracted::FailClosed
    } else {
        Extracted::NoSed
    }
}

fn tokenize_single_sed_call(command: &str) -> Option<Vec<String>> {
    let tokens = shell_words::split(command).ok()?;
    let segments = split_by_operators(&tokens);
    if segments.len() != 1 {
        return None;
    }
    let stripped = strip_env_and_sudo_prefix(&segments[0]);
    if stripped.first().map(String::as_str) != Some("sed") {
        return None;
    }
    Some(stripped[1..].to_vec())
}

fn split_by_operators(tokens: &[String]) -> Vec<Vec<String>> {
    let mut segments = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for tok in tokens {
        if SEGMENT_OPERATORS.contains(&tok.as_str()) {
            if !current.is_empty() {
                segments.push(std::mem::take(&mut current));
            }
        } else {
            current.push(tok.clone());
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn strip_env_and_sudo_prefix(tokens: &[String]) -> Vec<String> {
    let mut start = 0;
    while start < tokens.len() {
        let tok = &tokens[start];
        if is_env_assignment(tok) {
            start += 1;
            continue;
        }
        if tok == "sudo" {
            start += 1;
            while start < tokens.len() && tokens[start].starts_with('-') {
                start += 1;
            }
            continue;
        }
        break;
    }
    tokens[start..].to_vec()
}

fn is_env_assignment(tok: &str) -> bool {
    let Some(eq) = tok.find('=') else {
        return false;
    };
    let key = &tok[..eq];
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn mentions_sed_token(command: &str) -> bool {
    command
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|tok| tok == "sed")
}

// ---------------------------------------------------------------------------
// Internal: per-call inspection
// ---------------------------------------------------------------------------

const BLOCK_PARSE: &str = "sed command could not be safely parsed";

const BLOCK_INPLACE_READ_ONLY: &str =
    "sed -i / --in-place (in-place editing) is not allowed in read-only mode";

const BLOCK_DANGEROUS: &str = "sed expression contains dangerous operations \
                              (write `w`/`W`, execute `e`/`E`, block groups, \
                              negation, escape tricks, or non-ASCII characters)";

fn inspect_sed_call(argv: &[String], mode: PermissionMode) -> Option<&'static str> {
    let parsed = split_sed_args(argv);

    if mode == PermissionMode::ReadOnly && uses_in_place_flag(&parsed.flags) {
        return Some(BLOCK_INPLACE_READ_ONLY);
    }

    for expr in &parsed.expressions {
        if contains_dangerous_operations(expr) {
            return Some(BLOCK_DANGEROUS);
        }
    }

    None
}

fn uses_in_place_flag(flags: &[String]) -> bool {
    for flag in flags {
        if flag == "-i" || flag == "--in-place" || flag.starts_with("--in-place=") {
            return true;
        }
        // Combined short flags like `-ni`, `-Ei` count as in-place too.
        if flag.starts_with('-') && !flag.starts_with("--") && flag.len() > 2 {
            for ch in flag.chars().skip(1) {
                if ch == 'i' {
                    return true;
                }
            }
        }
        // `-i SUFFIX` is handled by the caller passing the flag verbatim; we
        // also catch `-iSUFFIX` (BSD sed style) here.
        if let Some(rest) = flag.strip_prefix("-i") {
            if !rest.is_empty() && !rest.starts_with('-') {
                return true;
            }
        }
    }
    false
}

fn block(reason: &'static str) -> ValidationResult {
    ValidationResult::Block {
        reason: reason.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Internal: argv → flags / expressions / file args
// ---------------------------------------------------------------------------

struct SedArgs<'a> {
    flags: Vec<String>,
    expressions: Vec<&'a str>,
    has_file_arguments: bool,
}

fn split_sed_args(args: &[String]) -> SedArgs<'_> {
    let mut flags: Vec<String> = Vec::new();
    let mut expressions: Vec<&str> = Vec::new();
    let mut has_file_arguments = false;
    let mut consumed_e = false;
    let mut consumed_inline_expression = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();

        if (arg == "-e" || arg == "--expression") && i + 1 < args.len() {
            consumed_e = true;
            flags.push(args[i].clone());
            expressions.push(args[i + 1].as_str());
            i += 2;
            continue;
        }

        if let Some(rest) = arg.strip_prefix("--expression=") {
            consumed_e = true;
            flags.push("--expression".to_string());
            expressions.push(rest);
            i += 1;
            continue;
        }

        if let Some(rest) = arg.strip_prefix("-e=") {
            consumed_e = true;
            flags.push("-e".to_string());
            expressions.push(rest);
            i += 1;
            continue;
        }

        if arg.starts_with('-') && arg != "-" && arg != "--" {
            flags.push(args[i].clone());
            i += 1;
            continue;
        }

        // Non-flag positional argument.
        if !consumed_e && !consumed_inline_expression {
            expressions.push(arg);
            consumed_inline_expression = true;
        } else {
            has_file_arguments = true;
        }
        i += 1;
    }

    SedArgs {
        flags,
        expressions,
        has_file_arguments,
    }
}

// ---------------------------------------------------------------------------
// Internal: allowlist Pattern 1 / Pattern 2
// ---------------------------------------------------------------------------

const PRINT_ALLOWED_FLAGS: &[&str] = &[
    "-n",
    "--quiet",
    "--silent",
    "-E",
    "--regexp-extended",
    "-r",
    "-z",
    "--zero-terminated",
    "--posix",
];

const SUBST_BASE_FLAGS: &[&str] = &["-E", "--regexp-extended", "-r", "--posix"];
const SUBST_FILE_WRITE_FLAGS: &[&str] = &["-i", "--in-place"];

static PRINT_EXPR_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^(?:\d+|\d+,\d+)?p$").expect("static regex")
});

static SUBST_FLAGS_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^[gpimIM]*[1-9]?[gpimIM]*$").expect("static regex")
});

fn is_line_printing_pattern(parsed: &SedArgs<'_>) -> bool {
    if parsed.expressions.is_empty() {
        return false;
    }
    if !flags_in_allowlist(&parsed.flags, PRINT_ALLOWED_FLAGS) {
        return false;
    }
    if !has_n_flag(&parsed.flags) {
        return false;
    }
    for expr in &parsed.expressions {
        for cmd in expr.split(';') {
            if !PRINT_EXPR_RE.is_match(cmd.trim()) {
                return false;
            }
        }
    }
    true
}

fn is_substitution_pattern(parsed: &SedArgs<'_>, allow_file_writes: bool) -> bool {
    if !allow_file_writes && parsed.has_file_arguments {
        return false;
    }
    let mut allowed: Vec<&str> = SUBST_BASE_FLAGS.to_vec();
    if allow_file_writes {
        allowed.extend(SUBST_FILE_WRITE_FLAGS);
    }
    if !flags_in_allowlist(&parsed.flags, &allowed) {
        return false;
    }
    if parsed.expressions.len() != 1 {
        return false;
    }
    let expr = parsed.expressions[0].trim();
    if !expr.starts_with('s') {
        return false;
    }
    let Some(rest) = expr.strip_prefix("s/") else {
        return false;
    };
    let Some(flags_part) = strip_substitution_body(rest) else {
        return false;
    };
    SUBST_FLAGS_RE.is_match(flags_part)
}

fn flags_in_allowlist(flags: &[String], allowed: &[&str]) -> bool {
    for flag in flags {
        if allowed.contains(&flag.as_str()) {
            continue;
        }
        // Combined short flag like `-nE`.
        if flag.starts_with('-') && !flag.starts_with("--") && flag.len() > 2 {
            if !combined_short_chars_allowed(flag, allowed) {
                return false;
            }
            continue;
        }
        return false;
    }
    true
}

fn combined_short_chars_allowed(flag: &str, allowed: &[&str]) -> bool {
    for ch in flag.chars().skip(1) {
        let single = format!("-{ch}");
        if !allowed.contains(&single.as_str()) {
            return false;
        }
    }
    true
}

fn has_n_flag(flags: &[String]) -> bool {
    for flag in flags {
        if matches!(flag.as_str(), "-n" | "--quiet" | "--silent") {
            return true;
        }
        if flag.starts_with('-') && !flag.starts_with("--") && flag.contains('n') {
            return true;
        }
    }
    false
}

/// Given the body after `s/` of a substitution expression, returns the
/// flags slice if the body has exactly two unescaped `/` delimiters left.
fn strip_substitution_body(body: &str) -> Option<&str> {
    let bytes = body.as_bytes();
    let mut delimiter_count = 0usize;
    let mut last_delim = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'/' {
            delimiter_count += 1;
            last_delim = i;
        }
        i += 1;
    }
    if delimiter_count != 2 {
        return None;
    }
    Some(&body[last_delim + 1..])
}

// ---------------------------------------------------------------------------
// Internal: denylist (containsDangerousOperations)
// ---------------------------------------------------------------------------

static NEGATION_AFTER_ADDR_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"[/\d$]!").expect("static regex")
});

static GNU_STEP_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"\d\s*~\s*\d|,\s*~\s*\d|\$\s*~\s*\d").expect("static regex")
});

static GNU_OFFSET_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r",\s*[+-]").expect("static regex")
});

static BACKSLASH_DELIM_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"\\[|#%@]").expect("static regex")
});

static ESCAPED_SLASH_W_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"\\/.*[wW]").expect("static regex")
});

static SLASH_THEN_DANGEROUS_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"/[^/]*\s+[wWeE]").expect("static regex")
});

static MALFORMED_SUBST_RE: Lazy<Regex> = Lazy::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^s/[^/]*/[^/]*/[^/]*$").expect("static regex")
});

static WRITE_CMD_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    let raws = [
        r"^[wW]\s*\S+",
        r"^\d+\s*[wW]\s*\S+",
        r"^\$\s*[wW]\s*\S+",
        r"^/[^/]*/[IMim]*\s*[wW]\s*\S+",
        r"^\d+,\d+\s*[wW]\s*\S+",
        r"^\d+,\$\s*[wW]\s*\S+",
        r"^/[^/]*/[IMim]*,/[^/]*/[IMim]*\s*[wW]\s*\S+",
    ];
    #[allow(clippy::expect_used)]
    raws.iter()
        .map(|r| Regex::new(r).expect("static regex"))
        .collect()
});

static EXEC_CMD_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    let raws = [
        r"^e",
        r"^\d+\s*e",
        r"^\$\s*e",
        r"^/[^/]*/[IMim]*\s*e",
        r"^\d+,\d+\s*e",
        r"^\d+,\$\s*e",
        r"^/[^/]*/[IMim]*,/[^/]*/[IMim]*\s*e",
    ];
    #[allow(clippy::expect_used)]
    raws.iter()
        .map(|r| Regex::new(r).expect("static regex"))
        .collect()
});

/// Mirror of upstream `containsDangerousOperations`.
///
/// Returns `true` for any expression containing a positively-detected
/// dangerous construct. Conservative: when in doubt, treat as unsafe.
#[must_use]
pub fn contains_dangerous_operations(expression: &str) -> bool {
    let cmd = expression.trim();
    if cmd.is_empty() {
        return false;
    }

    if has_non_ascii(cmd)
        || cmd.contains('{')
        || cmd.contains('}')
        || cmd.contains('\n')
        || has_bare_comment(cmd)
    {
        return true;
    }

    if cmd.starts_with('!') || NEGATION_AFTER_ADDR_RE.is_match(cmd) {
        return true;
    }

    if GNU_STEP_RE.is_match(cmd) || cmd.starts_with(',') || GNU_OFFSET_RE.is_match(cmd) {
        return true;
    }

    if cmd.contains("s\\") || BACKSLASH_DELIM_RE.is_match(cmd) {
        return true;
    }

    if ESCAPED_SLASH_W_RE.is_match(cmd) || SLASH_THEN_DANGEROUS_RE.is_match(cmd) {
        return true;
    }

    if cmd.starts_with("s/") && !MALFORMED_SUBST_RE.is_match(cmd) {
        return true;
    }

    if has_dangerous_trailing(cmd) {
        return true;
    }

    if WRITE_CMD_PATTERNS.iter().any(|re| re.is_match(cmd)) {
        return true;
    }

    if EXEC_CMD_PATTERNS.iter().any(|re| re.is_match(cmd)) {
        return true;
    }

    if substitution_flags_have_danger(cmd) {
        return true;
    }

    if y_command_has_danger(cmd) {
        return true;
    }

    false
}

fn has_non_ascii(cmd: &str) -> bool {
    cmd.bytes().any(|b| b > 0x7F)
}

fn has_bare_comment(cmd: &str) -> bool {
    let Some(hash) = cmd.find('#') else {
        return false;
    };
    // `s#pat#repl#` uses `#` as a delimiter and is dangerous but is caught
    // by the malformed-substitution rule above (we only allow `/`). A bare
    // `#` outside that special case starts a comment.
    if hash > 0 {
        let prev = cmd.as_bytes()[hash - 1];
        if prev == b's' {
            return false;
        }
    }
    true
}

fn has_dangerous_trailing(cmd: &str) -> bool {
    if cmd.len() < 2 {
        return false;
    }
    let last = cmd.as_bytes()[cmd.len() - 1];
    if !matches!(last, b'w' | b'W' | b'e' | b'E') {
        return false;
    }
    // Properly-formed substitution like `s/a/b/g` does not end in w/W/e/E;
    // any "s." command that does is suspicious. The upstream check matches
    // `^s.` and then verifies a proper substitution form using any
    // delimiter; if neither matches, treat as dangerous.
    if !cmd.starts_with('s') {
        return false;
    }
    let bytes = cmd.as_bytes();
    let Some(delim) = bytes.get(1).copied() else {
        return false;
    };
    if delim == b'\\' || delim == b'\n' {
        return true;
    }
    // Count unescaped delimiters; a proper s<d>pat<d>repl<d>flags has 3.
    let mut count = 0usize;
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == delim {
            count += 1;
        }
        i += 1;
    }
    if count != 3 {
        return true;
    }
    // The trailing char itself is w/W/e/E: that *is* the dangerous flag.
    true
}

fn substitution_flags_have_danger(cmd: &str) -> bool {
    if !cmd.starts_with('s') {
        return false;
    }
    let bytes = cmd.as_bytes();
    let Some(delim) = bytes.get(1).copied() else {
        return false;
    };
    if delim == b'\\' || delim == b'\n' {
        return false;
    }
    // Walk to find the third unescaped delimiter; flags start after it.
    let mut found = 0usize;
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == delim {
            found += 1;
            if found == 3 {
                let flags = &cmd[i + 1..];
                return flags.chars().any(|c| matches!(c, 'w' | 'W' | 'e' | 'E'));
            }
        }
        i += 1;
    }
    false
}

fn y_command_has_danger(cmd: &str) -> bool {
    if !cmd.starts_with('y') {
        return false;
    }
    let bytes = cmd.as_bytes();
    let Some(delim) = bytes.get(1).copied() else {
        return false;
    };
    if delim == b'\\' || delim == b'\n' {
        return false;
    }
    cmd.chars().any(|c| matches!(c, 'w' | 'W' | 'e' | 'E'))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn block(command: &str, mode: PermissionMode) -> bool {
        matches!(validate_sed(command, mode), ValidationResult::Block { .. })
    }

    fn allow(command: &str, mode: PermissionMode) -> bool {
        matches!(validate_sed(command, mode), ValidationResult::Allow)
    }

    // --- Red tests from Phase 4.1 handoff ------------------------------

    #[test]
    fn req_safety_490_4_1_blocks_subst_with_e_flag() {
        assert!(block("sed 's/x/y/e' file", PermissionMode::WorkspaceWrite));
        assert!(block("sed 's/x/y/e' file", PermissionMode::ReadOnly));
    }

    #[test]
    fn req_safety_490_4_1_blocks_subst_with_w_flag_target() {
        assert!(block(
            "sed 's/x/y/w out.txt' file",
            PermissionMode::ReadOnly
        ));
        assert!(block(
            "sed 's/x/y/w out.txt' file",
            PermissionMode::WorkspaceWrite
        ));
    }

    #[test]
    fn req_safety_490_4_1_blocks_multi_e_with_dangerous_payload() {
        assert!(block(
            "sed -e 's/x/y/' -e 's/a/b/e' file",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_blocks_bare_e_command() {
        assert!(block(
            "sed -e 'e cat /etc/passwd' file",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_blocks_addressed_w_command() {
        assert!(block(
            "sed '1,10w /tmp/secrets' file",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_blocks_pattern_addressed_e() {
        assert!(block(
            "sed '/PASS/e logger danger' file",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_blocks_y_with_e() {
        assert!(block(
            "sed 'y/abc/xyz/e logger' file",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_blocks_non_ascii_homoglyph_w() {
        // Fullwidth `ｗ` should not bypass the rule.
        assert!(block("sed '1ｗ /tmp/x' file", PermissionMode::ReadOnly));
    }

    #[test]
    fn req_safety_490_4_1_blocks_block_group() {
        assert!(block("sed -n '/pat/{p;q}' file", PermissionMode::ReadOnly));
    }

    #[test]
    fn req_safety_490_4_1_blocks_comment_injection() {
        assert!(block(
            "sed -e 's/a/b/' -e '#w /tmp/sneaky' file",
            PermissionMode::ReadOnly
        ));
    }

    // --- Existing-behaviour preservation -------------------------------

    #[test]
    fn req_safety_490_4_1_preserves_basic_stdout_substitution() {
        assert!(allow("sed 's/old/new/' file.txt", PermissionMode::ReadOnly));
        assert!(allow(
            "sed 's/old/new/g' file.txt",
            PermissionMode::WorkspaceWrite
        ));
    }

    #[test]
    fn req_safety_490_4_1_preserves_line_printing() {
        assert!(allow("sed -n '1,10p' file", PermissionMode::ReadOnly));
        assert!(allow("sed -n '1p;2p;3p' file", PermissionMode::ReadOnly));
    }

    #[test]
    fn req_safety_490_4_1_preserves_blocks_inplace_read_only() {
        assert!(block(
            "sed -i 's/old/new/' file.txt",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_allows_inplace_in_workspace_write() {
        assert!(allow(
            "sed -i 's/old/new/' file.txt",
            PermissionMode::WorkspaceWrite
        ));
    }

    #[test]
    fn req_safety_490_4_1_ignores_non_sed_commands() {
        assert!(allow("cat file.txt", PermissionMode::ReadOnly));
        assert!(allow("grep pat file", PermissionMode::ReadOnly));
    }

    // --- Pipeline / chaining -------------------------------------------

    #[test]
    fn req_safety_490_4_1_walks_pipeline_segments() {
        // Dangerous sed is in the middle of a pipeline.
        assert!(block(
            "cat foo | sed 's/x/y/e' | tee out",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_walks_logical_and() {
        assert!(block(
            "cd /tmp && sed '1w /tmp/leak' file",
            PermissionMode::ReadOnly
        ));
    }

    #[test]
    fn req_safety_490_4_1_strips_env_prefix() {
        assert!(block("LANG=C sed 's/x/y/e' file", PermissionMode::ReadOnly));
    }

    #[test]
    fn req_safety_490_4_1_strips_sudo_prefix() {
        assert!(block("sudo sed 's/x/y/e' file", PermissionMode::ReadOnly));
    }

    // --- Fail-Safe / parse failure -------------------------------------

    #[test]
    fn req_safety_490_4_1_fail_closed_on_parse_failure_with_sed_mention() {
        // Unbalanced quote → shell_words::split errors.
        assert!(block("sed 's/old/new/ file.txt", PermissionMode::ReadOnly));
    }

    #[test]
    fn req_safety_490_4_1_fail_closed_keeps_allow_when_no_sed() {
        // Unbalanced quote but no sed → don't blame sed.
        assert!(allow("echo 'unterminated", PermissionMode::ReadOnly));
    }

    #[test]
    fn req_safety_490_4_1_does_not_confuse_sedutil_with_sed() {
        // A binary that contains the substring "sed" is not `sed` itself.
        assert!(allow("sedutil-cli --foo", PermissionMode::ReadOnly));
    }

    // --- Strict allowlist (opt-in) -------------------------------------

    #[test]
    fn req_safety_490_4_1_allowlist_accepts_print_pattern() {
        assert!(sed_command_is_allowed_by_allowlist(
            "sed -n '1,10p' file",
            false
        ));
        assert!(sed_command_is_allowed_by_allowlist(
            "sed -n '1p;2p;3p' file",
            false
        ));
    }

    #[test]
    fn req_safety_490_4_1_allowlist_accepts_simple_substitution_stdin_only() {
        assert!(sed_command_is_allowed_by_allowlist("sed 's/a/b/g'", false));
    }

    #[test]
    fn req_safety_490_4_1_allowlist_rejects_substitution_with_file_when_strict() {
        assert!(!sed_command_is_allowed_by_allowlist(
            "sed 's/a/b/' file.txt",
            false
        ));
    }

    #[test]
    fn req_safety_490_4_1_allowlist_accepts_inplace_when_allowed() {
        assert!(sed_command_is_allowed_by_allowlist(
            "sed -i 's/a/b/g' file.txt",
            true
        ));
    }

    #[test]
    fn req_safety_490_4_1_allowlist_rejects_dangerous_flag() {
        assert!(!sed_command_is_allowed_by_allowlist("sed 's/a/b/we'", true));
    }

    // --- Denylist regression ------------------------------------------

    #[test]
    fn req_safety_490_4_1_denylist_catches_classic_patterns() {
        assert!(contains_dangerous_operations("1w /tmp/x"));
        assert!(contains_dangerous_operations("e cat /etc/passwd"));
        assert!(contains_dangerous_operations("/secret/W /tmp/leak"));
        assert!(contains_dangerous_operations("s/a/b/w /tmp/x"));
        assert!(contains_dangerous_operations("s|a|b|e"));
        assert!(!contains_dangerous_operations("s/a/b/g"));
        assert!(!contains_dangerous_operations("1,10p"));
    }
}
