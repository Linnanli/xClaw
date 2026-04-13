use std::collections::{HashMap, HashSet};

use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use ironclaw::db::Database;
use serde::{Deserialize, Serialize};

pub const SIGNED_POLICY_CACHE_KEY: &str = "desktop_signed_managed_policy";
pub const SIGNED_POLICY_VERSION_KEY: &str = "desktop_signed_managed_policy_version";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedPolicyManifest {
    pub policy_version: u64,
    pub issued_at: String,
    pub expires_at: String,
    pub managed_mode: bool,
    pub allowed_skills: Vec<String>,
    pub allowed_extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManagedPolicyEnvelope {
    pub algorithm: String,
    pub key_id: String,
    pub manifest_payload: String,
    pub signature: String,
}

#[derive(Debug, Clone)]
pub struct ManagedPolicySnapshot {
    pub manifest: ManagedPolicyManifest,
    allowed_skills: HashSet<String>,
    allowed_extensions: HashSet<String>,
}

impl ManagedPolicySnapshot {
    pub fn allows_skill(&self, name: &str) -> bool {
        self.allowed_skills.contains(name)
    }

    pub fn allows_extension(&self, name: &str) -> bool {
        self.allowed_extensions.contains(name)
    }

    pub fn allowed_skill_set(&self) -> HashSet<String> {
        self.allowed_skills.clone()
    }

    pub fn allowed_extension_set(&self) -> HashSet<String> {
        self.allowed_extensions.clone()
    }
}

pub async fn fetch_signed_policy(
    http_client: &reqwest::Client,
    admin_url: &str,
    client_token: &str,
) -> Result<SignedManagedPolicyEnvelope, String> {
    let mut url = format!("{}/api/client-policy", admin_url.trim_end_matches('/'));
    if uuid::Uuid::parse_str(client_token).is_ok() {
        url = format!("{}?client_id={}", url, client_token);
    }

    let response = http_client
        .get(url)
        .bearer_auth(client_token)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch client policy: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Client policy endpoint returned {}", response.status()));
    }

    response
        .json::<SignedManagedPolicyEnvelope>()
        .await
        .map_err(|e| format!("Failed to parse client policy response: {}", e))
}

pub fn managed_policy_public_key_from_env() -> Result<String, String> {
    std::env::var("MANAGED_POLICY_PUBLIC_KEY_B64").map_err(|_| {
        "MANAGED_POLICY_PUBLIC_KEY_B64 is required when MANAGED_POLICY_PUBLIC_KEYS_JSON is unset"
            .to_string()
    })
}

pub fn managed_policy_public_keys_from_env() -> Result<HashMap<String, String>, String> {
    if let Ok(raw) = std::env::var("MANAGED_POLICY_PUBLIC_KEYS_JSON") {
        return parse_public_key_map(&raw);
    }

    let key = managed_policy_public_key_from_env()?;
    let key_id = default_policy_key_id();
    let mut key_map = HashMap::new();
    key_map.insert(key_id, key);
    Ok(key_map)
}

pub fn verify_signed_policy_with_env(
    envelope: &SignedManagedPolicyEnvelope,
) -> Result<ManagedPolicySnapshot, String> {
    let key_map = managed_policy_public_keys_from_env()?;
    let public_key = key_map
        .get(&envelope.key_id)
        .cloned()
        .ok_or_else(|| format!("No public key configured for key_id '{}'", envelope.key_id))?;
    verify_signed_policy(envelope, &public_key)
}

pub async fn cache_signed_policy_in_store(
    db: &dyn Database,
    owner_id: &str,
    envelope: &SignedManagedPolicyEnvelope,
) -> Result<(), String> {
    let value = serde_json::to_value(envelope)
        .map_err(|e| format!("Failed to serialize signed policy envelope: {}", e))?;
    db.set_setting(owner_id, SIGNED_POLICY_CACHE_KEY, &value)
        .await
        .map_err(|e| format!("Failed to cache signed policy envelope: {}", e))
}

pub async fn ensure_policy_version_monotonic(
    db: &dyn Database,
    owner_id: &str,
    incoming_version: u64,
) -> Result<(), String> {
    let latest = load_cached_policy_version(db, owner_id).await?;
    ensure_newer_policy_version(incoming_version, latest)?;
    persist_cached_policy_version(db, owner_id, incoming_version).await
}

fn ensure_newer_policy_version(incoming_version: u64, latest: Option<u64>) -> Result<(), String> {
    if let Some(latest) = latest {
        if incoming_version <= latest {
            return Err(format!(
                "Rejected non-increasing policy_version {} (latest accepted {})",
                incoming_version, latest
            ));
        }
    }
    Ok(())
}

