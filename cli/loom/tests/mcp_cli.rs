mod common;

use loom::testing::{ok, ScriptedSystem};
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

// Pi reads mcp.json with JSON.parse, so generated files must be strict JSON.
fn read_generated_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn stub_system(home: &Path) -> ScriptedSystem {
    pi_stub(home, "1.0.0")
}

fn pi_stub(home: &Path, pi_version: &str) -> ScriptedSystem {
    ScriptedSystem::new()
        .home(home)
        .cwd(home.join("project"))
        .on("pi list", ok("User packages:\n"))
        .on("pi --version", ok(format!("{pi_version}\n")))
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
        let stub = stub_system(&destination.home);
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
            "args": ["--tool-profile=analysis"]
        }),
        "tool:codebase-memory-mcp",
    );
    let destination = destination(name, SkillScope::Global);
    let stub = stub_system(&destination.home);

    mcp::install(server, &destination, &stub).unwrap();

    let config = read_generated_json(&mcp::config_path(&destination));
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
    let path = mcp::config_path(&destination);
    let binary = destination.home.join("bin").join(name);
    assert!(binary.is_absolute());
    write_json(
        &path,
        json!({"mcpServers": {(name): {
            "command": binary,
            "args": args
        }}}),
    );
    let stub = stub_system(&destination.home);
    assert!(mcp::configured(server, &destination, &stub));

    let missing = stub_system(&destination.home).without(name);
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
        json!({"command": name, "args": valid_args.clone(), "enabled": false}),
        json!({"command": name, "args": valid_args.clone(), "exposure": "hidden"}),
        json!({"command": name, "args": valid_args.clone(), "url": "https://private.invalid"}),
        json!({"command": format!("./{name}"), "args": valid_args.clone()}),
        json!({"command": format!("bin/{name}"), "args": valid_args.clone()}),
    ] {
        let destination = destination(&format!("{name}-conflict"), SkillScope::Global);
        let path = mcp::config_path(&destination);
        write_json(&path, json!({"mcpServers": {(name): entry}}));
        let before = fs::read(&path).unwrap();
        assert!(mcp::preflight(server, &destination).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
        fs::remove_dir_all(destination.home).unwrap();
    }
}

#[test]
fn context7_rejects_pi_mcp_adapter_and_disabled_builtin_mcp() {
    for settings in [
        json!({"packages":["npm:pi-mcp-adapter@2.32.1"]}),
        json!({"packages":[{"source":"npm:pi-mcp-adapter","extensions":[]}]}),
        json!({"extensions":["-builtin:mcp"]}),
    ] {
        let destination = destination("mcp-builtin-blocked", SkillScope::Global);
        write_json(&destination.home.join(".pi/agent/settings.json"), settings);
        assert!(mcp::preflight(mcp::Server::Context7, &destination).is_err());
        fs::remove_dir_all(destination.home).unwrap();
    }
    let destination = destination("mcp-project-adapter", SkillScope::Project);
    write_json(
        &destination.project_root.join(".pi/settings.json"),
        json!({"packages":["npm:pi-mcp-adapter"]}),
    );
    assert!(mcp::preflight(mcp::Server::Context7, &destination)
        .unwrap_err()
        .to_string()
        .contains("loom update"));
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
    let path = mcp::config_path(&destination);
    let pending = path.with_file_name(".mcp.json.loom-old");
    write_json(&pending, json!({"mcpServers":{"other":{"command":"keep"}}}));
    let before = fs::read(&pending).unwrap();
    plan(&destination);
    assert!(!path.exists());
    assert_eq!(fs::read(&pending).unwrap(), before);
    let stub = stub_system(&destination.home);
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
fn context7_plan_configures_and_uninstalls_each_scope() {
    for scope in [SkillScope::Global, SkillScope::Project] {
        let destination = destination("context7-install", scope);
        let plan = plan(&destination);
        assert_eq!(plan.prerequisite_count(), 0);
        assert_eq!(
            plan.resources()
                .map(|step| step.target.as_str())
                .collect::<Vec<_>>(),
            ["mcp-server:context7"]
        );
        let path = mcp::config_path(&destination);
        write_json(&path, json!({"mcpServers":{"other":{"command":"keep"}}}));
        let stub = stub_system(&destination.home);
        let report = common::install(&plan, &stub);
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(report.installed.contains(&"mcp-server:context7".into()));
        let after = fs::read(&path).unwrap();
        let config: serde_json::Value = serde_json::from_slice(&after).unwrap();
        assert_eq!(
            config["mcpServers"]["context7"],
            json!({"url":"https://mcp.context7.com/mcp"})
        );
        assert!(!destination.home.join(".config/mise").exists());
        assert!(mcp::configured(mcp::Server::Context7, &destination, &stub));
        mcp::install(mcp::Server::Context7, &destination, &stub).unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
        let mut state = ownership::InstallState::load(&destination.home).unwrap();
        let owned = state.resources.values().next().unwrap();
        assert!(owned.id.ends_with("mcp-server:context7"));
        assert!(!owned
            .depends_on
            .iter()
            .any(|dependency| dependency.contains("mcp-adapter")));
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
        assert_eq!(config, json!({"mcpServers":{"other":{"command":"keep"}}}));
        fs::remove_dir_all(destination.home).unwrap();
    }
}

#[test]
fn context7_conflicts_stop_before_commands_and_preserve_secrets() {
    for value in [
        json!({"url":"https://private.invalid/TOKEN"}),
        json!({"url":"https://mcp.context7.com/mcp","enabled":false}),
        json!({"url":"https://mcp.context7.com/mcp","exposure":"hidden"}),
        json!({"url":"https://mcp.context7.com/mcp","command":"other"}),
    ] {
        let destination = destination("context7-conflict", SkillScope::Project);
        let plan = plan(&destination);
        let path = mcp::config_path(&destination);
        write_json(&path, json!({"mcpServers":{"context7":value}}));
        let before = fs::read(&path).unwrap();
        let stub = stub_system(&destination.home);
        let report = common::install(&plan, &stub);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].target, "mcp-server:context7");
        assert!(!format!("{report:?}").contains("TOKEN"));
        assert!(stub.calls().is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(destination.home).unwrap();
    }
}

