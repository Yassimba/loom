use loom::manifest::PI_TOOL_KEY;
use loom::{
    execute_install_plan, execute_install_plan_with, execute_install_plan_with_control,
    CommandResult, CommandSpec, InstallPlan, InstallStep, SkillAgent, SkillDestination, SkillScope,
    StepAction, StepStatus, System,
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
            step("mise", "mise", "install-mise"),
            step("Herdr", "herdr", "install-herdr"),
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

#[test]
fn cancellation_skips_resources_before_launch() {
    let plan = InstallPlan {
        prerequisites: Vec::new(),
        resources: vec![
            step("pi-package:one", "pi", "install-one"),
            step("pi-package:two", "pi", "install-two"),
        ],
    };
    let system = FakeSystem {
        commands: Mutex::new(Vec::new()),
    };
    let cancelled = AtomicBool::new(true);
    let mut statuses = Vec::new();

    let report =
        execute_install_plan_with_control(&plan, &system, &cancelled, &mut |index, status| {
            statuses.push((index, status))
        });

    assert!(system.commands.lock().unwrap().is_empty());
    assert_eq!(
        statuses,
        [
            (0, StepStatus::Skipped("cancelled".into())),
            (1, StepStatus::Skipped("cancelled".into())),
        ]
    );
    assert_eq!(report.failures.len(), 2);
}

#[test]
fn resources_in_the_same_manager_lane_never_overlap() {
    let plan = InstallPlan {
        prerequisites: Vec::new(),
        resources: vec![
            step("pi-package:one", "pi", "install-one"),
            step("pi-package:two", "pi", "install-two"),
        ],
    };
    let system = OverlapSystem::new();

    let report = execute_install_plan(&plan, &system);

    assert!(report.failures.is_empty());
    assert!(
        !system.overlapped.load(Ordering::SeqCst),
        "resources sharing a manager must not race their registry"
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
                let manifest = PathBuf::from(&command.args[3]).join("loom-main/manifest/loom.toml");
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
            target: "Herdr".into(),
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
    assert_eq!(report.failures[0].target, "Herdr");
    assert_eq!(report.failures[0].message, "network unavailable");
    assert_eq!(report.failures[1].target, "herdr-plugin:jumplist");
    assert_eq!(report.failures[1].message, "Herdr is unavailable");
    let mut commands = system.commands.into_inner().unwrap();
    commands.sort();
    assert_eq!(
        commands,
        vec!["pi", "sh -c curl -fsSL https://herdr.dev/install.sh | sh"]
    );
}

#[test]
fn failed_prerequisite_skips_later_prerequisites_in_its_lane() {
    let plan = InstallPlan {
        prerequisites: vec![
            InstallStep {
                target: "Herdr".into(),
                manager: "herdr".into(),
                action: StepAction::Command(CommandSpec::new(
                    "sh",
                    ["-c", "curl -fsSL https://herdr.dev/install.sh | sh"],
                )),
                verification: None,
            },
            step("prepare-herdr", "herdr", "prepare-herdr"),
        ],
        resources: vec![step("herdr-plugin:jumplist", "herdr", "herdr")],
    };
    let system = FakeSystem {
        commands: std::sync::Mutex::new(Vec::new()),
    };
    let mut statuses = Vec::new();

    let report = execute_install_plan_with(&plan, &system, &mut |index, status| {
        statuses.push((index, status));
    });

    assert_eq!(
        report
            .failures
            .iter()
            .map(|failure| failure.target.as_str())
            .collect::<Vec<_>>(),
        vec!["Herdr", "prepare-herdr", "herdr-plugin:jumplist"]
    );
    assert!(statuses.contains(&(1, StepStatus::Skipped("Herdr is unavailable".into()))));
    assert_eq!(system.commands.into_inner().unwrap().len(), 1);
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
            target: "Herdr".into(),
            manager: "herdr".into(),
            action: StepAction::Command(CommandSpec::new("sh", ["-c", "install herdr"])),
            verification: None,
        }],
        resources: vec![step("herdr-plugin:jumplist", "herdr", "herdr")],
    };

    let report = execute_install_plan(&plan, &HiddenCommandSystem);

    assert!(report.installed.is_empty());
    assert_eq!(report.failures[0].target, "Herdr");
    assert_eq!(
        report.failures[0].message,
        "installer completed, but herdr is still unavailable on PATH"
    );
    assert_eq!(report.failures[1].message, "Herdr is unavailable");
}

