//! Regression test for E0624: `ToolRegistry::register_message_tools` and
//! `ToolRegistry::register_job_tools` were previously crate-private,
//! breaking `desktop-client/src/engine.rs` Phase 7 bootstrap (which still
//! calls them by method until migrated to `bootstrap_tools` in a follow-up
//! PR).
//!
//! This integration test lives in an *external* crate-test target, so it
//! can only compile if both methods are `pub`. If anyone reverts the
//! visibility to `pub(crate)` / private, this file fails to compile and
//! the regression is caught in CI before the desktop-client build does.

use ironclaw::tools::ToolRegistry;

/// Compile-time witness: take method references on `ToolRegistry`. If
/// either method is not `pub`, the file fails to compile from this
/// external test crate.
#[test]
fn req_register_message_tools_method_is_pub() {
    let _m = ToolRegistry::register_message_tools;
    let _ = &_m as *const _;
}

#[test]
fn req_register_job_tools_method_is_pub() {
    let _m = ToolRegistry::register_job_tools;
    let _ = &_m as *const _;
}
