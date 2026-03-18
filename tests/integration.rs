use std::fs;

use predicates::prelude::*;
use tempfile::TempDir;

fn setup() -> (TempDir, TempDir) {
    let urd_home = TempDir::new().expect("failed to create urd_home tempdir");
    let work_dir = TempDir::new().expect("failed to create work_dir tempdir");
    (urd_home, work_dir)
}

fn urd(urd_home: &TempDir, work_dir: &TempDir) -> assert_cmd::Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("urd");
    cmd.env("URD_HOME", urd_home.path());
    cmd.current_dir(work_dir.path());
    cmd
}

// -- keys init --

#[test]
fn keys_init_creates_key() {
    let (home, work) = setup();

    urd(&home, &work)
        .args(["keys", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Key generated"))
        .stdout(predicate::str::contains("Key file:"));

    // key-id file should exist
    assert!(home.path().join("key-id").exists());

    // Read the key ID and verify the key file exists
    let key_id = std::fs::read_to_string(home.path().join("key-id")).unwrap();
    assert!(home
        .path()
        .join("keys")
        .join(format!("{}.key", key_id.trim()))
        .exists());
}

#[test]
fn keys_init_refuses_if_already_initialized() {
    let (home, work) = setup();

    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["keys", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("key already configured"));
}

#[test]
fn keys_status_shows_initialized() {
    let (home, work) = setup();

    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["keys", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Key ID:"))
        .stdout(predicate::str::contains("(found)"));
}

#[test]
fn keys_status_shows_not_initialized() {
    let (home, work) = setup();

    urd(&home, &work)
        .args(["keys", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No key configured"));
}

// -- list empty store --

#[test]
fn list_empty_store() {
    let (home, work) = setup();

    urd(&home, &work)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Store is empty"));
}

// -- set and get --

#[test]
fn set_and_get_plaintext_value() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "supabase.url", "--env", "dev", "http://localhost:54321"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Set supabase.url for dev"));

    urd(&home, &work)
        .args(["get", "supabase.url", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("http://localhost:54321"));
}

#[test]
fn set_multiple_environments_and_get_each() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "supabase.url", "--env", "dev", "http://localhost:54321"])
        .assert()
        .success();

    urd(&home, &work)
        .args([
            "set",
            "supabase.url",
            "--env",
            "prod",
            "https://myproject.supabase.co",
        ])
        .assert()
        .success();

    // Get dev
    urd(&home, &work)
        .args(["get", "supabase.url", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("http://localhost:54321"));

    // Get prod
    urd(&home, &work)
        .args(["get", "supabase.url", "--env", "prod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("https://myproject.supabase.co"));

    // Get all envs (no --env flag)
    urd(&home, &work)
        .args(["get", "supabase.url"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dev:"))
        .stdout(predicate::str::contains("prod:"));

    // Get multiple specific envs
    urd(&home, &work)
        .args(["get", "supabase.url", "--env", "dev", "--env", "prod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dev:"))
        .stdout(predicate::str::contains("prod:"));
}

#[test]
fn set_overwrites_existing_value() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Set initial value
    urd(&home, &work)
        .args(["set", "app.port", "--env", "dev", "3000"])
        .assert()
        .success();

    urd(&home, &work)
        .args(["get", "app.port", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3000"));

    // Overwrite
    urd(&home, &work)
        .args(["set", "app.port", "--env", "dev", "8080"])
        .assert()
        .success();

    urd(&home, &work)
        .args(["get", "app.port", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("8080"));
}

#[test]
fn mutation_does_not_affect_other_environments() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "app.port", "--env", "dev", "3000"])
        .assert()
        .success();

    urd(&home, &work)
        .args(["set", "app.port", "--env", "prod", "80"])
        .assert()
        .success();

    // Mutate only dev
    urd(&home, &work)
        .args(["set", "app.port", "--env", "dev", "8080"])
        .assert()
        .success();

    // Dev is updated
    urd(&home, &work)
        .args(["get", "app.port", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("8080"));

    // Prod is unchanged
    urd(&home, &work)
        .args(["get", "app.port", "--env", "prod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("80"));
}

// -- list --

#[test]
fn list_shows_items_after_set() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "app.port", "--env", "dev", "3000"])
        .assert()
        .success();

    urd(&home, &work)
        .args(["set", "supabase.url", "--env", "dev", "http://localhost"])
        .assert()
        .success();

    urd(&home, &work)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("app.port"))
        .stdout(predicate::str::contains("dev: 3000"))
        .stdout(predicate::str::contains("supabase.url"))
        .stdout(predicate::str::contains("dev: http://localhost"));
}

#[test]
fn list_shows_values_with_env_filter() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "app.port", "--env", "dev", "--env", "prod", "3000"])
        .assert()
        .success();

    // Filter to just prod
    let output = urd(&home, &work)
        .args(["list", "--env", "prod"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("prod: 3000"));
    assert!(!stdout.contains("dev:"));
}

#[test]
fn list_reveals_sensitive_values() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "api.key", "--env", "dev", "--sensitive", "secret123"])
        .assert()
        .success();

    // Without --reveal, shows redacted
    urd(&home, &work)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(sensitive)"));

    // With --reveal, shows actual value
    urd(&home, &work)
        .args(["list", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("secret123"));
}

// -- remove --

#[test]
fn remove_item() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "app.port", "--env", "dev", "3000"])
        .assert()
        .success();

    urd(&home, &work)
        .args(["remove", "app.port"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed app.port"));

    urd(&home, &work)
        .args(["get", "app.port", "--env", "dev"])
        .assert()
        .failure();
}

#[test]
fn get_nonexistent_item_fails() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["get", "does.not.exist", "--env", "dev"])
        .assert()
        .failure();
}

