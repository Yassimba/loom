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
            .env("USERPROFILE", &home)
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

#[test]
fn bundled_skills_command_lists_enabled_packages_and_keeps_reconcile_stdout_empty() {
    let home = common::temp_home("bundled-cmd");
    let pkg = home.join(".pi/agent/npm/node_modules/@dietrichgebert/ponytail");
    std::fs::create_dir_all(pkg.join("skills/ponytail")).unwrap();
    std::fs::write(
        pkg.join("package.json"),
        r#"{"pi":{"skills":["./skills"]}}"#,
    )
    .unwrap();
    std::fs::write(pkg.join("skills/ponytail/SKILL.md"), "# bundled\n").unwrap();
    std::fs::write(
        home.join(".pi/agent/settings.json"),
        r#"{"packages":["npm:@dietrichgebert/ponytail@4.9.0"]}"#,
    )
    .unwrap();
    let shared = home.join(".agents/skills/ponytail");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("SKILL.md"), "# shared\n").unwrap();

    let loom = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_loom"));
        command
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .stdin(Stdio::null());
        command
    };
    let listed = loom().arg("bundled-skills").output().unwrap();
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&listed.stdout).trim(), "ponytail");

    let reconciled = loom()
        .args(["bundled-skills", "--reconcile"])
        .output()
        .unwrap();
    assert!(
        reconciled.status.success(),
        "{}",
        String::from_utf8_lossy(&reconciled.stderr)
    );
    assert!(String::from_utf8_lossy(&reconciled.stdout)
        .trim()
        .is_empty());
    assert!(
        std::fs::read_to_string(home.join(".pi/agent/settings.json"))
            .unwrap()
            .contains(".agents/skills/ponytail/SKILL.md")
    );
    std::fs::remove_dir_all(home).unwrap();
}
