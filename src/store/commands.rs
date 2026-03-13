use anyhow::{Context, Result};
use dialoguer::{Input, Select};

use crate::cli::{GetArgs, ListArgs, RemoveArgs, SetArgs};
use crate::crypto;
use crate::crypto::SensitivityLevel;

use super::paths::store_path;
use super::types::{Sensitivity, apply_default_environments, load_store, save_store};

#[allow(clippy::too_many_lines)]
pub fn set(mut args: SetArgs) -> Result<()> {
    let path = store_path()?;
    let mut store = load_store(&path)?;

    if let (Some(id), Some(value), false) =
        (args.id.as_deref(), args.value.as_deref(), args.env.is_empty())
    {
        let id = id.to_owned();
        let value = value.to_owned();

        let level = sensitivity_level_from_flags(args.sensitive, args.secret)
            .or_else(|| infer_sensitivity_level(&store, &id));

        let stored_value = if let Some(level) = level {
            crypto::encrypt_value(&value, level)?
        } else {
            value
        };

        let meta = store.meta.clone();
        let item = store.entry(id.clone()).or_default();
        for env in &args.env {
            item.values.insert(env.clone(), stored_value.clone());
        }
        apply_default_environments(&meta, item);
        save_store(&path, &store)?;
        println!("Set {id} for {}", args.env.join(", "));
    } else {
        // Interactive mode — Escape on Select prompts goes back one step
        let env_options: Vec<String> = if store.meta.default_environments.is_empty() {
            vec!["dev".into(), "prod".into(), "staging".into()]
        } else {
            store.meta.default_environments.clone()
        };
        let sensitivity_options = &["plaintext", "sensitive", "secret"];
        let has_id = args.id.is_some();
        let has_env = !args.env.is_empty();
        let has_sensitivity = args.sensitive || args.secret;

        // Steps that support back-navigation via Escape
        #[allow(clippy::items_after_statements)]
        #[derive(Clone, Copy)]
        enum Step { Id, Env, Sensitivity, Value, Description, Origin, Tags, Done }

        let first_step = if has_id && has_env {
            Step::Sensitivity
        } else if has_id {
            Step::Env
        } else {
            Step::Id
        };

        let mut step = first_step;
        let mut id: String = args.id.take().unwrap_or_default();
        let mut envs: Vec<String> = args.env;
        let mut level: Option<SensitivityLevel> = sensitivity_level_from_flags(args.sensitive, args.secret);
        let mut stored_value = String::new();
        let mut description: Option<String> = None;
        let mut origin: Option<String> = None;
        let mut tags: Vec<String>;

        loop {
            match step {
                Step::Id => {
                    id = Input::new().with_prompt("Config item ID").interact_text()?;
                    step = if has_env { Step::Sensitivity } else { Step::Env };
                }
                Step::Env => {
                    let selection = Select::new()
                        .with_prompt("Environment")
                        .items(&env_options)
                        .default(0)
                        .interact_opt()?;
                    if let Some(i) = selection {
                        envs = vec![env_options[i].clone()];
                        step = if has_sensitivity { Step::Value } else { Step::Sensitivity };
                    } else {
                        if has_id { return Ok(()); }
                        step = Step::Id;
                    }
                }
                Step::Sensitivity => {
                    let default_idx = store
                        .get(&id)
                        .and_then(|item| item.sensitivity.as_ref())
                        .map_or(0, |s| match s {
                            Sensitivity::Plaintext => 0,
                            Sensitivity::Sensitive => 1,
                            Sensitivity::Secret => 2,
                        });
                    let selection = Select::new()
                        .with_prompt("Sensitivity")
                        .items(sensitivity_options)
                        .default(default_idx)
                        .interact_opt()?;
                    if let Some(i) = selection {
                        level = match i {
                            1 => Some(SensitivityLevel::Sensitive),
                            2 => Some(SensitivityLevel::Secret),
                            _ => None,
                        };
                        step = Step::Value;
                    } else {
                        if has_env { return Ok(()); }
                        step = Step::Env;
                    }
                }
                Step::Value => {
                    let value: String = if let Some(v) = args.value.take() {
                        v
                    } else {
                        Input::new().with_prompt("Value").interact_text()?
                    };

                    stored_value = if let Some(level) = level {
                        crypto::encrypt_value(&value, level)?
                    } else {
                        value
                    };

                    step = Step::Description;
                }
                Step::Description => {
                    let input: String = Input::new()
                        .with_prompt("Description (optional, enter to skip)")
                        .allow_empty(true)
                        .interact_text()?;
                    description = if input.is_empty() { None } else { Some(input) };
                    step = Step::Origin;
                }
                Step::Origin => {
                    let input: String = Input::new()
                        .with_prompt("Origin (optional, enter to skip)")
                        .allow_empty(true)
                        .interact_text()?;
                    origin = if input.is_empty() { None } else { Some(input) };
                    step = Step::Tags;
                }
                Step::Tags => {
                    let input: String = Input::new()
                        .with_prompt("Tags (optional, comma-separated)")
                        .allow_empty(true)
                        .interact_text()?;
                    tags = if input.is_empty() {
                        Vec::new()
                    } else {
                        input.split(',').map(|s| s.trim().to_string()).collect()
                    };

                    // Save everything
                    let meta = store.meta.clone();
                    let item = store.entry(id.clone()).or_default();
                    for env in &envs {
                        item.values.insert(env.clone(), stored_value.clone());
                    }
                    if description.is_some() {
                        item.description = description.take();
                    }
                    if origin.is_some() {
                        item.origin = origin.take();
                    }
                    if !tags.is_empty() {
                        item.tags.clone_from(&tags);
                    }
                    if let Some(sens) = &level {
                        item.sensitivity = Some(match sens {
                            SensitivityLevel::Sensitive => Sensitivity::Sensitive,
                            SensitivityLevel::Secret => Sensitivity::Secret,
                        });
                    }
                    item.environments.clone_from(&envs);
                    apply_default_environments(&meta, item);

                    save_store(&path, &store)?;
                    println!("Set {id} for {}", envs.join(", "));
                    step = Step::Done;
                }
                Step::Done => break,
            }
        }
    }

    Ok(())
}

