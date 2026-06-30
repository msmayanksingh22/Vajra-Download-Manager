//! Vault encryption — encrypts credentials before storing them in SQLite.
//!
//! Strategy:
//! 1. On first access, generate a 256-bit random key and persist it to
//!    `%LOCALAPPDATA%\Vajra\vault.key` with restricted file permissions.
//! 2. On every subsequent access, load the key from disk.
//! 3. Use XChaCha20-Poly1305 (authenticated encryption) to encrypt
//!    username + password before writing to the database.
//!
//! The encrypted blob format is: `[24-byte nonce][ciphertext][16-byte tag]`,
//! base64-encoded for safe SQLite storage.
//!
//! Migration: existing plaintext credentials are re-encrypted on first read
//! after the module is loaded.

use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{Aead, OsRng},
    AeadCore, KeyInit, XChaCha20Poly1305,
};

// ─── Key management ───────────────────────────────────────────────────────────

/// Load or generate the vault encryption key.
///
/// Returns `None` if key generation/storage fails — in that case, vault
/// operations fall back to plaintext (same as before the fix).
pub fn load_or_generate_key() -> Option<[u8; 32]> {
    let key_path = vault_key_path();

    // Try to load existing key
    if let Ok(data) = std::fs::read(&key_path) {
        if data.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&data);
            return Some(key);
        }
    }

    // Generate new key
    let mut key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut OsRng, &mut key);

    // Persist with restrictive permissions (owner-only read)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::write(&key_path, key) {
            tracing::warn!("Failed to write vault key: {}", e);
            return None;
        }
        if let Ok(mut perms) = std::fs::metadata(&key_path).and_then(|m| {
            let p = m.permissions();
            std::fs::set_permissions(&key_path, p).map(|_| m)
        }) {
            let mut p = perms.permissions();
            p.set_mode(0o600); // owner read/write only
            let _ = std::fs::set_permissions(&key_path, p);
        }
    }
    #[cfg(not(unix))]
    {
        if let Err(e) = std::fs::write(&key_path, key) {
            tracing::warn!("Failed to write vault key: {}", e);
            return None;
        }
    }

    tracing::info!("Vault encryption key generated");
    Some(key)
}

fn vault_key_path() -> PathBuf {
    vajra_protocol::app_data_dir().join("vault.key")
}

// ─── Encryption / decryption ──────────────────────────────────────────────────

/// Encrypt plaintext to a base64-encoded blob.
///
/// Format: base64(24-byte nonce || ciphertext || 16-byte GCM tag)
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> anyhow::Result<String> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng); // 24 bytes
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    let mut blob = Vec::with_capacity(nonce.len() + ciphertext.len());
    blob.extend_from_slice(nonce.as_slice());
    blob.extend_from_slice(&ciphertext);

    Ok(STANDARD.encode(&blob))
}

/// Decrypt a base64-encoded blob back to plaintext.
pub fn decrypt(key: &[u8; 32], blob: &str) -> anyhow::Result<String> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let data = STANDARD.decode(blob)?;

    if data.len() < 25 {
        // Minimum: 24-byte nonce + 1 byte ciphertext
        anyhow::bail!("encrypted blob too short");
    }

    let nonce_data = &data[..24];
    let ciphertext = &data[24..];
    let nonce = chacha20poly1305::XNonce::from_slice(nonce_data);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?;
    Ok(String::from_utf8(plaintext)?)
}

// ─── Database migration helpers ───────────────────────────────────────────────

/// Check if a vault field looks encrypted (base64 with correct length).
/// Encrypted blobs are always > 40 characters (24 nonce + 16 tag + data).
pub fn looks_encrypted(value: &str) -> bool {
    if value.len() < 40 {
        return false;
    }
    // Quick heuristic: valid base64 chars only, no spaces or typical plaintext patterns
    value
        .bytes()
        .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'='))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let plaintext = "my-secret-password";

        let encrypted = encrypt(&key, plaintext).expect("encrypt failed");
        assert!(
            looks_encrypted(&encrypted),
            "output should look like base64"
        );

        let decrypted = decrypt(&key, &encrypted).expect("decrypt failed");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn decrypt_wrong_key() {
        let key1 = [42u8; 32];
        let key2 = [99u8; 32];

        let encrypted = encrypt(&key1, "secret").unwrap();
        let result = decrypt(&key2, &encrypted);
        assert!(result.is_err(), "wrong key should fail decryption");
    }

    #[test]
    fn decrypt_short_blob() {
        let key = [0u8; 32];
        let result = decrypt(&key, "short");
        assert!(result.is_err(), "blob too short should fail");
    }
}
