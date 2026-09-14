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

fn read_generated_jsonc(path: &Path) -> serde_json::Value {
    let source = fs::read_to_string(path).unwrap().replace(",}", "}");
    serde_json::from_str(&source).unwrap()
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
    fail_adapter: bool,
    missing_binary: Option<&'static str>,
}

impl Stub {
    fn new(home: &Path) -> Self {
        Self {
            home: home.into(),
            commands: Mutex::new(Vec::new()),
            fail_adapter: false,
            missing_binary: None,
        }
    }
}

impl System for Stub {
    fn command_exists(&self, name: &str) -> bool {
        self.missing_binary != Some(name)
    }

    fn refresh_path(&self) {}

    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }

    fn current_dir(&self) -> Option<PathBuf> {
        Some(self.home.join("project"))
    }

    fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
        self.commands.lock().unwrap().push(command.display());
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
        Ok(CommandResult {
            success,
            stdout,
            stderr: String::new(),
        })
    }
}

fn plan(destination: &SkillDestination) -> loom::InstallPlan {
    plan_server(destination, "context7")
}

fn plan_server(destination: &SkillDestination, name: &str) -> loom::InstallPlan {
    let catalog = loom::Catalog::embedded().unwrap();
    let resources = loom::expand_skill_dependencies(
        &catalog.resources,
        catalog.find(&[format!("mcp-server:{name}")]).unwrap(),
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
fn unreviewed_mcp_server_is_rejected_before_mutation() {
    let home = common::temp_home("mcp-unreviewed");
    assert!(mcp::Server::from_name("unreviewed-server").is_err());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_loom"))
        .args([
            "add",
            "--mcp-server",
            "unreviewed-server",
            "--agent",
            "pi",
            "--yes",
        ])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("PATH", "")
        .current_dir(&home)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unreviewed-server"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_dir(&home).unwrap().count(), 0);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn mcp_with_shared_agent_selection_does_not_require_pending_skill_copies() {
    std::env::set_var("LOOM_REPO_DIR", common::repo_root());
    for scope in [SkillScope::Global, SkillScope::Project] {
        let mut destination = destination("mcp-shared-agents", scope);
        destination.agents = SkillAgent::ALL.to_vec();
        let stub = Stub::new(&destination.home);
        let claude = destination.home.join(".claude.json");
        write_json(&claude, json!({"mcpServers":{"keep":{"command":"custom"}}}));
        let before = fs::read(&claude).unwrap();
        let report = common::install(&plan(&destination), &stub);
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(mcp::config_path(&destination).is_file());
        assert_eq!(fs::read(claude).unwrap(), before);
        destination.agents = vec![SkillAgent::Claude];
        assert!(mcp::preflight(mcp::Server::Context7, &destination).is_err());
        fs::remove_dir_all(destination.home).unwrap();
    }
}

#[test]
fn codebase_memory_writes_exact_entry_and_records_tool_dependency() {
    let (server, name, expected, tool) = (
        mcp::Server::CodebaseMemory,
        "codebase-memory-mcp",
        json!({
            "command": "codebase-memory-mcp",
            "args": ["--tool-profile=analysis"],
            "directTools": false
        }),
        "tool:codebase-memory-mcp",
    );
    let destination = destination(name, SkillScope::Global);
    adapter(&destination.home, "2.33.0");
    let stub = Stub::new(&destination.home);

    mcp::install(server, &destination, &stub).unwrap();

    let config = read_generated_jsonc(&mcp::config_path(&destination));
    assert_eq!(config["mcpServers"][name], expected);
    let state = ownership::InstallState::load(&destination.home).unwrap();
    assert!(state.resources[&format!("mcp-server:{name}")]
        .depends_on
        .contains(&tool.into()));
    fs::remove_dir_all(destination.home).unwrap();
}

#[test]
fn codebase_memory_accepts_absolute_binary_and_requires_it() {
    let (server, name, args) = (
        mcp::Server::CodebaseMemory,
        "codebase-memory-mcp",
        json!(["--tool-profile=analysis"]),
    );
    let destination = destination(&format!("{name}-absolute"), SkillScope::Global);
    adapter(&destination.home, "2.33.0");
    let path = mcp::config_path(&destination);
    write_json(
        &path,
        json!({"mcpServers": {(name): {
            "command": format!("/opt/loom/bin/{name}"),
            "args": args,
            "directTools": false
        }}}),
    );
    let stub = Stub::new(&destination.home);
    assert!(mcp::configured(server, &destination, &stub));

    let missing = Stub {
        missing_binary: Some(name),
        ..Stub::new(&destination.home)
    };
    assert!(!mcp::configured(server, &destination, &missing));
    assert!(mcp::install(server, &destination, &missing)
        .unwrap_err()
        .to_string()
        .contains("prerequisites missing"));
    fs::remove_dir_all(destination.home).unwrap();
}

#[test]
fn codebase_memory_conflicts_fail_before_mutation() {
    let (server, name, valid_args) = (
        mcp::Server::CodebaseMemory,
        "codebase-memory-mcp",
        json!(["--tool-profile=analysis"]),
    );
    for entry in [
        json!({"command": name, "args": valid_args.clone(), "directTools": true}),
        json!({"command": name, "args": valid_args.clone(), "directTools": false, "disabled": true}),
        json!({"command": name, "args": valid_args.clone(), "directTools": false, "socket": "private"}),
        json!({"command": name, "args": valid_args.clone(), "directTools": false, "url": "https://private.invalid"}),
        json!({"command": format!("./{name}"), "args": valid_args.clone(), "directTools": false}),
        json!({"command": format!("bin/{name}"), "args": valid_args.clone(), "directTools": false}),
    ] {
        let destination = destination(&format!("{name}-conflict"), SkillScope::Global);
        adapter(&destination.home, "2.33.0");
        let path = mcp::config_path(&destination);
        write_json(&path, json!({"mcpServers": {(name): entry}}));
        let before = fs::read(&path).unwrap();
        assert!(mcp::preflight(server, &destination).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
        fs::remove_dir_all(destination.home).unwrap();
    }
}

#[test]
fn context7_upgrades_an_older_official_adapter() {
    std::env::set_var("LOOM_REPO_DIR", common::repo_root());
    let destination = destination("mcp-adapter-upgrade", SkillScope::Global);
    adapter(&destination.home, "2.31.0");
    let stub = Stub::new(&destination.home);

    let report = common::install(&plan(&destination), &stub);

    assert!(report.failures.is_empty(), "{report:?}");
    assert!(stub
        .commands
        .lock()
        .unwrap()
        .contains(&format!("pi install {}", mcp::ADAPTER_SPEC)));
    assert!(!mcp::adapter_needed(&destination).unwrap());
    fs::remove_dir_all(destination.home).unwrap();
}

#[test]
fn context7_rejects_disabled_unverified_and_project_adapter_sources() {
    for package in [
        json!({"source":mcp::ADAPTER_SPEC,"extensions":[]}),
        json!("npm:pi-mcp-adapter@3.0.0"),
        json!("/some/private/pi-mcp-adapter"),
        json!({"source":mcp::ADAPTER_SPEC,"autoload":false}),
    ] {
        let destination = destination("mcp-adapter-conflict", SkillScope::Global);
        write_json(
            &destination.home.join(".pi/agent/settings.json"),
            json!({"packages":[package]}),
        );
        assert!(mcp::preflight(mcp::Server::Context7, &destination).is_err());
        fs::remove_dir_all(destination.home).unwrap();
    }
    let destination = destination("mcp-project-adapter", SkillScope::Project);
    write_json(
        &destination.project_root.join(".pi/settings.json"),
        json!({"packages":[mcp::ADAPTER_SPEC]}),
    );
    assert!(mcp::preflight(mcp::Server::Context7, &destination)
        .unwrap_err()
        .to_string()
        .contains("project or duplicate"));
    fs::remove_dir_all(destination.home).unwrap();
}

#[cfg(unix)]
#[test]
fn context7_never_follows_config_symlinks() {
    let destination = destination("mcp-symlink", SkillScope::Global);
    let path = mcp::config_path(&destination);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let other = destination.home.join("other.json");
    fs::write(&other, "{}").unwrap();
    std::os::unix::fs::symlink(&other, &path).unwrap();
    assert!(mcp::preflight(mcp::Server::Context7, &destination)
        .unwrap_err()
        .to_string()
        .contains("symlinked"));
    assert_eq!(fs::read_to_string(other).unwrap(), "{}");
    fs::remove_dir_all(destination.home).unwrap();
}

#[test]
fn context7_recovers_interrupted_config_before_merge_and_removal() {
    let destination = destination("mcp-recovery", SkillScope::Project);
    adapter(&destination.home, "2.33.0");
    let path = mcp::config_path(&destination);
    let pending = path.with_file_name(".mcp.json.loom-old");
    write_json(&pending, json!({"mcpServers":{"other":{"command":"keep"}}}));
    let before = fs::read(&pending).unwrap();
    plan(&destination);
    assert!(!path.exists());
    assert_eq!(fs::read(&pending).unwrap(), before);
    let stub = Stub::new(&destination.home);
    mcp::install(mcp::Server::Context7, &destination, &stub).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap()
            ["mcpServers"]["other"],
        json!({"command":"keep"})
    );
    assert!(!pending.exists());
    let state = ownership::InstallState::load(&destination.home).unwrap();
    let receipt = &state.resources.values().next().unwrap().receipts[0];
    fs::rename(&path, &pending).unwrap();
    assert_eq!(
        uninstall::receipt_status(receipt),
        uninstall::ReceiptStatus::Clean
    );
    assert!(!mcp::configured(mcp::Server::Context7, &destination, &stub));
    if let ownership::Receipt::McpEntry { path, name, digest } = receipt {
        mcp::remove_entry(path, name, digest).unwrap();
    }
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap(),
        json!({"mcpServers":{"other":{"command":"keep"}}})
    );
    fs::remove_dir_all(destination.home).unwrap();
}

#[test]
fn context7_plan_installs_only_the_adapter_then_configures_and_uninstalls_each_scope() {
    for scope in [SkillScope::Global, SkillScope::Project] {
        let destination = destination("context7-install", scope);
        let plan = plan(&destination);
        assert_eq!(plan.prerequisite_count(), 0);
        assert_eq!(plan.resources().count(), 2);
        assert_eq!(
            plan.resources().next().unwrap().target,
            "pi-package:pi-mcp-adapter"
        );
        assert_eq!(
            plan.resources().nth(1).unwrap().target,
            "mcp-server:context7"
        );
        let path = mcp::config_path(&destination);
        write_json(&path, json!({"mcp-servers":{"other":{"command":"keep"}}}));
        let stub = Stub::new(&destination.home);
        let report = common::install(&plan, &stub);
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(report.installed.contains(&"mcp-server:context7".into()));
        let after = fs::read(&path).unwrap();
        let config: serde_json::Value = serde_json::from_slice(&after).unwrap();
        assert_eq!(
            config["mcp-servers"]["context7"],
            json!({"url":"https://mcp.context7.com/mcp","directTools":false})
        );
        assert!(!destination.home.join(".config/mise").exists());
        assert!(mcp::configured(mcp::Server::Context7, &destination, &stub));
        mcp::install(mcp::Server::Context7, &destination, &stub).unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
        let mut state = ownership::InstallState::load(&destination.home).unwrap();
        let owned = state.resources.values().next().unwrap();
        assert!(owned.id.ends_with("mcp-server:context7"));
        assert!(owned
            .depends_on
            .contains(&"pi-package:pi-mcp-adapter".into()));
        let removal = uninstall::build_uninstall_plan(
            &state,
            &uninstall::UninstallRequest {
                selected: Some(vec![owned.id.clone()]),
                force_modified: false,
            },
            &destination.project_root,
            uninstall::receipt_status,
        )
        .unwrap();
        let report = uninstall::execute_uninstall_plan(
            &removal,
            &mut state,
            &destination.home,
            &stub,
            &AtomicBool::new(false),
        );
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(!mcp::configured(mcp::Server::Context7, &destination, &stub));
        let config: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(config, json!({"mcp-servers":{"other":{"command":"keep"}}}));
        fs::remove_dir_all(destination.home).unwrap();
    }
}

#[test]
fn context7_conflicts_stop_before_commands_and_preserve_secrets() {
    for value in [
        json!({"url":"https://private.invalid/TOKEN","directTools":false}),
        json!({"url":"https://mcp.context7.com/mcp","directTools":false,"disabled":true}),
        json!({"url":"https://mcp.context7.com/mcp","directTools":true}),
        json!({"url":"https://mcp.context7.com/mcp","directTools":false,"command":"other"}),
        json!({"url":"https://mcp.context7.com/mcp","directTools":false,"socket":"private"}),
    ] {
        let destination = destination("context7-conflict", SkillScope::Project);
        let plan = plan(&destination);
        let path = mcp::config_path(&destination);
        write_json(&path, json!({"mcpServers":{"context7":value}}));
        let before = fs::read(&path).unwrap();
        let stub = Stub::new(&destination.home);
        let report = common::install(&plan, &stub);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].target, "mcp-server:context7");
        assert!(!format!("{report:?}").contains("TOKEN"));
        assert!(stub.commands.lock().unwrap().is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(destination.home).unwrap();
    }
}

