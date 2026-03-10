use anyhow::{Context, Result};
use dialoguer::{Input, Select};

use crate::cli::{GetArgs, ListArgs, RemoveArgs, SetArgs};
use crate::crypto;
use crate::crypto::SensitivityLevel;

use super::paths::store_path;
use super::types::{load_store, save_store};

pub fn set(args: SetArgs, global: bool) -> Result<()> {
    let path = store_path(global)?;
    let mut store = load_store(&path)?;

    let has_all_args = args.id.is_some() && !args.env.is_empty() && args.value.is_some();

    if has_all_args {
        let id = args.id.unwrap();
        let value = args.value.unwrap();

        let level = sensitivity_level_from_flags(args.sensitive, args.secret)
            .or_else(|| infer_sensitivity_level(&store, &id));

        let stored_value = if let Some(level) = level {
            crypto::encrypt_value(&value, level)?
        } else {
            value
        };

        let entry = store.entry(id.clone()).or_default();
        for env in &args.env {
            entry.insert(env.clone(), stored_value.clone());
        }
        save_store(&path, &store)?;
        println!("Set {id} for {}", args.env.join(", "));
    } else {
        // Interactive mode
        let id: String = if let Some(id) = args.id {
            id
        } else {
            Input::new().with_prompt("Config item ID").interact_text()?
        };

        let envs: Vec<String> = if args.env.is_empty() {
            let env_options = &["dev", "prod", "staging"];
            let selection = Select::new()
                .with_prompt("Environment")
                .items(env_options)
                .default(0)
                .interact()?;
            vec![env_options[selection].to_string()]
        } else {
            args.env
        };

        let level = if args.sensitive {
            Some(SensitivityLevel::Sensitive)
        } else if args.secret {
            Some(SensitivityLevel::Secret)
        } else {
            let selection = Select::new()
                .with_prompt("Sensitivity")
                .items(["plaintext", "sensitive", "secret"])
                .default(0)
                .interact()?;
            match selection {
                1 => Some(SensitivityLevel::Sensitive),
                2 => Some(SensitivityLevel::Secret),
                _ => None,
            }
        };

        let value: String = if let Some(v) = args.value {
            v
        } else {
            Input::new().with_prompt("Value").interact_text()?
        };

        let stored_value = if let Some(level) = level {
            crypto::encrypt_value(&value, level)?
        } else {
            value
        };

        let entry = store.entry(id.clone()).or_default();
        for env in &envs {
            entry.insert(env.clone(), stored_value.clone());
        }
        save_store(&path, &store)?;
        println!("Set {id} for {}", envs.join(", "));
    }

    Ok(())
}

pub fn get(args: &GetArgs, global: bool) -> Result<()> {
    let path = store_path(global)?;
    let store = load_store(&path)?;

    let env_values = store
        .get(&args.id)
        .with_context(|| format!("item '{}' not found", args.id))?;

    let envs_to_show: Vec<&String> = if args.env.is_empty() {
        env_values.keys().collect()
    } else {
        args.env.iter().collect()
    };

    let show_label = envs_to_show.len() > 1;

    for env in &envs_to_show {
        let value = env_values
            .get(env.as_str())
            .with_context(|| format!("no value for env '{env}'"))?;

        let display = if let Some(level) = crypto::parse_sensitivity(value) {
            if args.reveal {
                crypto::decrypt_value(value)?
            } else {
                format!("({})", level.tag())
            }
        } else {
            value.clone()
        };

        if show_label {
            println!("{env}: {display}");
        } else {
            println!("{display}");
        }
    }

    Ok(())
}

pub fn list(args: &ListArgs, global: bool) -> Result<()> {
    let path = store_path(global)?;
    let store = load_store(&path)?;

    if store.is_empty() {
        println!("Store is empty.");
        return Ok(());
    }

    for (id, envs) in &store {
        let filtered_envs: Vec<(&String, &String)> = if args.env.is_empty() {
            envs.iter().collect()
        } else {
            envs.iter()
                .filter(|(k, _)| args.env.contains(k))
                .collect()
        };

        if filtered_envs.is_empty() {
            continue;
        }

        let sensitivity = envs
            .values()
            .find_map(|v| crypto::parse_sensitivity(v));
        let marker = sensitivity
            .map_or_else(String::new, |level| format!(" [{}]", level.tag()));

        println!("{id}{marker}");
        for (env, value) in &filtered_envs {
            let display = if let Some(level) = crypto::parse_sensitivity(value) {
                if args.reveal {
                    crypto::decrypt_value(value)?
                } else {
                    format!("({})", level.tag())
                }
            } else {
                (*value).clone()
            };
            println!("  {env}: {display}");
        }
    }

    Ok(())
}

pub fn remove(args: &RemoveArgs, global: bool) -> Result<()> {
    let path = store_path(global)?;
    let mut store = load_store(&path)?;

    if store.remove(&args.id).is_some() {
        save_store(&path, &store)?;
        println!("Removed {}", args.id);
    } else {
        println!("Item '{}' not found", args.id);
    }

    Ok(())
}

const fn sensitivity_level_from_flags(sensitive: bool, secret: bool) -> Option<SensitivityLevel> {
    if secret {
        Some(SensitivityLevel::Secret)
    } else if sensitive {
        Some(SensitivityLevel::Sensitive)
    } else {
        None
    }
}

fn infer_sensitivity_level(
    store: &super::types::Store,
    id: &str,
) -> Option<SensitivityLevel> {
    store
        .get(id)
        .and_then(|envs| envs.values().find_map(|v| crypto::parse_sensitivity(v)))
}
