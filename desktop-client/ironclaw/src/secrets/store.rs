//! Re-export shim for the migrated secrets storage backends.
//!
//! All storage impls (`PostgresSecretsStore`, `LibSqlSecretsStore`,
//! `InMemorySecretsStore`) and the `SecretsStore` trait now live in
//! `dasclaw_runtime::secrets::*` (F3.2 phase 2 PR 4c, #641). This shim only
//! exists so existing ironclaw call sites (~20+ across settings/config/
//! testing/extension_tools/manager) keep resolving the same paths
//! (`crate::secrets::store::SecretsStore`,
//! `crate::secrets::store::in_memory::InMemorySecretsStore`) without
//! requiring a workspace-wide import sweep.

pub use dasclaw_runtime::secrets::SecretsStore;

#[cfg(feature = "postgres")]
pub use dasclaw_runtime::secrets::PostgresSecretsStore;

#[cfg(feature = "libsql")]
pub use dasclaw_runtime::secrets::LibSqlSecretsStore;

/// Compatibility shim: ironclaw historically exposed the in-memory store at
/// `crate::secrets::store::in_memory::InMemorySecretsStore`. Preserve that
/// path so the existing call sites don't have to be touched.
pub mod in_memory {
    pub use dasclaw_runtime::secrets::InMemorySecretsStore;
}

#[cfg(test)]
mod tests {
    use crate::secrets::store::SecretsStore;
    use crate::secrets::types::CreateSecretParams;
    use crate::testing::credentials::{
        TEST_OPENAI_API_KEY_SHORT, TEST_SECRET_VALUE, TEST_STRIPE_KEY, test_secrets_store,
    };

    fn test_store() -> crate::secrets::store::in_memory::InMemorySecretsStore {
        test_secrets_store()
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let store = test_store();
        let params = CreateSecretParams::new("api_key", TEST_SECRET_VALUE);

        store.create("user1", params).await.unwrap();

        let decrypted = store.get_decrypted("user1", "api_key").await.unwrap();
        assert_eq!(decrypted.expose(), TEST_SECRET_VALUE);
    }

    #[tokio::test]
    async fn test_exists() {
        let store = test_store();
        let params = CreateSecretParams::new("my_secret", "value");

        assert!(!store.exists("user1", "my_secret").await.unwrap());
        store.create("user1", params).await.unwrap();
        assert!(store.exists("user1", "my_secret").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete() {
        let store = test_store();
        let params = CreateSecretParams::new("to_delete", "value");

        store.create("user1", params).await.unwrap();
        assert!(store.exists("user1", "to_delete").await.unwrap());

        store.delete("user1", "to_delete").await.unwrap();
        assert!(!store.exists("user1", "to_delete").await.unwrap());
    }

    #[tokio::test]
    async fn test_list() {
        let store = test_store();

        store
            .create("user1", CreateSecretParams::new("key1", "v1"))
            .await
            .unwrap();
        store
            .create(
                "user1",
                CreateSecretParams::new("key2", "v2").with_provider("openai"),
            )
            .await
            .unwrap();
        store
            .create("user2", CreateSecretParams::new("key3", "v3"))
            .await
            .unwrap();

        let list = store.list("user1").await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_is_accessible() {
        let store = test_store();
        store
            .create(
                "user1",
                CreateSecretParams::new("openai_key", TEST_OPENAI_API_KEY_SHORT),
            )
            .await
            .unwrap();
        store
            .create(
                "user1",
                CreateSecretParams::new("stripe_key", TEST_STRIPE_KEY),
            )
            .await
            .unwrap();

        // Exact match
        let allowed = vec!["openai_key".to_string()];
        assert!(
            store
                .is_accessible("user1", "openai_key", &allowed)
                .await
                .unwrap()
        );
        assert!(
            !store
                .is_accessible("user1", "stripe_key", &allowed)
                .await
                .unwrap()
        );

        // Glob pattern
        let allowed = vec!["openai_*".to_string()];
        assert!(
            store
                .is_accessible("user1", "openai_key", &allowed)
                .await
                .unwrap()
        );
        assert!(
            !store
                .is_accessible("user1", "stripe_key", &allowed)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_expired_secret_returns_error() {
        let store = test_store();
        let expires_at = chrono::Utc::now() - chrono::Duration::hours(1);
        let params = CreateSecretParams::new("expired_key", "value").with_expiry(expires_at);

        store.create("user1", params).await.unwrap();

        let result = store.get("user1", "expired_key").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::secrets::SecretError::Expired
        ));
    }

    #[tokio::test]
    async fn test_non_expired_secret_succeeds() {
        let store = test_store();
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);
        let params = CreateSecretParams::new("fresh_key", "value").with_expiry(expires_at);

        store.create("user1", params).await.unwrap();

        let result = store.get("user1", "fresh_key").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_user_isolation() {
        let store = test_store();

        store
            .create(
                "user1",
                CreateSecretParams::new("shared_name", "user1_value"),
            )
            .await
            .unwrap();
        store
            .create(
                "user2",
                CreateSecretParams::new("shared_name", "user2_value"),
            )
            .await
            .unwrap();

        let v1 = store.get_decrypted("user1", "shared_name").await.unwrap();
        let v2 = store.get_decrypted("user2", "shared_name").await.unwrap();

        assert_eq!(v1.expose(), "user1_value");
        assert_eq!(v2.expose(), "user2_value");
    }
}