pub async fn load_verified_policy_from_store(
    db: &dyn Database,
    owner_id: &str,
) -> Result<Option<ManagedPolicySnapshot>, String> {
    let value = db
        .get_setting(owner_id, SIGNED_POLICY_CACHE_KEY)
        .await
        .map_err(|e| format!("Failed to load signed policy envelope: {}", e))?;

    let Some(value) = value else {
        return Ok(None);
    };

    let envelope = serde_json::from_value::<SignedManagedPolicyEnvelope>(value)
        .map_err(|e| format!("Invalid signed policy envelope payload: {}", e))?;
    let policy = verify_signed_policy_with_env(&envelope)?;

    if let Some(latest) = load_cached_policy_version(db, owner_id).await? {
        if policy.manifest.policy_version < latest {
            return Err(format!(
                "Cached policy version {} is older than latest accepted version {}",
                policy.manifest.policy_version, latest
            ));
        }
    }

    Ok(Some(policy))
}

pub fn verify_signed_policy(
    envelope: &SignedManagedPolicyEnvelope,
    public_key_b64: &str,
) -> Result<ManagedPolicySnapshot, String> {
    if envelope.algorithm.to_ascii_lowercase() != "ed25519" {
        return Err(format!("Unsupported policy signature algorithm: {}", envelope.algorithm));
    }

    let verifying_key = decode_public_key(public_key_b64)?;
    let signature = decode_signature(&envelope.signature)?;

    verifying_key
        .verify(envelope.manifest_payload.as_bytes(), &signature)
        .map_err(|_| "Policy signature verification failed".to_string())?;

    let manifest = serde_json::from_str::<ManagedPolicyManifest>(&envelope.manifest_payload)
        .map_err(|e| format!("Failed to parse manifest payload: {}", e))?;

    let expires_at = parse_rfc3339(&manifest.expires_at, "expires_at")?;
    if expires_at < Utc::now() {
        return Err("Policy has expired".to_string());
    }

    if manifest.policy_version == 0 {
        return Err("policy_version must be greater than 0".to_string());
    }

    let issued_at = parse_rfc3339(&manifest.issued_at, "issued_at")?;
    if issued_at > expires_at {
        return Err("issued_at cannot be later than expires_at".to_string());
    }

    let max_future = Utc::now() + chrono::Duration::minutes(5);
    if issued_at > max_future {
        return Err("issued_at is too far in the future".to_string());
    }

    Ok(ManagedPolicySnapshot {
        allowed_skills: manifest.allowed_skills.iter().cloned().collect(),
        allowed_extensions: manifest.allowed_extensions.iter().cloned().collect(),
        manifest,
    })
}

fn parse_rfc3339(value: &str, field: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("Invalid {} timestamp: {}", field, e))
}

fn decode_public_key(public_key_b64: &str) -> Result<VerifyingKey, String> {
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64)
        .map_err(|e| format!("Invalid MANAGED_POLICY_PUBLIC_KEY_B64: {}", e))?;
    let key_array: [u8; 32] = key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "MANAGED_POLICY_PUBLIC_KEY_B64 must decode to 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&key_array)
        .map_err(|e| format!("Invalid MANAGED_POLICY public key: {}", e))
}

fn decode_signature(signature_b64: &str) -> Result<Signature, String> {
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| format!("Invalid policy signature encoding: {}", e))?;
    let sig_array: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Policy signature must decode to 64 bytes".to_string())?;
    Ok(Signature::from_bytes(&sig_array))
}

async fn load_cached_policy_version(db: &dyn Database, owner_id: &str) -> Result<Option<u64>, String> {
    let value = db
        .get_setting(owner_id, SIGNED_POLICY_VERSION_KEY)
        .await
        .map_err(|e| format!("Failed to load signed policy version: {}", e))?;
    let Some(value) = value else {
        return Ok(None);
    };

    serde_json::from_value::<u64>(value)
        .map(Some)
        .map_err(|e| format!("Invalid signed policy version payload: {}", e))
}

async fn persist_cached_policy_version(
    db: &dyn Database,
    owner_id: &str,
    version: u64,
) -> Result<(), String> {
    db.set_setting(owner_id, SIGNED_POLICY_VERSION_KEY, &serde_json::json!(version))
        .await
        .map_err(|e| format!("Failed to persist signed policy version: {}", e))
}

fn default_policy_key_id() -> String {
    std::env::var("MANAGED_POLICY_KEY_ID").unwrap_or_else(|_| "managed-policy-key-v1".to_string())
}

