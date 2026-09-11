use crate::ui::{tidy_path, Mark, Out};
use crate::{skills, Catalog, CommandSpec, NodeStatus, ResourceKind, System};

/// One independent update lane; lanes run concurrently and report whole
/// blocks so nothing interleaves.
pub struct Lane {
    pub ok: bool,
    pub label: &'static str,
    pub detail: String,
    pub notes: Vec<String>,
}

impl Lane {
    fn ok(label: &'static str, detail: impl Into<String>) -> Self {
        Self {
            ok: true,
            label,
            detail: detail.into(),
            notes: Vec::new(),
        }
    }

    fn failed(label: &'static str, detail: impl Into<String>) -> Self {
        Self {
            ok: false,
            label,
            detail: crate::ui::failure_text(&detail.into()),
            notes: Vec::new(),
        }
    }
}

struct CommandLane {
    label: &'static str,
    detail: String,
    commands: Vec<CommandSpec>,
}

fn progress_status(completed: usize, total: usize, running: &[&str], elapsed: u64) -> String {
    let active = running.join(" + ");
    format!("{completed}/{total} complete · {active} · {elapsed}s")
}

fn pi_package_commands(catalog: &Catalog, listed: &str, native_windows: bool) -> Vec<CommandSpec> {
    catalog
        .resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::PiPackage && resource.group != "Wiki")
        // MCP setup preserves the shared gateway; a catalog reinstall would
        // downgrade compatible newer installs to the setup prerequisite pin.
        .filter(|resource| resource.install_target != "pi-mcp-adapter")
        .filter(|resource| !native_windows || !resource.windows_wsl)
        .flat_map(|resource| {
            let spec = resource.pi_install_spec();
            let global =
                crate::install::pi_package_installed(listed, &resource.install_target, false)
                    .then(|| CommandSpec::new("pi", ["install", &spec]));
            let local =
                crate::install::pi_package_installed(listed, &resource.install_target, true)
                    .then(|| CommandSpec::new("pi", ["install", "-l", &spec]));
            [global, local].into_iter().flatten()
        })
        .collect()
}

/// How `loom update` should treat Herdr on this machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HerdrGate {
    /// Herdr is not installed.
    None,
    /// This process is a Herdr pane; replacing Herdr here breaks the session.
    Inside,
    /// A Herdr server is running outside this process; closing it first is safe.
    StopServer,
    /// No running Herdr to protect.
    Ready,
}

pub fn herdr_gate(present: bool, inside: bool, server_running: bool) -> HerdrGate {
    if !present {
        HerdrGate::None
    } else if inside {
        HerdrGate::Inside
    } else if server_running {
        HerdrGate::StopServer
    } else {
        HerdrGate::Ready
    }
}

