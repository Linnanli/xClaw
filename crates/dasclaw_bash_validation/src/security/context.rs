//! [`ValidationContext`] — read-only bundle handed to every validator.
//!
//! Slice 2.1.a only populates `original_command`. Phase 2.1.b will compute
//! `fully_unquoted_pre_strip`, `unquoted_keep_quote_chars`, AST cache, etc.
//! (mirroring upstream `ValidationContext` in `bashSecurity.ts` L162-L196).

/// Read-only bundle of pre-computed views of the command, shared across
/// validators within a single security gate evaluation.
#[derive(Debug, Clone)]
pub struct ValidationContext<'a> {
    /// Exact bytes the user provided, untouched.
    original_command: &'a str,
}

impl<'a> ValidationContext<'a> {
    pub fn new(command: &'a str) -> Self {
        Self {
            original_command: command,
        }
    }

    pub fn original_command(&self) -> &str {
        self.original_command
    }
}
