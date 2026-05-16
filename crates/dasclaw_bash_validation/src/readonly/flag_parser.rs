//! Flag parser primitives for the `readOnlyValidation` allowlist engine.
//!
//! Semantic port of `validateFlags` + `validateFlagArgument` + `FLAG_PATTERN`
//! from `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts`
//! (lines 1640-1898, upstream commit pinned by Phase 3.2 plan §12).
//!
//! Phase 3.2.A scope: parser primitives only. The 5 external command tables
//! (`GIT_READ_ONLY_COMMANDS`, etc.) land in 3.2.B/C and call into the API
//! exposed here.
//!
//! # Fail-Safe contract
//!
//! - Unknown flag → `false` (reject).
//! - Bundled short flags with any arg-taking member → `false` (parser
//!   differential with GNU getopt; see §SECURITY note in `validate_flags`).
//! - `-FLAG=` (empty inline value) → never consumes the next token
//!   (parser differential with GNU getopt for short options).
//! - String-typed argument starting with `-` → rejected, except the
//!   documented `git --sort -refname` exception.
//! - Tools with `respects_double_dash = false` (e.g. `pyright`) keep
//!   parsing tokens after `--` as flags.
//!
//! # Why no `regex` dependency
//!
//! Upstream uses `FLAG_PATTERN = /^-[a-zA-Z0-9_-]/` — a 4-class check on
//! one character. Doing it inline as [`is_flag_pattern_char`] avoids
//! pulling regex into the hot path and keeps the parser allocation-free.

/// Type of argument a flag accepts.
///
/// Mirrors upstream `FlagArgType` (TS union of string literals). The
/// `Brace` variant corresponds to upstream `'{}'`; `Eof` to upstream
/// `'EOF'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagArgType {
    /// No argument (`--color`, `-n`).
    None,
    /// Integer argument (`--context=3`).
    Number,
    /// Any string argument (`--relative=path`).
    String,
    /// Single character (delimiter).
    Char,
    /// Literal `"{}"` only.
    Brace,
    /// Literal `"EOF"` only.
    Eof,
}

/// Configuration for a single command's safe-flag set.
///
/// Mirrors upstream `ExternalCommandConfig`. The `safe_flags` table is
/// a sorted-or-not slice of `(flag, type)` pairs; lookup goes through
/// [`CommandConfig::lookup_flag`] so 3.2.B/C can later swap the
/// backing storage (phf, hash, sorted) without touching the parser.
#[derive(Debug)]
pub struct CommandConfig {
    /// Safe flag table. `(flag_name, argument_type)`.
    pub safe_flags: &'static [(&'static str, FlagArgType)],
    /// Optional extra dangerous-pattern callback. Returns `true` if
    /// the command is dangerous. Called on the **raw** command string
    /// plus tokens after the command name. Mirrors upstream
    /// `additionalCommandIsDangerousCallback`.
    pub additional_dangerous_callback: Option<fn(raw_command: &str, args: &[&str]) -> bool>,
    /// When `false`, the tool does NOT respect POSIX `--`. Validator
    /// continues parsing flags past `--`. Default upstream = `true`.
    pub respects_double_dash: bool,
}

impl CommandConfig {
    /// Look up a flag's argument type.
    #[inline]
    pub fn lookup_flag(&self, flag: &str) -> Option<FlagArgType> {
        self.safe_flags
            .iter()
            .find_map(|(k, v)| (*k == flag).then_some(*v))
    }
}

/// Options for [`validate_flags`].
#[derive(Debug, Default, Clone, Copy)]
pub struct ValidateOptions<'a> {
    /// Command name (`"git"`, `"grep"`, `"rg"`, `"xargs"`, ...) — drives
    /// command-specific shortcuts (git `-NN`, grep/rg attached numeric,
    /// xargs target-command break).
    pub command_name: Option<&'a str>,
    /// Raw original command string. Only passed to
    /// `additional_dangerous_callback` (currently unused inside
    /// `validate_flags` itself; reserved for future use by 3.2.B/C).
    pub raw_command: Option<&'a str>,
    /// xargs-style target command set. When set together with
    /// `command_name = Some("xargs")`, the first non-flag token (or
    /// the token immediately after `--`) is checked against this list
    /// to enable safe nesting.
    pub xargs_target_commands: Option<&'a [&'a str]>,
}

