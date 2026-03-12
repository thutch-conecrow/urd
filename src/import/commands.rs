use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::{Format, ImportArgs};
use crate::crypto;
use crate::store::commands::infer_sensitivity_level;
use crate::store::types::{load_store, save_store};

use crate::paths::store_path;

const fn sensitivity_level_from_flags(
    sensitive: bool,
    secret: bool,
) -> Option<crypto::SensitivityLevel> {
    if secret {
        Some(crypto::SensitivityLevel::Secret)
    } else if sensitive {
        Some(crypto::SensitivityLevel::Sensitive)
    } else {
        None
    }
}

pub fn import(args: &ImportArgs) -> Result<()> {
    let input = if args.path == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read from stdin")?;
        buf
    } else {
        std::fs::read_to_string(&args.path)
            .with_context(|| format!("could not read '{}'", args.path))?
    };

    let format = args
        .format
        .as_ref()
        .map_or_else(|| detect_format(&args.path), Clone::clone);

    let pairs = match format {
        Format::Dotenv => parse_dotenv(&input),
        Format::Yaml => parse_yaml(&input)?,
    };

    let path = store_path()?;
    let mut store = load_store(&path)?;
    let explicit_level = sensitivity_level_from_flags(args.sensitive, args.secret);

    let mut imported = 0u32;
    let mut skipped = 0u32;

    for (key, value) in &pairs {
        let item = store.entry(key.clone()).or_default();

        if args.skip_existing && item.values.contains_key(&args.env) {
            if args.dry_run {
                println!("  skip  {key} (already exists)");
            }
            skipped += 1;
            continue;
        }

        let level = explicit_level.or_else(|| infer_sensitivity_level(&store, key));

        if args.dry_run {
            let action = if level.is_some() { "encrypt" } else { "set" };
            println!("  {action:7} {key}={value}");
        } else {
            let stored_value = if let Some(level) = level {
                crypto::encrypt_value(value, level)?
            } else {
                value.clone()
            };

            let item = store.entry(key.clone()).or_default();
            item.values.insert(args.env.clone(), stored_value);
        }
        imported += 1;
    }

    if !args.dry_run {
        save_store(&path, &store)?;
    }

    let dry_label = if args.dry_run { " (dry run)" } else { "" };
    if skipped > 0 {
        println!("Imported {imported} items into {} ({skipped} skipped){dry_label}", args.env);
    } else {
        println!("Imported {imported} items into {}{dry_label}", args.env);
    }

    Ok(())
}

fn detect_format(path: &str) -> Format {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "yaml" | "yml" => Format::Yaml,
        _ => Format::Dotenv,
    }
}

fn parse_dotenv(input: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            eprintln!("warning: skipping line without '=': {trimmed}");
            continue;
        };
        let key = key.trim().to_string();
        let value = strip_quotes(value.trim());
        pairs.push((key, value));
    }
    pairs
}

fn strip_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_yaml(input: &str) -> Result<Vec<(String, String)>> {
    let map: BTreeMap<String, String> =
        serde_yaml::from_str(input).context("could not parse YAML as a flat string map")?;
    Ok(map.into_iter().collect())
}
