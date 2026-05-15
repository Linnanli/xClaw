//! Output redirection extraction via tree-sitter-bash AST.
//!
//! Slice 3.1.0 of bash-parity Phase 3.1 (epic #490, tracker #562).
//!
//! Mirrors the contract of upstream
//! [`claude-code-main/src/utils/bash/commands.ts::extractOutputRedirections`]
//! but is implemented on top of the tree-sitter AST already wired up in
//! [`dasclaw_shell_command::bash::try_parse_shell`] instead of `shell-quote`
//! tokenization. AST parsing handles heredocs, line continuations, and
//! redirected subshells natively, so the upstream workarounds for
//! `shell-quote`'s limitations (heredoc pre-extraction, line-continuation
//! joining, redirected-subshell tracking) are not required here.
//!
//! ## Fail-closed contract
//!
//! Any of the following → `has_dangerous_redirection = true`:
//!
//! * tree-sitter fails to parse the source
//! * the parsed tree contains errors (`Node::has_error`)
//! * a `file_redirect` node uses an operator we do not recognize as a plain
//!   output write (e.g. force-overwrite `>|`, force-clobber `>!`)
//! * a `file_redirect` target node contains runtime expansion
//!   (`$VAR`, command substitution, glob, history expansion, brace expansion,
//!   tilde expansion, …)
//! * a `file_redirect` target is the empty string
//!
//! Captured redirections only include targets whose text is verifiably a
//! literal path string at parse time, suitable for path-boundary validation
//! by later Phase 3.1 slices.

use dasclaw_shell_command::bash::try_parse_shell;
use tree_sitter::Node;

/// The shell operator used to attach a redirect target to a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RedirectOperator {
    /// `>` — truncate-write the target.
    Write,
    /// `>>` — append-write the target.
    Append,
}

/// A single literal output redirection captured from a bash source string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputRedirection {
    /// The literal path text the shell would write to. Tilde expansion,
    /// variable substitution, command substitution, globs, and braces are
    /// all rejected before this struct is constructed, so callers can treat
    /// the value as an already-decoded path string.
    pub target: String,
    pub operator: RedirectOperator,
}

/// Aggregate result of scanning a bash source for output redirections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedirectionExtraction {
    /// All `>`/`>>` redirects whose target was a literal path string.
    pub redirections: Vec<OutputRedirection>,
    /// Set when at least one output redirect was detected that the parser
    /// could not classify as a literal path (see module-level docs). Callers
    /// MUST treat this as "could write to anywhere" and prompt the user.
    pub has_dangerous_redirection: bool,
}

/// Scan a bash source string and return every literal output redirection plus
/// a fail-closed signal for any redirection that could not be classified.
///
/// See the module-level documentation for the fail-closed contract.
#[must_use]
pub fn extract_output_redirections(src: &str) -> RedirectionExtraction {
    let Some(tree) = try_parse_shell(src) else {
        return dangerous();
    };
    let root = tree.root_node();
    if root.has_error() {
        return dangerous();
    }

    let mut result = RedirectionExtraction::default();
    let src_bytes = src.as_bytes();
    // Collect file_redirect nodes in source order so the returned vector
    // preserves the left-to-right order that bash itself observes (important
    // for callers that want to attribute a violation to a specific operator).
    let mut redirects = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "file_redirect" {
            redirects.push(node);
            continue;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    redirects.sort_by_key(Node::start_byte);

    for node in redirects {
        match classify_file_redirect(node, src_bytes) {
            Classification::Captured(redir) => result.redirections.push(redir),
            Classification::Dangerous => result.has_dangerous_redirection = true,
            Classification::NotOutput => {}
        }
    }
    result
}

fn dangerous() -> RedirectionExtraction {
    RedirectionExtraction {
        redirections: Vec::new(),
        has_dangerous_redirection: true,
    }
}

enum Classification {
    Captured(OutputRedirection),
    Dangerous,
    /// The redirect is an input read (`<`, `<<<`) rather than an output
    /// write — irrelevant to this function's contract.
    NotOutput,
}

fn classify_file_redirect(node: Node<'_>, src: &[u8]) -> Classification {
    let mut operator: Option<RedirectOperator> = None;
    let mut saw_input_op = false;
    // tree-sitter's `file_redirect` puts the operator before the target, so
    // the last named child is the redirect target. Anonymous children include
    // optional file-descriptor prefixes (`2`) and the operator token itself.
    let mut target: Option<Node<'_>> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            target = Some(child);
            continue;
        }
        match child.kind() {
            ">" | "&>" => operator = Some(RedirectOperator::Write),
            ">>" | "&>>" => operator = Some(RedirectOperator::Append),
            "<" | "<<<" => saw_input_op = true,
            _ => {}
        }
    }

    match operator {
        Some(op) => match target {
            Some(t) => classify_target(t, src, op),
            // Output operator with no target — tree-sitter usually marks this
            // as an error which we already short-circuit, but be paranoid.
            None => Classification::Dangerous,
        },
        None if saw_input_op => Classification::NotOutput,
        // file_redirect without a recognized operator (`>|`, `>!`, fd-dup
        // `>&`, …) → fail-closed: we cannot guarantee semantics.
        None => Classification::Dangerous,
    }
}

