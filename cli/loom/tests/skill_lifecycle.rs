mod common;

use loom::InstallState;
use std::process::{Command, Stdio};

#[test]
fn setup_and_uninstall_never_claim_a_preexisting_copy_in_another_agent_tree() {
    let home = common::temp_home("skill-lifecycle");
    let project = home.join("project");
    let existing = home.join(".agents/skills/tdd");
    let installed = home.join(".codex/skills/tdd");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::write(existing.join("SKILL.md"), "# custom preexisting tdd\n").unwrap();
    let repo = common::repo_root();
    let loom = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_loom"));
        command
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("LOOM_REPO_DIR", &repo)
            .current_dir(&project)
            .stdin(Stdio::null());
        command
    };

    let setup = loom()
        .args([
            "setup", "--skill", "tdd", "--agent", "agents", "--agent", "codex", "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        setup.status.success(),
        "{}",
        String::from_utf8_lossy(&setup.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(existing.join("SKILL.md")).unwrap(),
        "# custom preexisting tdd\n"
    );
    assert!(installed.join("SKILL.md").is_file());
    let state = InstallState::load(&home).unwrap();
    let receipts = &state.resources["skill:tdd"].receipts;
    assert_eq!(receipts.len(), 1);

    let uninstall = loom()
        .args(["uninstall", "--skill", "tdd", "--yes"])
        .output()
        .unwrap();
    assert!(
        uninstall.status.success(),
        "{}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    assert!(existing.join("SKILL.md").is_file());
    assert!(!installed.exists());
    std::fs::remove_dir_all(home).unwrap();
}
