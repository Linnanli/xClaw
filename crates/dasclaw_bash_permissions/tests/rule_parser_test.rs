//! Tests for `rule_parser` — Slice 2.2.j (Issue #490 Phase 2.2).
//!
//! Semantic parity with upstream `permissionRuleParser.ts` L1-L198.
//! Test ID: `req_perm_490_p2_2_j_NN_<desc>`.

use dasclaw_bash_permissions::{
    escape_rule_content, get_legacy_tool_names, normalize_legacy_tool_name,
    permission_rule_value_from_string, permission_rule_value_to_string, unescape_rule_content,
};

// -----------------------------------------------------------------
// 01 — legacy tool name aliases (4 mappings, NOT Brief which is ANT-only)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_01_normalize_legacy_4_aliases() {
    assert_eq!(normalize_legacy_tool_name("Task"), "Agent");
    assert_eq!(normalize_legacy_tool_name("KillShell"), "TaskStop");
    assert_eq!(normalize_legacy_tool_name("AgentOutputTool"), "TaskOutput");
    assert_eq!(normalize_legacy_tool_name("BashOutputTool"), "TaskOutput");
}

// -----------------------------------------------------------------
// 02 — non-legacy names returned unchanged
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_02_non_legacy_passthrough() {
    assert_eq!(normalize_legacy_tool_name("Bash"), "Bash");
    assert_eq!(normalize_legacy_tool_name("Agent"), "Agent");
    assert_eq!(normalize_legacy_tool_name(""), "");
    assert_eq!(normalize_legacy_tool_name("UnknownTool"), "UnknownTool");
}

// -----------------------------------------------------------------
// 03 — Brief ANT-only regression PIN: must NOT be aliased.
// (Upstream L26-L28 wires `Brief` under feature('KAIROS') —
// Issue #490 red line forbids porting ANT-only segments.)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_03_brief_ant_only_not_aliased() {
    // `Brief` is not a legacy alias in our port. It must passthrough
    // unchanged. If a future commit accidentally restores the
    // ANT-only mapping `Brief -> "BriefTool"` this test will fire.
    assert_eq!(normalize_legacy_tool_name("Brief"), "Brief");
    // And no canonical name should claim "Brief" as one of its legacies.
    assert!(get_legacy_tool_names("BriefTool").is_empty());
    assert!(get_legacy_tool_names("Brief").is_empty());
}

// -----------------------------------------------------------------
// 04 — reverse lookup: get_legacy_tool_names
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_04_get_legacy_tool_names_reverse() {
    assert_eq!(get_legacy_tool_names("Agent"), vec!["Task"]);
    assert_eq!(get_legacy_tool_names("TaskStop"), vec!["KillShell"]);
    // TaskOutput has two legacy aliases — order must be stable (decl order).
    assert_eq!(
        get_legacy_tool_names("TaskOutput"),
        vec!["AgentOutputTool", "BashOutputTool"]
    );
    assert!(get_legacy_tool_names("Bash").is_empty());
    assert!(get_legacy_tool_names("UnknownTool").is_empty());
}

// -----------------------------------------------------------------
// 05 — escape: round-trip identity for content without specials
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_05_escape_passthrough_no_specials() {
    assert_eq!(escape_rule_content(""), "");
    assert_eq!(escape_rule_content("npm install"), "npm install");
    assert_eq!(escape_rule_content("git status"), "git status");
    assert_eq!(escape_rule_content("foo:*"), "foo:*");
}

// -----------------------------------------------------------------
// 06 — escape parentheses
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_06_escape_parens() {
    // Upstream L60 doctest: `escapeRuleContent('psycopg2.connect()') -> 'psycopg2.connect\\(\\)'`
    assert_eq!(
        escape_rule_content("psycopg2.connect()"),
        r"psycopg2.connect\(\)"
    );
    assert_eq!(escape_rule_content("("), r"\(");
    assert_eq!(escape_rule_content(")"), r"\)");
    assert_eq!(escape_rule_content("(a)(b)"), r"\(a\)\(b\)");
}

