//! Verbatim port of `GH_READ_ONLY_COMMANDS` and `ANT_ONLY_COMMAND_ALLOWLIST`
//! (gh + aki) from `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts`
//! (upstream L944-L1381) and `readOnlyValidation.ts` L1141-L1212.
//!
//! These commands are ant-only because they perform network requests, which
//! violates the read-only validation principle of no network access. The
//! refactored `get_command_allowlist()` in `bash_allowlist.rs` gates inclusion
//! on `USER_TYPE=ant`.
//!
//! All 18 gh commands share `gh_is_dangerous_callback`, which rejects any
//! token containing `://` (URL), `@` (SSH-style), or ≥2 slashes (3+ segments
//! beyond gh's expected `OWNER/REPO` form). Five `gh search …` commands omit
//! the callback because search queries legitimately contain those tokens
//! (matching upstream).

use std::sync::LazyLock;

use super::flag_parser::{CommandConfig, FlagArgType};

// SECURITY: Rejects any positional or flag-value token that looks like:
//   - HOST/OWNER/REPO (3+ slash-separated segments)
//   - Any token with `://` (URL)
//   - Any token with `@` (SSH-style)
// Covers BOTH `--repo VALUE` AND `--repo=VALUE` (cobra accepts both forms).
fn gh_is_dangerous_callback(_raw_command: &str, args: &[&str]) -> bool {
    for token in args {
        if token.is_empty() {
            continue;
        }
        // For flag tokens, extract VALUE after `=` for inspection. Without this
        // `--repo=evil.com/SECRET/x` would be skipped, bypassing the HOST check.
        let value: &str = if token.starts_with('-') {
            match token.find('=') {
                Some(eq_idx) => {
                    let v = &token[eq_idx + 1..];
                    if v.is_empty() {
                        continue;
                    }
                    v
                }
                None => continue, // flag without inline value, nothing to inspect
            }
        } else {
            token
        };
        // Skip values that are clearly not repo specs.
        if !value.contains('/') && !value.contains("://") && !value.contains('@') {
            continue;
        }
        // URL schemes: https://, http://, git://, ssh://
        if value.contains("://") {
            return true;
        }
        // SSH-style: git@host:owner/repo
        if value.contains('@') {
            return true;
        }
        // 3+ segments = HOST/OWNER/REPO (normal gh format is OWNER/REPO, 1 slash)
        let slash_count = value.bytes().filter(|b| *b == b'/').count();
        if slash_count >= 2 {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Per-command CommandConfig constants. 13 commands use the shared
// `gh_is_dangerous_callback`; the 5 `gh search …` variants omit it because
// search queries legitimately include `/`, `@`, and `://` characters.
// ---------------------------------------------------------------------------

// gh pr view — read-only PR details
const GH_PR_VIEW_FLAGS: &[(&str, FlagArgType)] = &[
    ("--json", FlagArgType::String),
    ("--comments", FlagArgType::None),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_PR_VIEW_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_PR_VIEW_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh pr list — read-only PR listing
const GH_PR_LIST_FLAGS: &[(&str, FlagArgType)] = &[
    ("--state", FlagArgType::String),
    ("-s", FlagArgType::String),
    ("--author", FlagArgType::String),
    ("--assignee", FlagArgType::String),
    ("--label", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--base", FlagArgType::String),
    ("--head", FlagArgType::String),
    ("--search", FlagArgType::String),
    ("--json", FlagArgType::String),
    ("--draft", FlagArgType::None),
    ("--app", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_PR_LIST_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_PR_LIST_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh pr diff — read-only PR diff
const GH_PR_DIFF_FLAGS: &[(&str, FlagArgType)] = &[
    ("--color", FlagArgType::String),
    ("--name-only", FlagArgType::None),
    ("--patch", FlagArgType::None),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_PR_DIFF_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_PR_DIFF_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh pr checks — read-only CI status checks
const GH_PR_CHECKS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--watch", FlagArgType::None),
    ("--required", FlagArgType::None),
    ("--fail-fast", FlagArgType::None),
    ("--json", FlagArgType::String),
    ("--interval", FlagArgType::Number),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_PR_CHECKS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_PR_CHECKS_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh issue view — read-only issue details
const GH_ISSUE_VIEW_FLAGS: &[(&str, FlagArgType)] = &[
    ("--json", FlagArgType::String),
    ("--comments", FlagArgType::None),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_ISSUE_VIEW_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_ISSUE_VIEW_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh issue list — read-only issue listing
const GH_ISSUE_LIST_FLAGS: &[(&str, FlagArgType)] = &[
    ("--state", FlagArgType::String),
    ("-s", FlagArgType::String),
    ("--assignee", FlagArgType::String),
    ("--author", FlagArgType::String),
    ("--label", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--milestone", FlagArgType::String),
    ("--search", FlagArgType::String),
    ("--json", FlagArgType::String),
    ("--app", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_ISSUE_LIST_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_ISSUE_LIST_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh repo view — uses positional argument, not --repo/-R flags
const GH_REPO_VIEW_FLAGS: &[(&str, FlagArgType)] = &[("--json", FlagArgType::String)];
const GH_REPO_VIEW_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_REPO_VIEW_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh run list — read-only workflow runs listing
const GH_RUN_LIST_FLAGS: &[(&str, FlagArgType)] = &[
    ("--branch", FlagArgType::String),
    ("-b", FlagArgType::String),
    ("--status", FlagArgType::String),
    ("-s", FlagArgType::String),
    ("--workflow", FlagArgType::String),
    // NOTE: -w is --workflow here, NOT --web (gh run list has no --web)
    ("-w", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--json", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
    ("--event", FlagArgType::String),
    ("-e", FlagArgType::String),
    ("--user", FlagArgType::String),
    ("-u", FlagArgType::String),
    ("--created", FlagArgType::String),
    ("--commit", FlagArgType::String),
    ("-c", FlagArgType::String),
];
const GH_RUN_LIST_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_RUN_LIST_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh run view — read-only workflow run details
const GH_RUN_VIEW_FLAGS: &[(&str, FlagArgType)] = &[
    ("--log", FlagArgType::None),
    ("--log-failed", FlagArgType::None),
    ("--exit-status", FlagArgType::None),
    ("--verbose", FlagArgType::None),
    // NOTE: -v is --verbose here, NOT --web
    ("-v", FlagArgType::None),
    ("--json", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
    ("--job", FlagArgType::String),
    ("-j", FlagArgType::String),
    ("--attempt", FlagArgType::Number),
    ("-a", FlagArgType::Number),
];
const GH_RUN_VIEW_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_RUN_VIEW_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh auth status — read-only auth state. --show-token/-t INTENTIONALLY excluded
// (leaks secrets).
const GH_AUTH_STATUS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--active", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--hostname", FlagArgType::String),
    ("-h", FlagArgType::String),
    ("--json", FlagArgType::String),
];
const GH_AUTH_STATUS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_AUTH_STATUS_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh pr status — read-only PR status overview
const GH_PR_STATUS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--conflict-status", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--json", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_PR_STATUS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_PR_STATUS_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh issue status — read-only issue status overview
const GH_ISSUE_STATUS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--json", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_ISSUE_STATUS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_ISSUE_STATUS_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh release list — read-only release listing
const GH_RELEASE_LIST_FLAGS: &[(&str, FlagArgType)] = &[
    ("--exclude-drafts", FlagArgType::None),
    ("--exclude-pre-releases", FlagArgType::None),
    ("--json", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--order", FlagArgType::String),
    ("-O", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_RELEASE_LIST_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_RELEASE_LIST_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh release view — read-only release details. --web/-w INTENTIONALLY excluded
// (opens browser).
const GH_RELEASE_VIEW_FLAGS: &[(&str, FlagArgType)] = &[
    ("--json", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_RELEASE_VIEW_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_RELEASE_VIEW_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh workflow list — read-only workflow file listing
const GH_WORKFLOW_LIST_FLAGS: &[(&str, FlagArgType)] = &[
    ("--all", FlagArgType::None),
    ("-a", FlagArgType::None),
    ("--json", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_WORKFLOW_LIST_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_WORKFLOW_LIST_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh workflow view — read-only workflow summary. --web/-w INTENTIONALLY excluded
// (opens browser).
const GH_WORKFLOW_VIEW_FLAGS: &[(&str, FlagArgType)] = &[
    ("--ref", FlagArgType::String),
    ("-r", FlagArgType::String),
    ("--yaml", FlagArgType::None),
    ("-y", FlagArgType::None),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_WORKFLOW_VIEW_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_WORKFLOW_VIEW_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh label list — read-only label listing. --web/-w INTENTIONALLY excluded
// (opens browser).
const GH_LABEL_LIST_FLAGS: &[(&str, FlagArgType)] = &[
    ("--json", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--order", FlagArgType::String),
    ("--search", FlagArgType::String),
    ("-S", FlagArgType::String),
    ("--sort", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
];
const GH_LABEL_LIST_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_LABEL_LIST_FLAGS,
    additional_dangerous_callback: Some(gh_is_dangerous_callback),
    respects_double_dash: true,
};

// gh search repos — search repositories. NO callback because search queries
// legitimately contain `/`, `@`, `://`. --web/-w INTENTIONALLY excluded.
const GH_SEARCH_REPOS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--archived", FlagArgType::None),
    ("--created", FlagArgType::String),
    ("--followers", FlagArgType::String),
    ("--forks", FlagArgType::String),
    ("--good-first-issues", FlagArgType::String),
    ("--help-wanted-issues", FlagArgType::String),
    ("--include-forks", FlagArgType::String),
    ("--json", FlagArgType::String),
    ("--language", FlagArgType::String),
    ("--license", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--match", FlagArgType::String),
    ("--number-topics", FlagArgType::String),
    ("--order", FlagArgType::String),
    ("--owner", FlagArgType::String),
    ("--size", FlagArgType::String),
    ("--sort", FlagArgType::String),
    ("--stars", FlagArgType::String),
    ("--topic", FlagArgType::String),
    ("--updated", FlagArgType::String),
    ("--visibility", FlagArgType::String),
];
const GH_SEARCH_REPOS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_SEARCH_REPOS_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// gh search issues — search issues. NO callback. --web/-w INTENTIONALLY excluded.
const GH_SEARCH_ISSUES_FLAGS: &[(&str, FlagArgType)] = &[
    ("--app", FlagArgType::String),
    ("--assignee", FlagArgType::String),
    ("--author", FlagArgType::String),
    ("--closed", FlagArgType::String),
    ("--commenter", FlagArgType::String),
    ("--comments", FlagArgType::String),
    ("--created", FlagArgType::String),
    ("--include-prs", FlagArgType::None),
    ("--interactions", FlagArgType::String),
    ("--involves", FlagArgType::String),
    ("--json", FlagArgType::String),
    ("--label", FlagArgType::String),
    ("--language", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--locked", FlagArgType::None),
    ("--match", FlagArgType::String),
    ("--mentions", FlagArgType::String),
    ("--milestone", FlagArgType::String),
    ("--no-assignee", FlagArgType::None),
    ("--no-label", FlagArgType::None),
    ("--no-milestone", FlagArgType::None),
    ("--no-project", FlagArgType::None),
    ("--order", FlagArgType::String),
    ("--owner", FlagArgType::String),
    ("--project", FlagArgType::String),
    ("--reactions", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
    ("--sort", FlagArgType::String),
    ("--state", FlagArgType::String),
    ("--team-mentions", FlagArgType::String),
    ("--updated", FlagArgType::String),
    ("--visibility", FlagArgType::String),
];
const GH_SEARCH_ISSUES_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_SEARCH_ISSUES_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// gh search prs — search pull requests. NO callback. --web/-w INTENTIONALLY excluded.
const GH_SEARCH_PRS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--app", FlagArgType::String),
    ("--assignee", FlagArgType::String),
    ("--author", FlagArgType::String),
    ("--base", FlagArgType::String),
    ("-B", FlagArgType::String),
    ("--checks", FlagArgType::String),
    ("--closed", FlagArgType::String),
    ("--commenter", FlagArgType::String),
    ("--comments", FlagArgType::String),
    ("--created", FlagArgType::String),
    ("--draft", FlagArgType::None),
    ("--head", FlagArgType::String),
    ("-H", FlagArgType::String),
    ("--interactions", FlagArgType::String),
    ("--involves", FlagArgType::String),
    ("--json", FlagArgType::String),
    ("--label", FlagArgType::String),
    ("--language", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--locked", FlagArgType::None),
    ("--match", FlagArgType::String),
    ("--mentions", FlagArgType::String),
    ("--merged", FlagArgType::None),
    ("--merged-at", FlagArgType::String),
    ("--milestone", FlagArgType::String),
    ("--no-assignee", FlagArgType::None),
    ("--no-label", FlagArgType::None),
    ("--no-milestone", FlagArgType::None),
    ("--no-project", FlagArgType::None),
    ("--order", FlagArgType::String),
    ("--owner", FlagArgType::String),
    ("--project", FlagArgType::String),
    ("--reactions", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
    ("--review", FlagArgType::String),
    ("--review-requested", FlagArgType::String),
    ("--reviewed-by", FlagArgType::String),
    ("--sort", FlagArgType::String),
    ("--state", FlagArgType::String),
    ("--team-mentions", FlagArgType::String),
    ("--updated", FlagArgType::String),
    ("--visibility", FlagArgType::String),
];
const GH_SEARCH_PRS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_SEARCH_PRS_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// gh search commits — search commits. NO callback. --web/-w INTENTIONALLY excluded.
const GH_SEARCH_COMMITS_FLAGS: &[(&str, FlagArgType)] = &[
    ("--author", FlagArgType::String),
    ("--author-date", FlagArgType::String),
    ("--author-email", FlagArgType::String),
    ("--author-name", FlagArgType::String),
    ("--committer", FlagArgType::String),
    ("--committer-date", FlagArgType::String),
    ("--committer-email", FlagArgType::String),
    ("--committer-name", FlagArgType::String),
    ("--hash", FlagArgType::String),
    ("--json", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--merge", FlagArgType::None),
    ("--order", FlagArgType::String),
    ("--owner", FlagArgType::String),
    ("--parent", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
    ("--sort", FlagArgType::String),
    ("--tree", FlagArgType::String),
    ("--visibility", FlagArgType::String),
];
const GH_SEARCH_COMMITS_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_SEARCH_COMMITS_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// gh search code — search code. NO callback. --web/-w INTENTIONALLY excluded.
const GH_SEARCH_CODE_FLAGS: &[(&str, FlagArgType)] = &[
    ("--extension", FlagArgType::String),
    ("--filename", FlagArgType::String),
    ("--json", FlagArgType::String),
    ("--language", FlagArgType::String),
    ("--limit", FlagArgType::Number),
    ("-L", FlagArgType::Number),
    ("--match", FlagArgType::String),
    ("--owner", FlagArgType::String),
    ("--repo", FlagArgType::String),
    ("-R", FlagArgType::String),
    ("--size", FlagArgType::String),
];
const GH_SEARCH_CODE_CONFIG: CommandConfig = CommandConfig {
    safe_flags: GH_SEARCH_CODE_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---------------------------------------------------------------------------
// aki — Anthropic internal knowledge-base search CLI. Network read-only (same
// policy as gh). --audit-csv INTENTIONALLY omitted (writes to disk).
// ---------------------------------------------------------------------------
const AKI_FLAGS: &[(&str, FlagArgType)] = &[
    ("-h", FlagArgType::None),
    ("--help", FlagArgType::None),
    ("-k", FlagArgType::None),
    ("--keyword", FlagArgType::None),
    ("-s", FlagArgType::None),
    ("--semantic", FlagArgType::None),
    ("--no-adaptive", FlagArgType::None),
    ("-n", FlagArgType::Number),
    ("--limit", FlagArgType::Number),
    ("-o", FlagArgType::Number),
    ("--offset", FlagArgType::Number),
    ("--source", FlagArgType::String),
    ("--exclude-source", FlagArgType::String),
    ("-a", FlagArgType::String),
    ("--after", FlagArgType::String),
    ("-b", FlagArgType::String),
    ("--before", FlagArgType::String),
    ("--collection", FlagArgType::String),
    ("--drive", FlagArgType::String),
    ("--folder", FlagArgType::String),
    ("--descendants", FlagArgType::None),
    ("-m", FlagArgType::String),
    ("--meta", FlagArgType::String),
    ("-t", FlagArgType::String),
    ("--threshold", FlagArgType::String),
    ("--kw-weight", FlagArgType::String),
    ("--sem-weight", FlagArgType::String),
    ("-j", FlagArgType::None),
    ("--json", FlagArgType::None),
    ("-c", FlagArgType::None),
    ("--chunk", FlagArgType::None),
    ("--preview", FlagArgType::None),
    ("-d", FlagArgType::None),
    ("--full-doc", FlagArgType::None),
    ("-v", FlagArgType::None),
    ("--verbose", FlagArgType::None),
    ("--stats", FlagArgType::None),
    ("-S", FlagArgType::Number),
    ("--summarize", FlagArgType::Number),
    ("--explain", FlagArgType::None),
    ("--examine", FlagArgType::String),
    ("--url", FlagArgType::String),
    ("--multi-turn", FlagArgType::Number),
    ("--multi-turn-model", FlagArgType::String),
    ("--multi-turn-context", FlagArgType::String),
    ("--no-rerank", FlagArgType::None),
    ("--audit", FlagArgType::None),
    ("--local", FlagArgType::None),
    ("--staging", FlagArgType::None),
];
const AKI_CONFIG: CommandConfig = CommandConfig {
    safe_flags: AKI_FLAGS,
    additional_dangerous_callback: None,
    respects_double_dash: true,
};

// ---------------------------------------------------------------------------
// GH_READ_ONLY_COMMANDS — 22 gh commands in upstream object literal order.
// Longest-prefix discipline: multi-word entries (`gh pr view`, `gh pr list`,
// `gh pr diff`, `gh pr checks`, `gh pr status`) must precede any hypothetical
// shorter `gh pr` entry; current set has none.
//
// SECURITY: All `gh search …` entries are AFTER the matching `gh issue …` /
// `gh pr …` entries; since first-match-wins on tokens, this is safe because
// `gh search issues` and `gh issue list` differ in their second token.
// ---------------------------------------------------------------------------
pub(crate) static GH_READ_ONLY_COMMANDS: LazyLock<Vec<(&'static str, &'static CommandConfig)>> =
    LazyLock::new(|| {
        vec![
            ("gh pr view", &GH_PR_VIEW_CONFIG),
            ("gh pr list", &GH_PR_LIST_CONFIG),
            ("gh pr diff", &GH_PR_DIFF_CONFIG),
            ("gh pr checks", &GH_PR_CHECKS_CONFIG),
            ("gh issue view", &GH_ISSUE_VIEW_CONFIG),
            ("gh issue list", &GH_ISSUE_LIST_CONFIG),
            ("gh repo view", &GH_REPO_VIEW_CONFIG),
            ("gh run list", &GH_RUN_LIST_CONFIG),
            ("gh run view", &GH_RUN_VIEW_CONFIG),
            ("gh auth status", &GH_AUTH_STATUS_CONFIG),
            ("gh pr status", &GH_PR_STATUS_CONFIG),
            ("gh issue status", &GH_ISSUE_STATUS_CONFIG),
            ("gh release list", &GH_RELEASE_LIST_CONFIG),
            ("gh release view", &GH_RELEASE_VIEW_CONFIG),
            ("gh workflow list", &GH_WORKFLOW_LIST_CONFIG),
            ("gh workflow view", &GH_WORKFLOW_VIEW_CONFIG),
            ("gh label list", &GH_LABEL_LIST_CONFIG),
            ("gh search repos", &GH_SEARCH_REPOS_CONFIG),
            ("gh search issues", &GH_SEARCH_ISSUES_CONFIG),
            ("gh search prs", &GH_SEARCH_PRS_CONFIG),
            ("gh search commits", &GH_SEARCH_COMMITS_CONFIG),
            ("gh search code", &GH_SEARCH_CODE_CONFIG),
        ]
    });

// ---------------------------------------------------------------------------
// ANT_ONLY_COMMANDS — gh + aki. Gated on USER_TYPE=ant in
// `bash_allowlist::get_command_allowlist()` (matches upstream L1201-L1208).
// ---------------------------------------------------------------------------
pub(crate) static ANT_ONLY_COMMANDS: LazyLock<Vec<(&'static str, &'static CommandConfig)>> =
    LazyLock::new(|| {
        let mut out: Vec<(&'static str, &'static CommandConfig)> = GH_READ_ONLY_COMMANDS.clone();
        out.push(("aki", &AKI_CONFIG));
        out
    });
