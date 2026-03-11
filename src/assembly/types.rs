use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// A resolved component within a topology.
#[derive(Debug, Clone)]
pub struct ComponentConfig {
    pub env: String,
    pub path: PathBuf,
}

/// A single override rule: a glob pattern mapping matched item IDs to a different environment.
#[derive(Debug, Clone)]
pub struct OverrideRule {
    pub pattern: String,
    pub env: String,
}

/// A fully parsed topology.
#[derive(Debug, Clone)]
pub struct Topology {
    pub components: BTreeMap<String, ComponentConfig>,
    pub overrides: BTreeMap<String, Vec<OverrideRule>>,
}

/// A component manifest declaring which store items map to which env vars.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub target: String,
    pub vars: BTreeMap<String, String>,
}

/// Load all topologies from a YAML file.
pub fn load_topologies(path: &Path) -> Result<BTreeMap<String, Topology>> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;

    let raw: BTreeMap<String, BTreeMap<String, serde_yaml::Value>> =
        serde_yaml::from_str(&contents)
            .with_context(|| format!("could not parse {}", path.display()))?;

    let mut topologies = BTreeMap::new();

    for (topo_name, entries) in &raw {
        let topology = parse_topology(topo_name, entries)?;
        topologies.insert(topo_name.clone(), topology);
    }

    Ok(topologies)
}

fn parse_topology(
    name: &str,
    entries: &BTreeMap<String, serde_yaml::Value>,
) -> Result<Topology> {
    let mut components = BTreeMap::new();
    let mut overrides: BTreeMap<String, Vec<OverrideRule>> = BTreeMap::new();

    for (key, value) in entries {
        if key == "overrides" {
            overrides = parse_overrides(name, value)?;
            continue;
        }

        let config = parse_component_entry(name, key, value)?;
        components.insert(key.clone(), config);
    }

    Ok(Topology {
        components,
        overrides,
    })
}

fn parse_component_entry(
    topo_name: &str,
    comp_name: &str,
    value: &serde_yaml::Value,
) -> Result<ComponentConfig> {
    match value {
        // Short form: `api: dev`
        serde_yaml::Value::String(env) => Ok(ComponentConfig {
            env: env.clone(),
            path: PathBuf::from(comp_name),
        }),
        // Long form: `api: { env: dev, path: services/api }`
        serde_yaml::Value::Mapping(map) => {
            let env = map
                .get(serde_yaml::Value::String("env".to_string()))
                .and_then(serde_yaml::Value::as_str)
                .with_context(|| {
                    format!(
                        "topology '{topo_name}': component '{comp_name}' is missing required 'env' field"
                    )
                })?
                .to_string();

            let path = map
                .get(serde_yaml::Value::String("path".to_string()))
                .and_then(serde_yaml::Value::as_str)
                .map_or_else(|| PathBuf::from(comp_name), PathBuf::from);

            Ok(ComponentConfig { env, path })
        }
        _ => anyhow::bail!(
            "topology '{topo_name}': component '{comp_name}' must be a string (env) or mapping (env + path)"
        ),
    }
}

fn parse_overrides(
    topo_name: &str,
    value: &serde_yaml::Value,
) -> Result<BTreeMap<String, Vec<OverrideRule>>> {
    let serde_yaml::Value::Mapping(comp_map) = value else {
        anyhow::bail!("topology '{topo_name}': 'overrides' must be a mapping");
    };

    let mut result = BTreeMap::new();

    for (comp_key, rules_value) in comp_map {
        let comp_name = comp_key
            .as_str()
            .with_context(|| format!("topology '{topo_name}': override key must be a string"))?;

        let serde_yaml::Value::Mapping(rules_map) = rules_value else {
            anyhow::bail!(
                "topology '{topo_name}': overrides for '{comp_name}' must be a mapping"
            );
        };

        let mut rules = Vec::new();
        for (pattern_key, env_value) in rules_map {
            let pattern = pattern_key.as_str().with_context(|| {
                format!("topology '{topo_name}': override pattern must be a string")
            })?;
            let env = env_value.as_str().with_context(|| {
                format!(
                    "topology '{topo_name}': override env for pattern '{pattern}' must be a string"
                )
            })?;
            rules.push(OverrideRule {
                pattern: pattern.to_string(),
                env: env.to_string(),
            });
        }

        result.insert(comp_name.to_string(), rules);
    }

    Ok(result)
}

/// Load a component manifest from a YAML file.
pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read manifest at {}", path.display()))?;

    serde_yaml::from_str(&contents)
        .with_context(|| format!("could not parse manifest at {}", path.display()))
}

/// Simple glob matching: supports `*` as a wildcard for any characters.
///
/// Splits the pattern on `*` and checks that all parts appear in order in the text.
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();

    // No wildcard — exact match
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut remaining = text;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if i == 0 {
            // First segment must be a prefix
            let Some(rest) = remaining.strip_prefix(part) else {
                return false;
            };
            remaining = rest;
        } else if i == parts.len() - 1 {
            // Last segment must be a suffix
            if !remaining.ends_with(part) {
                return false;
            }
            return true;
        } else {
            // Middle segments: find next occurrence
            let Some(pos) = remaining.find(part) else {
                return false;
            };
            remaining = &remaining[pos + part.len()..];
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_exact_match() {
        assert!(glob_matches("supabase.url", "supabase.url"));
        assert!(!glob_matches("supabase.url", "supabase.anon_key"));
    }

    #[test]
    fn glob_trailing_wildcard() {
        assert!(glob_matches("supabase.*", "supabase.url"));
        assert!(glob_matches("supabase.*", "supabase.anon_key"));
        assert!(!glob_matches("supabase.*", "stripe.secret_key"));
    }

    #[test]
    fn glob_leading_wildcard() {
        assert!(glob_matches("*.url", "supabase.url"));
        assert!(!glob_matches("*.url", "supabase.anon_key"));
    }

    #[test]
    fn glob_middle_wildcard() {
        assert!(glob_matches("supabase.*.key", "supabase.anon.key"));
        assert!(!glob_matches("supabase.*.key", "supabase.url"));
    }

    #[test]
    fn glob_star_matches_everything() {
        assert!(glob_matches("*", "anything.at.all"));
    }
}
