mod common;

use loom::{
    mcp, ownership, uninstall, CommandResult, CommandSpec, SkillAgent, SkillDestination,
    SkillScope, System,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Mutex};

fn destination(label: &str, scope: SkillScope) -> SkillDestination {
    let home = common::temp_home(label).canonicalize().unwrap();
    let project = home.join("project");
    fs::create_dir_all(project.join(".git")).unwrap();
    SkillDestination::new(vec![SkillAgent::Pi], scope, &home, &project)
}

fn write_json(path: &Path, value: serde_json::Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

fn adapter(home: &Path, version: &str) {
    let root = home.join(".pi/agent");
    write_json(
        &root.join("settings.json"),
        json!({"theme":"keep", "packages": [format!("npm:pi-mcp-adapter@{version}")]}),
    );
    let package = root.join("npm/node_modules/pi-mcp-adapter");
    write_json(
        &package.join("package.json"),
        json!({"name":"pi-mcp-adapter", "version":version, "pi":{"extensions":["index.ts"]}}),
    );
    fs::write(package.join("index.ts"), "export default function() {}").unwrap();
}

struct Stub {
    home: PathBuf,
    commands: Mutex<Vec<String>>,
    fail_mise: bool,
    fail_adapter: bool,
}

impl Stub {
    fn new(home: &Path) -> Self {
        Self {
            home: home.into(),
            commands: Mutex::new(Vec::new()),
            fail_mise: false,
            fail_adapter: false,
        }
    }
}
impl System for Stub {
    fn command_exists(&self, _: &str) -> bool {
        true
    }
    fn refresh_path(&self) {}
    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }
    fn current_dir(&self) -> Option<PathBuf> {
        Some(self.home.join("project"))
    }
    fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
        let display = command.display();
        assert!(
            !display.contains("sem mcp"),
            "installer must not launch MCP"
        );
        self.commands.lock().unwrap().push(display);
        let mut success = true;
        let mut stdout = String::new();
        if command.program == "pi" && command.args.first().map(String::as_str) == Some("install") {
            assert_eq!(command.args, ["install", mcp::ADAPTER_SPEC]);
            success = !self.fail_adapter;
            if success {
                adapter(&self.home, "2.32.1");
            }
        }
        if command.program == "pi" && command.args.first().map(String::as_str) == Some("list") {
            stdout = "User packages:\n  npm:pi-mcp-adapter@2.32.1\n".into();
        }
        if command.program == "mise" && command.args.first().map(String::as_str) == Some("install")
        {
            success = !self.fail_mise;
        }
        Ok(CommandResult {
            success,
            stdout,
            stderr: String::new(),
        })
    }
}

fn plan(destination: &SkillDestination) -> loom::InstallPlan {
    let catalog = loom::Catalog::embedded().unwrap();
    let resources = loom::expand_skill_dependencies(
        &catalog.resources,
        catalog.find(&["mcp-server:sem".into()]).unwrap(),
        &[SkillAgent::Pi],
    );
    loom::build_install_plan(
        &resources,
        loom::PrerequisiteStatus {
            pi: true,
            herdr: false,
            mise: true,
        },
        loom::Platform::Unix,
        destination,
    )
    .unwrap()
}

