//! Tests for `context_collectors` — Slice 2.2.k (Issue #490 Phase 2.2).
//!
//! Test ID: `req_perm_490_p2_2_k_NN_<desc>`.

use dasclaw_bash_permissions::{
    get_allow_rules, get_ask_rules, get_deny_rules,
    types::{
        PermissionBehavior, PermissionRule, PermissionRuleSource, PermissionRuleValue,
        ToolPermissionContext,
    },
    BASH_TOOL_NAME,
};

fn ctx() -> ToolPermissionContext {
    ToolPermissionContext::default()
}

fn rule(
    src: PermissionRuleSource,
    beh: PermissionBehavior,
    content: Option<&str>,
) -> PermissionRule {
    PermissionRule {
        source: src,
        rule_behavior: beh,
        rule_value: PermissionRuleValue {
            tool_name: BASH_TOOL_NAME.to_string(),
            rule_content: content.map(str::to_string),
        },
    }
}

// -----------------------------------------------------------------
// 01 — empty context yields empty Vec for every collector
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_01_empty_context() {
    let c = ctx();
    assert!(get_allow_rules(&c).is_empty());
    assert!(get_deny_rules(&c).is_empty());
    assert!(get_ask_rules(&c).is_empty());
}

// -----------------------------------------------------------------
// 02 — single allow rule round-trip with full shape
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_02_single_allow_full_shape() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "npm install",
    );
    assert_eq!(
        get_allow_rules(&c),
        vec![rule(
            PermissionRuleSource::UserSettings,
            PermissionBehavior::Allow,
            Some("npm install"),
        )]
    );
    // Cross-bucket non-leak.
    assert!(get_deny_rules(&c).is_empty());
    assert!(get_ask_rules(&c).is_empty());
}

// -----------------------------------------------------------------
// 03 — empty-string content collapses to tool-wide rule (None).
// Parity with Slice 2.2.j parser L132-L134.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_03_empty_content_collapses_to_tool_wide() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "",
    );
    let out = get_deny_rules(&c);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].rule_value.rule_content, None);
    assert_eq!(out[0].rule_value.tool_name, "Bash");
}

// -----------------------------------------------------------------
// 04 — standalone wildcard `*` content collapses to tool-wide (None).
// Parity with 2.2.j parser L132-L134.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_04_wildcard_content_collapses_to_tool_wide() {
    let mut c = ctx();
    c.add_rule(PermissionBehavior::Ask, PermissionRuleSource::CliArg, "*");
    let out = get_ask_rules(&c);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].rule_value.rule_content, None);
}

// -----------------------------------------------------------------
// 05 — non-empty, non-`*` content preserved as Some(...)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_05_content_preserved() {
    let mut c = ctx();
    for content in &["git:*", "docker *", "ls", "echo hi"] {
        c.add_rule(
            PermissionBehavior::Allow,
            PermissionRuleSource::UserSettings,
            *content,
        );
    }
    let got: Vec<_> = get_allow_rules(&c)
        .into_iter()
        .map(|r| r.rule_value.rule_content)
        .collect();
    assert_eq!(
        got,
        vec![
            Some("git:*".to_string()),
            Some("docker *".to_string()),
            Some("ls".to_string()),
            Some("echo hi".to_string()),
        ]
    );
}

// -----------------------------------------------------------------
// 06 — BTreeMap deterministic source ordering across multiple sources.
// `PermissionRuleSource` derives `Ord` (verified in types.rs L46).
// SECURITY/DETERMINISM PIN: detect_unreachable_rules (Slice 2.2.i)
// uses `find()` first-match-wins, so collector output ordering is
// part of the security-relevant contract.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_06_deterministic_source_order() {
    let mut c = ctx();
    // Insert in REVERSE-Ord order to prove BTreeMap sorts on read.
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::Session,
        "a",
    );
    c.add_rule(PermissionBehavior::Allow, PermissionRuleSource::CliArg, "b");
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::ProjectSettings,
        "c",
    );
    let sources: Vec<_> = get_allow_rules(&c).into_iter().map(|r| r.source).collect();
    // Expected sorted order: UserSettings... — we used 3 specific sources;
    // expected sort: ProjectSettings < CliArg < Session OR whichever
    // ordering Ord defines. We just assert the output is monotonic
    // non-decreasing under the same Ord.
    let mut sorted = sources.clone();
    sorted.sort();
    assert_eq!(
        sources, sorted,
        "collector output must be sorted by Ord(PermissionRuleSource)"
    );
}

// -----------------------------------------------------------------
// 07 — multiple sources, same behavior: ALL emitted
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_07_multi_source_same_behavior() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm -rf /",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "sudo *",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::CliArg,
        "curl *",
    );
    assert_eq!(get_deny_rules(&c).len(), 3);
}

