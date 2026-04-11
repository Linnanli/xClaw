use admin_backend::extensions_state::{
    enabled_after_rescan, review_status_after_rescan, REVIEW_STATUS_APPROVED,
    REVIEW_STATUS_PENDING, REVIEW_STATUS_SCAN_FAILED,
};
use admin_backend::extensions_validation::validate_skill_package;

#[test]
fn req_extensions_001_rescan_safe_keeps_approved_when_already_approved() {
    let next = review_status_after_rescan(REVIEW_STATUS_APPROVED, true);
    assert_eq!(next, REVIEW_STATUS_APPROVED);
}

#[test]
fn req_extensions_002_rescan_safe_moves_non_approved_to_pending() {
    let from_pending = review_status_after_rescan(REVIEW_STATUS_PENDING, true);
    let from_failed = review_status_after_rescan(REVIEW_STATUS_SCAN_FAILED, true);

    assert_eq!(from_pending, REVIEW_STATUS_PENDING);
    assert_eq!(from_failed, REVIEW_STATUS_PENDING);
}

#[test]
fn req_extensions_003_rescan_unsafe_always_marks_scan_failed() {
    let from_approved = review_status_after_rescan(REVIEW_STATUS_APPROVED, false);
    let from_pending = review_status_after_rescan(REVIEW_STATUS_PENDING, false);

    assert_eq!(from_approved, REVIEW_STATUS_SCAN_FAILED);
    assert_eq!(from_pending, REVIEW_STATUS_SCAN_FAILED);
}

#[test]
fn req_extensions_004_enabled_after_safe_rescan_preserves_enabled_for_approved_skill() {
    assert!(enabled_after_rescan(true, REVIEW_STATUS_APPROVED, true));
    assert!(!enabled_after_rescan(false, REVIEW_STATUS_APPROVED, true));
}

#[test]
fn req_extensions_005_enabled_after_rescan_is_false_for_non_approved_or_unsafe() {
    assert!(!enabled_after_rescan(true, REVIEW_STATUS_PENDING, true));
    assert!(!enabled_after_rescan(true, REVIEW_STATUS_APPROVED, false));
    assert!(!enabled_after_rescan(false, REVIEW_STATUS_PENDING, false));
}

#[test]
fn req_extensions_006_validate_skill_package_accepts_minimal_valid_frontmatter() {
    let content = "---\nname: valid-skill\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - trigger\n---\n# body";
    assert!(validate_skill_package(content).is_ok());
}

#[test]
fn req_extensions_007_validate_skill_package_rejects_missing_frontmatter() {
    let err = validate_skill_package("# no frontmatter").expect_err("should reject content");
    assert!(err.to_string().contains("缺少 YAML frontmatter"));
}

#[test]
fn req_extensions_008_validate_skill_package_rejects_invalid_name() {
    let content = "---\nname: -bad\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - trigger\n---\n# body";
    let err = validate_skill_package(content).expect_err("should reject invalid name");
    assert!(err.to_string().contains("name 格式无效"));
}

#[test]
fn req_extensions_009_validate_skill_package_rejects_empty_activation() {
    let content = "---\nname: valid-skill\nversion: 1.0.0\ndescription: x\nactivation: {}\n---\n# body";
    let err = validate_skill_package(content).expect_err("should reject empty activation");
    assert!(err.to_string().contains("activation.keywords 或 activation.patterns"));
}

#[test]
fn req_extensions_010_validate_skill_package_rejects_short_keyword() {
    let content = "---\nname: valid-skill\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - trigger\nkeywords:\n  - ab\n---\n# body";
    let err = validate_skill_package(content).expect_err("should reject short keyword");
    assert!(err.to_string().contains("每个关键词至少 3 个字符"));
}

#[test]
fn req_extensions_011_validate_skill_package_rejects_oversized_content() {
    let mut content = String::from("---\nname: valid-skill\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - trigger\n---\n");
    content.push_str(&"a".repeat(70 * 1024));

    let err = validate_skill_package(&content).expect_err("should reject oversized content");
    assert!(err.to_string().contains("超过 64 KiB"));
}