#[test]
fn sem_mcp_merge_is_idempotent_private_and_entry_owned_in_both_scopes() {
    for scope in [SkillScope::Global, SkillScope::Project] {
        let d = destination("mcp-merge", scope);
        adapter(&d.home, "2.33.0"); // Supported newer stable version is preserved.
        let stub = Stub::new(&d.home);
        let target = mcp::config_path(&d);
        write_json(
            &target,
            json!({"settings":{"directTools":true},"mcpServers":{"other":{"env":{"TOKEN":"private-sentinel"}}}, "custom":true}),
        );
        let original = fs::read(&target).unwrap();
        mcp::install(&d, &stub).unwrap();
        let after = fs::read(&target).unwrap();
        let value = serde_json::from_str::<serde_json::Value>(std::str::from_utf8(&after).unwrap())
            .unwrap();
        assert_eq!(
            value["mcpServers"]["sem"],
            json!({"command":"sem","args":["mcp"],"directTools":false})
        );
        assert_eq!(
            value["mcpServers"]["other"]["env"]["TOKEN"],
            "private-sentinel"
        );
        assert_eq!(value["custom"], true);
        assert_eq!(value["settings"]["directTools"], true);
        assert!(mcp::configured(&d, &stub));
        mcp::install(&d, &stub).unwrap();
        assert_eq!(fs::read(&target).unwrap(), after);
        assert!(stub.commands.lock().unwrap().is_empty());
        let mut state = ownership::InstallState::load(&d.home).unwrap();
        assert_eq!(state.resources.len(), 1);
        let owned = state.resources.values().next().unwrap();
        assert_eq!(owned.receipts.len(), 1);
        let receipt = owned.receipts[0].clone();
        let ledger = fs::read_to_string(d.home.join(ownership::STATE_PATH)).unwrap();
        assert!(!ledger.contains("private-sentinel"));
        assert!(!ledger.contains("lifecycle"));
        assert_eq!(
            uninstall::receipt_status(&receipt),
            uninstall::ReceiptStatus::Clean
        );
        let backup = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .flatten()
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".mcp.json.loom-backup-")
            })
            .unwrap()
            .path();
        assert_eq!(fs::read(&backup).unwrap(), original);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(backup).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let request = uninstall::UninstallRequest {
            selected: Some(vec![owned.id.clone()]),
            force_modified: false,
        };
        let removal = uninstall::build_uninstall_plan(
            &state,
            &request,
            &d.project_root,
            uninstall::receipt_status,
        )
        .unwrap();
        let report = uninstall::execute_uninstall_plan(
            &removal,
            &mut state,
            &d.home,
            &stub,
            &AtomicBool::new(false),
        );
        assert!(report.failures.is_empty(), "{report:?}");
        let value =
            serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&target).unwrap())
                .unwrap();
        assert!(value["mcpServers"].get("sem").is_none());
        assert_eq!(
            value["mcpServers"]["other"]["env"]["TOKEN"],
            "private-sentinel"
        );
        assert!(d
            .home
            .join(".pi/agent/npm/node_modules/pi-mcp-adapter/package.json")
            .exists());
        assert!(
            stub.commands.lock().unwrap().is_empty(),
            "shared dependencies must not be uninstalled"
        );
        fs::remove_dir_all(d.home).unwrap();
    }
}