#[test]
fn context7_preserves_user_auth_and_modified_owned_config() {
    let destination = destination("context7-auth", SkillScope::Global);
    adapter(&destination.home, "2.33.0");
    let path = mcp::config_path(&destination);
    let entry = json!({"url":"https://mcp.context7.com/mcp","directTools":false,"headers":{"Authorization":"Bearer private-sentinel"},"lifecycle":"lazy"});
    write_json(&path, json!({"mcpServers":{"context7":entry}}));
    let before = fs::read(&path).unwrap();
    let stub = Stub::new(&destination.home);
    mcp::install(mcp::Server::Context7, &destination, &stub).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(ownership::InstallState::load(&destination.home)
        .unwrap()
        .resources
        .is_empty());
    write_json(&path, json!({}));
    mcp::install(mcp::Server::Context7, &destination, &stub).unwrap();
    let state = ownership::InstallState::load(&destination.home).unwrap();
    let receipt = &state.resources["mcp-server:context7"].receipts[0];
    write_json(&path, json!({"mcpServers":{"context7":entry}}));
    assert_eq!(
        uninstall::receipt_status(receipt),
        uninstall::ReceiptStatus::Modified
    );
    if let ownership::Receipt::McpEntry { path, name, digest } = receipt {
        assert!(mcp::remove_entry(path, name, digest).is_err());
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir_all(destination.home).unwrap();
}

#[cfg(unix)]
#[test]
fn context7_cli_add_and_status_use_context7_identity() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let destination = destination("context7-cli", SkillScope::Global);
    adapter(&destination.home, "2.33.0");
    write_json(
        &destination.home.join(".pi/agent/settings.json"),
        json!({"theme":"keep", "packages":["npm:pi-mcp-adapter@2.33.0", "npm:@yassimba/pi-loom@latest"]}),
    );
    let bin = destination.home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let pi = bin.join("pi");
    fs::write(
        &pi,
        "#!/bin/sh\n[ \"$1\" = list ] || exit 91\nprintf 'User packages:\\n  npm:pi-mcp-adapter@2.33.0\\n  npm:@yassimba/pi-loom@latest\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&pi, fs::Permissions::from_mode(0o755)).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_loom"))
            .args(args)
            .env("HOME", &destination.home)
            .env("USERPROFILE", &destination.home)
            .env("XDG_CONFIG_HOME", destination.home.join(".config"))
            .env("LOOM_REPO_DIR", common::repo_root())
            .env_remove("PI_CODING_AGENT_DIR")
            .env_remove("PI_MCP_CONFIG_MODE")
            .env_remove("LOOM_BOOTSTRAP")
            .env("PATH", &bin)
            .current_dir(&destination.project_root)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let output = run(&["add", "--mcp-server", "context7", "--agent", "pi", "--yes"]);
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
    assert!(!destination.home.join(".config/mise").exists());
    let state = ownership::InstallState::load(&destination.home).unwrap();
    assert!(state.resources.contains_key("mcp-server:context7"));
    let status = run(&["status"]);
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(
        text.lines()
            .any(|line| line.contains("context7") && line.contains("gateway configured")),
        "{text}"
    );
    fs::remove_dir_all(destination.home).unwrap();
}

