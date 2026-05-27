//! PR2 GUI-only blind-spot: `dasclaw_runtime::secrets::keychain::KeychainConfig`
//! round-trip from a non-GUI consumer (ADR-153 §1).
//!
//! Why this test exists in `dasclaw_cli` and not closer to the source:
//! the keychain code is shared across CLI + desktop hosts, but only the
//! GUI host (`desktop-client/ironclaw`) currently exercises it on macOS.
//! A regression in the platform back-end's service-name handling or
//! account-suffix collision would silently break CLI master-key
//! bootstrapping. This e2e pins the contract from the CLI side.
//!
//! Platform gating:
//!   - macOS: real `Security.framework` round-trip (this test).
//!   - Linux: requires a running D-Bus secret service (CI runners
//!     usually lack one); covered separately in the runtime crate.
//!   - Windows: same family of issues; runtime-crate-side coverage
//!     handles it.
//!
//! For now we gate the whole file to `target_os = "macos"`.
//!
//! Isolation:
//!   - Each test run synthesises a unique `service_name` from pid + a
//!     monotonic nanos counter, so concurrent CI shards do not collide.
//!   - The test calls `delete_master_key` before *and* after asserting,
//!     so a previously-crashed run cannot poison the slot.

#![cfg(target_os = "macos")]

use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use dasclaw_runtime::secrets::keychain::{KeychainConfig, generate_master_key};

/// Build a `KeychainConfig` with a process-unique service name.
///
/// `KeychainConfig` requires `&'static str` for both fields so the
/// struct is `const`-constructible; we leak a small `Box<str>` per test
/// (≤ one allocation per process; gated to macOS only) to satisfy that
/// lifetime without resorting to `OnceLock` gymnastics.
fn unique_keychain_config() -> KeychainConfig {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let service: &'static str =
        Box::leak(format!("dasclaw_test_cli_pr2_{}_{}", process::id(), nanos).into_boxed_str());
    KeychainConfig::new(service, "master_key")
}

/// `req_dasclaw_cli_secrets_pr2_keychain_store_get_round_trip` — a
/// CLI consumer can store, retrieve, and delete a master key through
/// the public `KeychainConfig` API without reaching for any GUI-only
/// state. Catches drift in:
///   - the service-name + account namespace contract,
///   - the `Vec<u8>` round-trip (no UTF-8 lossy conversion on macOS),
///   - the `has_master_key` truthiness after delete.
#[tokio::test(flavor = "multi_thread")]
async fn req_dasclaw_cli_secrets_pr2_keychain_store_get_round_trip()
-> Result<(), Box<dyn std::error::Error>> {
    let cfg = unique_keychain_config();

    // Defensive pre-clean: a previously-crashed run might have left the
    // slot populated. Ignore the result; the slot may legitimately be
    // empty.
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
