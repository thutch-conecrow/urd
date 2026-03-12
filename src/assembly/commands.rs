use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::AssembleArgs;
use crate::crypto::decrypt_value;
use crate::paths;
use crate::store::types::{Store, load_store};

use super::types::{
    ComponentConfig, ComponentSource, OverrideRule, discover_component_source, find_expressions,
    glob_matches, load_topologies,
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
        vec![topology
            .components
            .keys()
            .find(|k| *k == name)
            .expect("checked above")]
    } else {
        topology.components.keys().collect()
    };

    for comp_name in component_names {
        let config = &topology.components[comp_name];
        let overrides = topology.overrides.get(comp_name.as_str());
        assemble_component(
            comp_name,
            config,
            overrides.map_or(&[], Vec::as_slice),
            &store,
            args.allow_missing,
        )?;
    }

    Ok(())
}

fn assemble_component(
    name: &str,
    config: &ComponentConfig,
    overrides: &[OverrideRule],
    store: &Store,
    allow_missing: bool,
) -> Result<()> {
    let source = discover_component_source(&config.path)
        .with_context(|| format!("component '{name}'"))?;

    match source {
        ComponentSource::Manifest(manifest) => {
            assemble_from_manifest(name, config, overrides, store, &manifest, allow_missing)
        }
        ComponentSource::Template(template) => {
            assemble_from_template(name, config, overrides, store, &template, allow_missing)
        }
    }
}

fn assemble_from_manifest(
    name: &str,
    config: &ComponentConfig,
    overrides: &[OverrideRule],
    store: &Store,
    manifest: &super::types::Manifest,
    allow_missing: bool,
) -> Result<()> {
    let mut lines = Vec::new();

    for (var_name, item_id) in &manifest.vars {
        let value = resolve_item(name, item_id, var_name, &config.env, overrides, store, allow_missing)?;
        lines.push(format!("{var_name}={}", value.unwrap_or_default()));
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

fn assemble_from_template(
    name: &str,
    config: &ComponentConfig,
    overrides: &[OverrideRule],
    store: &Store,
    template: &super::types::Template,
    allow_missing: bool,
) -> Result<()> {
    let mut output_lines = Vec::new();
    let mut resolved_count = 0u32;

    for line in &template.lines {
        let expressions = find_expressions(line);
        if expressions.is_empty() {
            output_lines.push(line.clone());
            continue;
        }

        let mut result = line.clone();
        // Process expressions in reverse order so positions stay valid
        for (start, end, item_id) in expressions.iter().rev() {
            let context_hint = format!("template expression '{{{{{item_id}}}}}' in component '{name}'");
            let value =
                resolve_item(name, item_id, &context_hint, &config.env, overrides, store, allow_missing)?;
            result.replace_range(start..end, &value.unwrap_or_default());
            resolved_count += 1;
        }

        output_lines.push(result);
    }

    let output_path = config.path.join(&template.target);
    write_env_file(&output_path, &output_lines)?;

    println!(
        "Wrote {} ({resolved_count} resolved)",
        output_path.display(),
    );

    Ok(())
}

/// Resolve a single store item to its decrypted value.
///
/// Returns `Ok(Some(value))` on success, `Ok(None)` if missing and `allow_missing` is true,
/// or an error if missing and `allow_missing` is false.
fn resolve_item(
    comp_name: &str,
    item_id: &str,
    context: &str,
    default_env: &str,
    overrides: &[OverrideRule],
    store: &Store,
    allow_missing: bool,
) -> Result<Option<String>> {
    let Some(item) = store.get(item_id) else {
        if allow_missing {
            eprintln!(
                "warning: store item '{item_id}' not found ({context})"
            );
            return Ok(None);
        }
        anyhow::bail!(
            "store item '{item_id}' not found (referenced by {context} in component '{comp_name}')"
        );
    };

    let env = resolve_env(item_id, default_env, overrides);

    let Some(raw_value) = item.values.get(env) else {
        if allow_missing {
            eprintln!(
                "warning: no '{env}' value for item '{item_id}' ({context})"
            );
            return Ok(None);
        }
        anyhow::bail!(
            "no '{env}' value for item '{item_id}' (needed by {context} in component '{comp_name}')"
        );
    };

    let value = if raw_value.starts_with("ENC[aes:") {
        decrypt_value(raw_value)
            .with_context(|| format!("failed to decrypt '{item_id}' ({env})"))?
    } else {
        raw_value.clone()
    };

    Ok(Some(value))
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
