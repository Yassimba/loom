use loom::{
    build_uninstall_plan, execute_uninstall_plan, CommandResult, CommandSpec, InstallState,
    OwnedResource, OwnershipScope, Receipt, ReceiptStatus, System, UninstallRequest,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn owned(
    id: &str,
    scope: OwnershipScope,
    dependencies: &[&str],
    receipt: Receipt,
) -> OwnedResource {
    OwnedResource {
        id: id.into(),
        scope,
        depends_on: dependencies.iter().map(ToString::to_string).collect(),
        receipts: vec![receipt],
    }
}

fn manager(id: &str) -> Receipt {
    Receipt::Manager {
        manager: "test".into(),
        target: id.into(),
    }
}

fn state(resources: Vec<OwnedResource>) -> InstallState {
    InstallState {
        schema_version: 1,
        resources: resources
            .into_iter()
            .map(|resource| (resource.id.clone(), resource))
            .collect::<BTreeMap<_, _>>(),
    }
}

struct FakeSystem {
    home: PathBuf,
    commands: Mutex<Vec<String>>,
}

impl System for FakeSystem {
    fn command_exists(&self, _name: &str) -> bool {
        true
    }

    fn refresh_path(&self) {}

    fn run(&self, command: &CommandSpec) -> anyhow::Result<CommandResult> {
        let shown = command.display();
        self.commands.lock().unwrap().push(shown.clone());
        Ok(CommandResult {
            success: !shown.contains("broken"),
            stdout: if shown == "pi list" {
                "npm:pi-markdown-preview".into()
            } else {
                String::new()
            },
            stderr: if shown.contains("broken") {
                "nope".into()
            } else {
                String::new()
            },
        })
    }

    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }
}

fn temp_home(label: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!(
        "loom-uninstall-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&home).unwrap();
    home
}

#[test]
fn keeping_a_resource_locks_its_transitive_dependencies() {
    let state = state(vec![
        owned(
            "pi-package:chat",
            OwnershipScope::Global,
            &["tool:pi"],
            manager("chat"),
        ),
        owned(
            "tool:pi",
            OwnershipScope::Global,
            &["core:node"],
            manager("pi"),
        ),
        owned("core:node", OwnershipScope::Global, &[], manager("node")),
        owned("skill:tdd", OwnershipScope::Global, &[], manager("tdd")),
    ]);

    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest {
            selected: Some(vec![
                "tool:pi".into(),
                "core:node".into(),
                "skill:tdd".into(),
            ]),
            force_modified: false,
        },
        Path::new("/work/current"),
        |_| ReceiptStatus::Clean,
    )
    .unwrap();

    assert_eq!(plan.remove_ids(), vec!["skill:tdd"]);
    assert_eq!(
        plan.locked,
        BTreeMap::from([
            (
                "core:node".into(),
                "required by kept pi-package:chat".into()
            ),
            ("tool:pi".into(), "required by kept pi-package:chat".into()),
        ])
    );
}

#[test]
fn another_projects_resources_are_hidden_and_keep_their_dependencies() {
    let state = state(vec![
        owned(
            "project-skill:other:tdd",
            OwnershipScope::Project {
                root: PathBuf::from("/work/other"),
            },
            &["core:loom"],
            manager("other"),
        ),
        owned("core:loom", OwnershipScope::Global, &[], manager("loom")),
        owned(
            "skill:local",
            OwnershipScope::Project {
                root: PathBuf::from("/work/current"),
            },
            &[],
            manager("local"),
        ),
    ]);

    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work/current"),
        |_| ReceiptStatus::Clean,
    )
    .unwrap();

    assert_eq!(plan.visible, vec!["core:loom", "skill:local"]);
    assert_eq!(plan.hidden, vec!["project-skill:other:tdd"]);
    assert_eq!(plan.remove_ids(), vec!["skill:local"]);
    assert_eq!(
        plan.locked.get("core:loom").map(String::as_str),
        Some("required by kept project-skill:other:tdd")
    );
}

#[test]
fn modified_receipts_are_preserved_until_forced() {
    let state = state(vec![owned(
        "skill:tdd",
        OwnershipScope::Global,
        &[],
        manager("tdd"),
    )]);

    let preserved = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work"),
        |_| ReceiptStatus::Modified,
    )
    .unwrap();
    assert!(preserved.steps.is_empty());
    assert_eq!(preserved.modified_preserved, vec!["skill:tdd"]);

    let forced = build_uninstall_plan(
        &state,
        &UninstallRequest {
            selected: None,
            force_modified: true,
        },
        Path::new("/work"),
        |_| ReceiptStatus::Modified,
    )
    .unwrap();
    assert_eq!(forced.remove_ids(), vec!["skill:tdd"]);
}

#[test]
fn modified_preservation_also_locks_dependencies() {
    let state = state(vec![
        owned(
            "skill:edited",
            OwnershipScope::Global,
            &["core:loom"],
            manager("edited"),
        ),
        owned("core:loom", OwnershipScope::Global, &[], manager("loom")),
    ]);

    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work"),
        |receipt| match receipt {
            Receipt::Manager { target, .. } if target == "edited" => ReceiptStatus::Modified,
            _ => ReceiptStatus::Clean,
        },
    )
    .unwrap();

    assert_eq!(plan.modified_preserved, ["skill:edited"]);
    assert_eq!(
        plan.locked.get("core:loom").map(String::as_str),
        Some("required by kept skill:edited")
    );
    assert!(plan.steps.is_empty());
}

