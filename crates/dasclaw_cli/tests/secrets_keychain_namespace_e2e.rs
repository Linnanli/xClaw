//! CLI-side contract sentinel for `dasclaw_runtime::secrets::keychain::KeychainConfig`
//! (issue #897).
//!
//! # Why this file is contract-only now
//!
//! The previous revision of this file ran a real macOS Keychain round-trip
//! and was gated `#![cfg(target_os = "macos")]`, which left Linux and Windows
//! completely uncovered from any test suite. Issue #897 moved the real-OS
//! cross-platform coverage into
//! `crates/dasclaw_runtime/tests/keychain_e2e.rs`, where it lives next to
//! the implementation and runs on all three platforms (Linux gracefully
//! skips when `DBUS_SESSION_BUS_ADDRESS` is unset).
//!
//! What's left for the CLI crate is a **public-API contract sentinel**:
//! `dasclaw_cli/src` does not itself construct a `KeychainConfig` today
//! (verified by `grep -rn "KeychainConfig" crates/dasclaw_cli/src/` returning
//! no hits), but the CLI binary links `dasclaw_runtime` transitively and any
//! future CLI feature that bootstraps a master key will rely on the same
//! public surface. This file pins that surface — field names, public
//! constructor, `const`-constructibility — so a silent rename or visibility
//! change in `dasclaw_runtime` breaks a CLI test, not a downstream user.
//!
//! The file is **not** gated on any platform: the contract test exercises
//! only struct construction and field access, which has no OS dependency.
//!
//! An additional `#[ignore]`'d macOS smoke test is preserved at the bottom
//! as a CLI-side backup; run it explicitly with
//! `cargo test -p dasclaw_cli --test secrets_keychain_namespace_e2e -- --ignored`.

use dasclaw_runtime::secrets::keychain::KeychainConfig;

/// `req_dasclaw_cli_secrets_keychain_config_public_contract` — the
/// public surface of `KeychainConfig` that the CLI relies on
/// transitively must remain stable:
///
/// 1. `KeychainConfig::new(service, account)` is callable in `const`
///    context with two `&'static str` arguments.
/// 2. Both `service_name` and `account` are publicly readable fields
///    that round-trip the constructor arguments verbatim.
/// 3. The struct derives `Copy + Clone + PartialEq + Eq + Debug` so
///    hosts can pass it by value and compare it in tests.
///
/// A silent rename of either field, a loss of `pub` visibility, or a
/// change to a non-`const` constructor would break this test before it
/// breaks downstream CLI code.
#[test]
fn req_dasclaw_cli_secrets_keychain_config_public_contract() {
    // (1) `const`-constructibility: if `KeychainConfig::new` ever
    // becomes non-`const`, this binding fails to compile.
    const NS: KeychainConfig = KeychainConfig::new("svc-x", "acc-y");

    // (2) Field-access contract: both fields stay `pub` and carry the
    // exact bytes passed to the constructor (no normalization).
    assert_eq!(NS.service_name, "svc-x");
    assert_eq!(NS.account, "acc-y");

    // (3) Derived-trait contract: `Copy` (implicit move-after-use),
    // `Clone`, `PartialEq`, and `Debug` must remain on the type. The
    // assertions below would not compile otherwise.
    let copy_a = NS;
    let copy_b = NS;
    assert_eq!(copy_a, copy_b);
    let cloned = copy_a;
    assert_eq!(cloned, NS);
    let _ = format!("{NS:?}");

    // (4) Equality is structural — different inputs produce inequal
    // configs. Guards against an accidental `PartialEq` override that
    // would treat all configs as equal.
    let other = KeychainConfig::new("svc-x", "acc-z");
    assert_ne!(NS, other);
}

// -------------------------------------------------------------------
// Optional macOS smoke test (ignored by default).
//
// Cross-platform real-OS coverage lives in
// `crates/dasclaw_runtime/tests/keychain_e2e.rs`. The block below is
// kept as a CLI-side backup that a developer can run on demand when
// debugging keychain-related CLI regressions on macOS:
//
//   cargo test -p dasclaw_cli --test secrets_keychain_namespace_e2e \
//       -- --ignored
// -------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos_smoke {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use dasclaw_runtime::secrets::keychain::{KeychainConfig, generate_master_key};

    fn unique_keychain_config() -> KeychainConfig {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let service: &'static str = Box::leak(
            format!("dasclaw_test_cli_smoke_{}_{}", process::id(), nanos).into_boxed_str(),
        );
        KeychainConfig::new(service, "master_key")
    }

    #[tokio::test]
    #[ignore = "macOS keychain smoke; run with `-- --ignored`"]
    async fn req_dasclaw_cli_secrets_macos_smoke_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let cfg = unique_keychain_config();

        let _ = cfg.delete_master_key().await;

        let key = generate_master_key();
        assert_eq!(key.len(), 32, "master key must be 32 bytes");

        cfg.store_master_key(&key).await?;
        assert!(
            cfg.has_master_key().await,
            "key must be present after store"
        );

        let read_back = cfg.get_master_key().await?;
        assert_eq!(read_back, key, "round-trip must return identical bytes");

        cfg.delete_master_key().await?;
        assert!(
            !cfg.has_master_key().await,
            "key must be absent after delete"
        );

        Ok(())
    }
}
