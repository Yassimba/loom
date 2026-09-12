mod common;

use loom::{digest_path, InstallState, OwnedPathKind, OwnedResource, OwnershipScope, Receipt};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};

/// Manager discovery is confined to this fixture, including Windows PATH refresh.
fn loom(home: &Path, project: &Path) -> Command {
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    if cfg!(windows) {
        // A real PowerShell would reintroduce the user's registry PATH.
        for name in ["powershell", "npm"] {
            std::fs::write(bin.join(format!("{name}.cmd")), "@exit /b 0\r\n").unwrap();
        }
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_loom"));
    if cfg!(windows) {
        command.env("PATHEXT", ".CMD");
    }
    command
        .args(["update", "--yes"])
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("MISE_CONFIG_DIR", home.join(".config/mise"))
        .env("PATH", bin)
        .env("LOOM_REPO_DIR", common::repo_root())
        .current_dir(project)
        .stdin(Stdio::null());
    command
}

#[test]
fn update_preserves_a_catalog_named_skill_without_an_ownership_receipt() {
    let home = common::temp_home("update-cli-unowned-skill");
    let project = home.join("project");
    let skill = home.join(".agents/skills/tdd");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "# custom unowned tdd\n").unwrap();

    let output = loom(&home, &project).output().unwrap();

    assert_eq!(
        std::fs::read_to_string(skill.join("SKILL.md")).unwrap(),
        "# custom unowned tdd\n"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("unowned or modified, preserved"));
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn update_recovers_an_owned_skill_interrupted_between_renames() {
    let home = common::temp_home("update-cli-interrupted-skill");
    let project = home.join("project");
    let target = home.join(".agents/skills/tdd");
    let backup = home.join(".agents/skills/.tdd.loom-old");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("SKILL.md"), "# old owned tdd\n").unwrap();
    let digest = digest_path(&target).unwrap();
    std::fs::rename(&target, &backup).unwrap();
    let mut state = InstallState {
        schema_version: 1,
        resources: BTreeMap::new(),
    };
    state.record(OwnedResource {
        id: "skill:tdd".into(),
        scope: OwnershipScope::Global,
        depends_on: vec!["core:loom".into()],
        receipts: vec![Receipt::Path {
            path: target.clone(),
            path_kind: OwnedPathKind::Tree,
            digest,
            before: None,
        }],
    });
    state.save(&home).unwrap();

    let output = loom(&home, &project).output().unwrap();

    assert!(target.join("SKILL.md").is_file());
    assert!(!backup.exists());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 updated"));
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.contains("Update complete") || line.contains("Update incomplete"))
            .count(),
        1,
        "{stdout}"
    );
    assert!(!stdout.contains("Up to date"), "{stdout}");
    let state = InstallState::load(&home).unwrap();
    let refreshed = digest_path(&target).unwrap();
    assert_eq!(state.owned_path_digest(&target), Some(refreshed.as_str()));
    std::fs::remove_dir_all(home).unwrap();
}
