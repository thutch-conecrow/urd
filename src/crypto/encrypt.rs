use std::fs;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use crate::paths;

/// Load the AES-256-GCM key for the current directory.
pub fn load_key() -> Result<aes_gcm::Key<Aes256Gcm>> {
    let id_path = paths::key_id_path()?;
    let id = fs::read_to_string(&id_path)
        .with_context(|| format!("no key-id found at {} — run `urd keys init`", id_path.display()))?;
    let id = id.trim();

    let key_path = paths::key_file_path(id)?;
    let key_b64 = fs::read_to_string(&key_path)
        .with_context(|| format!("key file not found at {} — obtain the key from your team", key_path.display()))?;

    let key_bytes = BASE64.decode(key_b64.trim()).context("invalid key encoding")?;
    anyhow::ensure!(key_bytes.len() == 32, "invalid key length (expected 32 bytes)");

    Ok(*aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes))
}

/// Sensitivity level for encrypted values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitivityLevel {
    Sensitive,
    Secret,
}

impl SensitivityLevel {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Sensitive => "sensitive",
            Self::Secret => "secret",
        }
    }
}

pub fn encrypt_value(plaintext: &str, level: SensitivityLevel) -> Result<String> {
    let key = load_key()?;
    let cipher = Aes256Gcm::new(&key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| anyhow::anyhow!("encryption failed"))?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    let encoded = BASE64.encode(&combined);
    let tag = level.tag();
    Ok(format!("ENC[aes:{tag},{encoded}]"))
}

/// Parse the sensitivity level from an encrypted value string.
pub fn parse_sensitivity(value: &str) -> Option<SensitivityLevel> {
    if value.starts_with("ENC[aes:sensitive,") {
        Some(SensitivityLevel::Sensitive)
    } else if value.starts_with("ENC[aes:secret,") {
        Some(SensitivityLevel::Secret)
    } else {
        None
    }
}

pub fn decrypt_value(encrypted: &str) -> Result<String> {
    let inner = encrypted
        .strip_prefix("ENC[aes:sensitive,")
        .or_else(|| encrypted.strip_prefix("ENC[aes:secret,"))
        .and_then(|s| s.strip_suffix(']'))
        .context("invalid encrypted value format")?;

    let combined = BASE64.decode(inner).context("invalid base64 in encrypted value")?;
    anyhow::ensure!(combined.len() > 12, "encrypted value too short");

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let key = load_key()?;
    let cipher = Aes256Gcm::new(&key);

    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("decryption failed — wrong key?"))?;

    Ok(String::from_utf8(plaintext_bytes)?)
}