/// Validate the argument value for a flag of the given type.
///
/// Port of upstream `validateFlagArgument`.
pub fn validate_flag_argument(value: &str, arg_type: FlagArgType) -> bool {
    match arg_type {
        // 'none' should not be called for value validation; mirror
        // upstream's defensive `return false`.
        FlagArgType::None => false,
        FlagArgType::Number => !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        FlagArgType::String => true,
        FlagArgType::Char => value.chars().count() == 1,
        FlagArgType::Brace => value == "{}",
        FlagArgType::Eof => value == "EOF",
    }
}

/// Check whether a token looks like a flag per upstream `FLAG_PATTERN`
/// (`/^-[a-zA-Z0-9_-]/`).
///
/// Returns `true` iff the token has length ≥ 2, starts with `-`, and the
/// second byte is ASCII alphanumeric / underscore / hyphen.
#[inline]
pub fn matches_flag_pattern(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'-' {
        return false;
    }
    let c = bytes[1];
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

#[inline]
fn is_flag_pattern_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

/// Validate the flag portion of a tokenized command against a config.
///
/// Port of upstream `validateFlags`. See module-level docs for the
/// Fail-Safe contract.
pub fn validate_flags(
    tokens: &[&str],
    start_index: usize,
    config: &CommandConfig,
    options: &ValidateOptions<'_>,
) -> bool {
    let _ = options.raw_command; // reserved; see `ValidateOptions::raw_command`
    let n = tokens.len();
    let mut i = start_index;

    while i < n {
        let mut token = tokens[i];
        if token.is_empty() {
            i += 1;
            continue;
        }

        // xargs target-command handling: once we hit a non-flag token
        // (or `--` + next token), check it against the safe target set.
        if let (Some(targets), Some("xargs")) =
            (options.xargs_target_commands, options.command_name)
        {
            let is_non_flag_or_dashdash = !token.starts_with('-') || token == "--";
            if is_non_flag_or_dashdash {
                if token == "--" && i + 1 < n {
                    i += 1;
                    token = tokens[i];
                }
                if !token.is_empty() && targets.contains(&token) {
                    break;
                }
                return false;
            }
        }

        if token == "--" {
            if config.respects_double_dash {
                // i += 1; — upstream advances but loop breaks. Mirror it.
                break;
            }
            // Tool doesn't respect `--`: treat as positional, keep parsing.
            i += 1;
            continue;
        }

        if token.starts_with('-') && token.len() > 1 && matches_flag_pattern(token) {
            // Split on first `=` to mirror upstream `token.includes('=')` +
            // `valueParts.join('=')` (preserves `=` inside the value).
            let (flag, has_equals, inline_value) = match token.find('=') {
                Some(idx) => (&token[..idx], true, &token[idx + 1..]),
                None => (token, false, ""),
            };

            if flag.is_empty() {
                return false;
            }

            let flag_arg_type = config.lookup_flag(flag);

            let Some(flag_arg_type) = flag_arg_type else {
                // git shorthand: `-<number>` ≡ `-n <number>`.
                if options.command_name == Some("git") && is_git_dash_number(flag) {
                    i += 1;
                    continue;
                }

                // grep/rg: attached numeric arg (e.g. `-A20`).
                if matches!(options.command_name, Some("grep") | Some("rg"))
                    && flag.starts_with('-')
                    && !flag.starts_with("--")
                    && flag.len() > 2
                {
                    let potential_flag = &flag[..2];
                    let potential_value = &flag[2..];
                    if let Some(pf_type) = config.lookup_flag(potential_flag) {
                        if potential_value.bytes().all(|b| b.is_ascii_digit())
                            && !potential_value.is_empty()
                            && matches!(pf_type, FlagArgType::Number | FlagArgType::String)
                            && validate_flag_argument(potential_value, pf_type)
                        {
                            i += 1;
                            continue;
                        } else if config.lookup_flag(potential_flag).is_some() {
                            // Attached value present but invalid → reject.
                            // (Only enter this branch when value is all-digit
                            // attempt; otherwise fall through to bundled-flag
                            // logic below.)
                            if !potential_value.is_empty()
                                && potential_value.bytes().all(|b| b.is_ascii_digit())
                            {
                                return false;
                            }
                        }
                    }
                }

                // Combined short flags like `-nr`. SECURITY: every member
                // must be `None`-type to avoid GNU-getopt arg-consuming
                // parser differential.
                if flag.starts_with('-') && !flag.starts_with("--") && flag.len() > 2 {
                    let flag_bytes = flag.as_bytes();
                    for &ch in flag_bytes.iter().skip(1) {
                        // Members of a short bundle must themselves be
                        // valid single-flag characters.
                        if !is_flag_pattern_char(ch) {
                            return false;
                        }
                        let single = [b'-', ch];
                        // SAFETY: ch is ASCII alphanumeric/underscore/hyphen.
                        let single_str = std::str::from_utf8(&single).unwrap_or("");
                        match config.lookup_flag(single_str) {
                            None => return false,
                            Some(FlagArgType::None) => {}
                            Some(_) => return false, // arg-taking in bundle → reject
                        }
                    }
                    i += 1;
                    continue;
                }
                return false;
            };

            // Validate flag arguments.
            if matches!(flag_arg_type, FlagArgType::None) {
                if has_equals {
                    // `-FLAG=` for a no-arg flag is a misuse → reject.
                    return false;
                }
                i += 1;
                continue;
            }

            let arg_value: &str;
            if has_equals {
                // `-E=` must NOT consume next token. Use empty inline_value
                // (will fail validation for EOF/Brace/Char/Number types).
                arg_value = inline_value;
                i += 1;
            } else {
                let next_is_flag = i + 1 < n
                    && !tokens[i + 1].is_empty()
                    && tokens[i + 1].starts_with('-')
                    && tokens[i + 1].len() > 1
                    && matches_flag_pattern(tokens[i + 1]);
                if i + 1 >= n || next_is_flag {
                    return false; // missing required argument
                }
                arg_value = tokens[i + 1];
                i += 2;
            }

            // Defense-in-depth: string-typed arg starting with `-` is
            // suspicious unless it's the documented `git --sort` exception.
            if matches!(flag_arg_type, FlagArgType::String) && arg_value.starts_with('-') {
                let git_sort_exception = flag == "--sort"
                    && options.command_name == Some("git")
                    && is_git_sort_reverse_key(arg_value);
                if !git_sort_exception {
                    return false;
                }
            }

            if !validate_flag_argument(arg_value, flag_arg_type) {
                return false;
            }
        } else {
            // Non-flag positional — allowed.
            i += 1;
        }
    }

    true
}

/// `-<number>` shorthand check for git (`git log -5` ≡ `git log -n 5`).
#[inline]
fn is_git_dash_number(flag: &str) -> bool {
    let bytes = flag.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'-' {
        return false;
    }
    bytes[1..].iter().all(|b| b.is_ascii_digit())
}

/// `git --sort -refname` style reverse key: leading `-` then ASCII letter.
#[inline]
fn is_git_sort_reverse_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0] == b'-' && bytes[1].is_ascii_alphabetic()
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Test fixtures -----------------------------------------------

    const NONE_CFG: CommandConfig = CommandConfig {
        safe_flags: &[("-n", FlagArgType::None), ("--color", FlagArgType::None)],
        additional_dangerous_callback: None,
        respects_double_dash: true,
    };

    // grep-flavoured: numeric arg, attached form allowed.
    const GREP_CFG: CommandConfig = CommandConfig {
        safe_flags: &[
            ("-A", FlagArgType::Number),
            ("-B", FlagArgType::Number),
            ("-r", FlagArgType::None),
            ("-i", FlagArgType::None),
        ],
        additional_dangerous_callback: None,
        respects_double_dash: true,
    };

    // git-flavoured: string + number + Char.
    const GIT_CFG: CommandConfig = CommandConfig {
        safe_flags: &[
            ("-n", FlagArgType::Number),
            ("--sort", FlagArgType::String),
            ("--pretty", FlagArgType::String),
            ("--max-count", FlagArgType::Number),
        ],
        additional_dangerous_callback: None,
        respects_double_dash: true,
    };

    // xargs-flavoured: -I takes Brace literal, others None.
    const XARGS_CFG: CommandConfig = CommandConfig {
        safe_flags: &[
            ("-r", FlagArgType::None),
            ("-I", FlagArgType::Brace),
            ("-E", FlagArgType::Eof),
            ("-d", FlagArgType::Char),
        ],
        additional_dangerous_callback: None,
        respects_double_dash: true,
    };

    // pyright-flavoured: doesn't respect `--`.
    const PYRIGHT_CFG: CommandConfig = CommandConfig {
        safe_flags: &[("--outputjson", FlagArgType::None)],
        additional_dangerous_callback: None,
        respects_double_dash: false,
    };

    fn opts(name: &'static str) -> ValidateOptions<'static> {
        ValidateOptions {
            command_name: Some(name),
            raw_command: None,
            xargs_target_commands: None,
        }
    }

    // ---- validate_flag_argument --------------------------------------

    #[test]
    fn req_bash_validation_320_a_arg_type_none_rejects_all() {
        assert!(!validate_flag_argument("", FlagArgType::None));
        assert!(!validate_flag_argument("anything", FlagArgType::None));
    }

    #[test]
    fn req_bash_validation_320_a_arg_type_number_accepts_digits() {
        assert!(validate_flag_argument("0", FlagArgType::Number));
        assert!(validate_flag_argument("42", FlagArgType::Number));
        assert!(!validate_flag_argument("", FlagArgType::Number));
        assert!(!validate_flag_argument("-1", FlagArgType::Number));
        assert!(!validate_flag_argument("3a", FlagArgType::Number));
    }

    #[test]
    fn req_bash_validation_320_a_arg_type_string_accepts_anything() {
        assert!(validate_flag_argument("", FlagArgType::String));
        assert!(validate_flag_argument("anything goes", FlagArgType::String));
    }

    #[test]
    fn req_bash_validation_320_a_arg_type_char_requires_one() {
        assert!(validate_flag_argument(",", FlagArgType::Char));
        assert!(!validate_flag_argument("", FlagArgType::Char));
        assert!(!validate_flag_argument(",,", FlagArgType::Char));
        // Unicode scalar counts as one char.
        assert!(validate_flag_argument("é", FlagArgType::Char));
    }

    #[test]
    fn req_bash_validation_320_a_arg_type_brace_literal() {
        assert!(validate_flag_argument("{}", FlagArgType::Brace));
        assert!(!validate_flag_argument("{ }", FlagArgType::Brace));
        assert!(!validate_flag_argument("", FlagArgType::Brace));
    }

    #[test]
    fn req_bash_validation_320_a_arg_type_eof_literal() {
        assert!(validate_flag_argument("EOF", FlagArgType::Eof));
        assert!(!validate_flag_argument("eof", FlagArgType::Eof));
    }

    // ---- matches_flag_pattern ----------------------------------------

    #[test]
    fn req_bash_validation_320_a_flag_pattern_basic() {
        assert!(matches_flag_pattern("-n"));
        assert!(matches_flag_pattern("--color"));
        assert!(matches_flag_pattern("-A20"));
        assert!(matches_flag_pattern("-_foo"));
        assert!(!matches_flag_pattern("-"));
        assert!(!matches_flag_pattern("foo"));
        assert!(!matches_flag_pattern(""));
        // `-=` second byte is `=` which is not flag-pattern-char.
        assert!(!matches_flag_pattern("-="));
    }

    // ---- validate_flags: happy paths ---------------------------------

    #[test]
    fn req_bash_validation_320_a_validates_no_flag_args() {
        let tokens = vec!["ls"];
        assert!(validate_flags(&tokens, 1, &NONE_CFG, &opts("ls")));
    }

    #[test]
    fn req_bash_validation_320_a_validates_known_none_flag() {
        let tokens = vec!["ls", "-n", "--color"];
        assert!(validate_flags(&tokens, 1, &NONE_CFG, &opts("ls")));
    }

    #[test]
    fn req_bash_validation_320_a_validates_number_flag_inline() {
        let tokens = vec!["grep", "-A=3"];
        assert!(validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_validates_number_flag_separate() {
        let tokens = vec!["grep", "-A", "3"];
        assert!(validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_validates_attached_numeric_grep() {
        let tokens = vec!["grep", "-A20"];
        assert!(validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_validates_attached_numeric_rg() {
        let tokens = vec!["rg", "-B5"];
        assert!(validate_flags(&tokens, 1, &GREP_CFG, &opts("rg")));
    }

    #[test]
    fn req_bash_validation_320_a_validates_git_dash_number_shorthand() {
        let tokens = vec!["git", "log", "-5"];
        assert!(validate_flags(&tokens, 2, &GIT_CFG, &opts("git")));
    }

    #[test]
    fn req_bash_validation_320_a_validates_combined_short_none_flags() {
        let tokens = vec!["grep", "-ri"];
        assert!(validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_breaks_on_double_dash_when_respected() {
        // After `--`, anything goes (including unknown `--evil`).
        let tokens = vec!["grep", "-r", "--", "--evil"];
        assert!(validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_keeps_parsing_when_double_dash_not_respected() {
        // pyright doesn't respect `--`: `--outputjson` after `--` still
        // gets validated; `--evil` would reject.
        let tokens = vec!["pyright", "--", "--outputjson"];
        assert!(validate_flags(&tokens, 1, &PYRIGHT_CFG, &opts("pyright")));
        let bad = vec!["pyright", "--", "--evil"];
        assert!(!validate_flags(&bad, 1, &PYRIGHT_CFG, &opts("pyright")));
    }

    #[test]
    fn req_bash_validation_320_a_allows_positional_tokens() {
        let tokens = vec!["grep", "-r", "pattern", "src/"];
        assert!(validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_git_sort_reverse_exception_inline() {
        // The reverse-sort exception is only reachable via inline `--sort=...`,
        // because the separate-token form `--sort -refname` is rejected
        // earlier by the next_is_flag check (both upstream and our port).
        let tokens = vec!["git", "for-each-ref", "--sort=-refname"];
        assert!(validate_flags(&tokens, 2, &GIT_CFG, &opts("git")));
    }

    #[test]
    fn req_bash_validation_320_a_git_sort_separate_dash_value_rejected() {
        // Documented upstream behavior: `--sort -refname` (separate tokens)
        // is rejected because `-refname` matches FLAG_PATTERN, triggering
        // the "next token looks like a flag → missing argument" path before
        // the git-sort exception is consulted.
        let tokens = vec!["git", "for-each-ref", "--sort", "-refname"];
        assert!(!validate_flags(&tokens, 2, &GIT_CFG, &opts("git")));
    }

    // ---- validate_flags: failure paths -------------------------------

    #[test]
    fn req_bash_validation_320_a_rejects_unknown_flag() {
        let tokens = vec!["ls", "--evil"];
        assert!(!validate_flags(&tokens, 1, &NONE_CFG, &opts("ls")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_number_flag_with_non_digit() {
        let tokens = vec!["grep", "-A=3a"];
        assert!(!validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
        let tokens2 = vec!["grep", "-A", "abc"];
        assert!(!validate_flags(&tokens2, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_missing_required_arg_at_end() {
        let tokens = vec!["grep", "-A"];
        assert!(!validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_missing_required_arg_followed_by_flag() {
        let tokens = vec!["grep", "-A", "-r"];
        assert!(!validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_none_flag_with_equals() {
        // `-n=` for a no-arg flag is a misuse.
        let tokens = vec!["ls", "-n=foo"];
        assert!(!validate_flags(&tokens, 1, &NONE_CFG, &opts("ls")));
        let tokens2 = vec!["ls", "-n="];
        assert!(!validate_flags(&tokens2, 1, &NONE_CFG, &opts("ls")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_attached_numeric_invalid_value() {
        let tokens = vec!["grep", "-A0a"];
        assert!(!validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_combined_short_with_arg_taking_member() {
        // SECURITY S21 prelude: `-rI` bundled with `-I` (arg-taking, Brace).
        // GNU getopt would consume next token as `-I`'s arg. We reject.
        let tokens = vec!["xargs", "-rI", "echo", "rm", "evil"];
        assert!(!validate_flags(&tokens, 1, &XARGS_CFG, &opts("xargs")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_combined_short_with_unknown_member() {
        let tokens = vec!["grep", "-rZ"]; // -Z unknown
        assert!(!validate_flags(&tokens, 1, &GREP_CFG, &opts("grep")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_string_arg_starting_with_dash() {
        let tokens = vec!["git", "log", "--pretty", "-format-as-flag"];
        assert!(!validate_flags(&tokens, 2, &GIT_CFG, &opts("git")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_eof_inline_empty() {
        // SECURITY: `-E=` must NOT consume next token (parser differential
        // with GNU getopt short options). `validateFlagArgument('', 'EOF')`
        // → false → reject.
        let tokens = vec!["xargs", "-E=", "EOF", "echo", "foo"];
        assert!(!validate_flags(&tokens, 1, &XARGS_CFG, &opts("xargs")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_char_inline_empty() {
        let tokens = vec!["xargs", "-d="];
        assert!(!validate_flags(&tokens, 1, &XARGS_CFG, &opts("xargs")));
    }

    #[test]
    fn req_bash_validation_320_a_rejects_brace_non_literal() {
        let tokens = vec!["xargs", "-I", "X"];
        assert!(!validate_flags(&tokens, 1, &XARGS_CFG, &opts("xargs")));
    }

    // ---- xargs target-command handling -------------------------------

    #[test]
    fn req_bash_validation_320_a_xargs_accepts_safe_target() {
        let tokens = vec!["xargs", "-r", "echo", "foo"];
        let safe_targets: &[&str] = &["echo", "cat"];
        let o = ValidateOptions {
            command_name: Some("xargs"),
            raw_command: None,
            xargs_target_commands: Some(safe_targets),
        };
        assert!(validate_flags(&tokens, 1, &XARGS_CFG, &o));
    }

    #[test]
    fn req_bash_validation_320_a_xargs_rejects_unsafe_target() {
        // SECURITY S21: rm not in safe target set.
        let tokens = vec!["xargs", "-r", "rm", "{}"];
        let safe_targets: &[&str] = &["echo", "cat"];
        let o = ValidateOptions {
            command_name: Some("xargs"),
            raw_command: None,
            xargs_target_commands: Some(safe_targets),
        };
        assert!(!validate_flags(&tokens, 1, &XARGS_CFG, &o));
    }

    #[test]
    fn req_bash_validation_320_a_xargs_target_after_dashdash() {
        let tokens = vec!["xargs", "-r", "--", "echo", "x"];
        let safe_targets: &[&str] = &["echo"];
        let o = ValidateOptions {
            command_name: Some("xargs"),
            raw_command: None,
            xargs_target_commands: Some(safe_targets),
        };
        assert!(validate_flags(&tokens, 1, &XARGS_CFG, &o));
    }

    // ---- start_index sanity ------------------------------------------

    #[test]
    fn req_bash_validation_320_a_handles_empty_token_input() {
        let tokens: Vec<&str> = vec![];
        assert!(validate_flags(&tokens, 0, &NONE_CFG, &opts("ls")));
    }

    #[test]
    fn req_bash_validation_320_a_skips_empty_tokens_in_middle() {
        let tokens = vec!["ls", "", "-n"];
        assert!(validate_flags(&tokens, 1, &NONE_CFG, &opts("ls")));
    }

    // ---- CommandConfig::lookup_flag ----------------------------------

    #[test]
    fn req_bash_validation_320_a_lookup_flag_hits_and_misses() {
        assert_eq!(GREP_CFG.lookup_flag("-A"), Some(FlagArgType::Number));
        assert_eq!(GREP_CFG.lookup_flag("-r"), Some(FlagArgType::None));
        assert_eq!(GREP_CFG.lookup_flag("--missing"), None);
    }
}