pub fn get(args: &GetArgs) -> Result<()> {
    let path = store_path()?;
    let store = load_store(&path)?;

    let item = store
        .get(&args.id)
        .with_context(|| format!("item '{}' not found", args.id))?;

    let envs_to_show: Vec<&String> = if args.env.is_empty() {
        item.values.keys().collect()
    } else {
        args.env.iter().collect()
    };

    let show_label = envs_to_show.len() > 1;

    for env in &envs_to_show {
        let value = item
            .values
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

pub fn list(args: &ListArgs) -> Result<()> {
    let path = store_path()?;
    let store = load_store(&path)?;

    if store.is_empty() {
        println!("Store is empty.");
        return Ok(());
    }

    for (id, item) in store.iter() {
        let filtered_values: Vec<(&String, &String)> = if args.env.is_empty() {
            item.values.iter().collect()
        } else {
            item.values
                .iter()
                .filter(|(k, _)| args.env.contains(k))
                .collect()
        };

        if filtered_values.is_empty() && item.values.is_empty() && item.description.is_none() {
            continue;
        }

        let sensitivity = item
            .values
            .values()
            .find_map(|v| crypto::parse_sensitivity(v));
        let marker = sensitivity
            .map_or_else(String::new, |level| format!(" [{}]", level.tag()));

        println!("{id}{marker}");
        for (env, value) in &filtered_values {
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

pub fn remove(args: &RemoveArgs) -> Result<()> {
    let path = store_path()?;
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

pub fn infer_sensitivity_level(
    store: &super::types::Store,
    id: &str,
) -> Option<SensitivityLevel> {
    store.get(id).and_then(|item| {
        item.sensitivity
            .as_ref()
            .and_then(Sensitivity::to_sensitivity_level)
            .or_else(|| {
                item.values
                    .values()
                    .find_map(|v| crypto::parse_sensitivity(v))
            })
    })
}
