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
        .stdout(predicate::str::contains("Wrote api/.env (3 vars)"))
        .stdout(predicate::str::contains("Wrote web/.env.local (1 vars)"));

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
