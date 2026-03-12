use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Sensitivity declaration for a catalog item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Sensitivity {
    Plaintext,
    Sensitive,
    Secret,
}

impl Sensitivity {
    pub const fn to_sensitivity_level(&self) -> Option<crate::crypto::SensitivityLevel> {
        match self {
            Self::Plaintext => None,
            Self::Sensitive => Some(crate::crypto::SensitivityLevel::Sensitive),
            Self::Secret => Some(crate::crypto::SensitivityLevel::Secret),
        }
    }
}

/// A single urd item: catalog metadata + per-environment values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Item {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<Sensitivity>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
}

/// The full store: item ID -> Item (metadata + values).
pub type Store = BTreeMap<String, Item>;

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
