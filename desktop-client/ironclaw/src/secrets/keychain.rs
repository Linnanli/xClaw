//! OS keychain integration for secrets master key storage.
//!
//! Provides platform-specific keychain support:
//! - macOS: security-framework (Keychain Services)
//! - Linux: secret-service (GNOME Keyring, KWallet)
//! - Windows: Credential Manager (CredWriteW/CredReadW/CredDeleteW; DPAPI-sealed per-user)
//!
//! # Example
//!
//! ```ignore
//! use ironclaw::secrets::keychain::{store_master_key, get_master_key, delete_master_key};
//!
//! // Generate and store a new master key
//! let key = generate_master_key();
//! store_master_key(&key)?;
//!
//! // Later, retrieve it
//! let key = get_master_key()?;
//! ```

use crate::secrets::SecretError;

/// Service name for keychain entries.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
const SERVICE_NAME: &str = "ironclaw";

/// Account name for the master key.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
const MASTER_KEY_ACCOUNT: &str = "master_key";

/// Generate a random 32-byte master key.
pub fn generate_master_key() -> Vec<u8> {
    use rand::RngCore;
    use rand::rngs::OsRng;
    let mut key = vec![0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Generate a master key as a hex string.
pub fn generate_master_key_hex() -> String {
    let bytes = generate_master_key();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
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

    /// Store the master key in the macOS Keychain.
    pub async fn store_master_key(key: &[u8]) -> Result<(), SecretError> {
        // Convert to hex for storage (keychain prefers strings)
        let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();

        set_generic_password(SERVICE_NAME, MASTER_KEY_ACCOUNT, key_hex.as_bytes())
            .map_err(|e| SecretError::KeychainError(format!("Failed to store in keychain: {}", e)))
    }

    /// Retrieve the master key from the macOS Keychain.
    pub async fn get_master_key() -> Result<Vec<u8>, SecretError> {
        let password = get_generic_password(SERVICE_NAME, MASTER_KEY_ACCOUNT).map_err(|e| {
            SecretError::KeychainError(format!("Failed to get from keychain: {}", e))
        })?;

        // Parse hex string back to bytes
        let hex_str = String::from_utf8(password)
            .map_err(|_| SecretError::KeychainError("Invalid UTF-8 in keychain".to_string()))?;

        hex_to_bytes(&hex_str)
    }

    /// Delete the master key from the macOS Keychain.
    pub async fn delete_master_key() -> Result<(), SecretError> {
        delete_generic_password(SERVICE_NAME, MASTER_KEY_ACCOUNT).map_err(|e| {
            SecretError::KeychainError(format!("Failed to delete from keychain: {}", e))
        })
    }

    /// Check if a master key exists in the keychain.
    pub async fn has_master_key() -> bool {
        get_generic_password(SERVICE_NAME, MASTER_KEY_ACCOUNT).is_ok()
    }
}

// ============================================================================
// Linux implementation using secret-service
// ============================================================================

#[cfg(target_os = "linux")]
mod platform {
    use secret_service::{EncryptionType, SecretService};

    use super::*;

    /// Store the master key in the Linux secret service (GNOME Keyring, KWallet).
    pub async fn store_master_key(key: &[u8]) -> Result<(), SecretError> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                SecretError::KeychainError(format!("Failed to connect to secret service: {}", e))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to get collection: {}", e)))?;

        // Unlock if needed
        if collection.is_locked().await.unwrap_or(true) {
            collection.unlock().await.map_err(|e| {
                SecretError::KeychainError(format!("Failed to unlock collection: {}", e))
            })?;
        }

        // Convert to hex for storage
        let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();

        collection
            .create_item(
                &format!("{} master key", SERVICE_NAME),
                [("service", SERVICE_NAME), ("account", MASTER_KEY_ACCOUNT)]
                    .into_iter()
                    .collect(),
                key_hex.as_bytes(),
                true, // Replace if exists
                "text/plain",
            )
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to create secret: {}", e)))?;

        Ok(())
    }

    /// Retrieve the master key from the Linux secret service.
    pub async fn get_master_key() -> Result<Vec<u8>, SecretError> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                SecretError::KeychainError(format!("Failed to connect to secret service: {}", e))
            })?;

        let items = ss
            .search_items(
                [("service", SERVICE_NAME), ("account", MASTER_KEY_ACCOUNT)]
                    .into_iter()
                    .collect(),
            )
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to search: {}", e)))?;

        let item = items
            .unlocked
            .first()
            .or(items.locked.first())
            .ok_or_else(|| SecretError::KeychainError("Master key not found".to_string()))?;

        // Unlock if needed
        if item.is_locked().await.unwrap_or(true) {
            item.unlock()
                .await
                .map_err(|e| SecretError::KeychainError(format!("Failed to unlock: {}", e)))?;
        }

        let secret = item
            .get_secret()
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to get secret: {}", e)))?;

        let hex_str = String::from_utf8(secret)
            .map_err(|_| SecretError::KeychainError("Invalid UTF-8 in secret".to_string()))?;

        hex_to_bytes(&hex_str)
    }

    /// Delete the master key from the Linux secret service.
    pub async fn delete_master_key() -> Result<(), SecretError> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                SecretError::KeychainError(format!("Failed to connect to secret service: {}", e))
            })?;

        let items = ss
            .search_items(
                [("service", SERVICE_NAME), ("account", MASTER_KEY_ACCOUNT)]
                    .into_iter()
                    .collect(),
            )
            .await
            .map_err(|e| SecretError::KeychainError(format!("Failed to search: {}", e)))?;

        for item in items.unlocked.iter().chain(items.locked.iter()) {
            item.delete()
                .await
                .map_err(|e| SecretError::KeychainError(format!("Failed to delete: {}", e)))?;
        }

        Ok(())
    }

    /// Check if a master key exists in the secret service.
    pub async fn has_master_key() -> bool {
        let ss = match SecretService::connect(EncryptionType::Dh).await {
            Ok(ss) => ss,
            Err(_) => return false,
        };

        let items = match ss
            .search_items(
                [("service", SERVICE_NAME), ("account", MASTER_KEY_ACCOUNT)]
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
// "ironclaw:master_key" via CredWriteW. The Credential Manager seals the
// blob with DPAPI bound to the current Windows user, so the key is not
// recoverable from disk by other users on the same machine and never
// touches plaintext outside this process and the OS credential store.
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

    use windows::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME, GetLastError, WIN32_ERROR};
    use windows::Win32::Security::Credentials::{
        CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW,
        CredFree, CredReadW, CredWriteW,
    };
    use windows::core::{PCWSTR, PWSTR};

    use super::*;

    /// Build the credential target name as a NUL-terminated wide string.
    /// Format: "ironclaw:master_key" (matches `SERVICE_NAME:MASTER_KEY_ACCOUNT`).
    fn target_name_wide() -> Vec<u16> {
        let s = format!("{}:{}", SERVICE_NAME, MASTER_KEY_ACCOUNT);
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Convert the last Win32 error to a `SecretError::KeychainError`.
    fn keychain_err(action: &str) -> SecretError {
        // Safety: `GetLastError` is always callable; returns thread-local LastError.
        let code: WIN32_ERROR = unsafe { GetLastError() };
        SecretError::KeychainError(format!(
            "Failed to {} (Win32 error 0x{:08X})",
            action, code.0
        ))
    }

    /// Store the master key in the Windows Credential Manager.
    pub async fn store_master_key(key: &[u8]) -> Result<(), SecretError> {
        // Convert to hex for storage parity with macOS/Linux backends.
        let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
        let mut target = target_name_wide();
        let mut blob = key_hex.into_bytes();

        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            // CREDENTIALW struct fields are `PWSTR` (mutable wide strings) per the
            // `windows` 0.58 binding, while the `Cred*W` function arguments take
            // `PCWSTR`. Do not collapse these two — they are different types.
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
        result.map_err(|_| keychain_err("store in Credential Manager"))
    }

    /// Retrieve the master key from the Windows Credential Manager.
    pub async fn get_master_key() -> Result<Vec<u8>, SecretError> {
        let mut target = target_name_wide();
        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();

        // Safety: `target` is a valid NUL-terminated wide string. `cred_ptr` is
        // initialized via the FFI on success; we always pair successful reads
        // with `CredFree`.
        let result = unsafe {
            CredReadW(
                PCWSTR(target.as_mut_ptr()),
                CRED_TYPE_GENERIC,
                0,
                &mut cred_ptr,
            )
        };

        result.map_err(|_| keychain_err("read from Credential Manager"))?;

        if cred_ptr.is_null() {
            return Err(SecretError::KeychainError(
                "CredReadW returned success but null pointer".to_string(),
            ));
        }

        // Safety: `cred_ptr` is non-null and valid until `CredFree` below.
        // Copy the blob bytes into a Vec before freeing the OS allocation.
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

    /// Delete the master key from the Windows Credential Manager.
    pub async fn delete_master_key() -> Result<(), SecretError> {
        let mut target = target_name_wide();

        // Safety: `target` is a valid NUL-terminated wide string.
        let result = unsafe { CredDeleteW(PCWSTR(target.as_mut_ptr()), CRED_TYPE_GENERIC, 0) };

        match result {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => {
                // Idempotent: deleting a missing entry is not an error
                // (matches macOS/Linux behavior more closely).
                Ok(())
            }
            Err(_) => Err(keychain_err("delete from Credential Manager")),
        }
    }

    /// Check if a master key exists in the Credential Manager.
    pub async fn has_master_key() -> bool {
        let mut target = target_name_wide();
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

    pub async fn store_master_key(_key: &[u8]) -> Result<(), SecretError> {
        Err(SecretError::KeychainError(
            "Keychain not supported on this platform. Use SECRETS_MASTER_KEY env var.".to_string(),
        ))
    }

    pub async fn get_master_key() -> Result<Vec<u8>, SecretError> {
        Err(SecretError::KeychainError(
            "Keychain not supported on this platform. Use SECRETS_MASTER_KEY env var.".to_string(),
        ))
    }

    pub async fn delete_master_key() -> Result<(), SecretError> {
        Err(SecretError::KeychainError(
            "Keychain not supported on this platform".to_string(),
        ))
    }

    pub async fn has_master_key() -> bool {
        false
    }
}

// Re-export platform-specific functions
pub use platform::{delete_master_key, get_master_key, has_master_key, store_master_key};

/// Parse a hex string to bytes.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows", test))]
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, SecretError> {
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
    fn test_generate_master_key() {
        let key = generate_master_key();
        assert_eq!(key.len(), 32);

        // Should be different each time
        let key2 = generate_master_key();
        assert_ne!(key, key2);
    }

    #[test]
    fn test_generate_master_key_hex() {
        let hex = generate_master_key_hex();
        assert_eq!(hex.len(), 64); // 32 bytes * 2 hex chars
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hex_to_bytes() {
        let result = hex_to_bytes("deadbeef").unwrap();
        assert_eq!(result, vec![0xde, 0xad, 0xbe, 0xef]);

        let result = hex_to_bytes("00ff").unwrap();
        assert_eq!(result, vec![0x00, 0xff]);
    }

    #[test]
    fn test_hex_to_bytes_invalid() {
        assert!(hex_to_bytes("abc").is_err()); // Odd length
        assert!(hex_to_bytes("gg").is_err()); // Invalid chars
    }

    /// Windows Credential Manager round-trip — gated to Windows so CI runs it
    /// in the windows-ci.yml job. Local macOS/Linux dev does not exercise the
    /// Win32 path, but the cfg-gated module is still type-checked when this
    /// crate is built for `x86_64-pc-windows-msvc`.
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn windows_credential_manager_roundtrip() {
        // Use a deterministic key so failures point at the keychain layer,
        // not the RNG.
        let key: Vec<u8> = (0u8..32).collect();

        // Best-effort cleanup before/after to keep the test idempotent
        // when re-run on the same Windows agent.
        let _ = delete_master_key().await;

        store_master_key(&key)
            .await
            .expect("store_master_key must succeed");
        assert!(
            has_master_key().await,
            "credential should exist after store"
        );

        let read_back = get_master_key().await.expect("get_master_key must succeed");
        assert_eq!(read_back, key, "round-trip must preserve key bytes");

        delete_master_key()
            .await
            .expect("delete_master_key must succeed");
        assert!(
            !has_master_key().await,
            "credential should be gone after delete"
        );

        // Idempotent delete (mirrors macOS/Linux behavior).
        delete_master_key()
            .await
            .expect("delete on missing entry must not error");
    }
}
