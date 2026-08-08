use loom::manifest::PI_TOOL_KEY;
use loom::{
    execute_install_plan, CommandResult, CommandSpec, InstallPlan, InstallStep, SkillAgent,
    SkillDestination, SkillScope, StepAction, System,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

struct FakeSystem {
    commands: std::sync::Mutex<Vec<String>>,
}

impl System for FakeSystem {
    fn command_exists(&self, _name: &str) -> bool {
        false
    }

    fn refresh_path(&self) {}

    fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
        let display = command.display();
        self.commands.lock().unwrap().push(display.clone());
        if display.contains("herdr.dev/install") {
            Ok(CommandResult {
                success: false,
                stdout: String::new(),
                stderr: "network unavailable".into(),
            })
        } else {
            Ok(CommandResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }
}

fn step(target: &str, manager: &str, program: &str) -> InstallStep {
    InstallStep {
        target: target.into(),
        manager: manager.into(),
        action: StepAction::Command(CommandSpec::new(program, std::iter::empty::<String>())),
        verification: None,
    }
}

struct OverlapSystem {
    active: Mutex<usize>,
    both_started: Condvar,
    overlapped: AtomicBool,
}

impl OverlapSystem {
    fn new() -> Self {
        Self {
            active: Mutex::new(0),
            both_started: Condvar::new(),
            overlapped: AtomicBool::new(false),
        }
    }
}

impl System for OverlapSystem {
    fn command_exists(&self, _name: &str) -> bool {
        true
    }

    fn refresh_path(&self) {}

    fn run(&self, _command: &CommandSpec) -> anyhow::Result<CommandResult> {
        let mut active = self.active.lock().unwrap();
        *active += 1;
        if *active == 2 {
            self.overlapped.store(true, Ordering::SeqCst);
            self.both_started.notify_all();
        } else {
            let (guard, _) = self
                .both_started
                .wait_timeout_while(active, Duration::from_secs(1), |_| {
                    !self.overlapped.load(Ordering::SeqCst)
                })
                .unwrap();
            active = guard;
        }
        *active -= 1;
        drop(active);
        Ok(CommandResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

#[test]
fn independent_manager_lanes_start_before_either_finishes() {
    let plan = InstallPlan {
        prerequisites: vec![
            step("MISE", "mise", "install-mise"),
            step("HERDR", "herdr", "install-herdr"),
        ],
        resources: Vec::new(),
    };
    let system = OverlapSystem::new();

    let report = execute_install_plan(&plan, &system);

    assert!(report.failures.is_empty());
    assert!(
        system.overlapped.load(Ordering::SeqCst),
        "independent manager lanes should overlap"
    );
}

struct MisePiSystem {
    home: PathBuf,
    pi_ready: AtomicBool,
    pi_started_early: AtomicBool,
}

impl MisePiSystem {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let home =
            std::env::temp_dir().join(format!("loom-mise-pi-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&home).unwrap();
        Self {
            home,
            pi_ready: AtomicBool::new(false),
            pi_started_early: AtomicBool::new(false),
        }
    }
}

impl Drop for MisePiSystem {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.home).ok();
    }
}

impl System for MisePiSystem {
    fn command_exists(&self, _name: &str) -> bool {
        true
    }

    fn refresh_path(&self) {}

    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }

    fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
        match command.program.as_str() {
            "curl" => {
                let output = command
                    .args
                    .iter()
                    .skip_while(|arg| arg.as_str() != "-o")
                    .nth(1)
                    .unwrap();
                fs::write(output, b"tarball")?;
            }
            "tar" => {
                let manifest = self
                    .home
                    .join(".cache/loom/manifest-staging/loom-main/manifest/loom.toml");
                fs::create_dir_all(manifest.parent().unwrap())?;
                fs::write(
                    manifest,
                    format!(
                        "[tools]\n# core:begin\nnode = \"24.19.0\"\n# core:end\n\
                         \"{PI_TOOL_KEY}\" = \"0.73.1\"\n"
                    ),
                )?;
            }
            "mise" if command.args.first().map(String::as_str) == Some("install") => {
                std::thread::sleep(Duration::from_millis(100));
                self.pi_ready.store(true, Ordering::SeqCst);
            }
            "mise" => {}
            "pi" => {
                if !self.pi_ready.load(Ordering::SeqCst) {
                    self.pi_started_early.store(true, Ordering::SeqCst);
                }
            }
            other => panic!("unexpected command: {other}"),
        }
        Ok(CommandResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

#[test]
fn pi_packages_wait_when_mise_is_installing_pi() {
    let plan = InstallPlan {
        prerequisites: vec![InstallStep {
            target: "tools".into(),
            manager: "mise".into(),
            action: StepAction::SyncTools {
                tools: vec![PI_TOOL_KEY.into()],
            },
            verification: None,
        }],
        resources: vec![step("pi-package:sample", "pi", "pi")],
    };
    let system = MisePiSystem::new();

    let report = execute_install_plan(&plan, &system);

    assert!(report.failures.is_empty());
    assert_eq!(report.installed, vec!["pi-package:sample"]);
    assert!(
        !system.pi_started_early.load(Ordering::SeqCst),
        "Pi packages must wait until mise has installed Pi"
    );
}

#[test]
fn failed_prerequisite_skips_only_resources_that_need_that_manager() {
    let plan = InstallPlan {
        prerequisites: vec![InstallStep {
            target: "HERDR".into(),
            manager: "herdr".into(),
            action: StepAction::Command(CommandSpec::new(
                "sh",
                ["-c", "curl -fsSL https://herdr.dev/install.sh | sh"],
            )),
            verification: None,
        }],
        resources: vec![
            step("herdr-plugin:jumplist", "herdr", "herdr"),
            step("pi-package:sample", "pi", "pi"),
        ],
    };
    let system = FakeSystem {
        commands: std::sync::Mutex::new(Vec::new()),
    };

    let report = execute_install_plan(&plan, &system);

    assert_eq!(report.installed, vec!["pi-package:sample"]);
    assert_eq!(report.failures.len(), 2);
    assert_eq!(report.failures[0].target, "HERDR");
    assert_eq!(report.failures[0].message, "network unavailable");
    assert_eq!(report.failures[1].target, "herdr-plugin:jumplist");
    assert_eq!(report.failures[1].message, "HERDR is unavailable");
    let mut commands = system.commands.into_inner().unwrap();
    commands.sort();
    assert_eq!(
        commands,
        vec!["pi", "sh -c curl -fsSL https://herdr.dev/install.sh | sh"]
    );
}

struct HiddenCommandSystem;

impl System for HiddenCommandSystem {
    fn command_exists(&self, _name: &str) -> bool {
        false
    }

    fn refresh_path(&self) {}

    fn run(&self, _command: &CommandSpec) -> anyhow::Result<CommandResult> {
        Ok(CommandResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

#[test]
fn successful_bootstrap_must_make_its_manager_available() {
    let plan = InstallPlan {
        prerequisites: vec![InstallStep {
            target: "HERDR".into(),
            manager: "herdr".into(),
            action: StepAction::Command(CommandSpec::new("sh", ["-c", "install herdr"])),
            verification: None,
        }],
        resources: vec![step("herdr-plugin:jumplist", "herdr", "herdr")],
    };

    let report = execute_install_plan(&plan, &HiddenCommandSystem);

    assert!(report.installed.is_empty());
    assert_eq!(report.failures[0].target, "HERDR");
    assert_eq!(
        report.failures[0].message,
        "installer completed, but herdr is still unavailable on PATH"
    );
    assert_eq!(report.failures[1].message, "HERDR is unavailable");
}

/// Fakes curl and tar with filesystem side effects, so the native skill
/// installer runs end to end against a temp home.
struct SkillInstallSystem {
    home: PathBuf,
    /// The skills the "downloaded" repo contains.
    repo_skills: Vec<String>,
}

impl SkillInstallSystem {
    fn new(label: &str, repo_skills: &[&str]) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let home =
            std::env::temp_dir().join(format!("loom-exec-{label}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(home.join(".claude")).expect("create temp home");
        fs::create_dir_all(home.join(".config").join("opencode")).expect("create OpenCode config");
        Self {
            home,
            repo_skills: repo_skills.iter().map(ToString::to_string).collect(),
        }
    }

    fn staging(&self) -> PathBuf {
        self.home.join(".cache").join("loom").join("staging")
    }

    fn tree(&self) -> PathBuf {
        self.home.join(".claude").join("skills")
    }

    fn opencode_tree(&self) -> PathBuf {
        self.home.join(".config").join("opencode").join("skills")
    }
}

impl Drop for SkillInstallSystem {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.home).ok();
    }
}

impl System for SkillInstallSystem {
    fn command_exists(&self, _name: &str) -> bool {
        true
    }

    fn refresh_path(&self) {}

    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }

    fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
        match command.program.as_str() {
            "curl" => {
                let output = command
                    .args
                    .iter()
                    .skip_while(|arg| arg.as_str() != "-o")
                    .nth(1)
                    .expect("curl is invoked with -o <path>");
                fs::write(Path::new(output), b"tarball")?;
            }
            "tar" => {
                let repo = self.staging().join("loom-main");
                for name in &self.repo_skills {
                    let skill = repo.join("skills").join(name);
                    fs::create_dir_all(&skill)?;
                    fs::write(skill.join("SKILL.md"), format!("# {name}\n"))?;
                }
                let adapter = repo
                    .join("manifest")
                    .join("opencode")
                    .join("plugins")
                    .join("loom-session-env.js");
                fs::create_dir_all(adapter.parent().expect("adapter parent"))?;
                fs::write(&adapter, "export const LoomSessionEnv = true;\n")?;
            }
            other => panic!("unexpected command: {other}"),
        }
        Ok(CommandResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

fn copy_skills_plan(skills: &[&str], system: &SkillInstallSystem) -> InstallPlan {
    copy_skills_plan_for(
        skills,
        SkillDestination::new(
            vec![SkillAgent::Claude, SkillAgent::OpenCode],
            SkillScope::Global,
            &system.home,
            &system.home,
        ),
    )
}

fn copy_skills_plan_for(skills: &[&str], destination: SkillDestination) -> InstallPlan {
    InstallPlan {
        prerequisites: Vec::new(),
        resources: vec![InstallStep {
            target: "skills".into(),
            manager: "skills".into(),
            action: StepAction::CopySkills {
                skills: skills.iter().map(ToString::to_string).collect(),
                destination,
            },
            verification: None,
        }],
    }
}

#[test]
fn project_install_creates_only_selected_agent_targets() {
    // Capability/seam: scoped native skill installation. This fails if a
    // project choice leaks into global trees or unselected agents.
    let system = SkillInstallSystem::new("project", &["tdd"]);
    let project = system.home.join("project");
    fs::create_dir_all(project.join(".git")).unwrap();
    let destination = SkillDestination::new(
        vec![SkillAgent::Claude, SkillAgent::OpenCode],
        SkillScope::Project,
        &system.home,
        &project,
    );

    let report = execute_install_plan(&copy_skills_plan_for(&["tdd"], destination), &system);

    assert!(report.failures.is_empty());
    assert!(project.join(".claude/skills/tdd/SKILL.md").is_file());
    assert!(project.join(".opencode/skills/tdd/SKILL.md").is_file());
    assert!(project
        .join(".opencode/plugins/loom-session-env.js")
        .is_file());
    assert!(!project.join(".pi/skills/tdd/SKILL.md").exists());
    assert!(!system.tree().join("tdd/SKILL.md").exists());
}

#[test]
fn skills_are_copied_into_every_detected_tree() {
    // Capability/seam: native skill installation. This fails if an
    // OpenCode-only destination or its session adapter is skipped. No expiry.
    let system = SkillInstallSystem::new("copy", &["tdd", "commit"]);

    let report = execute_install_plan(&copy_skills_plan(&["tdd", "commit"], &system), &system);

    assert_eq!(report.installed, vec!["skills"]);
    assert!(report.failures.is_empty());
    assert!(system.tree().join("tdd").join("SKILL.md").is_file());
    assert!(system.tree().join("commit").join("SKILL.md").is_file());
    assert!(system
        .opencode_tree()
        .join("tdd")
        .join("SKILL.md")
        .is_file());
    assert!(system
        .home
        .join(".config/opencode/plugins/loom-session-env.js")
        .is_file());
    assert!(!system.staging().exists(), "staging should be cleaned up");
}

#[test]
fn a_skill_missing_from_the_downloaded_repo_fails_the_step() {
    let system = SkillInstallSystem::new("missing", &["commit"]);

    let report = execute_install_plan(&copy_skills_plan(&["tdd"], &system), &system);

    assert!(report.installed.is_empty());
    assert_eq!(report.failures[0].target, "skills");
    assert_eq!(
        report.failures[0].message,
        "downloaded repo is missing skills: tdd"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_skills_survive_an_install_untouched() {
    let system = SkillInstallSystem::new("symlink", &["tdd"]);
    let checkout = system.home.join("checkout").join("tdd");
    fs::create_dir_all(&checkout).unwrap();
    fs::write(checkout.join("SKILL.md"), "# local checkout\n").unwrap();
    fs::create_dir_all(system.tree()).unwrap();
    std::os::unix::fs::symlink(&checkout, system.tree().join("tdd")).unwrap();

    let report = execute_install_plan(&copy_skills_plan(&["tdd"], &system), &system);

    assert_eq!(report.installed, vec!["skills"]);
    let target = system.tree().join("tdd");
    assert!(target.symlink_metadata().unwrap().is_symlink());
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "# local checkout\n"
    );
}

#[test]
fn parallel_managers_still_report_in_plan_order() {
    let plan = InstallPlan {
        prerequisites: Vec::new(),
        resources: vec![
            step("pi-package:one", "pi", "pi"),
            step("herdr-plugin:two", "herdr", "herdr"),
            step("pi-package:three", "pi", "pi"),
        ],
    };

    let report = execute_install_plan(&plan, &HiddenCommandSystem);

    assert!(report.failures.is_empty());
    assert_eq!(
        report.installed,
        vec!["pi-package:one", "herdr-plugin:two", "pi-package:three"],
        "report order must follow the plan, not thread completion"
    );
}
