//! OS keychain integration for secrets master key storage.
//!
//! Provides platform-specific keychain support:
//! - macOS: security-framework (Keychain Services)
//! - Linux: secret-service (GNOME Keyring, KWallet)
//! - Windows: Credential Manager (CredWriteW/CredReadW/CredDeleteW; DPAPI-sealed per-user)
//!
//! # Why a `KeychainConfig` struct
//!
//! In ironclaw the service/account names used to be `const SERVICE_NAME =
//! "ironclaw"`. Hard-coding a host brand inside a shared framework crate is
//! wrong — any future host that links `dasclaw_runtime` (claw-code, codex
//! shell, headless agent runner, …) needs its own keychain namespace, not the
//! ironclaw one. So we parameterize:
//!
//! - The framework crate exposes [`KeychainConfig`] (`service_name` + `account`).
//! - Each host defines its own `const HOST_KEYCHAIN: KeychainConfig =
//!   KeychainConfig::new("<host>", "<account>")` and calls the methods on it.
//!
//! # Example
//!
//! ```ignore
//! use dasclaw_runtime::secrets::keychain::KeychainConfig;
//!
//! const MY_KEYCHAIN: KeychainConfig = KeychainConfig::new("my-host", "master_key");
//!
//! // Generate and store a new master key
//! let key = dasclaw_runtime::secrets::keychain::generate_master_key();
//! MY_KEYCHAIN.store_master_key(&key).await?;
//!
//! // Later, retrieve it
//! let key = MY_KEYCHAIN.get_master_key().await?;
//! ```

use crate::secrets::types::SecretError;

/// Host-supplied keychain namespace: a service name + account pair.
///
/// This struct decouples the shared `dasclaw_runtime` crate from any single
/// host brand. Hosts construct their own `const KeychainConfig` and call its
/// async methods.
///
/// Both fields are `&'static str` so the struct stays `const`-constructible and
/// platform back-ends can pass them directly to the OS APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeychainConfig {
    /// OS-level service / target prefix (e.g. `"ironclaw"`,
    /// `"claw-code"`, `"codex"`).
    pub service_name: &'static str,
    /// Account / item suffix (almost always `"master_key"` —
    /// kept as a field so a host can split multiple keys per service).
    pub account: &'static str,
}

impl KeychainConfig {
    /// Build a new keychain namespace.
    pub const fn new(service_name: &'static str, account: &'static str) -> Self {
        Self {
            service_name,
            account,
        }
    }

    /// Store the master key in the OS keychain.
    pub async fn store_master_key(&self, key: &[u8]) -> Result<(), SecretError> {
        platform::store_master_key(self, key).await
    }

    /// Retrieve the master key from the OS keychain.
    pub async fn get_master_key(&self) -> Result<Vec<u8>, SecretError> {
        platform::get_master_key(self).await
    }

    /// Delete the master key from the OS keychain.
    pub async fn delete_master_key(&self) -> Result<(), SecretError> {
        platform::delete_master_key(self).await
    }

    /// Check whether a master key exists in the OS keychain.
    pub async fn has_master_key(&self) -> bool {
        platform::has_master_key(self).await
    }
}