/// Fakes curl and tar with filesystem side effects, so the native skill
/// installer runs end to end against a temp home.
struct SkillInstallSystem {
    home: PathBuf,
    commands: Mutex<Vec<CommandSpec>>,
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
            commands: Mutex::new(Vec::new()),
            repo_skills: repo_skills.iter().map(ToString::to_string).collect(),
        }
    }

    fn staging(&self) -> PathBuf {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .find(|command| command.program == "tar")
            .map(|command| PathBuf::from(&command.args[3]))
            .expect("repository was extracted")
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
        self.commands.lock().unwrap().push(command.clone());
        match command.program.as_str() {
            "mise" => {}
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
                let repo = PathBuf::from(&command.args[3]).join("loom-main");
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
                fs::write(
                    repo.join("manifest/loom.toml"),
                    "[tools]\n# core:begin\nnode = \"24.19.0\"\n# core:end\ngh = \"2.97.0\"\n",
                )?;
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

#[test]
fn tools_and_skills_share_one_download_and_cleanup() {
    let system = SkillInstallSystem::new("shared-repository", &["tdd"]);
    let mut plan = copy_skills_plan(&["tdd"], &system);
    plan.prerequisites.push(InstallStep {
        target: "tools".into(),
        manager: "mise".into(),
        action: StepAction::SyncTools {
            tools: vec!["gh".into()],
        },
        verification: None,
    });

    let report = execute_install_plan(&plan, &system);

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(system.tree().join("tdd/SKILL.md").is_file());
    let selection = fs::read_to_string(system.home.join(".config/mise/conf.d/loom.toml")).unwrap();
    assert!(selection.contains("gh = \"2.97.0\""));
    let commands = system.commands.lock().unwrap();
    assert_eq!(
        commands
            .iter()
            .filter(|command| command.program == "curl")
            .count(),
        1
    );
    assert_eq!(
        commands
            .iter()
            .filter(|command| command.program == "tar")
            .count(),
        1
    );
    drop(commands);
    assert!(!system.staging().exists());
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

fn install_ponytail_package(home: &Path, settings: &str) -> PathBuf {
    let root = home.join(".pi/agent/npm/node_modules/@dietrichgebert/ponytail");
    fs::create_dir_all(root.join("skills/ponytail")).unwrap();
    fs::write(
        root.join("skills/ponytail/SKILL.md"),
        "---\nname: ponytail\ndescription: Bundled skill.\n---\n# bundled\n",
    )
    .unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"name":"@dietrichgebert/ponytail","version":"4.9.0","pi":{"skills":["./skills"]}}"#,
    )
    .unwrap();
    fs::write(home.join(".pi/agent/settings.json"), settings).unwrap();
    root
}

const PONYTAIL_SETTINGS: &str = r#"{"packages":["npm:@dietrichgebert/ponytail@4.9.0"]}"#;

#[test]
fn installed_bundle_skips_only_pi_and_preserves_skill_only_and_filtered_installs() {
    for settings in [
        Some(PONYTAIL_SETTINGS),
        None,
        Some(r#"{"packages":[{"source":"npm:@dietrichgebert/ponytail","skills":[]}]}"#),
    ] {
        let system = SkillInstallSystem::new("bundled-destinations", &["ponytail"]);
        if let Some(settings) = settings {
            install_ponytail_package(&system.home, settings);
        }
        let destination = SkillDestination::new(
            vec![SkillAgent::Pi, SkillAgent::Claude],
            SkillScope::Global,
            &system.home,
            &system.home,
        );
        let report =
            execute_install_plan(&copy_skills_plan_for(&["ponytail"], destination), &system);
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(system.tree().join("ponytail/SKILL.md").is_file());
        assert!(system
            .home
            .join(".agents/skills/ponytail/SKILL.md")
            .is_file());
        assert!(!system
            .home
            .join(".pi/agent/skills/ponytail/SKILL.md")
            .exists());
    }
}

fn record_owned_skill(home: &Path, path: &Path) {
    let mut state = loom::InstallState::load(home).unwrap();
    state.record(loom::OwnedResource {
        id: "skill:ponytail".into(),
        scope: loom::OwnershipScope::Global,
        depends_on: vec![],
        receipts: vec![loom::Receipt::Path {
            path: path.to_path_buf(),
            path_kind: loom::OwnedPathKind::Tree,
            digest: loom::digest_path(path).unwrap(),
            before: None,
        }],
    });
    state.save(home).unwrap();
}

#[test]
fn pi_install_migrates_only_unchanged_owned_legacy_copies() {
    for edited in [false, true] {
        let system = SkillInstallSystem::new("bundled-reconcile", &["ponytail"]);
        let package = install_ponytail_package(&system.home, PONYTAIL_SETTINGS);
        let path = system.home.join(".pi/agent/skills/ponytail");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), "# owned\n").unwrap();
        record_owned_skill(&system.home, &path);
        if edited {
            fs::write(path.join("SKILL.md"), "# custom\n").unwrap();
        }
        let destination = SkillDestination::new(
            vec![SkillAgent::Pi],
            SkillScope::Global,
            &system.home,
            &system.home,
        );
        let report =
            execute_install_plan(&copy_skills_plan_for(&["ponytail"], destination), &system);
        assert!(report.failures.is_empty(), "{report:?}");
        assert_eq!(path.exists(), edited);
        assert_eq!(
            loom::InstallState::load(&system.home)
                .unwrap()
                .resources
                .contains_key("skill:ponytail"),
            edited
        );
        assert!(package.join("skills/ponytail/SKILL.md").is_file());
        assert!(system
            .home
            .join(".agents/skills/ponytail/SKILL.md")
            .is_file());
    }
}