fn classify_target(node: Node<'_>, src: &[u8], operator: RedirectOperator) -> Classification {
    let Some(text) = extract_literal_target(node, src) else {
        return Classification::Dangerous;
    };
    if text.is_empty() || has_dangerous_chars(&text) {
        return Classification::Dangerous;
    }
    Classification::Captured(OutputRedirection {
        target: text,
        operator,
    })
}

/// Recover the literal path text from a redirect-target AST node, returning
/// `None` if any descendant requires runtime expansion. The result is the
/// concatenation of every literal segment in source order.
fn extract_literal_target(node: Node<'_>, src: &[u8]) -> Option<String> {
    match node.kind() {
        "word" => Some(node.utf8_text(src).ok()?.to_owned()),
        "raw_string" => {
            // `'literal'` — strip surrounding single quotes (no expansion).
            let text = node.utf8_text(src).ok()?;
            Some(strip_outer(text, '\'').to_owned())
        }
        "string" => {
            // `"..."` — accept only when every named child is plain literal
            // content. The presence of `simple_expansion`, `expansion`,
            // `command_substitution`, etc. indicates runtime substitution.
            let mut out = String::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() != "string_content" {
                    return None;
                }
                out.push_str(child.utf8_text(src).ok()?);
            }
            Some(out)
        }
        "concatenation" => {
            let mut out = String::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                out.push_str(&extract_literal_target(child, src)?);
            }
            Some(out)
        }
        // simple_expansion, expansion, command_substitution,
        // process_substitution, arithmetic_expansion, … — all require runtime
        // evaluation, so refuse to claim a literal target.
        _ => None,
    }
}

/// Returns true when a literal target contains a character pattern that bash
/// would still expand even though the AST classified the whole token as a
/// `word`/`raw_string`/`string`. tree-sitter-bash does **not** flag history
/// expansion (`!`), tilde expansion (`~`), zsh equals expansion (`=cmd`), or
/// glob characters (`* ? [ {`) at the redirect-target level — they only
/// matter at shell runtime — so we must guard them ourselves to stay aligned
/// with the upstream invariant ("every captured target is a literal path").
fn has_dangerous_chars(text: &str) -> bool {
    let first_byte_dangerous = matches!(text.as_bytes().first(), Some(b'!' | b'=' | b'~'));
    first_byte_dangerous
        || text.contains('$')
        || text.contains('`')
        || text.contains('*')
        || text.contains('?')
        || text.contains('[')
        || text.contains('{')
}

