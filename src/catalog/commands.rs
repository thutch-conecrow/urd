use anyhow::Result;

use super::types::{Catalog, Sensitivity, load_catalog};
use crate::paths;
use crate::store::types::{Store, load_store};

pub fn validate() -> Result<()> {
    let local_catalog = load_catalog(&paths::local_catalog_path())?;
    let global_catalog = load_catalog(&paths::global_catalog_path()?)?;

    let local_store = load_store(&paths::store_path(false)?)?;
    let global_store = load_store(&paths::store_path(true)?)?;

    let mut issues = Vec::new();

    let catalogs: Vec<(&str, &Catalog)> =
        vec![("local", &local_catalog), ("global", &global_catalog)];
    for (source, catalog) in &catalogs {
        for (id, item) in *catalog {
            for env in &item.environments {
                let env_keys: Vec<String> = if env == "all" {
                    vec!["dev".to_string(), "prod".to_string()]
                } else {
                    vec![env.clone()]
                };

                for e in &env_keys {
                    let in_local: Option<&String> =
                        local_store.get(id).and_then(|envs| envs.get(e));
                    let in_global: Option<&String> =
                        global_store.get(id).and_then(|envs| envs.get(e));

                    if in_local.is_none() && in_global.is_none() {
                        issues.push(format!("MISSING: {id} [{e}] (defined in {source} catalog)"));
                    }

                    if let Some(value) = in_local.or(in_global) {
                        let is_encrypted = value.starts_with("ENC[age,");
                        let should_encrypt = matches!(
                            item.sensitivity,
                            Sensitivity::Sensitive | Sensitivity::Secret
                        );

                        if should_encrypt && !is_encrypted {
                            issues.push(format!(
                                "UNENCRYPTED: {id} [{e}] is {:?} but stored as plaintext",
                                item.sensitivity
                            ));
                        }
                    }
                }
            }
        }
    }

    let stores: Vec<(&str, &Store)> = vec![("local", &local_store), ("global", &global_store)];
    for (source, s) in &stores {
        for id in s.keys() {
            let in_local = local_catalog.contains_key(id);
            let in_global = global_catalog.contains_key(id);

            if !in_local && !in_global {
                issues.push(format!("ORPHANED: {id} (in {source} store but no catalog)"));
            }
        }
    }

    if issues.is_empty() {
        println!("All catalog items are present and valid.");
    } else {
        for issue in &issues {
            println!("{issue}");
        }
        std::process::exit(1);
    }

    Ok(())
}