struct PackageInstallSystem {
    skills: SkillInstallSystem,
    fail: bool,
}
impl System for PackageInstallSystem {
    fn command_exists(&self, _: &str) -> bool {
        true
    }
    fn refresh_path(&self) {}
    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.skills.home.clone())
    }
    fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
        if command.program != "pi" {
            return self.skills.run(command);
        }
        if !self.fail {
            install_ponytail_package(&self.skills.home, PONYTAIL_SETTINGS);
        }
        Ok(CommandResult {
            success: !self.fail,
            stdout: "npm:@dietrichgebert/ponytail@4.9.0".into(),
            stderr: "package failed".into(),
        })
    }
}

#[test]
fn selected_bundle_finishes_before_copy_and_failed_package_keeps_standalone() {
    for fail in [false, true] {
        let system = PackageInstallSystem {
            skills: SkillInstallSystem::new("selected-bundle", &["ponytail"]),
            fail,
        };
        let existing = system.skills.home.join(".pi/agent/skills/ponytail");
        if fail {
            install_ponytail_package(&system.skills.home, PONYTAIL_SETTINGS);
            fs::create_dir_all(&existing).unwrap();
            fs::write(existing.join("SKILL.md"), "# existing standalone").unwrap();
            record_owned_skill(&system.skills.home, &existing);
        }
        let catalog = loom::Catalog::embedded().unwrap();
        let selected = catalog
            .find(&["pi-package:@dietrichgebert/ponytail".into()])
            .unwrap();
        let selected =
            loom::expand_skill_dependencies(&catalog.resources, selected, &[SkillAgent::Pi]);
        let destination = SkillDestination::new(
            vec![SkillAgent::Pi],
            SkillScope::Global,
            &system.skills.home,
            &system.skills.home,
        );
        let plan = loom::build_install_plan(
            &selected,
            loom::PrerequisiteStatus {
                pi: true,
                herdr: true,
                mise: true,
            },
            loom::Platform::Unix,
            &destination,
        )
        .unwrap();
        let report = execute_install_plan(&plan, &system);
        assert_eq!(report.failures.is_empty(), !fail);
        if fail {
            assert!(report
                .failures
                .iter()
                .any(|failure| failure.target == "skills" && failure.message.contains("retry")));
            assert!(!report.installed.contains(&"skills".into()));
            assert_eq!(
                fs::read_to_string(existing.join("SKILL.md")).unwrap(),
                "# existing standalone"
            );
        }
        assert_eq!(
            system
                .skills
                .home
                .join(".pi/agent/skills/ponytail/SKILL.md")
                .exists(),
            fail
        );
    }
}

