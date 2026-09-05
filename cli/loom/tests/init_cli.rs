mod common;

use loom::InstallState;
use std::process::{Command, Stdio};

#[test]
fn diagram_override_switches_and_default_init_preserves_it() {
    let home = common::temp_home("init-diagrams");
    let project = home.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_loom"))
            .args(args)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("LOOM_REPO_DIR", common::repo_root())
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&project)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let path = project.join(loom::diagrams::PROJECT_PATH);
    for style in ["economical", "polished", "inherit"] {
        let output = run(&["init", "--diagrams", style]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["style"], style);
        assert!(
            !project.join("AGENTS.md").exists(),
            "a diagram flag must not select other setup"
        );
    }
    std::fs::write(&path, "{\"style\":\"economical\",\"keep\":42}\n").unwrap();
    let before = std::fs::read(&path).unwrap();
    let output = run(&["init", "--yes", "--no-codegraph"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!home.join(".config/loom/diagrams.json").exists());
    assert!(run(&["init", "--diagrams", "polished"]).status.success());
    assert!(!run(&["init", "--diagrams", "unknown"]).status.success());
    let state = InstallState::load(&home).unwrap();
    assert!(state
        .resources
        .keys()
        .any(|id| id.starts_with("project:") && id.ends_with(":init")));
    std::fs::remove_dir_all(home).unwrap();
}

#[cfg(unix)]
#[test]
fn late_init_failure_still_records_completed_project_writes() {
    use std::os::unix::fs::PermissionsExt;

    let home = common::temp_home("init-cli-late-failure");
    let project = home.join("project");
    let bin = home.join("bin");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let br = bin.join("br");
    let bv = bin.join("bv");
    std::fs::write(&br, "#!/bin/sh\necho e2e-br-failure >&2\nexit 19\n").unwrap();
    std::fs::write(&bv, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&br, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&bv, std::fs::Permissions::from_mode(0o755)).unwrap();
    let repo = common::repo_root();

    let output = Command::new(env!("CARGO_BIN_EXE_loom"))
        .args([
            "init",
            "--tracker",
            "beads",
            "--domain",
            "single",
            "--editor",
            "cursor",
            "--coding-standards",
            "--no-codegraph",
            "--yes",
        ])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("LOOM_REPO_DIR", repo)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        project.join("AGENTS.md").is_file(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let state = InstallState::load(&home).unwrap();
    assert!(state
        .resources
        .keys()
        .any(|id| id.starts_with("project:") && id.ends_with(":init")));
    std::fs::remove_dir_all(home).unwrap();
}
