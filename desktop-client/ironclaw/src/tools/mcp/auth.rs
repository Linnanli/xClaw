//! OAuth orchestration for MCP servers — ironclaw desktop host.
//!
//! This module is a **thin host adapter** on top of the host-agnostic
//! OAuth 2.1 protocol layer in [`dasclaw_mcp::auth`]. The protocol surface
//! (PKCE, discovery, dynamic client registration, authorization-URL
//! construction, code-for-token exchange, direct refresh, and token
//! storage) lives in `dasclaw_mcp` so future hosts (admin-backend web,
//! CLI, …) can reuse the exact same wire-level implementation.
//!
//! ## What this file owns
//!
//! - [`authorize_mcp_server`] — the desktop GUI authorization flow: binds a
//!   localhost callback listener, opens the system browser, waits for the
//!   authorization code, then delegates token exchange and storage to
//!   `dasclaw_mcp::auth`.
//! - [`find_available_port`] — binds the ironclaw-specific fixed callback
//!   port via `oauth_defaults::bind_callback_listener`.
//! - [`wait_for_authorization_callback`] — drives the localhost callback
//!   handler in `oauth_defaults`.
//! - [`refresh_access_token`] — dispatches between the optional
//!   hosted-OAuth-proxy refresh path (an ironclaw deployment-mode feature
//!   keyed off `IRONCLAW_OAUTH_EXCHANGE_URL`) and the direct token-endpoint
//!   refresh in `dasclaw_mcp::auth::refresh_access_token_direct`.
//!
//! ## Why the split
//!
//! Migrated from a 2 341-line monolithic `auth.rs` in F3.2 phase 2 PR 5b
//! (#641). The pre-split file mixed three concerns that ship on different
//! hosts:
//!
//! 1. **OAuth wire protocol** (host-agnostic) → moved to `dasclaw_mcp::auth`.
//! 2. **Localhost callback delivery + browser opening** (desktop-only) →
//!    stayed here.
//! 3. **Hosted-proxy refresh fallback** (ironclaw deployment-specific) →
//!    stayed here, wrapping the direct refresh.
//!
//! Future-AI note: any change to the *protocol* (spec rev, new RFC) belongs
//! in `dasclaw_mcp::auth`. Any change to *how a code reaches our process*
//! (a new redirect URI shape, a CLI device-flow, an admin-backend HTTPS
//! callback) belongs in the corresponding host crate, not here.

use std::collections::HashMap;
use std::sync::Arc;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use tokio::net::TcpListener;

pub use dasclaw_mcp::auth::{
    AccessToken, AuthError, ClientCredentials, DEFAULT_DCR_CLIENT_NAME, PkceChallenge,
    build_authorization_url, canonical_resource_uri, discover_full_oauth_metadata,
    discover_oauth_endpoints, exchange_code_for_token, get_access_token, is_authenticated,
    refresh_access_token_direct, refresh_lock, register_client, store_client_id,
    store_client_secret, store_tokens, validate_url_safe,
};
// Re-export the host-agnostic surface so existing call-sites
// (`crate::tools::mcp::auth::AuthError`, etc.) keep working without churn.
pub use dasclaw_mcp::auth::{
    AuthorizationServerMetadata, ClientRegistrationRequest, ClientRegistrationResponse,
    ProtectedResourceMetadata, build_well_known_uri, discover_authorization_server,
    discover_protected_resource,
};

use crate::cli::oauth_defaults::{self, OAUTH_CALLBACK_PORT};
use crate::secrets::SecretsStore;
use crate::tools::mcp::config::McpServerConfig;

