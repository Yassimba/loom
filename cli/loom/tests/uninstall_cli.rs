use loom::{InstallState, OwnedResource, OwnershipScope, Receipt};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn temp_home(label: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!(
        "loom-uninstall-cli-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&home).unwrap();
    home
}

fn write_owned_skill(home: &std::path::Path) -> InstallState {
    let mut state = InstallState {
        schema_version: 1,
        resources: BTreeMap::new(),
    };
    state.record(OwnedResource {
        id: "skill:tdd".into(),
        scope: OwnershipScope::Global,
        depends_on: Vec::new(),
        receipts: vec![Receipt::Manager {
            manager: "never-run".into(),
            target: "tdd".into(),
        }],
    });
    state.save(home).unwrap();
    state
}

fn loom(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_loom"));
    command
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("MISE_CONFIG_DIR", home.join(".config/mise"))
        .current_dir(home)
        .stdin(Stdio::null());
    command
}

#[test]
fn dry_run_keeps_the_ledger_unchanged() {
    let home = temp_home("dry-run");
    let before = write_owned_skill(&home);

    let output = loom(&home)
        .args(["uninstall", "--skill", "tdd", "--dry-run"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Dry run; no changes made"));
    assert_eq!(InstallState::load(&home).unwrap(), before);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn noninteractive_removal_requires_yes() {
    let home = temp_home("needs-yes");
    write_owned_skill(&home);

    let output = loom(&home).args(["uninstall", "--all"]).output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("needs --yes"));
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn all_conflicts_with_named_selectors() {
    let home = temp_home("conflict");
    let output = loom(&home)
        .args(["uninstall", "--all", "--skill", "tdd", "--yes"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    std::fs::remove_dir_all(home).unwrap();
}
