//! Fail-Closed wrapper around `dasclaw_shell_command::bash::try_parse_shell`.
//!
//! Re-uses the workspace's single tree-sitter-bash wiring so we have one AST
//! source of truth (plan §4.2). The Fail-Closed contract is:
//!
//! - Parse failure → [`ParseFail::TreeSitterError`]
//! - Parse with `ERROR` nodes in the tree → [`ParseFail::ErrorNode`]
//!
//! Slice 2.1.b will extend [`BashAst`] with a typed walk (allowlist of
//! `ALLOWED_KINDS`) that returns [`ParseFail::UnknownNode`] on any node not
//! on the allowlist; the deferred engine then maps that to
//! [`crate::security::DecisionReason::ParseFailure`].

use thiserror::Error;
use tree_sitter::Tree;

/// Reasons the AST layer can refuse to produce a `BashAst`.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseFail {
    /// tree-sitter-bash returned `None` (corrupted grammar load, OOM, etc.).
    #[error("tree-sitter-bash failed to parse the command")]
    TreeSitterError,
    /// Parse succeeded but the tree contains `ERROR` nodes (syntactic
    /// fragments, mismatched quotes, etc.).
    #[error("bash AST contains ERROR nodes")]
    ErrorNode,
}

/// Owning wrapper around the validated tree-sitter `Tree`.
///
/// Slice 2.1.a stores only the tree; Slice 2.1.b will add cached structural
/// queries (heredoc list, quote-context map, etc.).
#[derive(Debug)]
pub struct BashAst {
    tree: Tree,
}

impl BashAst {
    pub fn root_has_error(&self) -> bool {
        self.tree.root_node().has_error()
    }

    /// Returns the underlying tree for consumers that need raw access.
    /// Slice 2.1.b will replace most callers with typed accessors.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }
}

/// Parse `command` via the shared `dasclaw_shell_command` parser, returning
/// a [`BashAst`] on success or a [`ParseFail`] otherwise.
pub fn parse_for_security(command: &str) -> Result<BashAst, ParseFail> {
    let tree =
        dasclaw_shell_command::bash::try_parse_shell(command).ok_or(ParseFail::TreeSitterError)?;
    if tree.root_node().has_error() {
        return Err(ParseFail::ErrorNode);
    }
    Ok(BashAst { tree })
}
