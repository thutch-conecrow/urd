use std::collections::BTreeMap;
use std::fs;
use std::ops::{Deref, DerefMut};
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

/// Store-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoreMeta {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_environments: Vec<String>,
}

/// Serde wrapper for the new on-disk format: `{ meta, items }`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreFile {
    #[serde(default)]
    meta: StoreMeta,
    #[serde(default)]
    items: BTreeMap<String, Item>,
}

/// The full store: metadata + item map.
#[derive(Debug, Clone, Default)]
pub struct Store {
    pub meta: StoreMeta,
    pub items: BTreeMap<String, Item>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Deref for Store {
    type Target = BTreeMap<String, Item>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl DerefMut for Store {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}

/// Apply default environments to an item that doesn't already have its own.
pub fn apply_default_environments(meta: &StoreMeta, item: &mut Item) {
    if item.environments.is_empty() && !meta.default_environments.is_empty() {
        item.environments.clone_from(&meta.default_environments);
    }
}

pub fn load_store(path: &Path) -> Result<Store> {
    if !path.exists() {
        return Ok(Store::new());
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;

    if contents.trim().is_empty() {
        return Ok(Store::new());
    }

    // Try the new `{ meta, items }` format first.
    if let Ok(file) = serde_yaml::from_str::<StoreFile>(&contents) {
        return Ok(Store {
            meta: file.meta,
            items: file.items,
        });
    }

    // Fall back to the legacy bare-map format.
    let items: BTreeMap<String, Item> = serde_yaml::from_str(&contents)
        .with_context(|| format!("could not parse {}", path.display()))?;

    Ok(Store {
        meta: StoreMeta::default(),
        items,
    })
}

pub fn save_store(path: &Path, store: &Store) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = StoreFile {
        meta: store.meta.clone(),
        items: store.items.clone(),
    };

    let yaml = serde_yaml::to_string(&file)?;
    fs::write(path, yaml).with_context(|| format!("could not write {}", path.display()))?;

    Ok(())
}
