use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Environment name -> value (plaintext or ENC[age,...])
pub type EnvValues = BTreeMap<String, String>;

/// The full store: item ID -> environment -> value
pub type Store = BTreeMap<String, EnvValues>;

pub fn load_store(path: &Path) -> Result<Store> {
    if !path.exists() {
        return Ok(Store::new());
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;

    if contents.trim().is_empty() {
        return Ok(Store::new());
    }

    let store: Store = serde_yaml::from_str(&contents)
        .with_context(|| format!("could not parse {}", path.display()))?;

    Ok(store)
}

pub fn save_store(path: &Path, store: &Store) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let yaml = serde_yaml::to_string(store)?;
    fs::write(path, yaml).with_context(|| format!("could not write {}", path.display()))?;

    Ok(())
}
