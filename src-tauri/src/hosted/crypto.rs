use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn random() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
pub fn hash(secret: &str) -> String {
    format!("{:x}", Sha256::digest(secret.as_bytes()))
}
pub fn password(value: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Argon2::default()
        .hash_password(value.as_bytes(), &salt)
        .map(|v| v.to_string())
        .map_err(|_| "password hashing failed".into())
}
pub fn verify(value: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded).ok().is_some_and(|p| {
        Argon2::default()
            .verify_password(value.as_bytes(), &p)
            .is_ok()
    })
}
pub fn key(encoded: &str) -> Result<[u8; 32], String> {
    STANDARD
        .decode(encoded.trim())
        .map_err(|_| "MASTER_KEY must be base64 encoding of 32 random bytes".to_string())?
        .try_into()
        .map_err(|_| "MASTER_KEY must decode to 32 bytes".into())
}
pub fn seal(key: &[u8; 32], user: i64, secret: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "invalid encryption key")?;
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let payload = aes_gcm::aead::Payload {
        msg: secret.as_bytes(),
        aad: &user.to_be_bytes(),
    };
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), payload)
        .map_err(|_| "encryption failed")?;
    Ok(format!(
        "v1.{}.{}",
        STANDARD.encode(nonce),
        STANDARD.encode(ciphertext)
    ))
}
pub fn open(key: &[u8; 32], user: i64, value: &str) -> Result<String, String> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 || parts[0] != "v1" {
        return Err("invalid encrypted credential".into());
    }
    let nonce = STANDARD
        .decode(parts[1])
        .map_err(|_| "invalid credential nonce")?;
    if nonce.len() != 12 {
        return Err("invalid credential nonce".into());
    }
    let ciphertext = STANDARD
        .decode(parts[2])
        .map_err(|_| "invalid credential")?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "invalid key")?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            aes_gcm::aead::Payload {
                msg: &ciphertext,
                aad: &user.to_be_bytes(),
            },
        )
        .map_err(|_| "credential cannot be decrypted; verify MASTER_KEY")?;
    String::from_utf8(plaintext).map_err(|_| "invalid credential encoding".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encryption_is_random_and_tenant_bound() {
        let key = [7; 32];
        let a = seal(&key, 1, "sk-test-secret").unwrap();
        let b = seal(&key, 1, "sk-test-secret").unwrap();
        assert_ne!(a, b);
        assert!(!a.contains("sk-test-secret"));
        assert_eq!(open(&key, 1, &a).unwrap(), "sk-test-secret");
        assert!(open(&key, 2, &a).is_err());
        assert!(open(&[8; 32], 1, &a).is_err());
    }
}