fn strip_outer(s: &str, q: char) -> &str {
    let bytes = s.as_bytes();
    let qb = q as u8;
    if bytes.len() >= 2 && bytes[0] == qb && bytes[bytes.len() - 1] == qb {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &str) -> RedirectionExtraction {
        extract_output_redirections(src)
    }

    fn captured(target: &str, operator: RedirectOperator) -> OutputRedirection {
        OutputRedirection {
            target: target.to_owned(),
            operator,
        }
    }

    #[test]
    fn no_redirect_returns_empty_not_dangerous() {
        let r = extract("echo hello");
        assert!(r.redirections.is_empty());
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn captures_truncate_write() {
        let r = extract("echo hi > /tmp/a.txt");
        assert_eq!(
            r.redirections,
            vec![captured("/tmp/a.txt", RedirectOperator::Write)]
        );
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn captures_append_write() {
        let r = extract("echo hi >> /tmp/a.log");
        assert_eq!(
            r.redirections,
            vec![captured("/tmp/a.log", RedirectOperator::Append)]
        );
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn captures_stderr_fd_redirect_as_write() {
        // `2> errlog` — the FD prefix is anonymous and does not change the
        // path-validation contract.
        let r = extract("cmd 2> errlog");
        assert_eq!(
            r.redirections,
            vec![captured("errlog", RedirectOperator::Write)]
        );
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn multiple_redirects_all_captured() {
        let r = extract("cmd > a.out 2>> b.err");
        assert_eq!(
            r.redirections,
            vec![
                captured("a.out", RedirectOperator::Write),
                captured("b.err", RedirectOperator::Append),
            ]
        );
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn captures_double_quoted_literal_target() {
        let r = extract(r#"echo > "/etc/hosts.bak""#);
        assert_eq!(
            r.redirections,
            vec![captured("/etc/hosts.bak", RedirectOperator::Write)]
        );
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn captures_single_quoted_literal_target() {
        let r = extract("echo > '/etc/passwd.bak'");
        assert_eq!(
            r.redirections,
            vec![captured("/etc/passwd.bak", RedirectOperator::Write)]
        );
        assert!(!r.has_dangerous_redirection);
    }

    // ----- SECURITY PINS: targets that MUST be flagged dangerous -----

    #[test]
    fn pin_variable_target_is_dangerous() {
        let r = extract("echo > $HOME/.bashrc");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_quoted_variable_target_is_dangerous() {
        let r = extract(r#"echo > "$HOME/.bashrc""#);
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_tilde_target_is_dangerous() {
        let r = extract("echo > ~/.bashrc");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_glob_target_is_dangerous() {
        let r = extract("echo > *.sh");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_brace_expansion_target_is_dangerous() {
        let r = extract("echo > {a,b}.txt");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_command_substitution_target_is_dangerous() {
        let r = extract("echo > $(date).log");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_backtick_substitution_target_is_dangerous() {
        let r = extract("echo > `date`.log");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_history_expansion_target_is_dangerous() {
        let r = extract("echo > !!");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn pin_concatenated_expansion_is_dangerous() {
        // `$HOME/file` parses as concatenation(simple_expansion + word).
        let r = extract("echo > $HOME/file");
        assert!(r.redirections.is_empty());
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn input_redirect_is_not_an_output() {
        let r = extract("cat < input.txt");
        assert!(r.redirections.is_empty());
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn heredoc_body_does_not_leak_inner_redirect() {
        // The `> /etc/passwd` inside the heredoc body is literal text, not a
        // redirect of the outer `cat`. tree-sitter classifies the body as
        // `heredoc_body`, not `file_redirect`, so we capture nothing.
        let src = "cat <<EOF\n> /etc/passwd\nEOF\n";
        let r = extract(src);
        assert!(r.redirections.is_empty());
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn parse_failure_fails_closed() {
        // Unbalanced quotes → tree-sitter marks the tree with errors.
        let r = extract("echo > \"/etc/passwd");
        assert!(r.has_dangerous_redirection);
        assert!(r.redirections.is_empty());
    }

    #[test]
    fn line_continuation_does_not_smuggle_path() {
        // The shell-quote-based upstream port had to special-case `\<newline>`
        // because it produced empty-string tokens. tree-sitter joins the
        // continuation natively, so we capture `/etc/passwd` like any other
        // literal target. (Workspace-boundary checks land in slice 3.1.b.)
        let src = "echo > \\\n/etc/passwd";
        let r = extract(src);
        assert_eq!(
            r.redirections,
            vec![captured("/etc/passwd", RedirectOperator::Write)]
        );
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn force_overwrite_operator_is_dangerous() {
        // `>|` / `>!` are rare force-clobber forms. We do not whitelist them;
        // they should bubble up to user prompt rather than be silently treated
        // as plain writes.
        let r = extract("echo >| /tmp/force.log");
        assert!(r.has_dangerous_redirection);
    }

    #[test]
    fn combined_stdout_stderr_redirect_captured_as_write() {
        // `&>` writes both fds to the target. Path-boundary semantics are
        // identical to `>`, so we capture it as a Write.
        let r = extract("cmd &> /tmp/combined.log");
        assert_eq!(
            r.redirections,
            vec![captured("/tmp/combined.log", RedirectOperator::Write)]
        );
        assert!(!r.has_dangerous_redirection);
    }

    #[test]
    fn pipeline_does_not_become_redirect() {
        let r = extract("cmd | grep foo");
        assert!(r.redirections.is_empty());
        assert!(!r.has_dangerous_redirection);
    }
}
