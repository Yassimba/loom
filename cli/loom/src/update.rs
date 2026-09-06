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
            detail: detail.into(),
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
    let mut scope = None;
    let mut user = Vec::new();
    let mut project = Vec::new();
    for line in listed.lines().map(str::trim) {
        match line {
            "User packages:" => scope = Some(false),
            "Project packages:" => scope = Some(true),
            _ => match scope {
                Some(false) => user.push(line),
                Some(true) => project.push(line),
                None => {}
            },
        }
    }
    catalog
        .resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::PiPackage)
        // MCP setup preserves the shared gateway; a catalog reinstall would
        // downgrade compatible newer installs to the setup prerequisite pin.
        .filter(|resource| resource.install_target != "pi-mcp-adapter")
        .filter(|resource| !native_windows || !resource.windows_wsl)
        .flat_map(|resource| {
            let spec = resource.pi_install_spec();
            let global = user
                .iter()
                .any(|line| line.contains(&resource.install_target))
                .then(|| CommandSpec::new("pi", ["install", &spec]));
            let local = project
                .iter()
                .any(|line| line.contains(&resource.install_target))
                .then(|| CommandSpec::new("pi", ["install", "-l", &spec]));
            [global, local].into_iter().flatten()
        })
        .collect()
}

pub fn run_updates(system: &(dyn System + Sync), catalog: &Catalog) -> bool {
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
    let mut pi_compat_targets = Vec::new();
    if system.command_exists("pi") {
        // Cataloged reinstalls instead of `pi update --all`: external packages
        // use their pin, first-party packages request npm's latest release, and
        // Pi itself is the mise manifest's job. Packages outside the catalog
        // are left alone.
        let listed = system
            .run_probe(&CommandSpec::new("pi", ["list"]))
            .ok()
            .filter(|result| result.success)
            .map(|result| format!("{}\n{}", result.stdout, result.stderr))
            .unwrap_or_default();
        let commands = pi_package_commands(catalog, &listed, cfg!(windows));
        pi_compat_targets = catalog
            .resources
            .iter()
            .filter(|resource| crate::pi_compat::is_managed(&resource.id))
            .filter(|resource| listed.contains(&resource.install_target))
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
    if system.command_exists("herdr") {
        tasks.push(CommandLane {
            label: "Herdr",
            detail: "plugins".into(),
            commands: vec![CommandSpec::new("herdr", ["plugin", "update", "--all"])],
        });
    }
    // The manifest lane owns tool updates, including this CLI's own pin.
    // Loom is only ever installed through mise, so a missing mise means the
    // bootstrap was undone; point back at it instead of self-updating.
    let mise = system.command_exists("mise");

    // Skills, projects, tools, Pi, and Herdr touch disjoint state, so every
    // lane runs at once; rows print in a fixed order once all are done.
    let repository = &skills::Repository::default();
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
        jobs.push((label, Box::new(move || run_command_lane(system, task))));
    }
    // Rows keep a fixed order, so the report cannot stream; a status line
    // names the lanes still running instead, or the wait reads as a hang
    // (Pi reinstalls and the repo download take minutes together).
    let labels = jobs.iter().map(|(label, _)| *label).collect::<Vec<_>>();
    let (sender, results) = std::sync::mpsc::channel::<(usize, Lane)>();
    let mut lanes = Vec::new();
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
            let active = running.join(" + ");
            if out.is_terminal() || active != last_active {
                out.progress(progress_status(
                    total - running.len(),
                    total,
                    &running,
                    started.elapsed().as_secs(),
                ));
                last_active = active;
            }
        }
    });
    out.progress_done();
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
    let updated = lanes.len() - failed;
    if failed == 0 {
        out.verdict(true, format!("Up to date · {updated} lanes refreshed"));
    } else {
        out.verdict(false, format!("{updated} refreshed · {failed} failed"));
    }
    failed == 0
}

/// Refresh mise's conf.d copy of the published manifest and install its pins.
/// Tools move only when a new manifest landed on main since the last sync.
fn sync_tool_manifest(system: &dyn System, repository: &skills::Repository) -> Lane {
    match crate::manifest::sync_selected_from(
        system,
        &[],
        &std::sync::atomic::AtomicBool::new(false),
        repository,
    ) {
        Ok(target) => {
            let home = system.home_dir().unwrap_or_default();
            Lane::ok("Tools", tidy_path(&target, &home))
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

fn run_command_lane(system: &dyn System, task: CommandLane) -> Lane {
    for command in &task.commands {
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        match system.run_controlled(command, crate::system::MANAGER_COMMAND_TIMEOUT, &cancelled) {
            Ok(result) if result.success => {}
            Ok(result) => {
                return Lane::failed(
                    task.label,
                    format!(
                        "{} — {}",
                        command.display(),
                        crate::install::command_failure_message(&result)
                    ),
                );
            }
            Err(error) => {
                return Lane::failed(task.label, format!("{}: {error}", command.display()))
            }
        }
    }
    Lane::ok(task.label, task.detail)
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
            let mut lane = Lane::ok(
                "Skills",
                format!("{total} refreshed across {} trees", reports.len()),
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
                    "{}  {}{detail}",
                    tidy_path(&report.tree, &home),
                    report.installed
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
        let feynman = catalog
            .resources
            .iter()
            .find(|resource| resource.id == "pi-package:@companion-ai/feynman")
            .unwrap()
            .pi_install_spec();

        let commands = pi_package_commands(&catalog, listed, false)
            .into_iter()
            .map(|command| command.display())
            .collect::<Vec<_>>();

        assert!(commands
            .iter()
            .any(|command| command == "pi install npm:pi-subagents@0.65.1"));
        assert!(commands
            .iter()
            .any(|command| command == "pi install npm:@yassimba/pi-add-dir@latest"));
        assert!(commands
            .iter()
            .any(|command| command == "pi install -l npm:pi-subagents@0.65.1"));
        assert!(commands.contains(&format!("pi install -l {feynman}")));
        assert!(!commands.contains(&format!("pi install {feynman}")));

        let windows_commands = pi_package_commands(&catalog, listed, true)
            .into_iter()
            .map(|command| command.display())
            .collect::<Vec<_>>();
        assert!(!windows_commands
            .iter()
            .any(|command| command.contains("@companion-ai/feynman")));
    }
}