/// Perform the OAuth 2.1 authorization flow for an MCP server (desktop GUI).
///
/// Supports two modes:
/// 1. Pre-configured OAuth: uses the `client_id` from server config.
/// 2. Dynamic Client Registration: discovers and registers with the server.
///
/// Flow:
/// 1. Binds the localhost callback listener on the fixed port.
/// 2. Discovers authorization endpoints (or runs DCR if no `client_id`).
/// 3. Generates PKCE challenge + CSRF state.
/// 4. Opens the system browser for user authorization.
/// 5. Awaits the callback and extracts the authorization code.
/// 6. Delegates token exchange to `dasclaw_mcp::auth::exchange_code_for_token`.
/// 7. Stores tokens via `dasclaw_mcp::auth::store_tokens`.
pub async fn authorize_mcp_server(
    server_config: &McpServerConfig,
    secrets: &Arc<dyn SecretsStore + Send + Sync>,
    user_id: &str,
) -> Result<AccessToken, AuthError> {
    let (listener, port) = find_available_port().await?;
    let host = oauth_defaults::callback_host();
    let redirect_uri = format!("http://{}:{}/callback", host, port);

    if !oauth_defaults::is_loopback_host(&host) {
        println!("Warning: MCP OAuth callback is using plain HTTP to a remote host ({host}).");
        println!("         Authorization codes will be transmitted unencrypted.");
        println!("         Consider SSH port forwarding instead:");
        println!("           ssh -L {port}:127.0.0.1:{port} user@{host}");
    }

    let (
        client_id,
        client_secret,
        client_secret_expires_at,
        authorization_url,
        token_url,
        use_pkce,
        scopes,
        mut extra_params,
    ) = if let Some(oauth) = &server_config.oauth {
        let (auth_url, tok_url) = discover_oauth_endpoints(server_config).await?;
        (
            oauth.client_id.clone(),
            None,
            None,
            auth_url,
            tok_url,
            oauth.use_pkce,
            oauth.scopes.clone(),
            oauth.extra_params.clone(),
        )
    } else {
        println!("  Discovering OAuth endpoints...");
        let auth_meta = discover_full_oauth_metadata(&server_config.url).await?;
        let registration_endpoint = auth_meta
            .registration_endpoint
            .ok_or(AuthError::NotSupported)?;

        println!("  Registering client dynamically...");
        let registration = register_client(
            &registration_endpoint,
            &redirect_uri,
            DEFAULT_DCR_CLIENT_NAME,
        )
        .await?;
        println!("  Client registered: {}", registration.client_id);

        (
            registration.client_id,
            registration.client_secret,
            registration.client_secret_expires_at,
            auth_meta.authorization_endpoint,
            auth_meta.token_endpoint,
            true,
            auth_meta.scopes_supported,
            HashMap::new(),
        )
    };

    let pkce = if use_pkce {
        Some(PkceChallenge::generate())
    } else {
        None
    };

    let mut state_bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut state_bytes);
    let state = URL_SAFE_NO_PAD.encode(state_bytes);
    extra_params.insert("state".to_string(), state);

    let resource = canonical_resource_uri(&server_config.url);

    validate_url_safe(&authorization_url)
        .await
        .map_err(|e| AuthError::DiscoveryFailed(format!("Unsafe authorization endpoint: {}", e)))?;

    let auth_url = build_authorization_url(
        &authorization_url,
        &client_id,
        &redirect_uri,
        &scopes,
        pkce.as_ref(),
        &extra_params,
        Some(&resource),
    );

    println!("  Opening browser for {} login...", server_config.name);
    if let Err(e) = open::that(&auth_url) {
        println!("  Could not open browser: {}", e);
        println!("  Please open this URL manually:");
        println!("  {}", auth_url);
    }

    println!("  Waiting for authorization...");

    let code = wait_for_authorization_callback(listener, &server_config.name).await?;

    println!("  Exchanging code for token...");
    let token = exchange_code_for_token(
        &token_url,
        &client_id,
        client_secret.as_deref(),
        &code,
        &redirect_uri,
        pkce.as_ref(),
        Some(&resource),
    )
    .await?;

    store_tokens(secrets, user_id, server_config, &token).await?;

    if server_config.oauth.is_none() {
        store_client_id(secrets, user_id, server_config, &client_id).await?;
        if let Some(ref client_secret) = client_secret {
            store_client_secret(
                secrets,
                user_id,
                server_config,
                client_secret,
                client_secret_expires_at,
            )
            .await?;
        }
    }

    Ok(token)
}

/// Bind the OAuth callback listener on the shared fixed port.
pub async fn find_available_port() -> Result<(TcpListener, u16), AuthError> {
    let listener = oauth_defaults::bind_callback_listener()
        .await
        .map_err(|_| AuthError::PortUnavailable)?;
    Ok((listener, OAUTH_CALLBACK_PORT))
}

/// Wait for the authorization callback and extract the code.
pub async fn wait_for_authorization_callback(
    listener: TcpListener,
    server_name: &str,
) -> Result<String, AuthError> {
    oauth_defaults::wait_for_callback(listener, "/callback", "code", server_name, None)
        .await
        .map_err(|e| match e {
            oauth_defaults::OAuthCallbackError::Denied => AuthError::AuthorizationDenied,
            oauth_defaults::OAuthCallbackError::Timeout => AuthError::Timeout,
            oauth_defaults::OAuthCallbackError::PortInUse(_, msg) => {
                AuthError::Http(format!("Port error: {}", msg))
            }
            oauth_defaults::OAuthCallbackError::StateMismatch { .. } => {
                AuthError::Http("CSRF state mismatch in OAuth callback".to_string())
            }
            oauth_defaults::OAuthCallbackError::Io(msg) => AuthError::Http(msg),
        })
}