// -----------------------------------------------------------------
// 07 — escape order: backslashes first, then parens. SECURITY PIN.
// If order is reversed (parens-first), `(` -> `\(` -> `\\(` and we'd
// lose injection-safety: an attacker-supplied `(` would round-trip to
// a *literal* `\(` then unescape to `\` then `(` (broken).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_07_escape_order_backslash_first() {
    // Upstream L61: `escapeRuleContent('echo "test\\nvalue"') -> 'echo "test\\\\nvalue"'`
    // (single backslash → double backslash).
    assert_eq!(
        escape_rule_content(r"echo test\nvalue"),
        r"echo test\\nvalue"
    );
    // The combined case that breaks if order is wrong:
    //   input: `\(`  (literal backslash + literal paren)
    //   correct: backslash→`\\`, then `(`→`\(`  ⇒ `\\\(`
    //   wrong:   `(`→`\(` first, then both backslashes→`\\` ⇒ `\\\\(`  ← breaks!
    assert_eq!(escape_rule_content(r"\("), r"\\\(");
}

// -----------------------------------------------------------------
// 08 — unescape inverse of escape
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_08_unescape_inverse() {
    assert_eq!(unescape_rule_content(r"\("), "(");
    assert_eq!(unescape_rule_content(r"\)"), ")");
    assert_eq!(unescape_rule_content(r"\\"), r"\");
    assert_eq!(
        unescape_rule_content(r"psycopg2.connect\(\)"),
        "psycopg2.connect()"
    );
    assert_eq!(
        unescape_rule_content(r"echo test\\nvalue"),
        r"echo test\nvalue"
    );
}

// -----------------------------------------------------------------
// 09 — escape/unescape round trip on a corpus.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_09_round_trip_corpus() {
    for input in &[
        "",
        "npm install",
        "git status",
        "python -c \"print(1)\"",
        "echo (",
        "echo )",
        "echo )(",
        r"a\b",
        r"a\(b",
        r"\\\(\)",
        "α∑→λ",           // non-ASCII passthrough
        "echo (a (b) c)", // nested parens
    ] {
        let escaped = escape_rule_content(input);
        let restored = unescape_rule_content(&escaped);
        assert_eq!(
            &restored, input,
            "round-trip failed for {input:?} (escaped: {escaped:?})"
        );
    }
}

// -----------------------------------------------------------------
// 10 — from_string: bare tool name (no parens)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_10_from_string_bare_tool() {
    assert_eq!(
        permission_rule_value_from_string("Bash"),
        ("Bash".to_string(), None)
    );
    assert_eq!(
        permission_rule_value_from_string("Read"),
        ("Read".to_string(), None)
    );
}

// -----------------------------------------------------------------
// 11 — from_string: tool with content
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_11_from_string_with_content() {
    assert_eq!(
        permission_rule_value_from_string("Bash(npm install)"),
        ("Bash".to_string(), Some("npm install".to_string()))
    );
    assert_eq!(
        permission_rule_value_from_string("Bash(git:*)"),
        ("Bash".to_string(), Some("git:*".to_string()))
    );
}

// -----------------------------------------------------------------
// 12 — from_string: escaped parens in content (upstream L92 doctest)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_12_from_string_escaped_parens() {
    assert_eq!(
        permission_rule_value_from_string(r"Bash(python -c \(1\))"),
        ("Bash".to_string(), Some("python -c (1)".to_string()))
    );
}

// -----------------------------------------------------------------
// 13 — from_string Fail-Safe: empty content collapses to tool-wide rule.
// Upstream L132-L134. Bash() and Bash(*) both mean "any Bash invocation".
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_13_from_string_empty_and_wildcard_collapse() {
    assert_eq!(
        permission_rule_value_from_string("Bash()"),
        ("Bash".to_string(), None)
    );
    assert_eq!(
        permission_rule_value_from_string("Bash(*)"),
        ("Bash".to_string(), None)
    );
}

// -----------------------------------------------------------------
// 14 — from_string Fail-Safe: malformed inputs treated as tool name.
// SECURITY PIN: never partial-parses, never panics. Upstream L99-L120.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_14_from_string_malformed_fail_safe() {
    // No closing paren — treat whole string as tool name.
    let (n, c) = permission_rule_value_from_string("Bash(npm");
    assert_eq!(n, "Bash(npm");
    assert_eq!(c, None);
    // Closing paren before opening (impossible match — find_last after
    // find_first finds same/earlier index).
    let (n, c) = permission_rule_value_from_string("Bash)npm(");
    assert_eq!(n, "Bash)npm(");
    assert_eq!(c, None);
    // Content after closing paren — treat as tool name.
    let (n, c) = permission_rule_value_from_string("Bash(npm)extra");
    assert_eq!(n, "Bash(npm)extra");
    assert_eq!(c, None);
    // Empty tool name — treat whole string as tool name.
    let (n, c) = permission_rule_value_from_string("(foo)");
    assert_eq!(n, "(foo)");
    assert_eq!(c, None);
}