/// Generate a random 32-byte master key.
pub fn generate_master_key() -> Vec<u8> {
    use rand::rngs::OsRng;
    use rand::RngCore;
    let mut key = vec![0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Generate a master key as a hex string.
pub fn generate_master_key_hex() -> String {
    bytes_to_hex(&generate_master_key())
}

/// Encode bytes as a lowercase hex string. Symmetric counterpart of
/// [`hex_to_bytes`]; shared by the three platform back-ends and key
/// generation so the encoding format stays in one place.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows", test))]
fn bytes_to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// Make the helper unconditionally visible to `generate_master_key_hex` on
// every platform (including the fallback build with no keychain).
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows", test)))]
fn bytes_to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// ============================================================================
// macOS implementation using security-framework
// ============================================================================

#[cfg(target_os = "macos")]
mod platform {
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    use super::*;

    pub async fn store_master_key(cfg: &KeychainConfig, key: &[u8]) -> Result<(), SecretError> {
        // Convert to hex for storage (keychain prefers strings)
        let key_hex = bytes_to_hex(key);

        set_generic_password(cfg.service_name, cfg.account, key_hex.as_bytes())
            .map_err(|e| SecretError::KeychainError(format!("Failed to store in keychain: {e}")))
    }

    pub async fn get_master_key(cfg: &KeychainConfig) -> Result<Vec<u8>, SecretError> {
        let password = get_generic_password(cfg.service_name, cfg.account)
            .map_err(|e| SecretError::KeychainError(format!("Failed to get from keychain: {e}")))?;

        let hex_str = String::from_utf8(password)
            .map_err(|_| SecretError::KeychainError("Invalid UTF-8 in keychain".to_string()))?;

        hex_to_bytes(&hex_str)
    }

    pub async fn delete_master_key(cfg: &KeychainConfig) -> Result<(), SecretError> {
        delete_generic_password(cfg.service_name, cfg.account)
            .map_err(|e| SecretError::KeychainError(format!("Failed to delete from keychain: {e}")))
    }

    pub async fn has_master_key(cfg: &KeychainConfig) -> bool {
        get_generic_password(cfg.service_name, cfg.account).is_ok()
    }
}

// ============================================================================
// Linux implementation using secret-service
// ============================================================================

#[cfg(target_os = "linux")]
mod platform {
    use secret_service::{EncryptionType, SecretService};

    use super::*;

    pub async fn store_master_key(cfg: &KeychainConfig, key: &[u8]) -> Result<(), SecretError> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                SecretError::KeychainError(format!("Failed to connect to secret service: {e}"))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to get collection: {e}")))?;

        if collection.is_locked().await.unwrap_or(true) {
            collection.unlock().await.map_err(|e| {
                SecretError::KeychainError(format!("Failed to unlock collection: {e}"))
            })?;
        }

        let key_hex = bytes_to_hex(key);
        let service = cfg.service_name;
        let account = cfg.account;

        collection
            .create_item(
                &format!("{service} master key"),
                [("service", service), ("account", account)]
                    .into_iter()
                    .collect(),
                key_hex.as_bytes(),
                true,
                "text/plain",
            )
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to create secret: {e}")))?;

        Ok(())
    }

    pub async fn get_master_key(cfg: &KeychainConfig) -> Result<Vec<u8>, SecretError> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                SecretError::KeychainError(format!("Failed to connect to secret service: {e}"))
            })?;

        let items = ss
            .search_items(
                [("service", cfg.service_name), ("account", cfg.account)]
                    .into_iter()
                    .collect(),
            )
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to search: {e}")))?;

        let item = items
            .unlocked
            .first()
            .or(items.locked.first())
            .ok_or_else(|| SecretError::KeychainError("Master key not found".to_string()))?;

        if item.is_locked().await.unwrap_or(true) {
            item.unlock()
                .await
                .map_err(|e| SecretError::KeychainError(format!("Failed to unlock: {e}")))?;
        }

        let secret = item
            .get_secret()
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to get secret: {e}")))?;

        let hex_str = String::from_utf8(secret)
            .map_err(|_| SecretError::KeychainError("Invalid UTF-8 in secret".to_string()))?;

        hex_to_bytes(&hex_str)
    }

    pub async fn delete_master_key(cfg: &KeychainConfig) -> Result<(), SecretError> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                SecretError::KeychainError(format!("Failed to connect to secret service: {e}"))
            })?;

        let items = ss
            .search_items(
                [("service", cfg.service_name), ("account", cfg.account)]
                    .into_iter()
                    .collect(),
            )
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to search: {e}")))?;

        for item in items.unlocked.iter().chain(items.locked.iter()) {
            item.delete()
                .await
                .map_err(|e| SecretError::KeychainError(format!("Failed to delete: {e}")))?;
        }

        Ok(())
    }

    pub async fn has_master_key(cfg: &KeychainConfig) -> bool {
        let ss = match SecretService::connect(EncryptionType::Dh).await {
            Ok(ss) => ss,
            Err(_) => return false,
        };

        let items = match ss
            .search_items(
                [("service", cfg.service_name), ("account", cfg.account)]
                    .into_iter()
                    .collect(),
            )
            .await
        {
            Ok(items) => items,
            Err(_) => return false,
        };

        !items.unlocked.is_empty() || !items.locked.is_empty()
    }
}

