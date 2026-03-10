use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub description: String,
    pub sensitivity: Sensitivity,
    #[serde(default)]
    pub origin: Option<String>,
    pub environments: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Sensitivity {
    Plaintext,
    Sensitive,
    Secret,
}

pub type Catalog = BTreeMap<String, CatalogItem>;

pub fn load_catalog(path: &Path) -> Result<Catalog> {
    if !path.exists() {
        return Ok(Catalog::new());
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;

    if contents.trim().is_empty() {
        return Ok(Catalog::new());
    }

    let catalog: Catalog = serde_yaml::from_str(&contents)
        .with_context(|| format!("could not parse {}", path.display()))?;

    Ok(catalog)
}