/// `herdr status --json` or `herdr status server --json`.
pub fn herdr_server_running(status_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(status_json) else {
        return false;
    };
    value
        .pointer("/server/running")
        .or_else(|| value.get("running"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub fn probe_herdr_server_running(system: &dyn System) -> bool {
    let Ok(result) = system.run_probe(&CommandSpec::new("herdr", ["status", "--json"])) else {
        return false;
    };
    result.success && herdr_server_running(&result.stdout)
}

pub const HERDR_SKIP_INSIDE: &str = "skipped · run `loom update` from a regular terminal";
pub const HERDR_SKIP_SERVER: &str =
    "skipped · close the server with `herdr server stop`, then rerun";

/// Plugin lane: run it, or skip with a report line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HerdrLane {
    Run,
    Skip(&'static str),
}

pub fn run_updates(system: &(dyn System + Sync), catalog: &Catalog, herdr: HerdrLane) -> bool {
    let out = Out::detect();
    out.title("update", concat!("v", env!("CARGO_PKG_VERSION")));

    // Warn-only: loom never installs or updates Node itself, but a Node
    // below Pi's floor is worth flagging before Pi's own update runs. A
    // missing Node stays silent here — there is nothing installed to age.
    let node = NodeStatus::detect(system);
    if matches!(node, NodeStatus::TooOld(..)) {
        if let Some(warning) = node.warning() {
            out.row(Mark::Bad, "Node", warning);
        }
    }

    let mut tasks = Vec::new();
    let mut inventory_error = None;
    let mut pi_compat_targets = Vec::new();
    if system.command_exists("pi") {
        // Cataloged reinstalls instead of `pi update --all`: external packages
        // use their pin, first-party packages request npm's latest release, and
        // Pi itself is the mise manifest's job. Packages outside the catalog
        // are left alone.
        let listed = match system.run_probe(&CommandSpec::new("pi", ["list"])) {
            Ok(result) if result.success => result.stdout,
            Ok(result) => {
                inventory_error = Some(crate::install::command_failure_message(&result));
                String::new()
            }
            Err(error) => {
                inventory_error = Some(error.to_string());
                String::new()
            }
        };
        let commands = pi_package_commands(catalog, &listed, cfg!(windows));
        pi_compat_targets = catalog
            .resources
            .iter()
            .filter(|resource| {
                resource.group != "Wiki" && crate::pi_compat::is_managed(&resource.id)
            })
            .filter(|resource| {
                crate::install::pi_package_installed(&listed, &resource.install_target, false)
            })
            .map(|resource| resource.id.clone())
            .collect();
        if !commands.is_empty() {
            tasks.push(CommandLane {
                label: "Pi packages",
                detail: format!("{} refreshed", commands.len()),
                commands,
            });
        }
    }
    let skipped_herdr = match herdr {
        HerdrLane::Run if system.command_exists("herdr") => {
            tasks.push(CommandLane {
                label: "Herdr",
                detail: "plugins".into(),
                commands: vec![CommandSpec::new("herdr", ["plugin", "update", "--all"])],
            });
            None
        }
        HerdrLane::Skip(reason) => Some(reason),
        HerdrLane::Run => None,
    };
    // The manifest lane owns tool updates, including this CLI's own pin.
    // Loom is only ever installed through mise, so a missing mise means the
    // bootstrap was undone; point back at it instead of self-updating.
    let mise = system.command_exists("mise");

    // Skills, projects, tools, Pi, and Herdr touch disjoint state, so every
    // lane runs at once; rows print in a fixed order once all are done.
    let repository = &skills::Repository::default();
    let active_details = &std::sync::Mutex::new(std::collections::BTreeMap::new());
    type Job<'a> = Box<dyn FnOnce() -> Lane + Send + 'a>;
    let mut jobs: Vec<(&'static str, Job<'_>)> = vec![
        (
            "Skills",
            Box::new(move || update_installed_skills(system, catalog, repository)),
        ),
        (
            "Projects",
            Box::new(move || sync_projects_lane(system, repository)),
        ),
    ];
    if mise {
        jobs.push((
            "Tools",
            Box::new(move || sync_tool_manifest(system, repository)),
        ));
    } else {
        jobs.push((
            "Tools",
            Box::new(|| {
                Lane::failed(
                    "Tools",
                    "mise is not on PATH; rerun the installer from the README",
                )
            }),
        ));
    }
    for task in tasks {
        let label = task.label;
        jobs.push((
            label,
            Box::new(move || {
                run_command_lane(system, task, &|detail| {
                    active_details.lock().unwrap().insert(label, detail);
                })
            }),
        ));
    }
    // Rows keep a fixed order, so the report cannot stream; a status line
    // names the lanes still running instead, or the wait reads as a hang
    // (Pi reinstalls and the repo download take minutes together).
    let labels = jobs.iter().map(|(label, _)| *label).collect::<Vec<_>>();
    let (sender, results) = std::sync::mpsc::channel::<(usize, Lane)>();
    let mut lanes = inventory_error
        .into_iter()
        .map(|error| (labels.len() + 2, Lane::failed("Pi inventory", error)))
        .collect::<Vec<_>>();
    std::thread::scope(|scope| {
        for (index, (_, job)) in jobs.into_iter().enumerate() {
            let sender = sender.clone();
            scope.spawn(move || {
                let _ = sender.send((index, job()));
            });
        }
        drop(sender);
        let mut running = labels.clone();
        let total = running.len();
        let started = std::time::Instant::now();
        let mut last_active = String::new();
        loop {
            match results.recv_timeout(std::time::Duration::from_millis(250)) {
                Ok((index, lane)) => {
                    running.retain(|label| *label != labels[index]);
                    lanes.push((index, lane));
                    if running.is_empty() {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            let details = active_details.lock().unwrap();
            let active_names = running
                .iter()
                .map(|label| {
                    details
                        .get(label)
                        .cloned()
                        .unwrap_or_else(|| (*label).to_string())
                })
                .collect::<Vec<_>>();
            drop(details);
            let active = active_names.join(" + ");
            if out.is_terminal() || active != last_active {
                out.progress(
                    progress_status(
                        total - running.len(),
                        total,
                        &active_names.iter().map(String::as_str).collect::<Vec<_>>(),
                        started.elapsed().as_secs(),
                    ),
                    (started.elapsed().as_millis() / 100) as usize,
                );
                last_active = active;
            }
        }
    });
    out.progress_done();
    if let Some(reason) = skipped_herdr {
        lanes.push((labels.len() + 4, Lane::ok("Herdr", reason)));
    }
    if !pi_compat_targets.is_empty() {
        lanes.push((
            labels.len(),
            reconcile_pi_compat(system, &pi_compat_targets),
        ));
    }
    // Package files are now stable; never reconcile during a reinstall or
    // after a failed Pi lane. Skills refresh and package installs run in parallel.
    if !lanes
        .iter()
        .any(|(_, lane)| lane.label == "Pi packages" && !lane.ok)
    {
        if let Some(home) = system.home_dir() {
            match crate::bundled_skills::reconcile_installed(&home) {
                Ok(notes) => {
                    for note in notes {
                        out.note(note);
                    }
                }
                Err(error) => lanes.push((labels.len() + 1, Lane::failed("Bundled skills", error))),
            }
        }
    }
    lanes.sort_by_key(|(index, _)| *index);

    let mut failed = 0;
    for (_, lane) in &lanes {
        let mark = if lane.ok { Mark::Ok } else { Mark::Bad };
        out.row(mark, lane.label, &lane.detail);
        for note in &lane.notes {
            out.note(note);
        }
        if !lane.ok {
            failed += 1;
        }
    }
    failed == 0
}

/// Refresh mise's conf.d copy of the published manifest and install its pins.
/// Tools move only when a new manifest landed on main since the last sync.
fn sync_tool_manifest(system: &dyn System, repository: &skills::Repository) -> Lane {
    let before = system
        .home_dir()
        .and_then(|home| std::fs::read(crate::manifest::conf_d_target(&home)).ok());
    match crate::manifest::sync_selected_from(
        system,
        &[],
        &std::sync::atomic::AtomicBool::new(false),
        repository,
    ) {
        Ok(target) => {
            let home = system.home_dir().unwrap_or_default();
            let current = std::fs::read(&target).ok();
            let state = if before.is_some() && before == current {
                "pins already current; installation checked"
            } else {
                "selection updated; installation checked"
            };
            Lane::ok("Tools", format!("{state} · {}", tidy_path(&target, &home)))
        }
        Err(message) => Lane::failed("Tools", message),
    }
}

fn reconcile_pi_compat(system: &dyn System, targets: &[String]) -> Lane {
    let mut changed = 0;
    for target in targets {
        match crate::pi_compat::apply_for_package(target, system) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(error) => return Lane::failed("Pi compatibility", error.to_string()),
        }
    }
    Lane::ok(
        "Pi compatibility",
        format!("{} verified · {changed} repaired", targets.len()),
    )
}

fn package_version(system: &dyn System, command: &CommandSpec) -> Option<String> {
    let spec = command.args.last()?.strip_prefix("npm:")?;
    let (name, _) = spec.rsplit_once('@')?;
    let root = if command.args.iter().any(|arg| arg == "-l") {
        system.current_dir()?.join(".pi")
    } else {
        std::env::var_os("PI_CODING_AGENT_DIR")
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| system.home_dir().map(|home| home.join(".pi/agent")))?
    };
    let content = std::fs::read(
        root.join("npm/node_modules")
            .join(name)
            .join("package.json"),
    )
    .ok()?;
    let package: serde_json::Value = serde_json::from_slice(&content).ok()?;
    package["version"]
        .as_str()
        .filter(|version| {
            version.len() <= 64
                && version.as_bytes().first().is_some_and(u8::is_ascii_digit)
                && version
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(&byte))
        })
        .map(str::to_owned)
}

fn run_command_lane(system: &dyn System, task: CommandLane, progress: &dyn Fn(String)) -> Lane {
    let mut lane = Lane::ok(task.label, task.detail);
    for (index, command) in task.commands.iter().enumerate() {
        let before = package_version(system, command);
        let target = if command.program == "pi" {
            command.args.last().map_or("package", String::as_str)
        } else {
            "plugins"
        };
        let scope = if command.args.iter().any(|arg| arg == "-l") {
            "this project"
        } else {
            "global"
        };
        progress(format!(
            "{}: installing {}/{} · {target} · {scope}",
            task.label,
            index + 1,
            task.commands.len()
        ));
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let result = system
            .run_controlled(command, crate::system::MANAGER_COMMAND_TIMEOUT, &cancelled)
            .map_err(|error| error.to_string())
            .and_then(|result| {
                if result.success {
                    Ok(())
                } else {
                    Err(crate::install::command_failure_message(&result))
                }
            });
        let result = result.and_then(|()| {
            progress(format!(
                "{}: verifying {}/{} · {target}",
                task.label,
                index + 1,
                task.commands.len()
            ));
            let probe = if command.program == "pi" {
                CommandSpec::new("pi", ["list"])
            } else {
                CommandSpec::new("herdr", ["plugin", "list"])
            };
            let result = system
                .run_probe(&probe)
                .map_err(|error| format!("verification failed: {error}"))?;
            if !result.success {
                return Err(format!(
                    "verification failed: {}",
                    crate::install::command_failure_message(&result)
                ));
            }
            if command.program == "pi"
                && !crate::install::pi_package_installed(&result.stdout, target, scope != "global")
            {
                return Err(
                    "verification did not find the selected package in its destination".into(),
                );
            }
            Ok(())
        });
        if let Err(error) = result {
            lane.ok = false;
            lane.detail = format!(
                "{index}/{} completed; remaining work not updated. {}",
                task.commands.len(),
                crate::ui::failure_text(&error)
            );
            lane.notes.push(format!("Failed: {target} · {scope}"));
            return lane;
        }
        let after = package_version(system, command);
        let outcome = match (before, after) {
            (Some(before), Some(after)) if before == after => format!("already current ({after})"),
            (Some(before), Some(after)) => format!("updated {before} → {after}"),
            (None, Some(after)) => format!("installed {after}"),
            _ => "refreshed; version not reported".into(),
        };
        lane.notes.push(format!("{target} · {scope} · {outcome}"));
    }
    lane
}

/// Refresh catalog skills in the exact global and current-project trees where
/// they already exist. Agent and scope choices remain stable across updates.
fn update_installed_skills(
    system: &dyn System,
    catalog: &Catalog,
    repository: &skills::Repository,
) -> Lane {
    match skills::refresh_installed_skills(system, repository, &catalog.resources) {
        Ok(reports) if reports.is_empty() => Lane::ok("Skills", "none installed"),
        Ok(reports) => {
            let home = system.home_dir().unwrap_or_default();
            let total: usize = reports.iter().map(|report| report.installed).sum();
            let unchanged: usize = reports.iter().map(|report| report.unchanged).sum();
            let mut lane = Lane::ok(
                "Skills",
                format!(
                    "{total} updated · {unchanged} already current across {} trees",
                    reports.len()
                ),
            );
            for report in reports {
                let mut notes = Vec::new();
                if report.skipped_existing > 0 {
                    notes.push(format!(
                        "{} unowned or modified, preserved",
                        report.skipped_existing
                    ));
                }
                if report.skipped_symlinks > 0 {
                    notes.push(format!("{} symlinked, left alone", report.skipped_symlinks));
                }
                let detail = if notes.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", notes.join(" · "))
                };
                lane.notes.push(format!(
                    "{}  {} updated · {} already current{detail}",
                    tidy_path(&report.tree, &home),
                    report.installed,
                    report.unchanged
                ));
            }
            lane
        }
        Err(message) => Lane::failed("Skills", message),
    }
}

fn sync_projects_lane(system: &dyn System, repository: &skills::Repository) -> Lane {
    let sync = crate::init::sync_projects_from(system, repository);
    Lane {
        ok: sync.ok,
        label: "Projects",
        detail: sync.summary,
        notes: sync.notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herdr_gate_skips_inside_and_offers_stop_when_the_server_is_up() {
        assert_eq!(herdr_gate(false, true, true), HerdrGate::None);
        assert_eq!(herdr_gate(true, true, true), HerdrGate::Inside);
        assert_eq!(herdr_gate(true, false, true), HerdrGate::StopServer);
        assert_eq!(herdr_gate(true, false, false), HerdrGate::Ready);
    }

    #[test]
    fn herdr_server_running_reads_status_json() {
        assert!(herdr_server_running(
            r#"{"server":{"status":"running","running":true}}"#
        ));
        assert!(herdr_server_running(r#"{"running":true}"#));
        assert!(!herdr_server_running(
            r#"{"server":{"status":"stopped","running":false}}"#
        ));
        assert!(!herdr_server_running("not json"));
    }

    #[test]
    fn package_verification_requires_the_exact_identity_and_destination() {
        let listed = "User packages:\n npm:pi-subagents-extra@1.0.0\n npm:@other/foo@1\nProject packages:\n npm:pi-subagents@0.66.0\n npm:@example/foo@1";
        assert!(!crate::install::pi_package_installed(
            listed,
            "pi-subagents",
            false
        ));
        assert!(crate::install::pi_package_installed(
            listed,
            "npm:pi-subagents@latest",
            true
        ));
        assert!(!crate::install::pi_package_installed(
            listed,
            "@example/foo",
            false
        ));
        assert!(crate::install::pi_package_installed(
            listed,
            "@example/foo",
            true
        ));
        assert!(!crate::install::pi_package_installed(
            "npm:pi-subagents",
            "pi-subagents",
            false
        ));
        let root = std::env::temp_dir().join(format!(
            "loom-path-pkg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package = root
            .join("github-agrici-daniel-claude-obsidian")
            .join("2.1.1");
        std::fs::create_dir_all(&package).unwrap();
        let listed = format!("Project packages:\n  {}\n", package.display());
        assert!(crate::install::pi_package_installed(
            &listed,
            "github:AgriciDaniel/claude-obsidian",
            true
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_reports_verified_versions_and_preserves_success_notes_on_failure() {
        struct PackageSystem {
            root: std::path::PathBuf,
            after: Option<&'static str>,
            wrong_scope: bool,
        }
        impl System for PackageSystem {
            fn command_exists(&self, _: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn home_dir(&self) -> Option<std::path::PathBuf> {
                Some(self.root.clone())
            }
            fn current_dir(&self) -> Option<std::path::PathBuf> {
                Some(self.root.clone())
            }
            fn run(&self, command: &CommandSpec) -> anyhow::Result<crate::CommandResult> {
                let failed = command
                    .args
                    .last()
                    .is_some_and(|arg| arg.contains("failing"));
                if command.args[0] == "install" && !failed {
                    if let Some(version) = self.after {
                        std::fs::write(
                            self.root
                                .join(".pi/npm/node_modules/@test/pkg/package.json"),
                            serde_json::to_vec(
                                &serde_json::json!({"name":"@test/pkg", "version":version}),
                            )?,
                        )?;
                    }
                }
                Ok(crate::CommandResult {
                    success: !failed,
                    stdout: if self.wrong_scope {
                        "User packages:\n npm:@test/pkg@2.0.0".into()
                    } else {
                        "Project packages:\n npm:@test/pkg@2.0.0".into()
                    },
                    stderr: if failed {
                        "ETIMEDOUT Authorization: Bearer SECRET".into()
                    } else {
                        String::new()
                    },
                })
            }
        }
        for (index, (before, after, fail_next, wrong_scope, expected)) in [
            (
                Some("1.0.0"),
                Some("2.0.0"),
                false,
                false,
                "updated 1.0.0 → 2.0.0",
            ),
            (
                Some("2.0.0"),
                Some("2.0.0"),
                false,
                false,
                "already current (2.0.0)",
            ),
            (None, Some("2.0.0"), false, false, "installed 2.0.0"),
            (None, None, false, false, "version not reported"),
            (
                None,
                Some("2.0.0\nSECRET"),
                false,
                false,
                "version not reported",
            ),
            (
                Some("1.0.0"),
                Some("2.0.0"),
                true,
                false,
                "updated 1.0.0 → 2.0.0",
            ),
            (
                Some("1.0.0"),
                Some("2.0.0"),
                false,
                true,
                "Installation could not be verified",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let root = std::env::temp_dir().join(format!(
                "loom-update-versions-{}-{index}",
                std::process::id()
            ));
            let package = root.join(".pi/npm/node_modules/@test/pkg/package.json");
            std::fs::create_dir_all(package.parent().unwrap()).unwrap();
            if let Some(version) = before {
                std::fs::write(
                    &package,
                    serde_json::to_vec(&serde_json::json!({"version":version})).unwrap(),
                )
                .unwrap();
            }
            let system = PackageSystem {
                root: root.clone(),
                after,
                wrong_scope,
            };
            let mut commands = vec![CommandSpec::new(
                "pi",
                ["install", "-l", "npm:@test/pkg@latest"],
            )];
            if fail_next {
                commands.push(CommandSpec::new(
                    "pi",
                    ["install", "-l", "npm:@test/failing@latest"],
                ));
            }
            let progress = std::cell::RefCell::new(Vec::new());
            let lane = run_command_lane(
                &system,
                CommandLane {
                    label: "Pi packages",
                    detail: "refresh".into(),
                    commands,
                },
                &|detail| progress.borrow_mut().push(detail),
            );
            assert_eq!(lane.ok, !fail_next && !wrong_scope);
            let text = format!("{}\n{}", lane.detail, lane.notes.join("\n"));
            assert!(text.contains(expected), "{text}");
            assert!(!text.contains("SECRET"), "{text}");
            assert!(text.contains("this project"), "{text}");
            assert!(progress
                .borrow()
                .iter()
                .any(|line| line.contains("installing 1/")));
            assert!(progress
                .borrow()
                .iter()
                .any(|line| line.contains("verifying 1/")));
            if fail_next {
                assert!(lane.detail.contains("1/2 completed"), "{text}");
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn progress_shows_measurable_completion_and_active_lanes() {
        assert_eq!(
            progress_status(2, 5, &["Tools", "Pi packages", "Herdr"], 12),
            "2/5 complete · Tools + Pi packages + Herdr · 12s"
        );
    }

    #[test]
    fn mcp_gateway_is_not_reinstalled_or_downgraded_by_updates() {
        let catalog = Catalog::embedded().unwrap();
        for version in ["2.32.1", "2.33.0"] {
            let listed = format!(
                "User packages:\n  npm:pi-mcp-adapter@{version}\nProject packages:\n  npm:pi-mcp-adapter@{version}\n"
            );
            assert!(pi_package_commands(&catalog, &listed, false).is_empty());
        }
    }

    #[test]
    fn pi_package_updates_preserve_user_and_project_scope() {
        let catalog = Catalog::embedded().unwrap();
        let listed = "User packages:\n  npm:pi-subagents\n  npm:@yassimba/pi-add-dir\n\nProject packages:\n  npm:pi-subagents\n  npm:@companion-ai/feynman@0.0.0\n";
        let commands = pi_package_commands(&catalog, listed, false)
            .into_iter()
            .map(|command| command.display())
            .collect::<Vec<_>>();

        assert!(commands
            .iter()
            .any(|command| command == "pi install npm:pi-subagents@0.66.0"));
        assert!(commands
            .iter()
            .any(|command| command == "pi install npm:@yassimba/pi-add-dir@latest"));
        assert!(commands
            .iter()
            .any(|command| command == "pi install -l npm:pi-subagents@0.66.0"));
        assert!(
            !commands.iter().any(|command| command.contains("feynman")),
            "Wiki packages are updated only in registered Vaults"
        );

        let windows_commands = pi_package_commands(&catalog, listed, true)
            .into_iter()
            .map(|command| command.display())
            .collect::<Vec<_>>();
        assert!(!windows_commands
            .iter()
            .any(|command| command.contains("@companion-ai/feynman")));
    }
}
