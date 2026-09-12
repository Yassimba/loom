use super::*;
use crate::settings::SettingChange;
use crate::{CommandResult, CommandSpec, InstallStep, Operation, Receipt, SkillScope};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

#[test]
fn cancelled_retry_keeps_reviewed_destination_completed_work_and_setting_ownership() {
    struct Fixture {
        home: PathBuf,
        current: Mutex<PathBuf>,
        commands: Mutex<Vec<CommandSpec>>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.home);
        }
    }
    impl System for Fixture {
        fn command_exists(&self, name: &str) -> bool {
            name == "herdr"
        }
        fn refresh_path(&self) {}
        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }
        fn current_dir(&self) -> Option<PathBuf> {
            Some(self.current.lock().unwrap().clone())
        }
        fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
            self.commands.lock().unwrap().push(command.clone());
            match command.program.as_str() {
                "curl" => {}
                "tar" => {
                    let skill = PathBuf::from(&command.args[3]).join("repo/skills/tdd");
                    fs::create_dir_all(&skill)?;
                    fs::write(skill.join("SKILL.md"), "# reviewed skill\n")?;
                }
                "herdr" => {}
                other => panic!("unexpected command: {other}"),
            }
            Ok(CommandResult {
                success: command.program != "herdr",
                stdout: String::new(),
                stderr: "fixture package failure".into(),
            })
        }
    }
    let home = std::env::temp_dir().join(format!(
        "loom-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let reviewed = home.join("reviewed-project");
    let other = home.join("other-project");
    fs::create_dir_all(&reviewed).unwrap();
    fs::create_dir_all(&other).unwrap();
    let reviewed = reviewed.canonicalize().unwrap();
    let system = Fixture {
        home: home.clone(),
        current: Mutex::new(reviewed.clone()),
        commands: Mutex::new(Vec::new()),
    };
    let destination = SkillDestination::new(
        vec![SkillAgent::Claude],
        SkillScope::Project,
        &home,
        &reviewed,
    );
    let resources = crate::Catalog::embedded()
        .unwrap()
        .find(&["skill:tdd".into()])
        .unwrap();
    let paths = SettingsPaths {
        herdr_config: home.join("herdr.toml"),
        zed_settings: home.join("zed.json"),
        zed_keymap: home.join("keymap.json"),
        pi_fff_config: home.join("pi-fff.json"),
        pi_adhd_flag: home.join("adhd"),
    };
    let before = "{\"unrelated\": true}\n";
    fs::write(&paths.zed_settings, before).unwrap();
    let settings = vec![SettingSpec {
        id: "zed:zoomed-padding".into(),
        group: "Zed".into(),
        label: "Zoomed panes edge-to-edge".into(),
        description: String::new(),
        related_resource: None,
        change: SettingChange::ZedValue {
            key: "zoomed_padding".into(),
            value: serde_json::json!(false),
        },
    }];
    let plan = InstallPlan {
        steps: vec![
            InstallStep {
                target: "skills".into(),
                operation: Operation::Skills {
                    skills: vec!["tdd".into()],
                    destination: destination.clone(),
                },
            },
            InstallStep {
                target: "herdr-plugin:unavailable".into(),
                operation: Operation::HerdrPlugin {
                    source: "unavailable".into(),
                    name: "unavailable".into(),
                },
            },
        ],
    };
    let ownership = InstallOwnership::capture(
        &resources,
        &settings,
        &paths,
        &destination,
        PrerequisiteStatus {
            pi: false,
            herdr: true,
            mise: false,
        },
    );
    let mut session = InstallSession::new(plan.clone(), settings, paths.clone());
    session.ownership = Some(ownership);
    let cancelled = AtomicBool::new(false);
    let first = session.run_attempt(&system, &cancelled, &mut |_, _| {});
    assert_eq!(first.installed, ["skills", "zed:zoomed-padding"]);
    assert_eq!(first.failures.len(), 1);
    assert_eq!(first.failures[0].target, "herdr-plugin:unavailable");
    assert_eq!(session.completed, [0, 2]);
    let written_setting = fs::read_to_string(&paths.zed_settings).unwrap();
    let command_count = system.commands.lock().unwrap().len();

    // A retry must use the reviewed project even if the caller has moved elsewhere.
    *system.current.lock().unwrap() = other.clone();
    let mut statuses = Vec::new();
    let second = session.run_attempt(&system, &cancelled, &mut |index, status| {
        if index == 0 && status == StepStatus::Installed {
            cancelled.store(true, Ordering::Relaxed);
        }
        statuses.push((index, status));
    });
    assert_eq!(session.plan, plan);
    assert_eq!(session.completed, [0]);
    assert_eq!(second.installed, ["skills"]);
    assert_eq!(second.failures.len(), 2);
    assert!(second
        .failures
        .iter()
        .all(|failure| failure.message == "cancelled"));
    assert_eq!(
        &statuses[..2],
        &[(0, StepStatus::Verifying), (0, StepStatus::Installed)]
    );
    assert_eq!(system.commands.lock().unwrap().len(), command_count);
    assert_eq!(
        fs::read_to_string(reviewed.join(".claude/skills/tdd/SKILL.md")).unwrap(),
        "# reviewed skill\n"
    );
    assert!(!other.join(".claude").exists());
    assert_eq!(
        fs::read_to_string(&paths.zed_settings).unwrap(),
        written_setting
    );

    // Recording only the last attempt would lose this setting's ownership and original bytes.
    session.record_ownership(&system).unwrap();
    let state = crate::InstallState::load(&home).unwrap();
    let setting = &state.resources["setting:zed:zoomed-padding"];
    assert_eq!(
        setting.receipts,
        [Receipt::Path {
            path: paths.zed_settings.clone(),
            path_kind: crate::OwnedPathKind::File,
            digest: crate::digest_path(&paths.zed_settings).unwrap(),
            before: Some(before.into()),
        }]
    );
    assert!(state.resources.values().any(|resource| {
        resource.scope
            == crate::OwnershipScope::Project {
                root: reviewed.clone(),
            }
            && resource.id.ends_with(":skill:tdd")
    }));
}

#[test]
fn fresh_tools_are_owned_without_changing_attempt_reports() {
    struct Fixture {
        home: PathBuf,
        fail_install: bool,
        ready: AtomicBool,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.home);
        }
    }
    impl System for Fixture {
        fn command_exists(&self, name: &str) -> bool {
            name == "mise" || self.ready.load(Ordering::Relaxed)
        }
        fn refresh_path(&self) {}
        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }
        fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
            let mut success = true;
            match command.program.as_str() {
                "curl" | "sh" => {}
                "tar" => {
                    let manifest = PathBuf::from(&command.args[3]).join("repo/manifest/loom.toml");
                    fs::create_dir_all(manifest.parent().unwrap())?;
                    fs::write(
                        manifest,
                        format!(
                            "[tools]\n# core:begin\nnode = \"24.19.0\"\n# core:end\n\
                             bun = \"1.0.0\"\ngh = \"2.0.0\"\n\"{}\" = \"0.73.1\"\n",
                            crate::manifest::PI_TOOL_KEY
                        ),
                    )?;
                }
                "mise" if command.args.first().map(String::as_str) == Some("install") => {
                    success = !self.fail_install;
                    self.ready.store(success, Ordering::Relaxed);
                }
                "mise" | "pi" => {}
                other => panic!("unexpected command: {other}"),
            }
            Ok(CommandResult {
                success,
                stdout: "User packages:\n  npm:pi-web-access\n".into(),
                stderr: if success { "" } else { "tool install failed" }.into(),
            })
        }
    }

    for (name, fail_install, cancel_before, cancel_after) in [
        ("success", false, false, false),
        ("failed", true, false, false),
        ("cancelled-before", false, true, false),
        ("cancelled-after", false, false, true),
    ] {
        let home = std::env::temp_dir().join(format!(
            "loom-tools-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let system = Fixture {
            home: home.clone(),
            fail_install,
            ready: AtomicBool::new(false),
        };
        let destination = SkillDestination::new(Vec::new(), SkillScope::Global, &home, &home);
        let paths = SettingsPaths {
            herdr_config: home.join("herdr.toml"),
            zed_settings: home.join("zed.json"),
            zed_keymap: home.join("keymap.json"),
            pi_fff_config: home.join("pi-fff.json"),
            pi_adhd_flag: home.join("adhd"),
        };
        let mut resources = crate::Catalog::embedded()
            .unwrap()
            .find(&["tool:bun".into(), "pi-package:pi-web-access".into()])
            .unwrap();
        resources[0].companions = vec!["gh".into()];
        let status = PrerequisiteStatus {
            pi: false,
            herdr: false,
            // A successful bootstrap must not claim tools whose installation fails.
            mise: !fail_install,
        };
        let plan =
            crate::build_install_plan(&resources, status, crate::Platform::Unix, &destination)
                .unwrap();
        let ownership = InstallOwnership::capture(&resources, &[], &paths, &destination, status);
        let mut session = InstallSession::new(plan, Vec::new(), paths);
        session.ownership = Some(ownership);
        let cancelled = AtomicBool::new(cancel_before);
        let first = session.run_attempt(&system, &cancelled, &mut |_, status| {
            if cancel_after && status == StepStatus::Prepared {
                cancelled.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(first.failures.is_empty(), name == "success", "{name}");
        assert_eq!(
            first.installed,
            if name == "success" {
                vec!["pi-package:pi-web-access"]
            } else {
                vec![]
            },
            "{name}"
        );
        session.record_ownership(&system).unwrap();
        let state = crate::InstallState::load(&home).unwrap();
        if fail_install || cancel_before {
            assert!(state.resources.is_empty(), "{name}");
            assert!(session.written.is_empty(), "{name}");
            continue;
        }
        assert_eq!(
            state
                .resources
                .get("tool:bun")
                .map(|resource| resource.receipts.as_slice()),
            Some(
                [
                    Receipt::MiseTool { key: "bun".into() },
                    Receipt::MiseTool { key: "gh".into() },
                ]
                .as_slice()
            ),
            "{name}"
        );
        assert_eq!(
            state.resources["tool:pi"].receipts,
            [Receipt::MiseTool {
                key: crate::manifest::PI_TOOL_KEY.into(),
            }]
        );
        assert_eq!(
            session.written.iter().filter(|id| *id == "tools").count(),
            1
        );

        // A cancelled retry keeps prior ownership even when completion is revoked.
        let written = session.written.clone();
        let retry = session.run_attempt(&system, &AtomicBool::new(true), &mut |_, _| {});
        assert!(retry.installed.is_empty());
        assert_eq!(session.written, written);
        session.record_ownership(&system).unwrap();
        assert_eq!(
            crate::InstallState::load(&home).unwrap().resources,
            state.resources
        );

        cancelled.store(false, Ordering::Relaxed);
        assert!(session
            .run_attempt(&system, &cancelled, &mut |_, _| {})
            .failures
            .is_empty());
        let verified = session.run_attempt(&system, &cancelled, &mut |_, _| {});
        assert!(verified.failures.is_empty());
        assert_eq!(verified.installed, ["tools", "pi-package:pi-web-access"]);
        assert_eq!(
            session.written.iter().filter(|id| *id == "tools").count(),
            1
        );
    }
}