#[test]
fn context7_and_skills_serialize_shared_ownership_transactions() {
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
            if command
                .args
                .get(1)
                .is_some_and(|spec| spec == "npm:hold-pi-lane")
            {
                let _ = self
                    .gate
                    .wait_timeout_while(
                        self.skills_started.lock().unwrap(),
                        Duration::from_millis(100),
                        |started| !*started,
                    )
                    .unwrap();
                return Ok(CommandResult {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                });
            }
            let mut result = self.stub.run(command)?;
            if command.program == "pi" && command.args.first().is_some_and(|arg| arg == "list") {
                result.stdout.push_str("  npm:hold-pi-lane\n");
            }
            Ok(result)
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
    plan.steps
        .retain(|step| step.target != "pi-package:pi-mcp-adapter");
    plan.steps.insert(
        0,
        loom::InstallStep {
            target: "pi-package:hold-pi-lane".into(),
            operation: loom::Operation::PiPackage {
                spec: "npm:hold-pi-lane".into(),
                name: "hold-pi-lane".into(),
                project: false,
            },
        },
    );
    plan.steps.push(loom::InstallStep {
        target: "skill:i-have-adhd".into(),
        operation: loom::Operation::Skills {
            skills: vec!["i-have-adhd".into()],
            destination: d.clone(),
        },
    });
    let system = OrderedSystem {
        stub: Stub::new(&d.home),
        skills_started: Mutex::new(false),
        gate: Condvar::new(),
    };
    let report = loom::execute_attempt(
        &plan,
        &system,
        &AtomicBool::new(false),
        &[],
        &mut |index, status| {
            if matches!(plan.steps[index].operation, loom::Operation::Skills { .. })
                && status == loom::StepStatus::Running
            {
                *system.skills_started.lock().unwrap() = true;
                system.gate.notify_all();
                let state = ownership::InstallState::load(&d.home).unwrap();
                assert!(
                    state.resources.contains_key("mcp-server:context7"),
                    "skills started before the MCP ownership commit"
                );
            }
        },
    );
    assert!(report.failures.is_empty(), "{report:?}");
    let state = ownership::InstallState::load(&d.home).unwrap();
    assert!(state.resources.contains_key("mcp-server:context7"));
    assert!(state.resources["pi-package:i-have-adhd"]
        .receipts
        .iter()
        .any(|receipt| matches!(receipt, ownership::Receipt::PiSkillExclusion { .. })));
    fs::remove_dir_all(d.home).unwrap();
}
