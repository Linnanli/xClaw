//! Requirement test for issue #110 — ADR-114 Ⅴ Cargo package + binary rename.
//!
//! These tests pin the Cargo.toml metadata so a future refactor cannot
//! silently break the `ironclaw → dasclaw` rename contract:
//!
//! * `[package].name` MUST be `"dasclaw"` (canonical package name).
//! * `[package].default-run` MUST be `"dasclaw"` so plain `cargo run` and
//!   downstream wrappers invoke the canonical binary.
//! * `[lib].name` MUST stay `"ironclaw"` so the 200+ `use ironclaw::…`
//!   imports across the workspace continue to resolve unchanged.
//! * The two binary entry points MUST exist on disk under `src/bin/` —
//!   `dasclaw.rs` (canonical) and `ironclaw.rs` (deprecation shim) — and
//!   are auto-discovered by Cargo, so `Cargo.toml` MUST NOT carry stale
//!   explicit `[[bin]]` entries pointing at the old `src/main.rs`.
//!
//! The test is a pure file-system + string scan. It does not pull in
//! `toml` or `cargo_metadata` to keep the dev-dep surface small and the
//! test fast (< 5 ms).

use std::fs;
use std::path::PathBuf;

fn cargo_toml_text() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("req_110: failed to read {}: {e}", path.display()))
}

#[test]
fn req_110_package_renamed_to_dasclaw() {
    let text = cargo_toml_text();
    assert!(
        text.contains("name = \"dasclaw\""),
        "req_110: [package].name must be \"dasclaw\" after ADR-114 Ⅴ rename"
    );
    assert!(
        !text.contains("\nname = \"ironclaw\"\nversion ="),
        "req_110: stale [package].name = \"ironclaw\" still present in Cargo.toml"
    );
}

#[test]
fn req_110_default_run_is_dasclaw() {
    let text = cargo_toml_text();
    assert!(
        text.contains("default-run = \"dasclaw\""),
        "req_110: [package].default-run must be \"dasclaw\" so `cargo run` \
         and packagers invoke the canonical binary by default"
    );
}

#[test]
fn req_110_lib_name_preserved_as_ironclaw() {
    let text = cargo_toml_text();
    assert!(
        text.contains("[lib]") && text.contains("name = \"ironclaw\""),
        "req_110: [lib].name must remain \"ironclaw\" so all `use ironclaw::…` \
         imports keep resolving without a workspace-wide source rename"
    );
}

#[test]
fn req_110_dual_bin_targets_present() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dasclaw_bin = manifest_dir.join("src/bin/dasclaw.rs");
    let ironclaw_bin = manifest_dir.join("src/bin/ironclaw.rs");
    assert!(
        dasclaw_bin.is_file(),
        "req_110: missing src/bin/dasclaw.rs (canonical entry, auto-discovered)"
    );
    assert!(
        ironclaw_bin.is_file(),
        "req_110: missing src/bin/ironclaw.rs (deprecation shim, auto-discovered)"
    );
    // Guard against accidental reintroduction of an explicit [[bin]] entry
    // pointing at the deleted `src/main.rs`, which would resurrect the
    // "file present in multiple build targets" warning the split fixed.
    let text = cargo_toml_text();
    assert!(
        !text.contains("path = \"src/main.rs\""),
        "req_110: stale path = \"src/main.rs\" found in Cargo.toml; \
         both bins must stay auto-discovered from src/bin/"
    );
}

#[test]
fn req_110_dev_dep_self_alias_via_package_field() {
    let text = cargo_toml_text();
    // The crate's own dev-dep entry must alias extern name `ironclaw` to
    // package `dasclaw`, otherwise tests using `use ironclaw::…` would
    // fail to resolve.
    let aliased =
        text.contains("ironclaw = { path = \".\"") && text.contains("package = \"dasclaw\"");
    assert!(
        aliased,
        "req_110: [dev-dependencies] entry for `ironclaw` must include \
         `package = \"dasclaw\"` so tests can keep `use ironclaw::…` imports"
    );
}