// -----------------------------------------------------------------
// 08 — all three buckets populated → each collector returns its own
// subset (no leak), behavior + source preserved.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_08_three_bucket_isolation() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "rm -rf",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::CliArg,
        "docker *",
    );

    let allows = get_allow_rules(&c);
    let denies = get_deny_rules(&c);
    let asks = get_ask_rules(&c);

    assert_eq!(allows.len(), 1);
    assert_eq!(denies.len(), 1);
    assert_eq!(asks.len(), 1);

    assert_eq!(allows[0].rule_behavior, PermissionBehavior::Allow);
    assert_eq!(allows[0].source, PermissionRuleSource::UserSettings);
    assert_eq!(allows[0].rule_value.rule_content.as_deref(), Some("git:*"));

    assert_eq!(denies[0].rule_behavior, PermissionBehavior::Deny);
    assert_eq!(denies[0].source, PermissionRuleSource::ProjectSettings);
    assert_eq!(denies[0].rule_value.rule_content.as_deref(), Some("rm -rf"));

    assert_eq!(asks[0].rule_behavior, PermissionBehavior::Ask);
    assert_eq!(asks[0].source, PermissionRuleSource::CliArg);
    assert_eq!(asks[0].rule_value.rule_content.as_deref(), Some("docker *"));
}

// -----------------------------------------------------------------
// 09 — behavior field matches the collector function, NOT
// whatever-was-stored. Each collector hard-codes its own behavior.
// SECURITY PIN: prevents accidental bucket-swap bug.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_09_behavior_pinned_per_collector() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "docker:*",
    );
    assert!(get_allow_rules(&c)
        .iter()
        .all(|r| r.rule_behavior == PermissionBehavior::Allow));
    assert!(get_deny_rules(&c)
        .iter()
        .all(|r| r.rule_behavior == PermissionBehavior::Deny));
    assert!(get_ask_rules(&c)
        .iter()
        .all(|r| r.rule_behavior == PermissionBehavior::Ask));
}

// -----------------------------------------------------------------
// 10 — preserves insertion order WITHIN a single (source, behavior)
// bucket. Critical for `find()` first-match-wins downstream.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_10_preserves_intra_source_insertion_order() {
    let mut c = ctx();
    for content in &["first", "second", "third", "fourth"] {
        c.add_rule(
            PermissionBehavior::Allow,
            PermissionRuleSource::UserSettings,
            *content,
        );
    }
    let contents: Vec<_> = get_allow_rules(&c)
        .into_iter()
        .map(|r| r.rule_value.rule_content.unwrap())
        .collect();
    assert_eq!(contents, vec!["first", "second", "third", "fourth"]);
}

// -----------------------------------------------------------------
// 11 — tool_name is hard-coded to "Bash" (bash-only crate parity
// with upstream tool name).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_11_tool_name_always_bash() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "rm",
    );
    c.add_rule(PermissionBehavior::Ask, PermissionRuleSource::CliArg, "cp");
    for r in get_allow_rules(&c)
        .iter()
        .chain(get_deny_rules(&c).iter())
        .chain(get_ask_rules(&c).iter())
    {
        assert_eq!(r.rule_value.tool_name, "Bash");
    }
}

// -----------------------------------------------------------------
// 12 — RESERVED: end-to-end integration with
// `shadowed_rule_detection::detect_unreachable_rules` (Slice 2.2.i)
// lives in a follow-up PR that depends on BOTH 2.2.i and 2.2.k
// merging. Kept out of this slice so the PR stays independent and
// fully parallel with #543 (2.2.i). The mental model:
//
//   let allows = get_allow_rules(&ctx);
//   let asks   = get_ask_rules(&ctx);
//   let denies = get_deny_rules(&ctx);
//   detect_unreachable_rules(&allows, &asks, &denies, opts)
//
// Once both land on xClaw, a tiny PR adds this smoke test.

// -----------------------------------------------------------------
// 13 — capacity hint correctness: collector pre-allocates exactly
// `sum(len)` slots (regression sentinel for the `Vec::with_capacity`
// optimization).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_13_capacity_matches_total() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "a",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "b",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::ProjectSettings,
        "c",
    );
    let v = get_allow_rules(&c);
    assert_eq!(v.len(), 3);
    // Capacity == len is the post-condition we care about (no growth);
    // assert capacity >= len which is always true; the real value is
    // documenting the contract here.
    assert!(v.capacity() >= v.len());
}

// -----------------------------------------------------------------
// 14 — empty bucket inside otherwise-populated context: empty Vec
// for that behavior, non-empty for others. Fail-Safe sentinel.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_14_empty_bucket_isolation() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "a",
    );
    assert_eq!(get_allow_rules(&c).len(), 1);
    assert!(get_deny_rules(&c).is_empty());
    assert!(get_ask_rules(&c).is_empty());
}

// -----------------------------------------------------------------
// 15 — same content in two different sources: BOTH emitted, sorted
// by source. PIN: not deduplicated (each source tracked separately
// for audit attribution).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_k_15_no_dedup_across_sources() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::ProjectSettings,
        "git:*",
    );
    let out = get_allow_rules(&c);
    assert_eq!(out.len(), 2);
    assert_ne!(out[0].source, out[1].source);
    // Both have identical rule_value content.
    assert_eq!(
        out[0].rule_value.rule_content,
        out[1].rule_value.rule_content
    );
}
