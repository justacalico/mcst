//! Argon2id password hashing.

use anyhow::{anyhow, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;

/// Hash a password with Argon2id and a random salt (PHC string format).
pub fn hash(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("password hash failed: {e}"))?;
    Ok(hash.to_string())
}

/// Verify a password against a stored PHC hash. Constant-time on the hash
/// comparison; returns `false` for malformed stored hashes.
pub fn verify(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Policy for acceptable passwords.
pub fn validate(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("password must be at least 8 characters");
    }
    if password.len() > 256 {
        return Err("password is too long");
    }
    Ok(())
}

/// Policy for usernames: 1-32 chars of [a-zA-Z0-9_.-].
pub fn validate_username(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || name.len() > 32 {
        return Err("username must be 1-32 characters");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err("username may only contain letters, digits, '_', '.', '-'");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(h.starts_with("$argon2"));
        assert!(verify("correct horse battery staple", &h));
        assert!(!verify("wrong", &h));
    }

    #[test]
    fn two_hashes_differ() {
        assert_ne!(hash("same").unwrap(), hash("same").unwrap());
    }

    #[test]
    fn verify_malformed_returns_false() {
        assert!(!verify("x", "not-a-hash"));
        assert!(!verify("x", ""));
    }

    #[test]
    fn password_policy() {
        assert!(validate("short").is_err());
        assert!(validate("").is_err());
        assert!(validate("long-enough-password").is_ok());
        assert!(validate(&"x".repeat(300)).is_err());
    }

    #[test]
    fn username_policy() {
        assert!(validate_username("admin").is_ok());
        assert!(validate_username("a.b-c_d").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("has space").is_err());
        assert!(validate_username("bad!name").is_err());
        assert!(validate_username(&"x".repeat(40)).is_err());
    }
}