#[test]
fn unrelated_failed_pi_package_does_not_block_standalone_skills() {
    let system = PackageInstallSystem {
        skills: SkillInstallSystem::new("unrelated-pi-failure", &["ponytail"]),
        fail: true,
    };
    let destination = SkillDestination::new(
        vec![SkillAgent::Pi],
        SkillScope::Global,
        &system.skills.home,
        &system.skills.home,
    );
    let mut plan = copy_skills_plan_for(&["ponytail"], destination);
    plan.resources
        .push(step("pi-package:unrelated", "pi", "pi"));
    let report = execute_install_plan(&plan, &system);
    assert!(report.installed.contains(&"skills".into()));
    assert!(system
        .skills
        .home
        .join(".agents/skills/ponytail/SKILL.md")
        .is_file());
    assert!(!system
        .skills
        .home
        .join(".pi/agent/skills/ponytail/SKILL.md")
        .exists());
}

fn pi_skill_discovery(home: &Path, project: &Path) -> serde_json::Value {
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/pi-skill-discovery.mjs");
    let output = std::process::Command::new("node")
        .arg(helper)
        .arg(project)
        .arg(home.join(".pi/agent"))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("PI_OFFLINE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn shared_global_and_project_copies_are_excluded_only_from_real_pi_discovery() {
    for (scope, shared_agent) in [
        (SkillScope::Global, SkillAgent::AgentsStandard),
        (SkillScope::Project, SkillAgent::Codex),
    ] {
        let system = SkillInstallSystem::new("shared-pi-discovery", &["ponytail"]);
        let project = system.home.join("project");
        fs::create_dir_all(project.join(".git")).unwrap();
        let package = install_ponytail_package(&system.home, PONYTAIL_SETTINGS);
        let destination = SkillDestination::new(
            vec![SkillAgent::Pi, shared_agent],
            scope,
            &system.home,
            &project,
        );
        let report = execute_install_plan(
            &copy_skills_plan_for(&["ponytail"], destination.clone()),
            &system,
        );
        assert!(report.failures.is_empty(), "{report:?}");
        let root = if scope == SkillScope::Global {
            &system.home
        } else {
            &project
        };
        let shared = root.join(".agents/skills/ponytail/SKILL.md");
        assert!(shared.is_file());
        fs::write(
            &shared,
            "---\nname: ponytail\ndescription: Shared skill.\n---\n# standalone\n",
        )
        .unwrap();
        let result = pi_skill_discovery(&system.home, &project);
        assert_eq!(result["skills"].as_array().unwrap().len(), 1, "{result}");
        assert_eq!(
            PathBuf::from(result["skills"][0]["filePath"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            package
                .join("skills/ponytail/SKILL.md")
                .canonicalize()
                .unwrap()
        );
        assert!(
            !result["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["type"] == "collision"),
            "{result}"
        );
        let settings = if scope == SkillScope::Global {
            root.join(".pi/agent/settings.json")
        } else {
            root.join(".pi/settings.json")
        };
        let configured = fs::read_to_string(&settings).unwrap();
        let mut without: serde_json::Value = serde_json::from_str(&configured).unwrap();
        without["skills"] = serde_json::json!([]);
        fs::write(&settings, without.to_string()).unwrap();
        let red = pi_skill_discovery(&system.home, &project);
        assert!(
            red["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["type"] == "collision"),
            "real discovery must reproduce the original collision: {red}"
        );
        fs::write(&settings, configured).unwrap();
        // Typed receipts remove only the contributed array element, not packages or other keys.
        let mut state = loom::InstallState::load(&system.home).unwrap();
        let plan = loom::uninstall::build_uninstall_plan(
            &state,
            &loom::uninstall::UninstallRequest::default(),
            &project,
            loom::uninstall::receipt_status,
        )
        .unwrap();
        let removed = loom::uninstall::execute_uninstall_plan(
            &plan,
            &mut state,
            &system.home,
            &system,
            &AtomicBool::new(false),
        );
        assert!(removed.failures.is_empty(), "{removed:?}");
        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(config["skills"], serde_json::json!([]));
        assert!(shared.is_file());
        assert!(
            package.join("skills/ponytail/SKILL.md").is_file(),
            "an external provider is not claimed"
        );
    }
}

#[test]
fn shared_exclusions_preserve_user_entries_and_reconcile_removed_provider() {
    let system = SkillInstallSystem::new("shared-exclusion-ownership", &["ponytail"]);
    install_ponytail_package(&system.home, PONYTAIL_SETTINGS);
    let settings = system.home.join(".pi/agent/settings.json");
    let destination = SkillDestination::new(
        vec![SkillAgent::AgentsStandard],
        SkillScope::Global,
        &system.home,
        &system.home,
    );
    let shared = system.home.join(".agents/skills/ponytail/SKILL.md");
    let entry = format!("-{}", shared.display());
    let original = serde_json::json!({"packages":["npm:@dietrichgebert/ponytail@4.9.0"],"theme":"custom","skills":[entry,"!unrelated"]});
    fs::write(&settings, original.to_string()).unwrap();
    let plan = copy_skills_plan_for(&["ponytail"], destination.clone());
    assert!(execute_install_plan(&plan, &system).failures.is_empty());
    assert!(
        loom::InstallState::load(&system.home)
            .unwrap()
            .resources
            .is_empty(),
        "preexisting exclusion remains user-owned"
    );
    let mut config = original.clone();
    config["skills"] = serde_json::json!(["!unrelated"]);
    fs::write(&settings, config.to_string()).unwrap();
    assert!(execute_install_plan(&plan, &system).failures.is_empty());
    assert!(
        execute_install_plan(&plan, &system).failures.is_empty(),
        "idempotent"
    );
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(config["skills"].as_array().unwrap().len(), 2);
    config["packages"] = serde_json::json!([]); // direct pi remove, then next Loom install/update
    fs::write(&settings, config.to_string()).unwrap();
    assert!(execute_install_plan(&plan, &system).failures.is_empty());
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(config["skills"], serde_json::json!(["!unrelated"]));
    assert_eq!(config["theme"], "custom");
    assert!(shared.is_file());
}

#[test]
fn project_only_provider_conflict_does_not_change_settings_or_skills() {
    let system = SkillInstallSystem::new("project-only-provider", &["ponytail"]);
    let project = system.home.join("project");
    fs::create_dir_all(project.join(".git")).unwrap();
    let global = install_ponytail_package(&system.home, PONYTAIL_SETTINGS);
    let local = project.join(".pi/npm/node_modules/@dietrichgebert/ponytail");
    fs::create_dir_all(local.parent().unwrap()).unwrap();
    fs::rename(global, &local).unwrap();
    let settings = system.home.join(".pi/agent/settings.json");
    fs::write(&settings, "{}").unwrap();
    fs::write(project.join(".pi/settings.json"), PONYTAIL_SETTINGS).unwrap();
    let shared = system.home.join(".agents/skills/ponytail/SKILL.md");
    fs::create_dir_all(shared.parent().unwrap()).unwrap();
    fs::write(&shared, "# shared").unwrap();
    let destination = SkillDestination::new(
        vec![SkillAgent::Pi, SkillAgent::Codex],
        SkillScope::Project,
        &system.home,
        &project,
    );
    let report = execute_install_plan(&copy_skills_plan_for(&["ponytail"], destination), &system);
    assert!(
        report.failures.iter().any(|failure| failure
            .message
            .contains("project-only Pi provider conflicts")),
        "{report:?}"
    );
    assert_eq!(fs::read_to_string(&settings).unwrap(), "{}");
    assert_eq!(
        fs::read_to_string(project.join(".pi/settings.json")).unwrap(),
        PONYTAIL_SETTINGS
    );
    assert_eq!(fs::read_to_string(shared).unwrap(), "# shared");
    assert!(!project.join(".agents/skills").exists());
}

#[test]
fn malformed_ledger_and_explicit_inclusions_do_not_get_overwritten() {
    let system = SkillInstallSystem::new("shared-exclusion-invalid", &["ponytail"]);
    install_ponytail_package(&system.home, PONYTAIL_SETTINGS);
    let settings = system.home.join(".pi/agent/settings.json");
    let shared = system.home.join(".agents/skills/ponytail/SKILL.md");
    fs::create_dir_all(shared.parent().unwrap()).unwrap();
    fs::write(&shared, "# shared").unwrap();
    let destination = SkillDestination::new(
        vec![SkillAgent::AgentsStandard],
        SkillScope::Global,
        &system.home,
        &system.home,
    );
    let plan = copy_skills_plan_for(&["ponytail"], destination);
    let ledger = system.home.join(loom::ownership::STATE_PATH);
    fs::create_dir_all(ledger.parent().unwrap()).unwrap();
    fs::write(&ledger, "malformed").unwrap();
    assert!(!execute_install_plan(&plan, &system).failures.is_empty());
    assert_eq!(fs::read_to_string(&settings).unwrap(), PONYTAIL_SETTINGS);
    assert_eq!(fs::read_to_string(&ledger).unwrap(), "malformed");
    fs::remove_file(ledger).unwrap();
    let explicit = serde_json::json!({"packages":["npm:@dietrichgebert/ponytail@4.9.0"], "skills":[format!("+{}", shared.display())]});
    fs::write(&settings, explicit.to_string()).unwrap();
    assert!(!execute_install_plan(&plan, &system).failures.is_empty());
    assert_eq!(fs::read_to_string(&settings).unwrap(), explicit.to_string());
    assert_eq!(fs::read_to_string(shared).unwrap(), "# shared");
}

#[cfg(unix)]
#[test]
fn shared_alias_to_bundled_file_is_not_excluded_from_pi() {
    let system = SkillInstallSystem::new("shared-bundle-alias", &["ponytail"]);
    let project = system.home.join("project");
    fs::create_dir_all(&project).unwrap();
    let package = install_ponytail_package(&system.home, PONYTAIL_SETTINGS);
    let shared = system.home.join(".agents/skills/ponytail");
    fs::create_dir_all(shared.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(package.join("skills/ponytail"), &shared).unwrap();
    let destination = SkillDestination::new(
        vec![SkillAgent::Pi, SkillAgent::AgentsStandard],
        SkillScope::Global,
        &system.home,
        &project,
    );
    let report = execute_install_plan(&copy_skills_plan_for(&["ponytail"], destination), &system);
    assert!(report.failures.is_empty(), "{report:?}");
    assert_eq!(
        fs::read_to_string(system.home.join(".pi/agent/settings.json")).unwrap(),
        PONYTAIL_SETTINGS
    );
    let result = pi_skill_discovery(&system.home, &project);
    assert_eq!(result["skills"].as_array().unwrap().len(), 1, "{result}");
    assert!(
        !result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["type"] == "collision"),
        "{result}"
    );
}
