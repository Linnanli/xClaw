use crate::error::{Error, Result};
use crate::models::TokenClaims;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;

pub struct AuthManager {
    jwt_secret: String,
    access_token_expiry: i64,
    refresh_token_expiry: i64,
}

impl AuthManager {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret,
            access_token_expiry: 3600,      // 1 hour
            refresh_token_expiry: 604800,   // 7 days
        }
    }

    pub fn hash_password(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(rand::thread_rng());
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::AuthFailed(e.to_string()))
            .map(|hash| hash.to_string())
    }

    pub fn verify_password(&self, password: &str, hash: &str) -> Result<()> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| Error::AuthFailed(e.to_string()))?;

        let argon2 = Argon2::default();
        argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| Error::InvalidCredentials)
    }

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
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| Error::AuthFailed(e.to_string()))
    }

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
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| Error::AuthFailed(e.to_string()))
    }

    pub fn verify_token(&self, token: &str) -> Result<TokenClaims> {
        decode::<TokenClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| {
            if e.to_string().contains("ExpiredSignature") {
                Error::TokenExpired
            } else {
                Error::InvalidToken
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let auth = AuthManager::new("test-secret".to_string());
        let password = "secure-password-123";

        let hash = auth.hash_password(password).unwrap();
        assert!(auth.verify_password(password, &hash).is_ok());
        assert!(auth.verify_password("wrong-password", &hash).is_err());
    }

    #[test]
    fn test_generate_and_verify_token() {
        let auth = AuthManager::new("test-secret".to_string());
        let user_id = "user-123";

        let token = auth.generate_access_token(user_id).unwrap();
        let claims = auth.verify_token(&token).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.token_type, "access");
    }
}