fn parse_public_key_map(raw: &str) -> Result<HashMap<String, String>, String> {
    let parsed = serde_json::from_str::<HashMap<String, String>>(raw)
        .map_err(|e| format!("Invalid MANAGED_POLICY_PUBLIC_KEYS_JSON: {}", e))?;
    if parsed.is_empty() {
        return Err("MANAGED_POLICY_PUBLIC_KEYS_JSON must not be empty".to_string());
    }

    let mut sanitized = HashMap::new();
    for (key_id, key_b64) in parsed {
        let normalized_key_id = key_id.trim().to_string();
        let normalized_key = key_b64.trim().to_string();
        if normalized_key_id.is_empty() {
            return Err("MANAGED_POLICY_PUBLIC_KEYS_JSON contains empty key_id".to_string());
        }
        if normalized_key.is_empty() {
            return Err(format!(
                "MANAGED_POLICY_PUBLIC_KEYS_JSON key '{}' has empty public key",
                normalized_key_id
            ));
        }
        sanitized.insert(normalized_key_id, normalized_key);
    }

    Ok(sanitized)
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use chrono::{Duration, Utc};
    use ed25519_dalek::{Signer, SigningKey};

    use super::{
        ManagedPolicyManifest, SignedManagedPolicyEnvelope, ensure_newer_policy_version,
        verify_signed_policy,
    };

    fn signed_envelope(
        manifest: &ManagedPolicyManifest,
        signing_key: &SigningKey,
    ) -> SignedManagedPolicyEnvelope {
        let payload = serde_json::to_string(manifest).expect("serialize manifest");
        let signature = signing_key.sign(payload.as_bytes());
        SignedManagedPolicyEnvelope {
            algorithm: "ed25519".to_string(),
            key_id: "test-key".to_string(),
            manifest_payload: payload,
            signature: base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()),
        }
    }

    #[test]
    fn test_verify_signed_policy_success() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let verifying_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());
        let manifest = ManagedPolicyManifest {
            policy_version: 1,
            issued_at: Utc::now().to_rfc3339(),
            expires_at: (Utc::now() + Duration::hours(1)).to_rfc3339(),
            managed_mode: true,
            allowed_skills: vec!["review-checklist".to_string()],
            allowed_extensions: vec!["github".to_string()],
        };
        let envelope = signed_envelope(&manifest, &signing_key);

        let verified = verify_signed_policy(&envelope, &verifying_key_b64).expect("verify policy");
        assert!(verified.allows_skill("review-checklist"));
        assert!(verified.allows_extension("github"));
    }

    #[test]
    fn test_verify_signed_policy_rejects_expired_policy() {
        let signing_key = SigningKey::from_bytes(&[9u8; 32]);
        let verifying_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());
        let manifest = ManagedPolicyManifest {
            policy_version: 1,
            issued_at: (Utc::now() - Duration::hours(2)).to_rfc3339(),
            expires_at: (Utc::now() - Duration::hours(1)).to_rfc3339(),
            managed_mode: true,
            allowed_skills: vec![],
            allowed_extensions: vec![],
        };
        let envelope = signed_envelope(&manifest, &signing_key);

        let result = verify_signed_policy(&envelope, &verifying_key_b64);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_signed_policy_rejects_zero_policy_version() {
        let signing_key = SigningKey::from_bytes(&[5u8; 32]);
        let verifying_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());
        let manifest = ManagedPolicyManifest {
            policy_version: 0,
            issued_at: Utc::now().to_rfc3339(),
            expires_at: (Utc::now() + Duration::hours(1)).to_rfc3339(),
            managed_mode: true,
            allowed_skills: vec![],
            allowed_extensions: vec![],
        };
        let envelope = signed_envelope(&manifest, &signing_key);

        let result = verify_signed_policy(&envelope, &verifying_key_b64);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_signed_policy_rejects_future_issued_at() {
        let signing_key = SigningKey::from_bytes(&[3u8; 32]);
        let verifying_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());
        let manifest = ManagedPolicyManifest {
            policy_version: 1,
            issued_at: (Utc::now() + Duration::minutes(10)).to_rfc3339(),
            expires_at: (Utc::now() + Duration::hours(1)).to_rfc3339(),
            managed_mode: true,
            allowed_skills: vec![],
            allowed_extensions: vec![],
        };
        let envelope = signed_envelope(&manifest, &signing_key);

        let result = verify_signed_policy(&envelope, &verifying_key_b64);
        assert!(result.is_err());
    }

    #[test]
    fn test_ensure_newer_policy_version_rejects_equal_version() {
        let result = ensure_newer_policy_version(42, Some(42));
        assert!(result.is_err());
    }

    #[test]
    fn test_ensure_newer_policy_version_rejects_older_version() {
        let result = ensure_newer_policy_version(41, Some(42));
        assert!(result.is_err());
    }

    #[test]
    fn test_ensure_newer_policy_version_accepts_newer_version() {
        let result = ensure_newer_policy_version(43, Some(42));
        assert!(result.is_ok());
    }
}
