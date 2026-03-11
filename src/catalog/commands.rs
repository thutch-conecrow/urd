use anyhow::{Context, Result};

use crate::cli::{CatalogAddArgs, CatalogListArgs, CatalogShowArgs};
use crate::paths;
use crate::store::types::{Sensitivity, load_store, save_store};

pub fn add(args: &CatalogAddArgs) -> Result<()> {
    let path = paths::store_path()?;
    let mut store = load_store(&path)?;

    let item = store.entry(args.id.clone()).or_default();

    if let Some(ref desc) = args.description {
        item.description = Some(desc.clone());
    }
    if let Some(ref sensitivity) = args.sensitivity {
        item.sensitivity = Some(sensitivity.clone());
    }
    if let Some(ref origin) = args.origin {
        item.origin = Some(origin.clone());
    }
    if !args.env.is_empty() {
        item.environments = args.env.clone();
    }
    if !args.tag.is_empty() {
        item.tags = args.tag.clone();
    }

    save_store(&path, &store)?;
    println!("Updated catalog entry for {}", args.id);

    Ok(())
}

pub fn remove(id: &str) -> Result<()> {
    let path = paths::store_path()?;
    let mut store = load_store(&path)?;

    if store.remove(id).is_some() {
        save_store(&path, &store)?;
        println!("Removed {id}");
    } else {
        println!("Item '{id}' not found");
    }

    Ok(())
}

pub fn list(args: &CatalogListArgs) -> Result<()> {
    let path = paths::store_path()?;
    let store = load_store(&path)?;

    if store.is_empty() {
        println!("Store is empty.");
        return Ok(());
    }

    for (id, item) in &store {
        // Filter by environment
        if !args.env.is_empty()
            && !item.environments.iter().any(|e| args.env.contains(e))
        {
            continue;
        }

        // Filter by tag
        if let Some(ref tag) = args.tag {
            if !item.tags.contains(tag) {
                continue;
            }
        }

        // Filter by sensitivity
        if let Some(ref sensitivity) = args.sensitivity {
            match &item.sensitivity {
                Some(s) if s == sensitivity => {}
                _ => continue,
            }
        }

        let sens_label = item
            .sensitivity
            .as_ref()
            .map_or(String::new(), |s| format!(" [{s:?}]"));

        println!("{id}{sens_label}");
        if let Some(ref desc) = item.description {
            println!("  description: {desc}");
        }
        if let Some(ref origin) = item.origin {
            println!("  origin: {origin}");
        }
        if !item.environments.is_empty() {
            println!("  environments: {}", item.environments.join(", "));
        }
        if !item.tags.is_empty() {
            println!("  tags: {}", item.tags.join(", "));
        }
    }

    Ok(())
}

pub fn show(args: &CatalogShowArgs) -> Result<()> {
    let path = paths::store_path()?;
    let store = load_store(&path)?;

    let item = store
        .get(&args.id)
        .with_context(|| format!("item '{}' not found", args.id))?;

    println!("{}", args.id);

    if let Some(ref desc) = item.description {
        println!("  description: {desc}");
    }
    if let Some(ref sensitivity) = item.sensitivity {
        println!("  sensitivity: {sensitivity:?}");
    }
    if let Some(ref origin) = item.origin {
        println!("  origin: {origin}");
    }
    if !item.environments.is_empty() {
        println!("  environments: {}", item.environments.join(", "));
    }
    if !item.tags.is_empty() {
        println!("  tags: {}", item.tags.join(", "));
    }
    if !item.values.is_empty() {
        println!("  values:");
        for (env, _value) in &item.values {
            println!("    {env}: (set)");
        }
    }

    Ok(())
}

pub fn validate() -> Result<()> {
    let path = paths::store_path()?;
    let store = load_store(&path)?;

    let mut issues = Vec::new();

    for (id, item) in &store {
        // Check: item has expected environments but missing values
        for env in &item.environments {
            if !item.values.contains_key(env) {
                issues.push(format!("MISSING: {id} [{env}] — declared but no value set"));
            }
        }

        // Check: sensitivity declared but values not encrypted
        if matches!(
            item.sensitivity,
            Some(Sensitivity::Sensitive) | Some(Sensitivity::Secret)
        ) {
            for (env, value) in &item.values {
                if !value.starts_with("ENC[age:") && !value.starts_with("ENC[age,") {
                    issues.push(format!(
                        "UNENCRYPTED: {id} [{env}] — declared {:?} but stored as plaintext",
                        item.sensitivity.as_ref().unwrap()
                    ));
                }
            }
        }

        // Check: has values but no description (incomplete catalog entry)
        if !item.values.is_empty() && item.description.is_none() {
            issues.push(format!("UNDOCUMENTED: {id} — has values but no description"));
        }
    }

    if issues.is_empty() {
        println!("All items are valid.");
    } else {
        for issue in &issues {
            println!("{issue}");
        }
        std::process::exit(1);
    }

    Ok(())
}
