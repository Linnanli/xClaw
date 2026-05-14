//! Strongly-typed enum of every check ID + result/reason types.
//!
//! Ported from upstream `BASH_SECURITY_CHECK_IDS` constants in
//! `claude-code-main/src/tools/BashTool/bashSecurity.ts` L77-L101 (1:1
//! numbering). The numeric mapping is preserved via [`SecurityCheckId::as_u32`]
//! so differential testing can compare results across the TS / Rust boundary.

/// Every distinct security check the engine can emit.
///
/// Numbering matches upstream `BASH_SECURITY_CHECK_IDS` exactly (1-23). The
/// extra [`Self::ParseFailure`] variant (`0`) is a Fail-Closed catch-all that
/// does not correspond to any upstream ID — emitted by the engine itself
/// when AST parsing fails before any validator can opine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SecurityCheckId {
    /// Synthetic — not in upstream. Engine-level Fail-Closed when AST parse fails.
    ParseFailure = 0,
    IncompleteCommands = 1,
    JqSystemFunction = 2,
    JqFileArguments = 3,
    ObfuscatedFlags = 4,
    ShellMetacharacters = 5,
    DangerousVariables = 6,
    Newlines = 7,
    DangerousPatternsCommandSubstitution = 8,
    DangerousPatternsInputRedirection = 9,
    DangerousPatternsOutputRedirection = 10,
    IfsInjection = 11,
    GitCommitSubstitution = 12,
    ProcEnvironAccess = 13,
    MalformedTokenInjection = 14,
    BackslashEscapedWhitespace = 15,
    BraceExpansion = 16,
    ControlCharacters = 17,
    UnicodeWhitespace = 18,
    MidWordHash = 19,
    ZshDangerousCommands = 20,
    BackslashEscapedOperators = 21,
    CommentQuoteDesync = 22,
    QuotedNewline = 23,
}

impl SecurityCheckId {
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Whether this check is sensitive to upstream misparsing.
    ///
    /// In upstream's two-phase pipeline (bashSecurity.ts L2343 `nonMisparsingValidators`),
    /// only `validateNewlines` and `validateRedirections` are explicitly
    /// non-misparsing — i.e., their `ask` result is *deferred* so that any
    /// later misparsing-sensitive validator gets priority. All other
    /// validators in the main pipeline are misparsing-sensitive.
    ///
    /// In Phase 2.1 we collapse `ask` → `Block` regardless, but we still
    /// preserve the deferred-non-misparsing ordering so that the surfaced
    /// `DecisionReason` matches upstream's user-visible precedence.
    pub fn is_misparsing(self) -> bool {
        !matches!(
            self,
            Self::Newlines
                | Self::DangerousPatternsInputRedirection
                | Self::DangerousPatternsOutputRedirection
        )
    }
}

/// Why a [`SecurityResult::Block`] was emitted.
///
/// Strongly typed (vs upstream's loose `decisionReason: { type, reason }`)
/// so downstream policy / telemetry can pattern-match exhaustively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionReason {
    /// Bash command-injection detected by a specific validator.
    CommandInjection {
        check_id: SecurityCheckId,
        /// Upstream `subId` (1-based; some validators emit multiple sub-cases).
        sub_id: u32,
        /// Human-readable explanation; safe to surface to operators.
        message: String,
    },
    /// The command failed to parse and the engine refused on a Fail-Closed
    /// basis (no validator had the chance to opine).
    ParseFailure { message: String },
}

/// Result of a single validator or of the overall engine.
///
/// Maps upstream `PermissionResult` semantically:
/// - upstream `behavior: 'allow'`        → [`SecurityResult::Allow`]
/// - upstream `behavior: 'passthrough'`  → [`SecurityResult::Passthrough`]
/// - upstream `behavior: 'ask'` / `'block'` (Phase 2.1) → [`SecurityResult::Block`]
///
/// See plan §0 (Porting model) for the rationale of collapsing `ask` into
/// `block`: Phase 2.1 has no Ask UX (desktop will land in Phase 2.3); the
/// safe default is to Block until the user explicitly upgrades the channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityResult {
    /// The command is explicitly trusted by this validator (rare; only
    /// `validate_empty` returns this currently).
    Allow,
    /// This validator has no opinion; continue to the next.
    Passthrough,
    /// The command must not run. Always Fail-Safe.
    Block { reason: DecisionReason },
}

impl SecurityResult {
    pub fn is_block(&self) -> bool {
        matches!(self, Self::Block { .. })
    }
}