#[test]
fn sem_mcp_preserves_existing_lifecycle_without_adopting_user_entry() {
    let d = destination("mcp-lifecycle", SkillScope::Project);
    adapter(&d.home, "2.32.1");
    let path = mcp::config_path(&d);
    write_json(
        &path,
        json!({"mcpServers":{"sem":{"command":"sem","args":["mcp"],"directTools":false,"lifecycle":"keep-alive","env":{"TOKEN":"private"}}}}),
    );
    let before = fs::read(&path).unwrap();
    mcp::install(&d, &Stub::new(&d.home)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(ownership::InstallState::load(&d.home)
        .unwrap()
        .resources
        .is_empty());
    fs::remove_dir_all(d.home).unwrap();
}

#[test]
fn sem_mcp_conflicts_and_malformed_files_fail_before_any_commands() {
    for value in [
        json!([]),
        json!({"mcpServers":[]}),
        json!({"mcp-servers":[]}),
        json!({"mcpServers":{"sem":{"url":"https://private.invalid/TOKEN"}}}),
        json!({"settings":{"disableProxyTool":true}}),
        json!({"mcpServers":{"sem":{"command":"sem","args":["mcp"],"directTools":true}}}),
    ] {
        let d = destination("mcp-conflict", SkillScope::Project);
        let initial_plan = plan(&d);
        let target = mcp::config_path(&d);
        write_json(&target, value);
        let before = fs::read(&target).unwrap();
        let stub = Stub::new(&d.home);
        let report = loom::execute_install_plan(&initial_plan, &stub);
        assert!(!report.failures.is_empty());
        assert!(!format!("{report:?}").contains("TOKEN"));
        assert!(stub.commands.lock().unwrap().is_empty());
        assert_eq!(fs::read(target).unwrap(), before);
        assert!(!d.home.join(".config/mise").exists());
        fs::remove_dir_all(d.home).unwrap();
    }
}

#[test]
fn sem_mcp_rejects_disabled_unverified_and_project_adapter_sources() {
    for package in [
        json!({"source":mcp::ADAPTER_SPEC,"extensions":[]}),
        json!("npm:pi-mcp-adapter@3.0.0"),
        json!("/some/private/pi-mcp-adapter"),
        json!({"source":mcp::ADAPTER_SPEC,"autoload":false}),
    ] {
        let d = destination("mcp-adapter-conflict", SkillScope::Global);
        write_json(
            &d.home.join(".pi/agent/settings.json"),
            json!({"packages":[package]}),
        );
        assert!(mcp::preflight(&d).is_err());
        fs::remove_dir_all(d.home).unwrap();
    }
    let d = destination("mcp-project-adapter", SkillScope::Project);
    write_json(
        &d.project_root.join(".pi/settings.json"),
        json!({"packages":[mcp::ADAPTER_SPEC]}),
    );
    assert!(mcp::preflight(&d)
        .unwrap_err()
        .to_string()
        .contains("project or duplicate"));
    fs::remove_dir_all(d.home).unwrap();
}

#[test]
fn sem_mcp_other_scopes_and_modified_owned_entries_are_preserved() {
    let d = destination("mcp-scope-conflict", SkillScope::Global);
    adapter(&d.home, "2.32.1");
    let stub = Stub::new(&d.home);
    mcp::install(&d, &stub).unwrap();
    let mut local = d.clone();
    local.scope = SkillScope::Project;
    assert!(mcp::preflight(&local)
        .unwrap_err()
        .to_string()
        .contains("already has a definition"));
    let path = mcp::config_path(&d);
    let state = ownership::InstallState::load(&d.home).unwrap();
    let receipt = &state.resources.values().next().unwrap().receipts[0];
    write_json(
        &path,
        json!({"mcpServers":{"sem":{"command":"sem","args":["mcp"],"directTools":false,"lifecycle":"eager"}}}),
    );
    assert_eq!(
        uninstall::receipt_status(receipt),
        uninstall::ReceiptStatus::Modified
    );
    if let ownership::Receipt::McpEntry { path, name, digest } = receipt {
        assert!(mcp::remove_entry(path, name, digest).is_err());
    } else {
        panic!("expected entry receipt");
    }
    assert!(path.exists());
    fs::remove_dir_all(d.home).unwrap();
}

#[cfg(unix)]
#[test]
fn sem_mcp_never_follows_config_symlinks() {
    let d = destination("mcp-symlink", SkillScope::Global);
    let path = mcp::config_path(&d);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let other = d.home.join("other.json");
    fs::write(&other, "{}").unwrap();
    std::os::unix::fs::symlink(&other, &path).unwrap();
    assert!(mcp::preflight(&d)
        .unwrap_err()
        .to_string()
        .contains("symlinked"));
    assert_eq!(fs::read_to_string(other).unwrap(), "{}");
    fs::remove_dir_all(d.home).unwrap();
}

#[test]
fn sem_mcp_prerequisite_order_and_failures_prevent_premature_config() {
    // Repository override is a fixture path only; manager calls below are stubs.
    std::env::set_var("LOOM_REPO_DIR", common::repo_root());
    for (fail_mise, fail_adapter) in [(false, false), (true, false), (false, true)] {
        let d = destination("mcp-order", SkillScope::Project);
        let plan = plan(&d);
        assert!(matches!(
            plan.prerequisites[0].action,
            loom::StepAction::SyncTools { .. }
        ));
        assert!(matches!(
            plan.resources[0].action,
            loom::StepAction::Command(_)
        ));
        assert!(matches!(
            plan.resources[1].action,
            loom::StepAction::ConfigureMcp { .. }
        ));
        let mut stub = Stub::new(&d.home);
        stub.fail_mise = fail_mise;
        stub.fail_adapter = fail_adapter;
        let report = loom::execute_install_plan(&plan, &stub);
        assert_eq!(
            report.failures.is_empty(),
            !fail_mise && !fail_adapter,
            "{report:?}"
        );
        assert_eq!(mcp::config_path(&d).exists(), !fail_mise && !fail_adapter);
        let commands = stub.commands.lock().unwrap();
        let mise = commands
            .iter()
            .position(|c| c == "mise install --yes")
            .unwrap();
        let pi = commands.iter().position(|c| c.starts_with("pi install"));
        if fail_mise {
            assert!(pi.is_none());
        } else {
            assert!(pi.unwrap() > mise);
        }
        assert!(!d.home.join(".pi/agent/mcp-cache.json").exists());
        fs::remove_dir_all(d.home).unwrap();
    }
}

#[test]
fn sem_mcp_cli_dry_run_shows_target_and_does_not_write() {
    use std::process::{Command, Stdio};
    for scope in ["global", "project"] {
        let d = destination("mcp-cli", SkillScope::Project);
        let output = Command::new(env!("CARGO_BIN_EXE_loom"))
            .args([
                "add",
                "--mcp-server",
                "sem",
                "--agent",
                "pi",
                "--scope",
                scope,
                "--dry-run",
            ])
            .env("HOME", &d.home)
            .env("USERPROFILE", &d.home)
            .env("XDG_CONFIG_HOME", d.home.join(".config"))
            .env("LOOM_REPO_DIR", common::repo_root())
            .env_remove("LOOM_BOOTSTRAP")
            .env_remove("PI_CODING_AGENT_DIR")
            .env_remove("PI_MCP_CONFIG_MODE")
            .env("PATH", "")
            .current_dir(&d.project_root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{text}");
        assert!(text.contains(mcp::ADAPTER_SPEC), "{text}");
        assert!(text.contains("directTools=false"), "{text}");
        assert!(text.contains("mcp.json"), "{text}");
        assert!(text.contains("lifecycle unchanged"), "{text}");
        assert!(!d.home.join(".pi").exists());
        assert!(!d.project_root.join(".pi").exists());
        assert!(!d.home.join(".config/mise").exists());
        fs::remove_dir_all(d.home).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn sem_mcp_cli_yes_configures_without_reinstalling_an_existing_gateway() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    for scope in [SkillScope::Global, SkillScope::Project] {
        let d = destination("mcp-cli-yes", scope);
        adapter(&d.home, "2.33.0");
        let settings = d.home.join(".pi/agent/settings.json");
        let before = fs::read(&settings).unwrap();
        let bin = d.home.join("bin");
        fs::create_dir_all(&bin).unwrap();
        for (name, script) in [
            ("mise", "#!/bin/sh\nprintf '%s\\n' \"mise $*\" >> \"$HOME/manager.log\"\nexit 0\n"),
            ("pi", "#!/bin/sh\n[ \"$1\" = list ] || exit 91\nprintf 'User packages:\\n  npm:pi-mcp-adapter@2.33.0\\n'\n"),
            ("sem", "#!/bin/sh\nprintf 'Sem must not be started' >> \"$HOME/server.log\"\nexit 99\n"),
        ] {
            let path = bin.join(name); fs::write(&path, script).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let output = Command::new(env!("CARGO_BIN_EXE_loom"))
            .args([
                "add",
                "--mcp-server",
                "sem",
                "--agent",
                "pi",
                "--scope",
                if scope == SkillScope::Global {
                    "global"
                } else {
                    "project"
                },
                "--yes",
            ])
            .env("HOME", &d.home)
            .env("USERPROFILE", &d.home)
            .env("XDG_CONFIG_HOME", d.home.join(".config"))
            .env("LOOM_REPO_DIR", common::repo_root())
            .env_remove("LOOM_BOOTSTRAP")
            .env_remove("PI_CODING_AGENT_DIR")
            .env_remove("PI_MCP_CONFIG_MODE")
            .env("PATH", &bin)
            .current_dir(&d.project_root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{text}");
        assert!(
            text.contains("configured; live health not checked"),
            "{text}"
        );
        assert!(mcp::config_path(&d).is_file());
        assert_eq!(fs::read(settings).unwrap(), before);
        assert!(!d.home.join("server.log").exists());
        assert_eq!(
            ownership::InstallState::load(&d.home)
                .unwrap()
                .resources
                .len(),
            1
        );
        assert!(
            fs::read_to_string(d.home.join(".config/mise/conf.d/loom.toml"))
                .unwrap()
                .contains("v0.24.0")
        );
        fs::remove_dir_all(d.home).unwrap();
    }
}

#[test]
fn sem_mcp_preserves_active_server_key_for_install_status_and_removal() {
    for canonical in [
        None,
        Some(serde_json::Value::Null),
        Some(json!({"active":{"command":"keep"}})),
    ] {
        let d = destination("mcp-alias", SkillScope::Project);
        adapter(&d.home, "2.33.0");
        let path = mcp::config_path(&d);
        let mut original = json!({"mcp-servers":{"other":{"command":"keep"}}});
        let key = if canonical.as_ref().is_some_and(|v| !v.is_null()) {
            "mcpServers"
        } else {
            "mcp-servers"
        };
        if let Some(value) = canonical {
            original["mcpServers"] = value;
        }
        write_json(&path, original.clone());
        let stub = Stub::new(&d.home);
        mcp::install(&d, &stub).unwrap();
        let installed: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            installed[key]["sem"],
            json!({"command":"sem","args":["mcp"],"directTools":false})
        );
        assert_eq!(
            installed["mcp-servers"]["other"],
            original["mcp-servers"]["other"]
        );
        assert!(mcp::configured(&d, &stub));
        let state = ownership::InstallState::load(&d.home).unwrap();
        let receipt = &state.resources.values().next().unwrap().receipts[0];
        assert_eq!(
            uninstall::receipt_status(receipt),
            uninstall::ReceiptStatus::Clean
        );
        if let ownership::Receipt::McpEntry { path, name, digest } = receipt {
            mcp::remove_entry(path, name, digest).unwrap();
        }
        let removed: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(removed, original);
        fs::remove_dir_all(d.home).unwrap();
    }
    let d = destination("mcp-alias-conflict", SkillScope::Project);
    let path = mcp::config_path(&d);
    write_json(&path, json!({"mcp-servers":{"sem":{"command":"other"}}}));
    assert!(mcp::preflight(&d)
        .unwrap_err()
        .to_string()
        .contains("conflicting"));
    fs::remove_dir_all(d.home).unwrap();
}

#[test]
fn sem_mcp_recovers_interrupted_config_before_merge_and_removal() {
    let d = destination("mcp-recovery", SkillScope::Project);
    adapter(&d.home, "2.33.0");
    let path = mcp::config_path(&d);
    let pending = path.with_file_name(".mcp.json.loom-old");
    write_json(&pending, json!({"mcpServers":{"other":{"command":"keep"}}}));
    let before = fs::read(&pending).unwrap();
    // Planning/status must inspect the recoverable snapshot without renaming it.
    plan(&d);
    assert!(!path.exists());
    assert_eq!(fs::read(&pending).unwrap(), before);
    let stub = Stub::new(&d.home);
    mcp::install(&d, &stub).unwrap();
    assert!(
        fs::read_to_string(&path).unwrap().contains("\"other\""),
        "recovery must preserve existing servers"
    );
    let installed: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(installed["mcpServers"]["other"], json!({"command":"keep"}));
    assert!(!pending.exists());
    let backup = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".mcp.json.loom-backup-")
        })
        .unwrap()
        .path();
    assert_eq!(fs::read(backup).unwrap(), before);
    let state = ownership::InstallState::load(&d.home).unwrap();
    let receipt = &state.resources.values().next().unwrap().receipts[0];
    fs::rename(&path, &pending).unwrap();
    assert_eq!(
        uninstall::receipt_status(receipt),
        uninstall::ReceiptStatus::Clean
    );
    assert!(!path.exists());
    assert!(
        !mcp::configured(&d, &stub),
        "pending recovery is not an active Pi configuration"
    );
    mcp::install(&d, &stub).unwrap();
    assert!(mcp::configured(&d, &stub));
    fs::rename(&path, &pending).unwrap();
    if let ownership::Receipt::McpEntry { path, name, digest } = receipt {
        mcp::remove_entry(path, name, digest).unwrap();
    }
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap(),
        json!({"mcpServers":{"other":{"command":"keep"}}})
    );
    fs::remove_dir_all(d.home).unwrap();
}

