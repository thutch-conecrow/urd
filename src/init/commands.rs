use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{KeyInit, OsRng};
use aes_gcm::Aes256Gcm;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use dialoguer::{Input, Select};

use crate::cli::InitArgs;
use crate::paths;
use crate::store::types::{load_store, save_store};

pub fn init(args: &InitArgs) -> Result<()> {
    let interactive = args.env.is_empty();

    init_keys()?;
    let envs = init_envs(interactive, &args.env)?;
    init_topologies(&envs)?;

    println!("\nDone. Next steps:");
    println!("  urd set              # add config values (interactive)");
    println!("  urd import .env -e dev  # import from an existing .env file");
    println!("  urd                  # launch the TUI");

    Ok(())
}

fn init_keys() -> Result<()> {
    let key_id_path = paths::key_id_path()?;

    if key_id_path.exists() {
        let id = fs::read_to_string(&key_id_path)?.trim().to_string();
        println!("Keys: already configured ({id}), skipping");
        return Ok(());
    }

    let mut id_bytes = [0u8; 4];
    OsRng.fill_bytes(&mut id_bytes);
    let mut id = String::with_capacity(8);
    for b in id_bytes {
        write!(id, "{b:02x}").expect("hex format");
    }

    let key = Aes256Gcm::generate_key(OsRng);
    let key_b64 = BASE64.encode(key);

    let key_path = paths::key_file_path(&id)?;
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&key_path, &key_b64)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
    }

    if let Some(parent) = key_id_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&key_id_path, &id)?;

    println!("Keys: generated ({id})");

    Ok(())
}

fn init_envs(interactive: bool, cli_envs: &[String]) -> Result<Vec<String>> {
    let store_path = paths::store_path()?;
    let mut store = load_store(&store_path)?;

    if !store.meta.default_environments.is_empty() {
        let existing = store.meta.default_environments.join(", ");
        println!("Environments: already configured ({existing}), skipping");
        return Ok(store.meta.default_environments.clone());
    }

    let envs = if interactive {
        pick_envs()?
    } else {
        cli_envs.to_vec()
    };

    store.meta.default_environments.clone_from(&envs);
    save_store(&store_path, &store)?;

    println!("Environments: {}", envs.join(", "));

    Ok(envs)
}

const ENV_PRESETS: &[(&str, &[&str])] = &[
    ("dev, prod", &["dev", "prod"]),
    ("local, dev, prod", &["local", "dev", "prod"]),
    ("local, staging, prod", &["local", "staging", "prod"]),
    ("dev, staging, prod", &["dev", "staging", "prod"]),
];

fn pick_envs() -> Result<Vec<String>> {
    let mut labels: Vec<&str> = ENV_PRESETS.iter().map(|(label, _)| *label).collect();
    labels.push("Custom");

    let selection = Select::new()
        .with_prompt("Default environments")
        .items(&labels)
        .default(0)
        .interact()?;

    if selection < ENV_PRESETS.len() {
        return Ok(ENV_PRESETS[selection]
            .1
            .iter()
            .map(|s| (*s).to_string())
            .collect());
    }

    let input: String = Input::new()
        .with_prompt("Environments (comma-separated)")
        .interact_text()?;

    Ok(input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn init_topologies(envs: &[String]) -> Result<()> {
    let topo_path = Path::new("topologies.yaml");

    if topo_path.exists() {
        println!("Topologies: topologies.yaml already exists, skipping");
        return Ok(());
    }

    let skeleton = build_topology_skeleton(envs);
    fs::write(topo_path, skeleton)?;

    println!("Topologies: created topologies.yaml");

    Ok(())
}

fn build_topology_skeleton(envs: &[String]) -> String {
    let mut out = String::from("# Topology presets — each maps components to environments\n");
    out.push_str("# Used with: urd assemble --topology <name>\n");
    out.push_str("#\n");
    out.push_str("# Components can be a simple env string or a mapping:\n");
    out.push_str("#   api: dev                  # looks for templates in ./api/\n");
    out.push_str("#   api:                      # explicit path\n");
    out.push_str("#     env: dev\n");
    out.push_str("#     path: services/backend\n\n");

    // Generate a commented-out "all-<env>" topology for each environment
    for env in envs {
        let _ = writeln!(out, "# all-{env}:");
        let _ = writeln!(out, "#   app: {env}");
        out.push_str("#   # api:\n");
        out.push_str("#   # web:\n");
        out.push('\n');
    }

    out
}
