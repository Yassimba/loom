use loom::testing::ScriptedSystem;
use loom::wiki::{run_wiki, WikiOperation, WikiRequest};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str, vault_exists: bool) -> (PathBuf, PathBuf, ScriptedSystem) {
    let home =
        std::env::temp_dir().join(format!("loom-wiki-workflow-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    let vault = home.join("vault");
    if vault_exists {
        fs::create_dir_all(&vault).unwrap();
    }
    let registry = home.join(".config/loom/wiki-vaults.json");
    fs::create_dir_all(registry.parent().unwrap()).unwrap();
    fs::write(
        registry,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "vaults": [{"path": vault, "feynman": false}]
        }))
        .unwrap(),
    )
    .unwrap();
    let system = ScriptedSystem::new().home(&home).cwd(&home);
    (home, vault, system)
}

fn request(operation: WikiOperation, vault: impl AsRef<Path>) -> WikiRequest {
    WikiRequest {
        operation,
        vault: vault.as_ref().to_path_buf(),
        feynman: false,
        confluence: false,
        qmd: false,
        yes: true,
    }
}

#[test]
fn unregister_removes_only_machine_state_and_preserves_vault_files() {
    let (home, vault, system) = fixture("unregister", true);
    fs::write(vault.join("knowledge.md"), "keep").unwrap();

    assert!(run_wiki(&request(WikiOperation::Unregister, "vault"), &system).unwrap());
    assert_eq!(
        fs::read_to_string(vault.join("knowledge.md")).unwrap(),
        "keep"
    );
    let registry = fs::read_to_string(home.join(".config/loom/wiki-vaults.json")).unwrap();
    assert!(!registry.contains(&vault.display().to_string()));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn unregister_rejects_a_path_that_is_not_registered() {
    let (home, vault, system) = fixture("unregister-missing", true);
    let err = run_wiki(
        &request(WikiOperation::Unregister, "missing-vault"),
        &system,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("not registered"), "{err}");
    let registry: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.join(".config/loom/wiki-vaults.json")).unwrap(),
    )
    .unwrap();
    let paths: Vec<PathBuf> = registry["vaults"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| PathBuf::from(entry["path"].as_str().unwrap()))
        .collect();
    assert!(paths.contains(&vault));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn launch_uses_the_registered_vault_as_pi_working_directory() {
    let (home, vault, system) = fixture("launch", true);

    let result = run_wiki(&request(WikiOperation::Launch, &vault), &system);
    if cfg!(windows) {
        assert!(result.unwrap_err().to_string().contains("WSL2"));
        assert!(system.calls().is_empty());
        fs::remove_dir_all(home).unwrap();
        return;
    }
    assert!(result.unwrap());
    let commands = system.calls();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].program, "pi");
    assert_eq!(commands[0].cwd.as_deref(), Some(vault.as_path()));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn opening_is_explicit_and_uses_an_obsidian_uri_for_the_registered_vault() {
    let (home, vault, system) = fixture("open", true);

    assert!(system.calls().is_empty());
    assert!(run_wiki(&request(WikiOperation::Open, &vault), &system).unwrap());
    let commands = system.calls();
    assert_eq!(commands.len(), 1);
    assert!(commands[0]
        .args
        .iter()
        .any(|arg| arg.starts_with("obsidian://open?path=")));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn missing_registered_vault_is_reported_without_recreation() {
    let (home, vault, system) = fixture("missing", false);

    assert!(!run_wiki(&request(WikiOperation::Status, &vault), &system).unwrap());
    assert!(!vault.exists());
    fs::remove_dir_all(home).unwrap();
}
