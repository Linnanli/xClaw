//! Configuration for cryptographic algorithm selection.

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::secrets::crypto_provider::{CryptoAlgorithm, CryptoProvider};
use crate::secrets::default_crypto::DefaultCryptoProvider;
use crate::secrets::gm_crypto::GMCryptoProvider;
use crate::secrets::types::SecretError;

/// Cryptographic configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoConfig {
    /// Algorithm suite to use.
    #[serde(default)]
    pub algorithm: CryptoAlgorithmConfig,
}

/// Algorithm configuration enum for serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CryptoAlgorithmConfig {
    /// Standard algorithms (AES-256-GCM, Ed25519, SHA256)
    Default,
    /// Chinese national cryptography (SM2, SM3, SM4)
    ChinaCrypto,
}

impl Default for CryptoAlgorithmConfig {
    fn default() -> Self {
        Self::Default
    }
}

impl From<CryptoAlgorithmConfig> for CryptoAlgorithm {
    fn from(config: CryptoAlgorithmConfig) -> Self {
        match config {
            CryptoAlgorithmConfig::Default => CryptoAlgorithm::Default,
            CryptoAlgorithmConfig::ChinaCrypto => CryptoAlgorithm::ChinaCrypto,
        }
    }
}

impl From<CryptoAlgorithm> for CryptoAlgorithmConfig {
    fn from(algo: CryptoAlgorithm) -> Self {
        match algo {
            CryptoAlgorithm::Default => CryptoAlgorithmConfig::Default,
            CryptoAlgorithm::ChinaCrypto => CryptoAlgorithmConfig::ChinaCrypto,
        }
    }
}

impl Default for CryptoConfig {
    fn default() -> Self {
        Self {
            algorithm: CryptoAlgorithmConfig::Default,
        }
    }
}

impl CryptoConfig {
    /// Create a new crypto config with the specified algorithm.
    pub fn new(algorithm: CryptoAlgorithmConfig) -> Self {
        Self { algorithm }
    }

    /// Create a crypto provider based on this configuration.
    pub fn create_provider(
        &self,
        master_key: SecretString,
    ) -> Result<Arc<dyn CryptoProvider>, SecretError> {
        match self.algorithm {
            CryptoAlgorithmConfig::Default => {
                let provider = DefaultCryptoProvider::new(master_key)?;
                Ok(Arc::new(provider))
            }
            CryptoAlgorithmConfig::ChinaCrypto => {
                let provider = GMCryptoProvider::new(master_key)?;
                Ok(Arc::new(provider))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CryptoConfig::default();
        assert_eq!(config.algorithm, CryptoAlgorithmConfig::Default);
    }

    #[test]
    fn test_algorithm_conversion() {
        let config_algo = CryptoAlgorithmConfig::Default;
        let algo: CryptoAlgorithm = config_algo.into();
        assert_eq!(algo, CryptoAlgorithm::Default);

        let config_algo2: CryptoAlgorithmConfig = algo.into();
        assert_eq!(config_algo2, CryptoAlgorithmConfig::Default);
    }

    #[test]
    fn test_create_default_provider() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();
        assert_eq!(provider.algorithm(), CryptoAlgorithm::Default);
    }

    #[test]
    fn test_create_gm_provider() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);
        let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
        let provider = config.create_provider(master_key).unwrap();
        assert_eq!(provider.algorithm(), CryptoAlgorithm::ChinaCrypto);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CryptoConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.algorithm, deserialized.algorithm);
    }

    #[test]
    fn test_serde_default() {
        let json = "{}";
        let config: CryptoConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.algorithm, CryptoAlgorithmConfig::Default);
    }
}
