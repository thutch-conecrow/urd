use std::path::PathBuf;

use anyhow::{Context, Result};

/// Returns the urd home directory.
///
/// Uses `URD_HOME` env var if set, otherwise `~/.config/urd`.
pub fn urd_home() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("URD_HOME") {
        return Ok(PathBuf::from(home));
    }

    dirs::config_dir()
        .context("could not determine config directory")
        .map(|d| d.join("urd"))
}

pub fn store_path() -> Result<PathBuf> {
    if std::env::var("URD_HOME").is_ok() {
        Ok(urd_home()?.join("store.yaml"))
    } else {
        Ok(PathBuf::from(".urd").join("store.yaml"))
    }
}

pub fn identity_path() -> Result<PathBuf> {
    Ok(urd_home()?.join("keys").join("identity.key"))
}