#[test]
fn sem_mcp_and_skills_serialize_shared_ownership_transactions() {
    use std::sync::Condvar;
    use std::time::Duration;
    struct OrderedSystem {
        stub: Stub,
        skills_started: Mutex<bool>,
        gate: Condvar,
    }
    impl System for OrderedSystem {
        fn command_exists(&self, _: &str) -> bool {
            true
        }
        fn refresh_path(&self) {}
        fn home_dir(&self) -> Option<PathBuf> {
            self.stub.home_dir()
        }
        fn current_dir(&self) -> Option<PathBuf> {
            self.stub.current_dir()
        }
        fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
            match command.program.as_str() {
                "hold-pi-lane" => {
                    // Give an incorrectly independent skills lane a coordinated
                    // opportunity to start before MCP commits its receipt.
                    let _ = self
                        .gate
                        .wait_timeout_while(
                            self.skills_started.lock().unwrap(),
                            Duration::from_millis(100),
                            |started| !*started,
                        )
                        .unwrap();
                }
                "check-mcp-receipt" => {
                    *self.skills_started.lock().unwrap() = true;
                    self.gate.notify_all();
                    let state = ownership::InstallState::load(&self.stub.home).unwrap();
                    anyhow::ensure!(
                        state.resources.contains_key("mcp-server:sem"),
                        "skills started before the MCP ownership commit"
                    );
                }
                _ => return self.stub.run(command),
            }
            Ok(CommandResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }
    std::env::set_var("LOOM_REPO_DIR", common::repo_root());
    let d = destination("mcp-ledger-order", SkillScope::Global);
    adapter(&d.home, "2.33.0");
    let bundled = loom::Catalog::embedded()
        .unwrap()
        .find(&["pi-package:i-have-adhd".into()])
        .unwrap()
        .remove(0);
    write_json(
        &d.home.join(".pi/agent/settings.json"),
        json!({"packages":["npm:pi-mcp-adapter@2.33.0", bundled.pi_install_spec()]}),
    );
    let package = d.home.join(".pi/agent/git/github.com/ayghri/i-have-adhd");
    write_json(
        &package.join("package.json"),
        json!({"pi":{"skills":["./skills"]}}),
    );
    fs::create_dir_all(package.join("skills/i-have-adhd")).unwrap();
    fs::write(
        package.join("skills/i-have-adhd/SKILL.md"),
        "# bundled skill",
    )
    .unwrap();
    let mut plan = plan(&d);
    plan.resources
        .retain(|step| step.target != "pi-package:pi-mcp-adapter");
    for (manager, program) in [("pi", "hold-pi-lane"), ("skills", "check-mcp-receipt")] {
        plan.prerequisites.push(loom::InstallStep {
            target: program.into(),
            manager: manager.into(),
            action: loom::StepAction::Command(CommandSpec::new(program, Vec::<String>::new())),
            verification: None,
        });
    }
    plan.resources.push(loom::InstallStep {
        target: "skill:i-have-adhd".into(),
        manager: "skills".into(),
        action: loom::StepAction::CopySkills {
            skills: vec!["i-have-adhd".into()],
            destination: d.clone(),
        },
        verification: None,
    });
    let system = OrderedSystem {
        stub: Stub::new(&d.home),
        skills_started: Mutex::new(false),
        gate: Condvar::new(),
    };
    let report = loom::execute_install_plan(&plan, &system);
    assert!(report.failures.is_empty(), "{report:?}");
    let state = ownership::InstallState::load(&d.home).unwrap();
    assert!(state.resources.contains_key("mcp-server:sem"));
    assert!(state.resources["pi-package:i-have-adhd"]
        .receipts
        .iter()
        .any(|receipt| matches!(receipt, ownership::Receipt::PiSkillExclusion { .. })));
    fs::remove_dir_all(d.home).unwrap();
}

#[test]
fn sem_mcp_cli_rejects_project_scope_in_exclusive_mode_before_changes() {
    use std::process::{Command, Stdio};
    for scope in ["global", "project"] {
        let d = destination("mcp-exclusive", SkillScope::Project);
        let output = Command::new(env!("CARGO_BIN_EXE_loom"))
            .args([
                "add",
                "--mcp-server",
                "sem",
                "--agent",
                "pi",
                "--scope",
                scope,
                "--dry-run",
            ])
            .env("HOME", &d.home)
            .env("USERPROFILE", &d.home)
            .env("XDG_CONFIG_HOME", d.home.join(".config"))
            .env("LOOM_REPO_DIR", common::repo_root())
            .env("PI_MCP_CONFIG_MODE", " ExClUsIvE ")
            .env_remove("PI_CODING_AGENT_DIR")
            .env_remove("LOOM_BOOTSTRAP")
            .env("PATH", "")
            .current_dir(&d.project_root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if scope == "project" {
            assert!(
                !output.status.success(),
                "project config would be ignored: {text}"
            );
            assert!(
                text.contains("PI_MCP_CONFIG_MODE") && text.contains("--scope global"),
                "{text}"
            );
        } else {
            assert!(output.status.success(), "{text}");
        }
        assert!(!d.home.join(".pi").exists());
        assert!(!d.project_root.join(".pi").exists());
        assert!(!d.home.join(".config/mise").exists());
        fs::remove_dir_all(d.home).unwrap();
    }
}

#[test]
fn sem_mcp_cli_dry_run_preserves_pending_config_ledger_and_selection() {
    use std::process::{Command, Stdio};
    for scope in [SkillScope::Global, SkillScope::Project] {
        for live in [false, true] {
            let d = destination("mcp-recovery-dry-run", scope);
            let target = mcp::config_path(&d);
            let pending = target.with_file_name(".mcp.json.loom-old");
            write_json(
                &pending,
                json!({"mcp-servers":{"other":{"command":"keep"}}}),
            );
            let before = fs::read(&pending).unwrap();
            let ledger = d.home.join(ownership::STATE_PATH);
            let ledger_pending = ledger.with_file_name(".install-state.json.loom-old");
            let ledger_before = br#"{"schemaVersion":1,"resources":{}}"#;
            fs::create_dir_all(ledger.parent().unwrap()).unwrap();
            fs::write(&ledger_pending, ledger_before).unwrap();
            let selection = loom::manifest::conf_d_target(&d.home);
            let selection_pending = selection.with_file_name(".loom.toml.loom-old");
            let selection_before = b"[tools]\n\"github:zdyxry/tokui\" = \"0.12.0\"\n";
            fs::create_dir_all(selection.parent().unwrap()).unwrap();
            fs::write(&selection_pending, selection_before).unwrap();
            // Different bytes prove the live file wins without deleting its stale backup.
            let ledger_live = b"{ \"schemaVersion\": 1, \"resources\": {} }\n";
            let selection_live = b"[tools]\n\"github:zdyxry/tokui\" = \"0.11.0\"\n";
            if live {
                fs::write(&target, b"{}\n").unwrap();
                fs::write(&ledger, ledger_live).unwrap();
                fs::write(&selection, selection_live).unwrap();
            }
            let output = Command::new(env!("CARGO_BIN_EXE_loom"))
                .args([
                    "add",
                    "--mcp-server",
                    "sem",
                    "--agent",
                    "pi",
                    "--scope",
                    if scope == SkillScope::Global {
                        "global"
                    } else {
                        "project"
                    },
                    "--dry-run",
                ])
                .env("HOME", &d.home)
                .env("USERPROFILE", &d.home)
                .env("XDG_CONFIG_HOME", d.home.join(".config"))
                .env("LOOM_REPO_DIR", common::repo_root())
                .env_remove("PI_CODING_AGENT_DIR")
                .env_remove("PI_MCP_CONFIG_MODE")
                .env_remove("LOOM_BOOTSTRAP")
                .env("PATH", "")
                .current_dir(&d.project_root)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(target.exists(), live);
            if live {
                assert_eq!(fs::read(&target).unwrap(), b"{}\n");
                assert_eq!(fs::read(&ledger).unwrap(), ledger_live);
                assert_eq!(fs::read(&selection).unwrap(), selection_live);
            }
            assert_eq!(ledger.exists(), live);
            assert_eq!(selection.exists(), live);
            assert_eq!(fs::read(&ledger_pending).unwrap(), ledger_before);
            assert_eq!(fs::read(&selection_pending).unwrap(), selection_before);
            for directory in [
                target.parent().unwrap(),
                ledger.parent().unwrap(),
                selection.parent().unwrap(),
            ] {
                assert_eq!(
                    fs::read_dir(directory).unwrap().count(),
                    if live { 2 } else { 1 }
                );
            }
            assert_eq!(fs::read(&pending).unwrap(), before);

            fs::remove_dir_all(d.home).unwrap();
        }
    }
}
