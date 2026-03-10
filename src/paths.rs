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

pub fn global_store_path() -> Result<PathBuf> {
    Ok(urd_home()?.join("store.yaml"))
}

pub fn local_store_path() -> PathBuf {
    PathBuf::from(".urd").join("store.yaml")
}

pub fn store_path(global: bool) -> Result<PathBuf> {
    if global {
        global_store_path()
    } else {
        Ok(local_store_path())
    }
}

pub fn identity_path() -> Result<PathBuf> {
    Ok(urd_home()?.join("keys").join("identity.key"))
}

pub fn global_catalog_path() -> Result<PathBuf> {
    Ok(urd_home()?.join("catalog.yaml"))
}

pub fn local_catalog_path() -> PathBuf {
    PathBuf::from(".urd").join("catalog.yaml")
}
