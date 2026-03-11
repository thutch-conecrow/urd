use std::fs;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{KeyInit, OsRng};
use aes_gcm::Aes256Gcm;
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use crate::cli::KeysCommand;
use crate::paths;

pub fn keys(cmd: &KeysCommand) -> Result<()> {
    match cmd {
        KeysCommand::Init => init(),
        KeysCommand::Status => status(),
        KeysCommand::Export => export(),
    }
}

fn init() -> Result<()> {
    let id_path = paths::key_id_path()?;

    if id_path.exists() {
        anyhow::bail!(
            "key already configured for this directory ({}). Remove it first to regenerate.",
            id_path.display()
        );
    }

    // Generate random 8-char hex ID
    let mut id_bytes = [0u8; 4];
    OsRng.fill_bytes(&mut id_bytes);
    let mut id = String::with_capacity(8);
    for b in id_bytes {
        std::fmt::Write::write_fmt(&mut id, format_args!("{b:02x}")).expect("hex format");
    }

    // Generate random 256-bit key
    let key = Aes256Gcm::generate_key(OsRng);
    let key_b64 = BASE64.encode(key);

    // Write key file
    let key_path = paths::key_file_path(&id)?;
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&key_path, &key_b64)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
    }

    // Write key-id file
    if let Some(parent) = id_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&id_path, &id)?;

    println!("Key generated: {id}");
    println!("Key file: {}", key_path.display());
    println!("Key ID written to {}", id_path.display());

    Ok(())
}

fn status() -> Result<()> {
    let id_path = paths::key_id_path()?;

    if !id_path.exists() {
        println!("No key configured for this directory. Run `urd keys init` to generate one.");
        return Ok(());
    }

    let id = fs::read_to_string(&id_path)?.trim().to_string();
    let key_path = paths::key_file_path(&id)?;

    println!("Key ID: {id}");
    if key_path.exists() {
        println!("Key file: {} (found)", key_path.display());
    } else {
        println!("Key file: {} (NOT FOUND)", key_path.display());
        println!("Obtain the key from your team and place it at the path above.");
    }

    Ok(())
}

fn export() -> Result<()> {
    let id_path = paths::key_id_path()?;
    let id = fs::read_to_string(&id_path)
        .context("no key configured — run `urd keys init` first")?;
    let id = id.trim();

    let key_path = paths::key_file_path(id)?;
    let key_b64 = fs::read_to_string(&key_path)
        .with_context(|| format!("key file not found at {}", key_path.display()))?;

    println!("{}", key_b64.trim());
    Ok(())
}
