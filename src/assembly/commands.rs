use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::AssembleArgs;
use crate::crypto::decrypt_value;
use crate::paths;
use crate::store::types::{Store, load_store};

use super::types::{
    ComponentConfig, OverrideRule, glob_matches, load_manifest, load_topologies,
};

pub fn assemble(args: &AssembleArgs) -> Result<()> {
    let topo_path = Path::new("topologies.yaml");
    if !topo_path.exists() {
        anyhow::bail!("topologies.yaml not found in current directory");
    }

    let topologies = load_topologies(topo_path)?;
    let topology = topologies
        .get(&args.topology)
        .with_context(|| format!("topology '{}' not found in topologies.yaml", args.topology))?;

    let store_path = paths::store_path()?;
    let store = load_store(&store_path)?;

    // Filter to a single component if requested
    let component_names: Vec<&String> = if let Some(ref name) = args.component {
        if !topology.components.contains_key(name) {
            anyhow::bail!(
                "component '{name}' not found in topology '{}'",
                args.topology
            );
        }
        vec![topology.components.keys().find(|k| *k == name).expect("checked above")]
    } else {
        topology.components.keys().collect()
    };

    for comp_name in component_names {
        let config = &topology.components[comp_name];
        let overrides = topology.overrides.get(comp_name.as_str());
        assemble_component(comp_name, config, overrides.map_or(&[], Vec::as_slice), &store)?;
    }

    Ok(())
}

fn assemble_component(
    name: &str,
    config: &ComponentConfig,
    overrides: &[OverrideRule],
    store: &Store,
) -> Result<()> {
    let manifest_path = config.path.join("env.manifest.yaml");
    let manifest = load_manifest(&manifest_path)
        .with_context(|| format!("component '{name}'"))?;

    let mut lines = Vec::new();

    for (var_name, item_id) in &manifest.vars {
        let item = store.get(item_id).with_context(|| {
            format!("store item '{item_id}' not found (referenced by {var_name} in component '{name}')")
        })?;

        let env = resolve_env(item_id, &config.env, overrides);

        let raw_value = item.values.get(env).with_context(|| {
            format!(
                "no '{env}' value for item '{item_id}' (needed by {var_name} in component '{name}')"
            )
        })?;

        let value = if raw_value.starts_with("ENC[aes:") {
            decrypt_value(raw_value)
                .with_context(|| format!("failed to decrypt '{item_id}' ({env})"))?
        } else {
            raw_value.clone()
        };

        lines.push(format!("{var_name}={value}"));
    }

    let output_path = config.path.join(&manifest.target);
    write_env_file(&output_path, &lines)?;

    println!(
        "Wrote {} ({} vars)",
        output_path.display(),
        manifest.vars.len()
    );

    Ok(())
}

fn resolve_env<'a>(item_id: &str, default_env: &'a str, overrides: &'a [OverrideRule]) -> &'a str {
    for rule in overrides {
        if glob_matches(&rule.pattern, item_id) {
            return &rule.env;
        }
    }
    default_env
}

fn write_env_file(path: &Path, lines: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = if lines.is_empty() {
        String::new()
    } else {
        let mut s = lines.join("\n");
        s.push('\n');
        s
    };

    fs::write(path, contents)
        .with_context(|| format!("could not write {}", path.display()))
}
