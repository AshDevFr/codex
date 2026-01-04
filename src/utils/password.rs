use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use anyhow::Result;

/// Hash a password using Argon2id
///
/// # Arguments
/// * `password` - The plaintext password to hash
///
/// # Returns
/// The password hash in PHC string format
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
        .to_string();

    Ok(password_hash)
}

/// Verify a password against a hash
///
/// # Arguments
/// * `password` - The plaintext password to verify
/// * `hash` - The password hash to verify against
///
/// # Returns
/// `Ok(true)` if the password matches, `Ok(false)` if it doesn't, or `Err` on parsing errors
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("Failed to parse password hash: {}", e))?;

    let argon2 = Argon2::default();

    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let password = "test_password_123";
        let hash = hash_password(password).expect("Failed to hash password");

        // Hash should not be empty
        assert!(!hash.is_empty());

        // Hash should start with $argon2
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "correct_password";
        let hash = hash_password(password).expect("Failed to hash password");

        let result = verify_password(password, &hash).expect("Failed to verify password");
        assert!(result, "Password verification should succeed");
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "correct_password";
        let wrong_password = "wrong_password";
        let hash = hash_password(password).expect("Failed to hash password");

        let result = verify_password(wrong_password, &hash).expect("Failed to verify password");
        assert!(!result, "Password verification should fail for wrong password");
    }

    #[test]
    fn test_hash_is_different_each_time() {
        let password = "same_password";
        let hash1 = hash_password(password).expect("Failed to hash password");
        let hash2 = hash_password(password).expect("Failed to hash password");

        // Hashes should be different due to random salt
        assert_ne!(hash1, hash2, "Hashes should be different due to random salts");

        // But both should verify correctly
        assert!(
            verify_password(password, &hash1).expect("Failed to verify"),
            "First hash should verify"
        );
        assert!(
            verify_password(password, &hash2).expect("Failed to verify"),
            "Second hash should verify"
        );
    }

    #[test]
    fn test_verify_with_invalid_hash() {
        let password = "test";
        let invalid_hash = "not_a_valid_hash";

        let result = verify_password(password, invalid_hash);
        assert!(result.is_err(), "Should fail with invalid hash format");
    }
}
