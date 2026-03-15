use crate::error::{AuthError, Result};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT token claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
    pub token_type: String,
}

/// JWT token manager.
pub struct JwtManager {
    secret: String,
    access_token_expiry: i64,
    refresh_token_expiry: i64,
}

impl JwtManager {
    /// Create a new JWT manager with the given secret.
    pub fn new(secret: String) -> Self {
        Self {
            secret,
            access_token_expiry: 3600,      // 1 hour
            refresh_token_expiry: 604800,   // 7 days
        }
    }

    /// Generate an access token (1 hour expiry).
    pub fn generate_access_token(&self, user_id: &str) -> Result<String> {
        let now = Utc::now().timestamp();
        let claims = TokenClaims {
            sub: user_id.to_string(),
            exp: now + self.access_token_expiry,
            iat: now,
            token_type: "access".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| AuthError::AuthFailed(e.to_string()))
    }

    /// Generate a refresh token (7 days expiry).
    pub fn generate_refresh_token(&self, user_id: &str) -> Result<String> {
        let now = Utc::now().timestamp();
        let claims = TokenClaims {
            sub: user_id.to_string(),
            exp: now + self.refresh_token_expiry,
            iat: now,
            token_type: "refresh".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| AuthError::AuthFailed(e.to_string()))
    }

    /// Verify and decode a token.
    pub fn verify_token(&self, token: &str) -> Result<TokenClaims> {
        decode::<TokenClaims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| {
            if e.to_string().contains("ExpiredSignature") {
                AuthError::TokenExpired
            } else {
                AuthError::InvalidToken
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_access_token() {
        let manager = JwtManager::new("test-secret".to_string());
        let user_id = "user-123";

        let token = manager.generate_access_token(user_id).unwrap();
        let claims = manager.verify_token(&token).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.token_type, "access");
    }

    #[test]
    fn test_generate_and_verify_refresh_token() {
        let manager = JwtManager::new("test-secret".to_string());
        let user_id = "user-123";

        let token = manager.generate_refresh_token(user_id).unwrap();
        let claims = manager.verify_token(&token).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.token_type, "refresh");
    }

    #[test]
    fn test_refresh_token_has_longer_expiry() {
        let manager = JwtManager::new("test-secret".to_string());
        let user_id = "user-123";

        let access_token = manager.generate_access_token(user_id).unwrap();
        let refresh_token = manager.generate_refresh_token(user_id).unwrap();

        let access_claims = manager.verify_token(&access_token).unwrap();
        let refresh_claims = manager.verify_token(&refresh_token).unwrap();

        assert!(refresh_claims.exp > access_claims.exp);
    }

    #[test]
    fn test_invalid_token_fails() {
        let manager = JwtManager::new("test-secret".to_string());
        let result = manager.verify_token("invalid.token.here");

        assert!(result.is_err());
    }

    #[test]
    fn test_different_secrets_produce_different_tokens() {
        let manager1 = JwtManager::new("secret1".to_string());
        let manager2 = JwtManager::new("secret2".to_string());
        let user_id = "user-123";

        let token1 = manager1.generate_access_token(user_id).unwrap();
        let token2 = manager2.generate_access_token(user_id).unwrap();

        assert_ne!(token1, token2);
        assert!(manager2.verify_token(&token1).is_err());
    }
}