// ============================================================================
// Windows implementation using Credential Manager (DPAPI-sealed per-user)
// ============================================================================
//
// Stores the master key as a generic credential under target name
// "<service_name>:<account>" via CredWriteW. The Credential Manager seals the
// blob with DPAPI bound to the current Windows user.
//
// API mapping:
//   - store_master_key  -> CredWriteW (CRED_TYPE_GENERIC, CRED_PERSIST_LOCAL_MACHINE)
//   - get_master_key    -> CredReadW
//   - delete_master_key -> CredDeleteW
//   - has_master_key    -> CredReadW + check exists

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;
    use std::slice;

    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME};
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS,
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    use super::*;

    /// Build the credential target name as a NUL-terminated wide string.
    /// Format: "<service_name>:<account>".
    fn target_name_wide(cfg: &KeychainConfig) -> Vec<u16> {
        let s = format!("{}:{}", cfg.service_name, cfg.account);
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Format a `windows::core::Error` returned from a Cred* call.
    ///
    /// Uses the HRESULT carried by the returned error directly — calling
    /// `GetLastError` after the binding has returned would race against any
    /// intervening Win32 call inside the `windows` crate's epilogue.
    fn keychain_err(action: &str, e: windows::core::Error) -> SecretError {
        SecretError::KeychainError(format!(
            "Failed to {action} (HRESULT 0x{:08X}: {})",
            e.code().0 as u32,
            e.message()
        ))
    }

    pub async fn store_master_key(cfg: &KeychainConfig, key: &[u8]) -> Result<(), SecretError> {
        let key_hex = bytes_to_hex(key);
        let mut target = target_name_wide(cfg);
        let mut blob = key_hex.into_bytes();

        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            Comment: PWSTR::null(),
            LastWritten: FILETIME::default(),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR::null(),
        };

        // Safety: `cred` is fully initialized; `target` and `blob` outlive the call.
        let result = unsafe { CredWriteW(&cred, 0) };
        result.map_err(|e| keychain_err("store in Credential Manager", e))
    }

    pub async fn get_master_key(cfg: &KeychainConfig) -> Result<Vec<u8>, SecretError> {
        let mut target = target_name_wide(cfg);
        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();

        // Safety: `target` is valid; `cred_ptr` initialized via FFI on success;
        // every successful read paired with `CredFree`.
        let result = unsafe {
            CredReadW(
                PCWSTR(target.as_mut_ptr()),
                CRED_TYPE_GENERIC,
                0,
                &mut cred_ptr,
            )
        };

        result.map_err(|e| keychain_err("read from Credential Manager", e))?;

        if cred_ptr.is_null() {
            return Err(SecretError::KeychainError(
                "CredReadW returned success but null pointer".to_string(),
            ));
        }

        // Safety: `cred_ptr` is non-null and valid until `CredFree` below.
        let bytes: Vec<u8> = unsafe {
            let cred = &*cred_ptr;
            let len = cred.CredentialBlobSize as usize;
            slice::from_raw_parts(cred.CredentialBlob, len).to_vec()
        };

        // Safety: balance the `CredReadW` allocation.
        unsafe { CredFree(cred_ptr as *const c_void) };

        let hex_str = String::from_utf8(bytes).map_err(|_| {
            SecretError::KeychainError("Invalid UTF-8 in Credential Manager blob".to_string())
        })?;

        hex_to_bytes(&hex_str)
    }

    pub async fn delete_master_key(cfg: &KeychainConfig) -> Result<(), SecretError> {
        let mut target = target_name_wide(cfg);

        // Safety: `target` is a valid NUL-terminated wide string.
        let result = unsafe { CredDeleteW(PCWSTR(target.as_mut_ptr()), CRED_TYPE_GENERIC, 0) };

        match result {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => {
                // Idempotent: deleting a missing entry is not an error.
                Ok(())
            }
            Err(e) => Err(keychain_err("delete from Credential Manager", e)),
        }
    }

    pub async fn has_master_key(cfg: &KeychainConfig) -> bool {
        let mut target = target_name_wide(cfg);
        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();

        // Safety: same as `get_master_key`.
        let result = unsafe {
            CredReadW(
                PCWSTR(target.as_mut_ptr()),
                CRED_TYPE_GENERIC,
                0,
                &mut cred_ptr,
            )
        };

        if result.is_ok() && !cred_ptr.is_null() {
            // Safety: balance the `CredReadW` allocation.
            unsafe { CredFree(cred_ptr as *const c_void) };
            true
        } else {
            false
        }
    }
}

// ============================================================================
// Fallback for unsupported platforms
// ============================================================================

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod platform {
    use super::*;

    pub async fn store_master_key(_cfg: &KeychainConfig, _key: &[u8]) -> Result<(), SecretError> {
        Err(SecretError::KeychainError(
            "Keychain not supported on this platform. Use SECRETS_MASTER_KEY env var.".to_string(),
        ))
    }

    pub async fn get_master_key(_cfg: &KeychainConfig) -> Result<Vec<u8>, SecretError> {
        Err(SecretError::KeychainError(
            "Keychain not supported on this platform. Use SECRETS_MASTER_KEY env var.".to_string(),
        ))
    }

    pub async fn delete_master_key(_cfg: &KeychainConfig) -> Result<(), SecretError> {
        Err(SecretError::KeychainError(
            "Keychain not supported on this platform".to_string(),
        ))
    }

    pub async fn has_master_key(_cfg: &KeychainConfig) -> bool {
        false
    }
}

