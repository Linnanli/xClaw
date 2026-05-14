//! Strongly-typed enum of every check ID + result/reason types.
//!
//! Ported from upstream `BASH_SECURITY_CHECK_IDS` constants in
//! `claude-code-main/src/tools/BashTool/bashSecurity.ts` L82-L125. Each
//! variant corresponds 1:1 to an upstream check ID; the numeric mapping is
//! preserved via [`SecurityCheckId::as_u32`] so that differential testing can
//! compare results across the TS / Rust boundary.

/// Every distinct security check the engine can emit.
///
/// `numbering` matches upstream `BASH_SECURITY_CHECK_IDS`. New variants must
/// preserve gaps so existing IDs never shift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SecurityCheckId {
    IncompleteCommands = 1,
    JqSystemFunction = 2,
    XargsRedirection = 3,
    SafeCommandSubstitution = 4,
    GitCommit = 5,
    JqCommand = 6,
    ShellMetacharacters = 7,
    DangerousVariables = 8,
    DangerousPatterns = 9,
    Redirections = 10,
    Newlines = 11,
    IfsInjection = 12,
    ProcEnvironAccess = 13,
    MalformedTokenInjection = 14,
    ObfuscatedFlags = 15,
    BackslashEscapedWhitespace = 16,
    BackslashEscapedOperators = 17,
    BraceExpansion = 18,
    UnicodeWhitespace = 19,
    MidWordHash = 20,
    CommentQuoteDesync = 21,
    HeredocQuoted = 22,
    TrailingBackslash = 23,
    /// Parse failure — Fail-Closed catch-all.
    ParseFailure = 24,
}

impl SecurityCheckId {
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Whether this check is sensitive to upstream misparsing (i.e. its
    /// `ask` result becomes `block` at the bashPermissions gate).
    ///
    /// Mirrors upstream `isBashSecurityCheckForMisparsing` (L130-L160). Used
    /// by the deferred-non-misparsing engine in Slice 2.1.b.
    pub fn is_misparsing(self) -> bool {
        matches!(
            self,
            Self::ShellMetacharacters
                | Self::DangerousVariables
                | Self::DangerousPatterns
                | Self::Redirections
                | Self::IfsInjection
                | Self::ProcEnvironAccess
                | Self::MalformedTokenInjection
                | Self::ObfuscatedFlags
                | Self::BackslashEscapedWhitespace
                | Self::BackslashEscapedOperators
                | Self::BraceExpansion
                | Self::MidWordHash
                | Self::CommentQuoteDesync
                | Self::TrailingBackslash
                | Self::ParseFailure
        )
    }
}

/// Why a particular [`SecurityResult::Block`] was emitted.
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
