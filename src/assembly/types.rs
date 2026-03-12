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

/// A parsed template file: lines with optional `{{ item.id }}` expressions.
#[derive(Debug, Clone)]
pub struct Template {
    pub target: PathBuf,
    pub lines: Vec<String>,
}

/// What assembly found for a component: either a YAML manifest or a template file.
#[derive(Debug)]
pub enum ComponentSource {
    Manifest(Manifest),
    Template(Template),
}

/// Load a component manifest from a YAML file.
pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read manifest at {}", path.display()))?;

    serde_yaml::from_str(&contents)
        .with_context(|| format!("could not parse manifest at {}", path.display()))
}

/// Discover the component source (manifest or template) for a component.
///
/// Checks in order: `env.manifest.yaml`, `env.template`, `.env.template`.
pub fn discover_component_source(comp_path: &Path) -> Result<ComponentSource> {
    let manifest_path = comp_path.join("env.manifest.yaml");
    if manifest_path.exists() {
        let manifest = load_manifest(&manifest_path)?;
        return Ok(ComponentSource::Manifest(manifest));
    }

    for template_name in ["env.template", ".env.template"] {
        let template_path = comp_path.join(template_name);
        if template_path.exists() {
            let template = load_template(&template_path, template_name)?;
            return Ok(ComponentSource::Template(template));
        }
    }

    anyhow::bail!(
        "no manifest or template found in {} (looked for env.manifest.yaml, env.template, .env.template)",
        comp_path.display()
    )
}

/// Load and parse a template file.
///
/// Extracts optional frontmatter `# target: <path>`, otherwise infers target
/// by stripping the `.template` suffix from the filename.
fn load_template(path: &Path, filename: &str) -> Result<Template> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read template at {}", path.display()))?;

    let mut lines: Vec<&str> = contents.lines().collect();
    let mut target = None;

    // Check for frontmatter: `# target: <path>`
    if let Some(first_line) = lines.first()
        && let Some(rest) = first_line.strip_prefix("# target:")
    {
        target = Some(PathBuf::from(rest.trim()));
        lines.remove(0);
    }

    // Infer target from filename if no frontmatter
    let target = target.unwrap_or_else(|| {
        let inferred = filename.strip_suffix(".template").unwrap_or(filename);
        PathBuf::from(inferred)
    });

    let lines = lines.iter().map(|l| (*l).to_string()).collect();

    Ok(Template { target, lines })
}

/// Extract `{{ item.id }}` expressions from a line, returning `(start, end, item_id)` tuples.
pub fn find_expressions(line: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    let mut search_from = 0;

    while let Some(start) = line[search_from..].find("{{") {
        let abs_start = search_from + start;
        if let Some(end) = line[abs_start..].find("}}") {
            let abs_end = abs_start + end + 2;
            let inner = line[abs_start + 2..abs_end - 2].trim().to_string();
            if !inner.is_empty() {
                results.push((abs_start, abs_end, inner));
            }
            search_from = abs_end;
        } else {
            break;
        }
    }

    results
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

    #[test]
    fn find_expressions_none() {
        assert!(find_expressions("PORT=3000").is_empty());
        assert!(find_expressions("# just a comment").is_empty());
        assert!(find_expressions("").is_empty());
    }

    #[test]
    fn find_expressions_single() {
        let exprs = find_expressions("API_KEY={{ stripe.secret_key }}");
        assert_eq!(exprs.len(), 1);
        assert_eq!(exprs[0].2, "stripe.secret_key");
    }

    #[test]
    fn find_expressions_multiple() {
        let exprs =
            find_expressions("URL={{ app.protocol }}://{{ app.host }}:{{ app.port }}");
        assert_eq!(exprs.len(), 3);
        assert_eq!(exprs[0].2, "app.protocol");
        assert_eq!(exprs[1].2, "app.host");
        assert_eq!(exprs[2].2, "app.port");
    }

    #[test]
    fn find_expressions_no_space() {
        let exprs = find_expressions("KEY={{stripe.key}}");
        assert_eq!(exprs.len(), 1);
        assert_eq!(exprs[0].2, "stripe.key");
    }

    #[test]
    fn find_expressions_unclosed() {
        assert!(find_expressions("KEY={{ broken").is_empty());
    }

    #[test]
    fn template_target_inferred_from_env_template() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("env.template");
        fs::write(&path, "PORT=3000\n").unwrap();
        let template = load_template(&path, "env.template").unwrap();
        assert_eq!(template.target, PathBuf::from("env"));
    }

    #[test]
    fn template_target_inferred_from_dotenv_template() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(".env.template");
        fs::write(&path, "PORT=3000\n").unwrap();
        let template = load_template(&path, ".env.template").unwrap();
        assert_eq!(template.target, PathBuf::from(".env"));
    }

    #[test]
    fn template_target_from_frontmatter() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(".env.template");
        fs::write(&path, "# target: .env.local\nPORT=3000\n").unwrap();
        let template = load_template(&path, ".env.template").unwrap();
        assert_eq!(template.target, PathBuf::from(".env.local"));
        assert_eq!(template.lines.len(), 1);
        assert_eq!(template.lines[0], "PORT=3000");
    }
}