/// Parse a hex string to bytes.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows", test))]
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, SecretError> {
    if hex.is_empty() {
        // Fail-Safe: an empty stored blob is a corrupted/uninitialized state,
        // not a valid zero-length key. Reject so upstream callers don't
        // silently observe `Decrypted::new(vec![])`.
        return Err(SecretError::KeychainError("Empty hex string".to_string()));
    }
    if !hex.len().is_multiple_of(2) {
        return Err(SecretError::KeychainError(
            "Invalid hex string length".to_string(),
        ));
    }

    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| SecretError::KeychainError("Invalid hex character".to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_dasclaw_runtime_secrets_keychain_generate_master_key_length() {
        let key = generate_master_key();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn req_dasclaw_runtime_secrets_keychain_generate_master_key_random() {
        let a = generate_master_key();
        let b = generate_master_key();
        assert_ne!(a, b, "two generated keys must differ");
    }

    #[test]
    fn req_dasclaw_runtime_secrets_keychain_generate_master_key_hex_shape() {
        let hex = generate_master_key_hex();
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn req_dasclaw_runtime_secrets_keychain_hex_to_bytes_roundtrip() {
        assert_eq!(
            hex_to_bytes("deadbeef").unwrap(),
            vec![0xde, 0xad, 0xbe, 0xef]
        );
        assert_eq!(hex_to_bytes("00ff").unwrap(), vec![0x00, 0xff]);
    }

    #[test]
    fn req_dasclaw_runtime_secrets_keychain_security_hex_to_bytes_rejects_bad_input() {
        // Fail-safe: malformed hex must not silently succeed
        assert!(hex_to_bytes("abc").is_err(), "odd length rejected");
        assert!(hex_to_bytes("gg").is_err(), "non-hex chars rejected");
        assert!(hex_to_bytes("ab cd").is_err(), "whitespace rejected");
        assert!(
            hex_to_bytes("").is_err(),
            "empty input rejected (Fail-Safe)"
        );
    }

    #[test]
    fn req_dasclaw_runtime_secrets_keychain_config_const_constructible() {
        // The whole point of the refactor: `KeychainConfig` is `const`-constructible
        // so each host can declare a `static` namespace with its own brand.
        const A: KeychainConfig = KeychainConfig::new("host-a", "master_key");
        const B: KeychainConfig = KeychainConfig::new("host-b", "master_key");
        assert_ne!(A, B, "different hosts must yield different namespaces");
        assert_eq!(A.service_name, "host-a");
        assert_eq!(A.account, "master_key");
    }

    #[test]
    fn req_dasclaw_runtime_secrets_keychain_config_distinguishes_account() {
        // Two configs with the same service_name but different accounts must be
        // treated as different namespaces (a host can store multiple keys).
        const KEY_A: KeychainConfig = KeychainConfig::new("svc", "key_a");
        const KEY_B: KeychainConfig = KeychainConfig::new("svc", "key_b");
        assert_ne!(KEY_A, KEY_B);
    }

    /// Windows Credential Manager round-trip — gated to Windows so CI runs it
    /// in the windows-ci.yml job. Uses a test-only `KeychainConfig` so the
    /// real ironclaw entry is not touched.
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_keychain_windows_credential_manager_roundtrip() {
        // Test-only namespace so we don't clobber the host's real keychain entry.
        const TEST_CFG: KeychainConfig =
            KeychainConfig::new("dasclaw-runtime-test", "master_key_test");

        // Use a deterministic key so failures point at the keychain layer.
        let key: Vec<u8> = (0u8..32).collect();

        // Best-effort cleanup before/after.
        let _ = TEST_CFG.delete_master_key().await;

        TEST_CFG
            .store_master_key(&key)
            .await
            .expect("store_master_key must succeed");
        assert!(
            TEST_CFG.has_master_key().await,
            "credential should exist after store"
        );

        let read_back = TEST_CFG
            .get_master_key()
            .await
            .expect("get_master_key must succeed");
        assert_eq!(read_back, key, "round-trip must preserve key bytes");

        TEST_CFG
            .delete_master_key()
            .await
            .expect("delete_master_key must succeed");
        assert!(
            !TEST_CFG.has_master_key().await,
            "credential should be gone after delete"
        );

        // Idempotent delete.
        TEST_CFG
            .delete_master_key()
            .await
            .expect("delete on missing entry must not error");
    }
}
