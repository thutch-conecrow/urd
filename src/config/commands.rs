use anyhow::Result;

use crate::cli::ConfigSetDefaultsArgs;
use crate::paths::store_path;
use crate::store::types::{load_store, save_store};

pub fn set_defaults(args: &ConfigSetDefaultsArgs) -> Result<()> {
    let path = store_path()?;
    let mut store = load_store(&path)?;

    store.meta.default_environments.clone_from(&args.envs);
    save_store(&path, &store)?;

    if args.envs.is_empty() {
        println!("Cleared default environments");
    } else {
        println!("Default environments: {}", args.envs.join(", "));
    }

    Ok(())
}

pub fn show() -> Result<()> {
    let path = store_path()?;
    let store = load_store(&path)?;

    if store.meta.default_environments.is_empty() {
        println!("No default environments configured");
    } else {
        println!(
            "Default environments: {}",
            store.meta.default_environments.join(", ")
        );
    }

    Ok(())
}