#[test]
fn executor_rechecks_content_before_deleting() {
    let home = temp_home("race");
    let path = home.join("skill");
    std::fs::write(&path, "owned").unwrap();
    let receipt = Receipt::Path {
        path: path.clone(),
        path_kind: loom::OwnedPathKind::File,
        digest: loom::digest_path(&path).unwrap(),
        before: None,
    };
    let mut state = state(vec![owned(
        "skill:tdd",
        OwnershipScope::Global,
        &[],
        receipt,
    )]);
    state.save(&home).unwrap();
    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work"),
        |_| ReceiptStatus::Clean,
    )
    .unwrap();
    std::fs::write(&path, "edited after review").unwrap();
    let system = FakeSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };

    let report = execute_uninstall_plan(&plan, &mut state, &home, &system, &AtomicBool::new(false));

    assert_eq!(report.failures.len(), 1);
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "edited after review"
    );
    assert!(state.resources.contains_key("skill:tdd"));
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn executor_removes_dependents_first_and_keeps_failed_receipts() {
    let home = temp_home("executor");
    let mut state = state(vec![
        owned(
            "tool:pi",
            OwnershipScope::Global,
            &["foundation:node"],
            manager("pi"),
        ),
        owned(
            "foundation:node",
            OwnershipScope::Global,
            &[],
            manager("broken-node"),
        ),
    ]);
    state.save(&home).unwrap();
    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work"),
        |_| ReceiptStatus::Clean,
    )
    .unwrap();
    let system = FakeSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };

    let report = execute_uninstall_plan(&plan, &mut state, &home, &system, &AtomicBool::new(false));

    assert_eq!(
        system.commands.into_inner().unwrap(),
        vec!["test uninstall pi", "test uninstall broken-node",]
    );
    assert_eq!(report.removed, vec!["tool:pi"]);
    assert_eq!(report.failures[0].target, "foundation:node");
    assert!(!state.resources.contains_key("tool:pi"));
    assert!(state.resources.contains_key("foundation:node"));
    assert_eq!(InstallState::load(&home).unwrap(), state);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn pi_uninstall_uses_a_package_source() {
    let home = temp_home("pi-source");
    let mut state = state(vec![owned(
        "pi-package:preview",
        OwnershipScope::Global,
        &[],
        Receipt::Manager {
            manager: "pi".into(),
            target: "pi-markdown-preview".into(),
        },
    )]);
    state.save(&home).unwrap();
    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work"),
        |_| ReceiptStatus::Clean,
    )
    .unwrap();
    let system = FakeSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };

    let report = execute_uninstall_plan(&plan, &mut state, &home, &system, &AtomicBool::new(false));

    assert_eq!(
        system.commands.into_inner().unwrap(),
        vec!["pi list", "pi uninstall npm:pi-markdown-preview"]
    );
    assert_eq!(report.removed, vec!["pi-package:preview"]);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn clean_file_receipts_restore_the_previous_content_and_missing_receipts_prune() {
    let home = temp_home("file-inverse");
    let clean_path = home.join("settings.json");
    std::fs::write(&clean_path, "loom value").unwrap();
    let missing_path = home.join("missing.txt");
    let mut state = state(vec![
        owned(
            "setting:editor",
            OwnershipScope::Global,
            &[],
            Receipt::Path {
                path: clean_path.clone(),
                path_kind: loom::OwnedPathKind::File,
                digest: loom::digest_path(&clean_path).unwrap(),
                before: Some("user value".into()),
            },
        ),
        owned(
            "project:init",
            OwnershipScope::Global,
            &[],
            Receipt::Path {
                path: missing_path,
                path_kind: loom::OwnedPathKind::File,
                digest: "unused".into(),
                before: None,
            },
        ),
    ]);
    state.save(&home).unwrap();
    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work"),
        loom::receipt_status,
    )
    .unwrap();
    let system = FakeSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };

    let report = execute_uninstall_plan(&plan, &mut state, &home, &system, &AtomicBool::new(false));

    assert_eq!(std::fs::read_to_string(clean_path).unwrap(), "user value");
    assert_eq!(report.missing_pruned, vec!["project:init"]);
    assert!(state.resources.is_empty());
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn executor_persists_each_successful_receipt_before_continuing() {
    let home = temp_home("receipt-checkpoint");
    let resource = OwnedResource {
        id: "pi-package:mixed".into(),
        scope: OwnershipScope::Global,
        depends_on: Vec::new(),
        receipts: vec![manager("good"), manager("broken")],
    };
    let mut state = state(vec![resource]);
    state.save(&home).unwrap();
    let plan = build_uninstall_plan(
        &state,
        &UninstallRequest::default(),
        Path::new("/work"),
        |_| ReceiptStatus::Clean,
    )
    .unwrap();
    let system = FakeSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };

    let report = execute_uninstall_plan(&plan, &mut state, &home, &system, &AtomicBool::new(false));

    assert_eq!(report.failures.len(), 1);
    assert_eq!(
        state.resources["pi-package:mixed"].receipts,
        vec![manager("broken")]
    );
    assert_eq!(InstallState::load(&home).unwrap(), state);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn unknown_or_unowned_selector_fails() {
    let state = state(vec![owned(
        "skill:tdd",
        OwnershipScope::Global,
        &[],
        manager("tdd"),
    )]);

    let error = build_uninstall_plan(
        &state,
        &UninstallRequest {
            selected: Some(vec!["skill:nope".into()]),
            force_modified: false,
        },
        Path::new("/work"),
        |_| ReceiptStatus::Clean,
    )
    .unwrap_err();

    assert_eq!(error.to_string(), "skill:nope is not owned by Loom here");
}