#[test]
fn context7_preserves_user_auth_and_modified_owned_config() {
    let destination = destination("context7-auth", SkillScope::Global);
    let path = mcp::config_path(&destination);
    let entry = json!({"url":"https://mcp.context7.com/mcp","headers":{"Authorization":"Bearer private-sentinel"},"exposure":"direct"});
    write_json(&path, json!({"mcpServers":{"context7":entry}}));
    let before = fs::read(&path).unwrap();
    let stub = stub_system(&destination.home);
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
    write_json(
        &destination.home.join(".pi/agent/settings.json"),
        json!({"theme":"keep", "packages":["npm:@yassimba/pi-loom@latest"]}),
    );
    let bin = destination.home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let pi = bin.join("pi");
    fs::write(
        &pi,
        "#!/bin/sh\n[ \"$1\" = list ] || exit 91\nprintf 'User packages:\\n  npm:@yassimba/pi-loom@latest\\n'\n",
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
            .any(|line| line.contains("context7") && line.contains("configured; live health")),
        "{text}"
    );
    fs::remove_dir_all(destination.home).unwrap();
}

#[test]
fn context7_and_skills_serialize_shared_ownership_transactions() {
    use std::sync::Condvar;
    use std::time::Duration;
    struct OrderedSystem {
        stub: ScriptedSystem,
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
    let bundled = loom::Catalog::embedded()
        .unwrap()
        .find(&["pi-package:i-have-adhd".into()])
        .unwrap()
        .remove(0);
    write_json(
        &d.home.join(".pi/agent/settings.json"),
        json!({"packages":[bundled.pi_install_spec()]}),
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
        stub: stub_system(&d.home),
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

#[test]
fn update_migrates_adapter_configs_receipts_and_package() {
    let destination = destination("mcp-migrate", SkillScope::Global);
    let (home, agent) = (&destination.home, destination.home.join(".pi/agent"));
    // Adapter 2.x: Loom wrote mcp.json with directTools and recorded the adapter.
    let old_entry = json!({"url":"https://mcp.context7.com/mcp","directTools":false});
    write_json(
        &agent.join("settings.json"),
        json!({"packages":["npm:pi-mcp-adapter@2.32.1"]}),
    );
    write_json(
        &agent.join("mcp.json"),
        json!({"mcpServers":{"context7":old_entry}}),
    );
    let mut state = ownership::InstallState::load(home).unwrap();
    for (id, depends_on, receipts) in [
        ("pi-package:pi-mcp-adapter", vec![], vec![]),
        (
            "mcp-server:context7",
            vec!["tool:pi".into(), "pi-package:pi-mcp-adapter".into()],
            vec![ownership::Receipt::McpEntry {
                path: agent.join("mcp.json"),
                name: "context7".into(),
                digest: mcp::entry_digest(&old_entry),
            }],
        ),
    ] {
        state.record(ownership::OwnedResource {
            id: id.into(),
            scope: ownership::OwnershipScope::Global,
            depends_on,
            receipts,
        });
    }
    state.save(home).unwrap();
    // Adapter 3.x: servers moved to mcp-adapter.json, with a mise-pinned path.
    write_json(
        &agent.join("mcp-adapter.json"),
        json!({"mcpServers":{"memory":{
            "command": home.join(".local/share/mise/installs/npm-codebase-memory-mcp/0.10.8/bin/codebase-memory-mcp"),
            "directTools": ["trace_path"]
        }}}),
    );

    let old_pi = pi_stub(home, "0.87.1");
    let project = destination.project_root.clone();
    assert!(loom::mcp_migration::migrate_adapter(&old_pi, home, &project).is_err());
    assert!(agent.join("mcp-adapter.json").is_file());

    let stub = stub_system(home);
    let detail = loom::mcp_migration::migrate_adapter(&stub, home, &project)
        .unwrap()
        .unwrap();
    assert!(detail.contains("pi-mcp-adapter removed"), "{detail}");
    assert_eq!(
        read_generated_json(&agent.join("mcp.json")),
        json!({"mcpServers":{
            "context7":{"url":"https://mcp.context7.com/mcp"},
            "memory":{"command":"codebase-memory-mcp","toolExposure":{"trace_path":"direct"}}
        }})
    );
    assert!(agent.join("mcp-adapter.json.loom-migrated").is_file());
    assert!(stub
        .shown()
        .contains(&"pi remove npm:pi-mcp-adapter".into()));
    let state = ownership::InstallState::load(home).unwrap();
    assert!(!state.resources.contains_key("pi-package:pi-mcp-adapter"));
    assert_eq!(
        state.resources["mcp-server:context7"].depends_on,
        ["tool:pi"]
    );
    let receipt = &state.resources["mcp-server:context7"].receipts[0];
    assert_eq!(
        uninstall::receipt_status(receipt),
        uninstall::ReceiptStatus::Clean
    );

    // Settings still list the adapter in this stub; a migrated config is a no-op.
    write_json(&agent.join("settings.json"), json!({"packages":[]}));
    assert!(loom::mcp_migration::migrate_adapter(&stub, home, &project)
        .unwrap()
        .is_none());
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn update_repoints_receipts_stranded_on_the_migrated_adapter_file() {
    let destination = destination("mcp-stranded", SkillScope::Global);
    let (home, agent) = (&destination.home, destination.home.join(".pi/agent"));
    // An older migration renamed mcp-adapter.json but kept an edited entry's receipt on it.
    write_json(
        &agent.join("mcp.json"),
        json!({"mcpServers":{"context7":{"url":"https://mcp.context7.com/mcp"}}}),
    );
    let mut state = ownership::InstallState::load(home).unwrap();
    state.record(ownership::OwnedResource {
        id: "mcp-server:context7".into(),
        scope: ownership::OwnershipScope::Global,
        depends_on: vec!["tool:pi".into()],
        receipts: vec![ownership::Receipt::McpEntry {
            path: agent.join("mcp-adapter.json"),
            name: "context7".into(),
            digest: "edited".into(),
        }],
    });
    // Serena left the catalog and its entry is gone: nothing left to own.
    state.record(ownership::OwnedResource {
        id: "mcp-server:serena".into(),
        scope: ownership::OwnershipScope::Global,
        depends_on: vec!["tool:pi".into()],
        receipts: vec![ownership::Receipt::McpEntry {
            path: agent.join("mcp-adapter.json"),
            name: "serena".into(),
            digest: "old".into(),
        }],
    });
    state.save(home).unwrap();

    let project = destination.project_root.clone();
    let detail = loom::mcp_migration::migrate_adapter(&stub_system(home), home, &project)
        .unwrap()
        .unwrap();
    assert!(detail.contains("2 MCP receipt"), "{detail}");
    assert!(detail.contains("1 retired MCP server"), "{detail}");
    let state = ownership::InstallState::load(home).unwrap();
    assert!(!state.resources.contains_key("mcp-server:serena"));
    let ownership::Receipt::McpEntry { path, .. } =
        &state.resources["mcp-server:context7"].receipts[0]
    else {
        panic!("expected an MCP receipt");
    };
    assert_eq!(path, &agent.join("mcp.json"));
    fs::remove_dir_all(home).unwrap();
}
