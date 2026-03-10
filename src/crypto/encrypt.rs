use std::fs;
use std::io::{Read, Write};

use age::armor::{ArmoredReader, ArmoredWriter, Format};
use age::x25519;
use anyhow::{Context, Result};

use crate::paths;

pub fn identity_path() -> Result<std::path::PathBuf> {
    paths::identity_path()
}

pub fn load_identity() -> Result<x25519::Identity> {
    let path = identity_path()?;
    let contents =
        fs::read_to_string(&path).with_context(|| format!("could not read {}", path.display()))?;

    let identity = contents
        .parse::<x25519::Identity>()
        .map_err(|e| anyhow::anyhow!("failed to parse identity: {e}"))?;

    Ok(identity)
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
    let identity = load_identity()?;
    let recipient = identity.to_public();
    let recipients: Vec<&dyn age::Recipient> = vec![&recipient];
    let encryptor = age::Encryptor::with_recipients(recipients.into_iter())
        .context("failed to create encryptor")?;

    let mut output = Vec::new();
    let armored_writer = ArmoredWriter::wrap_output(&mut output, Format::AsciiArmor)?;
    let mut writer = encryptor
        .wrap_output(armored_writer)
        .context("failed to wrap output")?;
    writer.write_all(plaintext.as_bytes())?;
    writer.finish().and_then(age::armor::ArmoredWriter::finish)?;

    let encoded = String::from_utf8(output)?;
    let compact = encoded
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");

    let tag = level.tag();
    Ok(format!("ENC[age:{tag},{compact}]"))
}

/// Parse the sensitivity level from an encrypted value string.
pub fn parse_sensitivity(value: &str) -> Option<SensitivityLevel> {
    if value.starts_with("ENC[age:sensitive,") {
        Some(SensitivityLevel::Sensitive)
    } else if value.starts_with("ENC[age:secret,") {
        Some(SensitivityLevel::Secret)
    } else if value.starts_with("ENC[age,") {
        // Legacy format without level — treat as sensitive
        Some(SensitivityLevel::Sensitive)
    } else {
        None
    }
}

pub fn decrypt_value(encrypted: &str) -> Result<String> {
    // Strip prefix: ENC[age:sensitive, or ENC[age:secret, or ENC[age,
    let inner = encrypted
        .strip_prefix("ENC[age:sensitive,")
        .or_else(|| encrypted.strip_prefix("ENC[age:secret,"))
        .or_else(|| encrypted.strip_prefix("ENC[age,"))
        .and_then(|s| s.strip_suffix(']'))
        .context("invalid encrypted value format")?;

    // Re-wrap at 64 characters as required by the armor format
    let wrapped: String = inner
        .as_bytes()
        .chunks(64)
        .map(|chunk| std::str::from_utf8(chunk).expect("valid utf8"))
        .collect::<Vec<_>>()
        .join("\n");

    let armored = format!(
        "-----BEGIN AGE ENCRYPTED FILE-----\n{wrapped}\n-----END AGE ENCRYPTED FILE-----"
    );

    let identity = load_identity()?;
    let decryptor = age::Decryptor::new(ArmoredReader::new(armored.as_bytes()))
        .context("failed to create decryptor")?;

    let identities: Vec<&dyn age::Identity> = vec![&identity];
    let mut reader = decryptor
        .decrypt(identities.into_iter())
        .context("failed to decrypt")?;

    let mut decrypted = Vec::new();
    reader.read_to_end(&mut decrypted)?;

    Ok(String::from_utf8(decrypted)?)
}
