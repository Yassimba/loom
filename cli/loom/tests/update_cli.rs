mod common;

use loom::{digest_path, InstallState, OwnedPathKind, OwnedResource, OwnershipScope, Receipt};
use std::collections::BTreeMap;
use std::process::{Command, Stdio};

#[test]
fn update_preserves_a_catalog_named_skill_without_an_ownership_receipt() {
    let home = common::temp_home("update-cli-unowned-skill");
    let project = home.join("project");
    let skill = home.join(".agents/skills/tdd");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "# custom unowned tdd\n").unwrap();
    let repo = common::repo_root();

    let output = Command::new(env!("CARGO_BIN_EXE_loom"))
        .args(["update", "--yes"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("LOOM_REPO_DIR", repo)
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .unwrap();

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

    let output = Command::new(env!("CARGO_BIN_EXE_loom"))
        .args(["update", "--yes"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("LOOM_REPO_DIR", common::repo_root())
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(target.join("SKILL.md").is_file());
    assert!(!backup.exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 refreshed"));
    let state = InstallState::load(&home).unwrap();
    let refreshed = digest_path(&target).unwrap();
    assert_eq!(state.owned_path_digest(&target), Some(refreshed.as_str()));
    std::fs::remove_dir_all(home).unwrap();
}
