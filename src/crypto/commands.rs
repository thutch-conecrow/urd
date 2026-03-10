use std::fs;

use age::secrecy::ExposeSecret;
use age::x25519;
use anyhow::{Context, Result};

use crate::cli::KeysCommand;

use super::encrypt::identity_path;

pub fn keys(cmd: &KeysCommand) -> Result<()> {
    match cmd {
        KeysCommand::Init => init(),
        KeysCommand::Status => status(),
        KeysCommand::Export => export(),
    }
}

fn init() -> Result<()> {
    let path = identity_path()?;

    if path.exists() {
        anyhow::bail!(
            "key already exists at {}. Remove it first to regenerate.",
            path.display()
        );
    }

    let identity = x25519::Identity::generate();
    let public_key = identity.to_public();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, identity.to_string().expose_secret())?;

    println!("Key generated at {}", path.display());
    println!("Public key: {public_key}");
    println!("\nKeep the identity file safe. Share only the public key.");

    Ok(())
}

fn status() -> Result<()> {
    let path = identity_path()?;
    if path.exists() {
        let identity = super::load_identity()?;
        println!("Key initialized: {}", path.display());
        println!("Public key: {}", identity.to_public());
    } else {
        println!("No key found. Run `urd keys init` to generate one.");
    }
    Ok(())
}

fn export() -> Result<()> {
    let identity = super::load_identity().context("no key found — run `urd keys init` first")?;
    println!("{}", identity.to_public());
    Ok(())
}
