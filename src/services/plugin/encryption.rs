//! Credential encryption service using AES-256-GCM
//!
//! All sensitive credentials (API keys, OAuth tokens) are encrypted at rest
//! using AES-256-GCM with a 96-bit nonce prepended to the ciphertext.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use std::env;
use std::sync::OnceLock;

/// Global encryption service instance
static ENCRYPTION_SERVICE: OnceLock<CredentialEncryption> = OnceLock::new();

/// The environment variable name for the encryption key
pub const ENCRYPTION_KEY_ENV: &str = "CODEX_ENCRYPTION_KEY";

/// Credential encryption service using AES-256-GCM
#[derive(Clone)]
pub struct CredentialEncryption {
    cipher: Aes256Gcm,
}

#[allow(dead_code)]
impl CredentialEncryption {
    /// Create a new encryption service with the given 256-bit key
    pub fn new(key: &[u8; 32]) -> Self {
        let key = Key::<Aes256Gcm>::from_slice(key);
        Self {
            cipher: Aes256Gcm::new(key),
        }
    }

    /// Create encryption service from a base64-encoded key
    pub fn from_base64_key(base64_key: &str) -> Result<Self> {
        let key_bytes = BASE64
            .decode(base64_key)
            .map_err(|e| anyhow!("Invalid base64 encryption key: {}", e))?;

        if key_bytes.len() != 32 {
            return Err(anyhow!(
                "Encryption key must be 32 bytes (256 bits), got {} bytes",
                key_bytes.len()
            ));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        Ok(Self::new(&key))
    }

    /// Create encryption service from environment variable
    pub fn from_env() -> Result<Self> {
        let key = env::var(ENCRYPTION_KEY_ENV).map_err(|_| {
            anyhow!(
                "Encryption key not set. Set {} environment variable with a base64-encoded 32-byte key",
                ENCRYPTION_KEY_ENV
            )
        })?;
        Self::from_base64_key(&key)
    }

    /// Get or initialize the global encryption service
    pub fn global() -> Result<&'static Self> {
        if let Some(service) = ENCRYPTION_SERVICE.get() {
            return Ok(service);
        }

        let service = Self::from_env()?;
        Ok(ENCRYPTION_SERVICE.get_or_init(|| service))
    }

    /// Encrypt data using AES-256-GCM
    ///
    /// Returns the nonce (12 bytes) prepended to the ciphertext
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Generate a random 96-bit (12 byte) nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the plaintext
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    /// Encrypt a string and return the encrypted data
    pub fn encrypt_string(&self, plaintext: &str) -> Result<Vec<u8>> {
        self.encrypt(plaintext.as_bytes())
    }