// -----------------------------------------------------------------
// 15 — from_string applies legacy alias resolution.
// Upstream L102+L114+L130+L134 all pipe through normalizeLegacyToolName.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_15_from_string_normalizes_legacy() {
    // Bare legacy name.
    assert_eq!(
        permission_rule_value_from_string("Task"),
        ("Agent".to_string(), None)
    );
    // Legacy name with content.
    assert_eq!(
        permission_rule_value_from_string("Task(do something)"),
        ("Agent".to_string(), Some("do something".to_string()))
    );
    // Empty-content collapse + legacy.
    assert_eq!(
        permission_rule_value_from_string("KillShell()"),
        ("TaskStop".to_string(), None)
    );
    // Malformed legacy passes through normalize on the whole string.
    assert_eq!(
        permission_rule_value_from_string("Task(npm"),
        ("Task(npm".to_string(), None) // whole string is NOT a legacy alias
    );
}

// -----------------------------------------------------------------
// 16 — to_string: bare and content forms
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_16_to_string_basic() {
    assert_eq!(permission_rule_value_to_string("Bash", None), "Bash");
    assert_eq!(permission_rule_value_to_string("Bash", Some("")), "Bash"); // empty == bare
    assert_eq!(
        permission_rule_value_to_string("Bash", Some("npm install")),
        "Bash(npm install)"
    );
}

// -----------------------------------------------------------------
// 17 — to_string: escapes parens in content (upstream L144 doctest)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_17_to_string_escapes_parens() {
    assert_eq!(
        permission_rule_value_to_string("Bash", Some("python -c \"print(1)\"")),
        r#"Bash(python -c "print\(1\)")"#
    );
}

// -----------------------------------------------------------------
// 18 — full round trip: from_string → to_string → from_string preserves
// the (tool_name, rule_content) pair for canonical inputs.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_18_round_trip_canonical() {
    for input in &[
        "Bash",
        "Bash(npm install)",
        "Bash(git:*)",
        "Bash(python -c \"print(1)\")",
        "Bash(echo a)(b))", // edge: paren in content at the end
        "Read",
        "Edit(/tmp/foo.txt)",
    ] {
        let (n1, c1) = permission_rule_value_from_string(input);
        let s = permission_rule_value_to_string(&n1, c1.as_deref());
        let (n2, c2) = permission_rule_value_from_string(&s);
        assert_eq!(
            (&n1, &c1),
            (&n2, &c2),
            "round trip changed semantics for {input:?} (mid: {s:?})"
        );
    }
}

// -----------------------------------------------------------------
// 19 — unescaped-char finder semantics PIN: backslash count parity.
// `\\(` (escaped backslash + bare paren) is UNESCAPED; `\(` (escaped
// paren) is ESCAPED. Upstream L161-L198 walks the backslash run before
// each candidate and checks parity.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_19_backslash_parity_semantics() {
    // 0 preceding backslashes ⇒ even ⇒ unescaped.
    // Input `Bash(npm)` — `(` at idx 4 has 0 backslashes ⇒ open.
    assert_eq!(
        permission_rule_value_from_string("Bash(npm)"),
        ("Bash".to_string(), Some("npm".to_string()))
    );
    // 1 preceding backslash ⇒ odd ⇒ escaped, no open paren found
    // ⇒ treat whole string as tool name.
    let (n, c) = permission_rule_value_from_string(r"Bash\(npm)");
    assert_eq!(n, r"Bash\(npm)");
    assert_eq!(c, None);
    // 2 preceding backslashes ⇒ even ⇒ unescaped open paren found.
    // (Literal `\\` then `(` opens the content; close paren at end.)
    let (n, c) = permission_rule_value_from_string(r"Bash\\(npm)");
    assert_eq!(n, r"Bash\\");
    assert_eq!(c, Some("npm".to_string()));
}

// -----------------------------------------------------------------
// 20 — non-ASCII content survives the parser intact.
// (Bytes-based finder must not break UTF-8 multi-byte sequences.)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_j_20_non_ascii_content() {
    assert_eq!(
        permission_rule_value_from_string("Bash(echo αβγ)"),
        ("Bash".to_string(), Some("echo αβγ".to_string()))
    );
    assert_eq!(
        permission_rule_value_to_string("Bash", Some("echo αβγ")),
        "Bash(echo αβγ)"
    );
    // Round-trip escape with multi-byte chars + parens.
    let s = "echo (αβ) γ";
    assert_eq!(unescape_rule_content(&escape_rule_content(s)), s);
}