/// Refresh an access token, dispatching between the hosted OAuth proxy and
/// the direct token endpoint.
///
/// - If `oauth_defaults::exchange_proxy_url()` is set, refreshes are routed
///   through the proxy gateway (requires `IRONCLAW_OAUTH_PROXY_AUTH_TOKEN`).
///   This branch is **ironclaw-specific** and lives here, not in
///   `dasclaw_mcp::auth`.
/// - Otherwise, delegates to
///   [`dasclaw_mcp::auth::refresh_access_token_direct`], which performs the
///   standard direct token-endpoint refresh.
///
/// Other hosts (admin-backend web, CLI) should call
/// `refresh_access_token_direct` directly; the proxy dispatch is not part
/// of the host-agnostic protocol surface.
pub async fn refresh_access_token(
    server_config: &McpServerConfig,
    secrets: &Arc<dyn SecretsStore + Send + Sync>,
    user_id: &str,
) -> Result<AccessToken, AuthError> {
    let proxy_url = match oauth_defaults::exchange_proxy_url() {
        Some(url) => url,
        None => {
            return refresh_access_token_direct(server_config, secrets, user_id).await;
        }
    };

    // Serialize concurrent refreshes for the same (server, user) so we don't
    // burn through refresh tokens or trigger rate-limits on the proxy.
    let lock = refresh_lock(&server_config.name, user_id).await;
    let _guard = lock.lock().await;

    // Fast-path: if a sibling refresh just stored a fresh access token, reuse it.
    // Mirror `refresh_access_token_direct`'s error shape: bubble real
    // infrastructure failures (DB / decrypt) instead of silently falling
    // through to the refresh-token lookup, which would mask them as
    // `RefreshFailed("No refresh token: ...")`.
    match secrets
        .get_decrypted(user_id, &server_config.token_secret_name())
        .await
    {
        Ok(token) => {
            return Ok(AccessToken {
                access_token: token.expose().to_string(),
                token_type: "Bearer".to_string(),
                expires_in: None,
                refresh_token: None,
                scope: None,
            });
        }
        Err(crate::secrets::SecretError::NotFound(_) | crate::secrets::SecretError::Expired) => {}
        Err(e) => return Err(AuthError::Secrets(e.to_string())),
    }

    let credentials = ClientCredentials::for_server(server_config, secrets, user_id).await?;

    let refresh_token = match secrets
        .get_decrypted(user_id, &server_config.refresh_token_secret_name())
        .await
    {
        Ok(token) => token,
        Err(crate::secrets::SecretError::NotFound(_) | crate::secrets::SecretError::Expired) => {
            secrets
                .get_decrypted(user_id, &server_config.legacy_refresh_token_secret_name())
                .await
                .map_err(|e| AuthError::RefreshFailed(format!("No refresh token: {}", e)))?
        }
        Err(e) => {
            return Err(AuthError::RefreshFailed(format!(
                "Failed to read refresh token: {e}"
            )));
        }
    };

    let token_url = if let Some(ref oauth) = server_config.oauth {
        if let Some(ref url) = oauth.token_url {
            url.clone()
        } else {
            discover_full_oauth_metadata(&server_config.url)
                .await?
                .token_endpoint
        }
    } else {
        discover_full_oauth_metadata(&server_config.url)
            .await?
            .token_endpoint
    };

    validate_url_safe(&token_url).await?;

    let resource = canonical_resource_uri(&server_config.url);
    let provider = format!("mcp:{}", server_config.name);
    let gateway_token = oauth_defaults::oauth_proxy_auth_token().ok_or_else(|| {
        AuthError::RefreshFailed(
            "OAuth refresh proxy is configured but no proxy auth token is available".to_string(),
        )
    })?;
    let token_response =
        oauth_defaults::refresh_token_via_proxy(oauth_defaults::ProxyRefreshTokenRequest {
            proxy_url: &proxy_url,
            gateway_token: &gateway_token,
            token_url: &token_url,
            client_id: &credentials.client_id,
            client_secret: credentials.client_secret.as_deref(),
            refresh_token: refresh_token.expose(),
            resource: Some(&resource),
            provider: Some(provider.as_str()),
        })
        .await
        .map_err(|e| AuthError::RefreshFailed(e.to_string()))?;

    let token = AccessToken {
        access_token: token_response.access_token,
        token_type: token_response
            .token_type
            .unwrap_or_else(|| "Bearer".to_string()),
        expires_in: token_response.expires_in,
        refresh_token: token_response.refresh_token,
        scope: token_response.scope,
    };

    store_tokens(secrets, user_id, server_config, &token).await?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::{
        Router,
        extract::{Form, State},
        routing::post,
    };
    use secrecy::SecretString;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use crate::config::helpers::lock_env;
    use crate::secrets::{CreateSecretParams, InMemorySecretsStore, SecretsCrypto};
    use crate::testing::credentials::TEST_GATEWAY_CRYPTO_KEY;

    #[derive(Clone, Debug, Default)]
    struct RecordedRefreshRequest {
        authorization: Option<String>,
        form: HashMap<String, String>,
    }

    #[derive(Clone, Default)]
    struct MockRefreshState {
        requests: Arc<Mutex<Vec<RecordedRefreshRequest>>>,
    }

    impl MockRefreshState {
        async fn requests(&self) -> Vec<RecordedRefreshRequest> {
            self.requests.lock().await.clone()
        }
    }

    fn test_secrets_store() -> Arc<dyn SecretsStore + Send + Sync> {
        Arc::new(InMemorySecretsStore::new(Arc::new(
            SecretsCrypto::new(SecretString::from(TEST_GATEWAY_CRYPTO_KEY.to_string()))
                .expect("test crypto"),
        )))
    }

    async fn start_refresh_server() -> Option<(String, MockRefreshState)> {
        async fn token_handler(
            State(state): State<MockRefreshState>,
            headers: axum::http::HeaderMap,
            Form(form): Form<HashMap<String, String>>,
        ) -> axum::Json<serde_json::Value> {
            state.requests.lock().await.push(RecordedRefreshRequest {
                authorization: headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string),
                form,
            });
            axum::Json(serde_json::json!({
                "access_token": "refreshed-access-token",
                "token_type": "Bearer",
                "refresh_token": "rotated-refresh-token",
                "expires_in": 3600
            }))
        }

        let state = MockRefreshState::default();
        let app = Router::new()
            .route("/token", post(token_handler))
            .route("/oauth/refresh", post(token_handler))
            .with_state(state.clone());
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("Skipping refresh server test: loopback bind denied by sandbox");
                return None;
            }
            Err(error) => panic!("failed to bind refresh test server: {error}"),
        };
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Some((format!("http://127.0.0.1:{}", addr.port()), state))
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: Tests use lock_env() to serialize environment access.
            unsafe {
                if let Some(ref value) = self.original {
                    std::env::set_var(self.key, value);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    fn set_env_var(key: &'static str, value: Option<&str>) -> EnvVarGuard {
        let original = std::env::var(key).ok();
        // SAFETY: Tests use lock_env() to serialize environment access.
        unsafe {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
        EnvVarGuard { key, original }
    }

    /// Proxy-refresh path is ironclaw-specific (env-var driven), so its
    /// dedicated test lives here, not in `dasclaw_mcp::auth`.
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn test_refresh_access_token_uses_proxy_when_configured() {
        let _env_guard = lock_env();
        let Some((base_url, state)) = start_refresh_server().await else {
            return;
        };
        let _proxy_url_guard = set_env_var("IRONCLAW_OAUTH_EXCHANGE_URL", Some(&base_url));
        let _proxy_token_guard = set_env_var(
            "IRONCLAW_OAUTH_PROXY_AUTH_TOKEN",
            Some("gateway-test-token"),
        );
        let expected_token_url = format!("{base_url}/token");

        let secrets = test_secrets_store();
        let user_id = "test-user";
        let server = McpServerConfig::new("notion", "https://mcp.notion.com/mcp").with_oauth(
            crate::tools::mcp::config::OAuthConfig::new("configured-client")
                .with_endpoints("http://127.0.0.1/authorize", expected_token_url.clone()),
        );

        secrets
            .create(
                user_id,
                CreateSecretParams::new(server.refresh_token_secret_name(), "refresh-token-123"),
            )
            .await
            .unwrap();
        store_client_secret(&secrets, user_id, &server, "stored-client-secret", None)
            .await
            .unwrap();

        refresh_access_token(&server, &secrets, user_id)
            .await
            .expect("proxy refresh succeeds");

        let requests = state.requests().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer gateway-test-token")
        );
        assert_eq!(
            requests[0].form.get("token_url").map(String::as_str),
            Some(expected_token_url.as_str())
        );
        assert_eq!(
            requests[0].form.get("provider").map(String::as_str),
            Some("mcp:notion")
        );
        assert_eq!(
            requests[0].form.get("client_secret").map(String::as_str),
            Some("stored-client-secret")
        );
        assert_eq!(
            requests[0].form.get("resource").map(String::as_str),
            Some("https://mcp.notion.com/mcp")
        );
    }
}
