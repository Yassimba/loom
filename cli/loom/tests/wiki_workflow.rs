use anyhow::Result;
use loom::wiki::{run_wiki, WikiOperation, WikiRequest};
use loom::{CommandResult, CommandSpec, System};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

struct FakeSystem {
    home: PathBuf,
    commands: Mutex<Vec<CommandSpec>>,
}

impl System for FakeSystem {
    fn command_exists(&self, _name: &str) -> bool {
        true
    }

    fn refresh_path(&self) {}

    fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
        self.commands.lock().unwrap().push(command.clone());
        Ok(CommandResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }

    fn current_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }
}

fn fixture(name: &str, vault_exists: bool) -> (PathBuf, PathBuf, FakeSystem) {
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
    let system = FakeSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };
    (home, vault, system)
}

fn request(operation: WikiOperation, vault: impl AsRef<Path>) -> WikiRequest {
    WikiRequest {
        operation,
        vault: vault.as_ref().to_path_buf(),
        feynman: false,
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
fn launch_uses_the_registered_vault_as_pi_working_directory() {
    let (home, vault, system) = fixture("launch", true);

    assert!(run_wiki(&request(WikiOperation::Launch, &vault), &system).unwrap());
    let commands = system.commands.into_inner().unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].program, "pi");
    assert_eq!(commands[0].cwd.as_deref(), Some(vault.as_path()));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn opening_is_explicit_and_uses_an_obsidian_uri_for_the_registered_vault() {
    let (home, vault, system) = fixture("open", true);

    assert!(system.commands.lock().unwrap().is_empty());
    assert!(run_wiki(&request(WikiOperation::Open, &vault), &system).unwrap());
    let commands = system.commands.into_inner().unwrap();
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
