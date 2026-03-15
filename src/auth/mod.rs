// Shared authentication module for both web gateway and admin backend

pub mod jwt;
pub mod password;

pub use jwt::{JwtManager, TokenClaims};
pub use password::PasswordManager;

use crate::error::{Error, Result};

/// Unified authentication manager for the platform
pub struct AuthManager {
    jwt_manager: JwtManager,
    password_manager: PasswordManager,
}

impl AuthManager {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_manager: JwtManager::new(jwt_secret),
            password_manager: PasswordManager::new(),
        }
    }

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String> {
        self.password_manager.hash(password)
    }

    /// Verify a password against a hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<()> {
        self.password_manager.verify(password, hash)
    }

    /// Generate an access token (1 hour expiry)
    pub fn generate_access_token(&self, user_id: &str) -> Result<String> {
        self.jwt_manager.generate_access_token(user_id)
    }

    /// Generate a refresh token (7 days expiry)
    pub fn generate_refresh_token(&self, user_id: &str) -> Result<String> {
        self.jwt_manager.generate_refresh_token(user_id)
    }

    /// Verify and decode a token
    pub fn verify_token(&self, token: &str) -> Result<TokenClaims> {
        self.jwt_manager.verify_token(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_manager_password_flow() {
        let auth = AuthManager::new("test-secret".to_string());
        let password = "secure-password-123";

        let hash = auth.hash_password(password).unwrap();
        assert!(auth.verify_password(password, &hash).is_ok());
        assert!(auth.verify_password("wrong-password", &hash).is_err());
    }

    #[test]
    fn test_auth_manager_token_flow() {
        let auth = AuthManager::new("test-secret".to_string());
        let user_id = "user-123";

        let token = auth.generate_access_token(user_id).unwrap();
        let claims = auth.verify_token(&token).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.token_type, "access");
    }
}
