//! Cross-platform OS-keychain end-to-end tests for
//! [`dasclaw_runtime::secrets::keychain::KeychainConfig`].
//!
//! Tracks issue #897: PR #891 left the only real keychain coverage in
//! `dasclaw_cli` gated to `target_os = "macos"`. The actual `keychain.rs`
//! back-end is fully crate-split per target:
//!
//!   - macOS: `security-framework` (Keychain Services)
//!   - Linux: `secret-service` over D-Bus (GNOME Keyring / KWallet)
//!   - Windows: `windows` crate / DPAPI-sealed Credential Manager
//!
//! All three are headless-capable, so this file exercises the real OS
//! API on every platform. Linux runners without a session D-Bus
//! gracefully skip (eprintln + early return) instead of failing — CI
//! containers commonly lack `DBUS_SESSION_BUS_ADDRESS`.
//!
//! Isolation: every test uses a uuid-derived service name so concurrent
//! shards and previously-crashed runs cannot collide. A `Cleanup`
//! drop-guard removes the slot even on panic.

use dasclaw_runtime::secrets::keychain::{KeychainConfig, generate_master_key};
use dasclaw_runtime::secrets::types::SecretError;
use uuid::Uuid;

/// Build a process-unique `KeychainConfig`.
///
/// `KeychainConfig` requires `&'static str` for both fields so the type
/// stays `const`-constructible. We leak a single small `Box<str>` per
/// test invocation to satisfy the lifetime — bounded by test count and
/// only inside the test binary.
fn unique_keychain_config(prefix: &str) -> KeychainConfig {
    let service: &'static str =
        Box::leak(format!("dasclaw_test_{}_{}", prefix, Uuid::new_v4().simple()).into_boxed_str());
    KeychainConfig::new(service, "master_key")
}

/// RAII guard that best-effort deletes the keychain slot on drop, even
/// if a test panics or returns early. Errors are ignored — the slot may
/// legitimately be empty (e.g. after a successful explicit delete).
struct Cleanup {
    cfg: KeychainConfig,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        // Spawn into whichever runtime is alive. Inside `#[tokio::test]`
        // we are still on the runtime thread, so `Handle::try_current`
        // succeeds.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let cfg = self.cfg;
            // `block_in_place` is not available on current_thread; use
            // a dedicated short-lived thread to avoid nesting runtimes.
            std::thread::scope(|s| {
                s.spawn(|| {
                    handle.block_on(async {
                        let _ = cfg.delete_master_key().await;
                    });
                });
            });
        }
    }
}

/// Shared contract executor used by every platform branch: store →
/// has → get → delete → has(false) → get → `SecretError::NotFound`.
async fn run_keychain_contract(cfg: KeychainConfig) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = Cleanup { cfg };

    // Defensive pre-clean (slot may be poisoned by a crashed prior run).
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

    // After delete, reading must surface an error rather than `Ok`.
    //
    // We deliberately do *not* pin the exact variant: the OS keychain
    // back-ends in `keychain.rs` (macOS Security.framework, Linux
    // secret-service, Windows Credential Manager) fold "item not
    // found" into the generic `SecretError::KeychainError(_)` rather
    // than `SecretError::NotFound(_)` — the latter is reserved for
    // the higher-level `secrets::store*` layer. The contract that
    // matters for the OS-keychain layer is "read-after-delete is not
    // `Ok`", which is what we assert here.
    match cfg.get_master_key().await {
        Err(SecretError::KeychainError(_)) | Err(SecretError::NotFound(_)) => Ok(()),
        Err(other) => {
            Err(format!("expected KeychainError or NotFound after delete, got: {other:?}").into())
        }
        Ok(_) => Err("expected an error after delete, got Ok".into()),
    }
}

// ---------------------------------------------------------------------------
// macOS — CI macOS runner has an ephemeral login keychain; no skipping.
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
#[tokio::test]
async fn req_runtime_keychain_macos_round_trip_then_notfound()
-> Result<(), Box<dyn std::error::Error>> {
    let cfg = unique_keychain_config("macos");
    run_keychain_contract(cfg).await
}

// ---------------------------------------------------------------------------
// Linux — requires a session D-Bus. CI containers usually lack one,
// so gracefully skip rather than fail.
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
#[tokio::test]
async fn req_runtime_keychain_linux_round_trip_then_notfound()
-> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
        eprintln!(
            "SKIP req_runtime_keychain_linux_round_trip_then_notfound: \
             DBUS_SESSION_BUS_ADDRESS not set; no session secret service available"
        );
        return Ok(());
    }
    let cfg = unique_keychain_config("linux");
    run_keychain_contract(cfg).await
}

// ---------------------------------------------------------------------------
// Windows — DPAPI / CredMan is available in any user session, headless
// or otherwise. No skip path.
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
#[tokio::test]
async fn req_runtime_keychain_windows_round_trip_then_notfound()
-> Result<(), Box<dyn std::error::Error>> {
    let cfg = unique_keychain_config("windows");
    run_keychain_contract(cfg).await
}