#[test]
fn set_multiple_envs_at_once() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["set", "app.secret", "--env", "dev", "--env", "prod", "shared_val"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Set app.secret for dev, prod"));

    urd(&home, &work)
        .args(["get", "app.secret", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("shared_val"));

    urd(&home, &work)
        .args(["get", "app.secret", "--env", "prod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("shared_val"));
}

// -- encrypted values --

#[test]
fn sensitive_value_is_redacted_by_default() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args([
            "set", "paddle.api_key", "--env", "dev", "--sensitive", "sk_test_abc123",
        ])
        .assert()
        .success();

    // Get without --reveal shows redacted label
    urd(&home, &work)
        .args(["get", "paddle.api_key", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(sensitive)"));

    // Get with --reveal shows the actual value
    urd(&home, &work)
        .args(["get", "paddle.api_key", "--env", "dev", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_test_abc123"));

    // List shows sensitivity level
    urd(&home, &work)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[sensitive]"));
}

#[test]
fn secret_value_is_redacted_by_default() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args([
            "set", "db.password", "--env", "prod", "--secret", "hunter2",
        ])
        .assert()
        .success();

    // Get without --reveal shows redacted label
    urd(&home, &work)
        .args(["get", "db.password", "--env", "prod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(secret)"));

    // Get with --reveal shows the actual value
    urd(&home, &work)
        .args(["get", "db.password", "--env", "prod", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hunter2"));

    // List shows sensitivity level
    urd(&home, &work)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[secret]"));
}

#[test]
fn sensitive_value_mutation_round_trips() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args([
            "set", "paddle.api_key", "--env", "dev", "--sensitive", "old_key",
        ])
        .assert()
        .success();

    urd(&home, &work)
        .args(["get", "paddle.api_key", "--env", "dev", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("old_key"));

    // Overwrite — infer_sensitivity_level should pick up the level
    urd(&home, &work)
        .args([
            "set", "paddle.api_key", "--env", "dev", "new_key",
        ])
        .assert()
        .success();

    urd(&home, &work)
        .args(["get", "paddle.api_key", "--env", "dev", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("new_key"));

    // Still shows as sensitive
    urd(&home, &work)
        .args(["get", "paddle.api_key", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(sensitive)"));
}

// -- assemble --

/// Helper: write a file relative to work_dir, creating parent dirs as needed.
fn write_file(work_dir: &TempDir, rel_path: &str, contents: &str) {
    let path = work_dir.path().join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(&path, contents).expect("write file");
}

/// Helper: read a file relative to work_dir.
fn read_file(work_dir: &TempDir, rel_path: &str) -> String {
    fs::read_to_string(work_dir.path().join(rel_path)).expect("read file")
}

/// Set up a store with a few items and write topology + manifests for assembly tests.
fn setup_assembly() -> (TempDir, TempDir) {
    let (home, work) = setup();

    // Init key
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Populate store
    urd(&home, &work)
        .args(["set", "app.url", "--env", "dev", "http://localhost:3000"])
        .assert()
        .success();
    urd(&home, &work)
        .args(["set", "app.url", "--env", "prod", "https://app.example.com"])
        .assert()
        .success();
    urd(&home, &work)
        .args(["set", "db.host", "--env", "dev", "localhost"])
        .assert()
        .success();
    urd(&home, &work)
        .args(["set", "db.host", "--env", "prod", "db.example.com"])
        .assert()
        .success();
    urd(&home, &work)
        .args(["set", "db.password", "--env", "dev", "--secret", "devpass"])
        .assert()
        .success();
    urd(&home, &work)
        .args(["set", "db.password", "--env", "prod", "--secret", "prodpass"])
        .assert()
        .success();

    // Write topologies
    write_file(
        &work,
        "topologies.yaml",
        "\
all-local:
  api: dev
  web: dev

all-prod:
  api: prod
  web: prod

hybrid:
  api: dev
  web: dev
  overrides:
    api:
      db.*: prod

with-path:
  api:
    env: dev
    path: services/backend
  web: dev
",
    );

    // Write manifests
    write_file(
        &work,
        "api/env.manifest.yaml",
        "\
target: \".env\"
vars:
  APP_URL: app.url
  DB_HOST: db.host
  DB_PASSWORD: db.password
",
    );

    write_file(
        &work,
        "web/env.manifest.yaml",
        "\
target: \".env.local\"
vars:
  NEXT_PUBLIC_APP_URL: app.url
",
    );

    (home, work)
}

#[test]
fn assemble_all_local() {
    let (home, work) = setup_assembly();

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote api/.env (3 lines)"))
        .stdout(predicate::str::contains("Wrote web/.env.local (1 lines)"));

    let api_env = read_file(&work, "api/.env");
    assert!(api_env.contains("APP_URL=http://localhost:3000"));
    assert!(api_env.contains("DB_HOST=localhost"));
    assert!(api_env.contains("DB_PASSWORD=devpass"));

    let web_env = read_file(&work, "web/.env.local");
    assert!(web_env.contains("NEXT_PUBLIC_APP_URL=http://localhost:3000"));
}

#[test]
fn assemble_all_prod() {
    let (home, work) = setup_assembly();

    urd(&home, &work)
        .args(["assemble", "--topology", "all-prod"])
        .assert()
        .success();

    let api_env = read_file(&work, "api/.env");
    assert!(api_env.contains("APP_URL=https://app.example.com"));
    assert!(api_env.contains("DB_HOST=db.example.com"));
    assert!(api_env.contains("DB_PASSWORD=prodpass"));
}

#[test]
fn assemble_with_overrides() {
    let (home, work) = setup_assembly();

    urd(&home, &work)
        .args(["assemble", "--topology", "hybrid"])
        .assert()
        .success();

    let api_env = read_file(&work, "api/.env");
    // app.url should be dev (not overridden)
    assert!(api_env.contains("APP_URL=http://localhost:3000"));
    // db.* should be overridden to prod
    assert!(api_env.contains("DB_HOST=db.example.com"));
    assert!(api_env.contains("DB_PASSWORD=prodpass"));

    // web has no overrides — all dev
    let web_env = read_file(&work, "web/.env.local");
    assert!(web_env.contains("NEXT_PUBLIC_APP_URL=http://localhost:3000"));
}

#[test]
fn assemble_single_component() {
    let (home, work) = setup_assembly();

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local", "--component", "web"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote web/.env.local"));

    // web file written
    assert!(work.path().join("web/.env.local").exists());
    // api file NOT written
    assert!(!work.path().join("api/.env").exists());
}

#[test]
fn assemble_with_explicit_path() {
    let (home, work) = setup_assembly();

    // Write manifest at the custom path
    write_file(
        &work,
        "services/backend/env.manifest.yaml",
        "\
target: \".env\"
vars:
  APP_URL: app.url
  DB_HOST: db.host
",
    );

    urd(&home, &work)
        .args(["assemble", "--topology", "with-path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote services/backend/.env"));

    let env = read_file(&work, "services/backend/.env");
    assert!(env.contains("APP_URL=http://localhost:3000"));
    assert!(env.contains("DB_HOST=localhost"));
}

#[test]
fn assemble_missing_topology_fails() {
    let (home, work) = setup_assembly();

    urd(&home, &work)
        .args(["assemble", "--topology", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("topology 'nonexistent' not found"));
}

#[test]
fn assemble_missing_component_fails() {
    let (home, work) = setup_assembly();

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local", "--component", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("component 'nope' not found"));
}

#[test]
fn assemble_missing_store_item_fails() {
    let (home, work) = setup_assembly();

    // Manifest referencing an item that doesn't exist
    write_file(
        &work,
        "api/env.manifest.yaml",
        "\
target: \".env\"
vars:
  MISSING: does.not.exist
",
    );

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local", "--component", "api"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does.not.exist"));
}

// -- catalog sensitivity inference on set --

#[test]
fn set_infers_encryption_from_catalog_secret() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Mark item as secret via catalog
    urd(&home, &work)
        .args(["catalog", "add", "db.password", "--sensitivity", "secret"])
        .assert()
        .success();

    // Set without --secret flag — should auto-encrypt based on catalog
    urd(&home, &work)
        .args(["set", "db.password", "--env", "dev", "hunter2"])
        .assert()
        .success();

    // Value should be redacted (encrypted)
    urd(&home, &work)
        .args(["get", "db.password", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(secret)"));

    // Reveal should show the original value
    urd(&home, &work)
        .args(["get", "db.password", "--env", "dev", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hunter2"));
}

#[test]
fn set_with_catalog_plaintext_stores_plaintext() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Mark item as plaintext via catalog
    urd(&home, &work)
        .args(["catalog", "add", "app.url", "--sensitivity", "plaintext"])
        .assert()
        .success();

    // Set without flags — should stay plaintext
    urd(&home, &work)
        .args(["set", "app.url", "--env", "dev", "http://localhost:3000"])
        .assert()
        .success();

    // Value should be shown as-is (not encrypted)
    urd(&home, &work)
        .args(["get", "app.url", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("http://localhost:3000"));
}

#[test]
fn set_explicit_flag_overrides_catalog_sensitivity() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Mark item as sensitive via catalog
    urd(&home, &work)
        .args(["catalog", "add", "api.key", "--sensitivity", "sensitive"])
        .assert()
        .success();

    // Set with explicit --secret flag — should override catalog's sensitive
    urd(&home, &work)
        .args(["set", "api.key", "--env", "dev", "--secret", "sk_abc123"])
        .assert()
        .success();

    // Should be stored as secret, not sensitive
    urd(&home, &work)
        .args(["get", "api.key", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(secret)"));

    urd(&home, &work)
        .args(["get", "api.key", "--env", "dev", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_abc123"));
}

// -- assemble with templates --

#[test]
fn assemble_template_basic() {
    let (home, work) = setup_assembly();

    // Remove manifest, add template instead
    fs::remove_file(work.path().join("api/env.manifest.yaml")).unwrap();
    write_file(
        &work,
        "api/.env.template",
        "\
NODE_ENV=dev
PORT=3000
HOST=0.0.0.0

# Database
DB_HOST={{ db.host }}
DB_PASSWORD={{ db.password }}

# App
APP_URL={{ app.url }}
",
    );

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local", "--component", "api"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote api/.env"));

    let env = read_file(&work, "api/.env");
    // Static lines preserved
    assert!(env.contains("NODE_ENV=dev"));
    assert!(env.contains("PORT=3000"));
    assert!(env.contains("HOST=0.0.0.0"));
    // Comments preserved
    assert!(env.contains("# Database"));
    assert!(env.contains("# App"));
    // Blank line preserved
    assert!(env.contains("\n\n"));
    // Expressions resolved
    assert!(env.contains("DB_HOST=localhost"));
    assert!(env.contains("DB_PASSWORD=devpass"));
    assert!(env.contains("APP_URL=http://localhost:3000"));
}

#[test]
fn assemble_template_with_frontmatter_target() {
    let (home, work) = setup_assembly();

    fs::remove_file(work.path().join("web/env.manifest.yaml")).unwrap();
    write_file(
        &work,
        "web/.env.template",
        "\
# target: .env.local
NEXT_PUBLIC_APP_URL={{ app.url }}
",
    );

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local", "--component", "web"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote web/.env.local"));

    let env = read_file(&work, "web/.env.local");
    assert!(env.contains("NEXT_PUBLIC_APP_URL=http://localhost:3000"));
    // Frontmatter line should not appear in output
    assert!(!env.contains("# target:"));
}

#[test]
fn assemble_template_with_overrides() {
    let (home, work) = setup_assembly();

    fs::remove_file(work.path().join("api/env.manifest.yaml")).unwrap();
    write_file(
        &work,
        "api/.env.template",
        "\
APP_URL={{ app.url }}
DB_HOST={{ db.host }}
DB_PASSWORD={{ db.password }}
",
    );

    urd(&home, &work)
        .args(["assemble", "--topology", "hybrid", "--component", "api"])
        .assert()
        .success();

    let env = read_file(&work, "api/.env");
    // app.url stays dev
    assert!(env.contains("APP_URL=http://localhost:3000"));
    // db.* overridden to prod
    assert!(env.contains("DB_HOST=db.example.com"));
    assert!(env.contains("DB_PASSWORD=prodpass"));
}

#[test]
fn assemble_template_missing_item_fails() {
    let (home, work) = setup_assembly();

    fs::remove_file(work.path().join("api/env.manifest.yaml")).unwrap();
    write_file(
        &work,
        "api/.env.template",
        "\
MISSING={{ does.not.exist }}
",
    );

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local", "--component", "api"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does.not.exist"));
}

#[test]
fn assemble_allow_missing_continues() {
    let (home, work) = setup_assembly();

    fs::remove_file(work.path().join("api/env.manifest.yaml")).unwrap();
    write_file(
        &work,
        "api/.env.template",
        "\
APP_URL={{ app.url }}
MISSING={{ does.not.exist }}
",
    );

    urd(&home, &work)
        .args([
            "assemble",
            "--topology",
            "all-local",
            "--component",
            "api",
            "--allow-missing",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("warning"));

    let env = read_file(&work, "api/.env");
    assert!(env.contains("APP_URL=http://localhost:3000"));
    // Missing value written as empty
    assert!(env.contains("MISSING="));
}

#[test]
fn assemble_allow_missing_with_manifest() {
    let (home, work) = setup_assembly();

    write_file(
        &work,
        "api/env.manifest.yaml",
        "\
target: \".env\"
vars:
  APP_URL: app.url
  MISSING: does.not.exist
",
    );

    urd(&home, &work)
        .args([
            "assemble",
            "--topology",
            "all-local",
            "--component",
            "api",
            "--allow-missing",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("warning"));

    let env = read_file(&work, "api/.env");
    assert!(env.contains("APP_URL=http://localhost:3000"));
    assert!(env.contains("MISSING="));
}

#[test]
fn assemble_env_template_discovery() {
    let (home, work) = setup_assembly();

    // Remove manifest, use env.template (not .env.template)
    fs::remove_file(work.path().join("web/env.manifest.yaml")).unwrap();
    write_file(
        &work,
        "web/env.template",
        "\
# target: .env.local
APP_URL={{ app.url }}
",
    );

    urd(&home, &work)
        .args(["assemble", "--topology", "all-local", "--component", "web"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote web/.env.local"));

    let env = read_file(&work, "web/.env.local");
    assert!(env.contains("APP_URL=http://localhost:3000"));
}

// -- import --

#[test]
fn import_dotenv_file() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    write_file(
        &work,
        "app.env",
        "\
# Database config
DB_HOST=localhost
DB_PORT=5432
APP_URL=\"http://localhost:3000\"
EMPTY_VAL=
",
    );

    urd(&home, &work)
        .args(["import", work.path().join("app.env").to_str().unwrap(), "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 4 items into dev"));

    urd(&home, &work)
        .args(["get", "DB_HOST", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("localhost"));

    urd(&home, &work)
        .args(["get", "DB_PORT", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("5432"));

    // Quotes should be stripped
    urd(&home, &work)
        .args(["get", "APP_URL", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("http://localhost:3000"));

    // Empty value
    urd(&home, &work)
        .args(["get", "EMPTY_VAL", "--env", "dev"])
        .assert()
        .success();
}

#[test]
fn import_yaml_file() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    write_file(
        &work,
        "config.yaml",
        "\
DATABASE_URL: postgres://localhost/mydb
API_KEY: sk_test_123
",
    );

    urd(&home, &work)
        .args(["import", work.path().join("config.yaml").to_str().unwrap(), "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 items into dev"));

    urd(&home, &work)
        .args(["get", "DATABASE_URL", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("postgres://localhost/mydb"));

    urd(&home, &work)
        .args(["get", "API_KEY", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_test_123"));
}

#[test]
fn import_from_stdin() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    urd(&home, &work)
        .args(["import", "-", "--env", "dev"])
        .write_stdin("MY_KEY=my_value\nOTHER=123\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 items into dev"));

    urd(&home, &work)
        .args(["get", "MY_KEY", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("my_value"));
}

#[test]
fn import_skip_existing() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Pre-populate a value
    urd(&home, &work)
        .args(["set", "DB_HOST", "--env", "dev", "original"])
        .assert()
        .success();

    write_file(
        &work,
        "new.env",
        "\
DB_HOST=overwritten
DB_PORT=5432
",
    );

    urd(&home, &work)
        .args([
            "import",
            work.path().join("new.env").to_str().unwrap(),
            "--env", "dev",
            "--skip-existing",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 skipped"));

    // Original value preserved
    urd(&home, &work)
        .args(["get", "DB_HOST", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("original"));

    // New value imported
    urd(&home, &work)
        .args(["get", "DB_PORT", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("5432"));
}

#[test]
fn import_secret_encrypts_all() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    write_file(
        &work,
        "secrets.env",
        "\
API_KEY=sk_live_abc
DB_PASSWORD=hunter2
",
    );

    urd(&home, &work)
        .args([
            "import",
            work.path().join("secrets.env").to_str().unwrap(),
            "--env", "prod",
            "--secret",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 items into prod"));

    // Both values should be encrypted as secret
    urd(&home, &work)
        .args(["get", "API_KEY", "--env", "prod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(secret)"));

    urd(&home, &work)
        .args(["get", "DB_PASSWORD", "--env", "prod", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hunter2"));
}

#[test]
fn import_respects_catalog_sensitivity() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Mark item as secret via catalog
    urd(&home, &work)
        .args(["catalog", "add", "DB_PASSWORD", "--sensitivity", "secret"])
        .assert()
        .success();

    write_file(
        &work,
        "vals.env",
        "\
DB_PASSWORD=hunter2
APP_PORT=3000
",
    );

    urd(&home, &work)
        .args(["import", work.path().join("vals.env").to_str().unwrap(), "--env", "dev"])
        .assert()
        .success();

    // DB_PASSWORD should be encrypted (from catalog)
    urd(&home, &work)
        .args(["get", "DB_PASSWORD", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(secret)"));

    // APP_PORT should be plaintext
    urd(&home, &work)
        .args(["get", "APP_PORT", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3000"));
}

#[test]
fn import_dry_run_does_not_write() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    write_file(
        &work,
        "app.env",
        "\
DB_HOST=localhost
DB_PORT=5432
",
    );

    urd(&home, &work)
        .args([
            "import",
            work.path().join("app.env").to_str().unwrap(),
            "--env", "dev",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry run"))
        .stdout(predicate::str::contains("DB_HOST=localhost"))
        .stdout(predicate::str::contains("DB_PORT=5432"));

    // Store should still be empty
    urd(&home, &work)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Store is empty"));
}

#[test]
fn import_dry_run_shows_skip_and_encrypt() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Pre-populate a value
    urd(&home, &work)
        .args(["set", "DB_HOST", "--env", "dev", "original"])
        .assert()
        .success();

    write_file(
        &work,
        "mixed.env",
        "\
DB_HOST=overwritten
API_KEY=secret123
",
    );

    urd(&home, &work)
        .args([
            "import",
            work.path().join("mixed.env").to_str().unwrap(),
            "--env", "dev",
            "--skip-existing",
            "--secret",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("skip"))
        .stdout(predicate::str::contains("encrypt"))
        .stdout(predicate::str::contains("dry run"));

    // Original value should be untouched
    urd(&home, &work)
        .args(["get", "DB_HOST", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("original"));

    // API_KEY should not exist (dry run)
    urd(&home, &work)
        .args(["get", "API_KEY", "--env", "dev"])
        .assert()
        .failure();
}

// -- config / default environments --

#[test]
fn config_show_empty_by_default() {
    let (home, work) = setup();

    urd(&home, &work)
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No default environments configured"));
}

#[test]
fn config_set_defaults_and_show() {
    let (home, work) = setup();

    urd(&home, &work)
        .args(["config", "set-defaults", "local", "prod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Default environments: local, prod"));

    urd(&home, &work)
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Default environments: local, prod"));
}

#[test]
fn config_set_defaults_clear() {
    let (home, work) = setup();

    urd(&home, &work)
        .args(["config", "set-defaults", "local", "prod"])
        .assert()
        .success();

    urd(&home, &work)
        .args(["config", "set-defaults"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleared default environments"));

    urd(&home, &work)
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No default environments configured"));
}

#[test]
fn new_items_inherit_default_environments() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Set defaults
    urd(&home, &work)
        .args(["config", "set-defaults", "local", "prod"])
        .assert()
        .success();

    // Set a value — should inherit default environments
    urd(&home, &work)
        .args(["set", "app.port", "--env", "local", "3000"])
        .assert()
        .success();

    // catalog show should list both local and prod as environments
    urd(&home, &work)
        .args(["catalog", "show", "app.port"])
        .assert()
        .success()
        .stdout(predicate::str::contains("environments: local, prod"));
}

#[test]
fn catalog_add_inherits_default_environments() {
    let (home, work) = setup();

    // Set defaults
    urd(&home, &work)
        .args(["config", "set-defaults", "local", "prod"])
        .assert()
        .success();

    // Add catalog entry without --env
    urd(&home, &work)
        .args(["catalog", "add", "db.host", "--description", "Database host"])
        .assert()
        .success();

    // Should have inherited default environments
    urd(&home, &work)
        .args(["catalog", "show", "db.host"])
        .assert()
        .success()
        .stdout(predicate::str::contains("environments: local, prod"));
}

#[test]
fn explicit_environments_not_overwritten_by_defaults() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Set defaults
    urd(&home, &work)
        .args(["config", "set-defaults", "local", "prod"])
        .assert()
        .success();

    // Add catalog entry with explicit --env (not matching defaults)
    urd(&home, &work)
        .args(["catalog", "add", "db.host", "--env", "staging", "--env", "prod"])
        .assert()
        .success();

    // Should keep explicit environments, not overwrite with defaults
    urd(&home, &work)
        .args(["catalog", "show", "db.host"])
        .assert()
        .success()
        .stdout(predicate::str::contains("environments: staging, prod"));
}

#[test]
fn legacy_store_format_loads_correctly() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Write a legacy bare-map format store directly
    let store_path = home.path().join("store.yaml");
    fs::write(
        &store_path,
        "\
app.port:
  description: Application port
  dev: '3000'
  prod: '8080'
",
    )
    .unwrap();

    // Should load and serve data correctly
    urd(&home, &work)
        .args(["get", "app.port", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3000"));

    // Mutate to trigger save in new format
    urd(&home, &work)
        .args(["set", "app.port", "--env", "staging", "4000"])
        .assert()
        .success();

    // Re-read — should now be in new format but still work
    urd(&home, &work)
        .args(["get", "app.port", "--env", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3000"));

    urd(&home, &work)
        .args(["get", "app.port", "--env", "staging"])
        .assert()
        .success()
        .stdout(predicate::str::contains("4000"));

    // Verify the file now has the new format with meta/items
    let contents = fs::read_to_string(&store_path).unwrap();
    assert!(contents.contains("meta:"));
    assert!(contents.contains("items:"));
}

#[test]
fn import_inherits_default_environments() {
    let (home, work) = setup();
    urd(&home, &work).args(["keys", "init"]).assert().success();

    // Set defaults
    urd(&home, &work)
        .args(["config", "set-defaults", "local", "prod"])
        .assert()
        .success();

    write_file(
        &work,
        "app.env",
        "\
DB_HOST=localhost
",
    );

    urd(&home, &work)
        .args(["import", work.path().join("app.env").to_str().unwrap(), "--env", "local"])
        .assert()
        .success();

    // Imported item should inherit default environments
    urd(&home, &work)
        .args(["catalog", "show", "DB_HOST"])
        .assert()
        .success()
        .stdout(predicate::str::contains("environments: local, prod"));
}