    /// Encrypt a JSON value and return the encrypted data
    pub fn encrypt_json<T: serde::Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        let json =
            serde_json::to_string(value).map_err(|e| anyhow!("Failed to serialize JSON: {}", e))?;
        self.encrypt(json.as_bytes())
    }

    /// Decrypt data encrypted with AES-256-GCM
    ///
    /// Expects the nonce (12 bytes) to be prepended to the ciphertext
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(anyhow!(
                "Invalid encrypted data: too short (minimum 12 bytes for nonce)"
            ));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow!("Decryption failed: invalid key or corrupted data"))
    }

    /// Decrypt to a UTF-8 string
    pub fn decrypt_string(&self, data: &[u8]) -> Result<String> {
        let plaintext = self.decrypt(data)?;
        String::from_utf8(plaintext)
            .map_err(|e| anyhow!("Decrypted data is not valid UTF-8: {}", e))
    }

    /// Decrypt to a JSON value
    pub fn decrypt_json<T: serde::de::DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        let plaintext = self.decrypt(data)?;
        serde_json::from_slice(&plaintext)
            .map_err(|e| anyhow!("Failed to parse decrypted JSON: {}", e))
    }

    /// Generate a new random encryption key (32 bytes)
    pub fn generate_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    /// Generate a new random encryption key and encode as base64
    pub fn generate_key_base64() -> String {
        BASE64.encode(Self::generate_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        // Fixed test key for reproducible tests
        [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ]
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let encryption = CredentialEncryption::new(&test_key());
        let plaintext = b"Hello, World!";

        let encrypted = encryption.encrypt(plaintext).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_string_roundtrip() {
        let encryption = CredentialEncryption::new(&test_key());
        let plaintext = "My secret API key: abc123";

        let encrypted = encryption.encrypt_string(plaintext).unwrap();
        let decrypted = encryption.decrypt_string(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_json_roundtrip() {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct Credentials {
            api_key: String,
            secret: String,
        }

        let encryption = CredentialEncryption::new(&test_key());
        let credentials = Credentials {
            api_key: "my-api-key".to_string(),
            secret: "my-secret".to_string(),
        };

        let encrypted = encryption.encrypt_json(&credentials).unwrap();
        let decrypted: Credentials = encryption.decrypt_json(&encrypted).unwrap();

        assert_eq!(credentials, decrypted);
    }

    #[test]
    fn test_encrypted_data_includes_nonce() {
        let encryption = CredentialEncryption::new(&test_key());
        let plaintext = b"Test data";

        let encrypted = encryption.encrypt(plaintext).unwrap();

        // Encrypted data should be at least 12 (nonce) + 16 (auth tag) + plaintext length
        assert!(encrypted.len() >= 12 + 16 + plaintext.len());
    }

    #[test]
    fn test_different_encryptions_produce_different_ciphertext() {
        let encryption = CredentialEncryption::new(&test_key());
        let plaintext = b"Test data";

        let encrypted1 = encryption.encrypt(plaintext).unwrap();
        let encrypted2 = encryption.encrypt(plaintext).unwrap();

        // Due to random nonce, same plaintext produces different ciphertext
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same plaintext
        let decrypted1 = encryption.decrypt(&encrypted1).unwrap();
        let decrypted2 = encryption.decrypt(&encrypted2).unwrap();
        assert_eq!(decrypted1, decrypted2);
    }

    #[test]
    fn test_decrypt_fails_with_wrong_key() {
        let encryption1 = CredentialEncryption::new(&test_key());
        let mut wrong_key = test_key();
        wrong_key[0] ^= 0xFF; // Flip one byte
        let encryption2 = CredentialEncryption::new(&wrong_key);

        let encrypted = encryption1.encrypt(b"Secret data").unwrap();
        let result = encryption2.decrypt(&encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_fails_with_corrupted_data() {
        let encryption = CredentialEncryption::new(&test_key());
        let mut encrypted = encryption.encrypt(b"Secret data").unwrap();

        // Corrupt the ciphertext
        if let Some(byte) = encrypted.last_mut() {
            *byte ^= 0xFF;
        }

        let result = encryption.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_fails_with_short_data() {
        let encryption = CredentialEncryption::new(&test_key());

        // Data shorter than nonce (12 bytes)
        let result = encryption.decrypt(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_base64_key() {
        let key = test_key();
        let base64_key = BASE64.encode(key);

        let encryption = CredentialEncryption::from_base64_key(&base64_key).unwrap();
        let plaintext = b"Test data";

        let encrypted = encryption.encrypt(plaintext).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_from_base64_key_invalid_length() {
        let short_key = BASE64.encode([0u8; 16]); // 16 bytes instead of 32
        let result = CredentialEncryption::from_base64_key(&short_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_key() {
        let key1 = CredentialEncryption::generate_key();
        let key2 = CredentialEncryption::generate_key();

        // Keys should be different (with overwhelming probability)
        assert_ne!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_generate_key_base64() {
        let base64_key = CredentialEncryption::generate_key_base64();
        let decoded = BASE64.decode(&base64_key).unwrap();
        assert_eq!(decoded.len(), 32);
    }
}
