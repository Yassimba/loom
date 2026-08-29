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
        // Pinned reinstalls instead of `pi update --all`: cataloged packages
        // move only when their pin (or a first-party publish) does, and Pi
        // itself is the mise manifest's job. Packages outside the catalog
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
                detail: format!("{} pinned", commands.len()),
                commands,
            });
        }
    }
    if system.command_exists("herdr") {
        tasks.push(CommandLane {
            label: "Herdr",
            detail: "herdr and its plugins".into(),
            commands: vec![
                CommandSpec::new("herdr", ["update"]),
                CommandSpec::new("herdr", ["plugin", "update", "--all"]),
            ],
        });
    }
    // The manifest lane owns tool updates, including this CLI's own pin.
    // Loom is only ever installed through mise, so a missing mise means the
    // bootstrap was undone; point back at it instead of self-updating.
    let mise = crate::manifest::mise_available(system);

    // Skills, projects, tools, Pi, and Herdr touch disjoint state, so every
    // lane runs at once; rows print in a fixed order once all are done.
    type Job<'a> = Box<dyn FnOnce() -> Lane + Send + 'a>;
    let mut jobs: Vec<(&'static str, Job<'_>)> = vec![
        (
            "Skills",
            Box::new(move || update_installed_skills(system, catalog)),
        ),
        ("Projects", Box::new(move || sync_projects_lane(system))),
    ];
    if mise {
        jobs.push(("Tools", Box::new(move || sync_tool_manifest(system))));
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
        let started = std::time::Instant::now();
        let mut last_set = String::new();
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
            let set = running.join(" · ");
            if out.is_terminal() || set != last_set {
                out.progress(format!("running  {set} · {}s", started.elapsed().as_secs()));
                last_set = set;
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
fn sync_tool_manifest(system: &dyn System) -> Lane {
    match crate::manifest::sync_and_install(system) {
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
fn update_installed_skills(system: &dyn System, catalog: &Catalog) -> Lane {
    match skills::refresh_installed_skills(system, &catalog.resources) {
        Ok(reports) if reports.is_empty() => Lane::ok("Skills", "none installed"),
        Ok(reports) => {
            let home = system.home_dir().unwrap_or_default();
            let total: usize = reports.iter().map(|report| report.installed).sum();
            let mut lane = Lane::ok(
                "Skills",
                format!("{total} refreshed across {} trees", reports.len()),
            );
            for report in reports {
                let skipped = if report.skipped_symlinks.is_empty() {
                    String::new()
                } else {
                    format!(" · {} symlinked, left alone", report.skipped_symlinks.len())
                };
                lane.notes.push(format!(
                    "{}  {}{skipped}",
                    tidy_path(&report.tree, &home),
                    report.installed
                ));
            }
            lane
        }
        Err(message) => Lane::failed("Skills", message),
    }
}

fn sync_projects_lane(system: &dyn System) -> Lane {
    let sync = crate::init::sync_projects(system);
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
    fn pi_package_updates_preserve_user_and_project_scope() {
        let catalog = Catalog::embedded().unwrap();
        let listed = "User packages:\n  npm:pi-subagents\n\nProject packages:\n  npm:pi-subagents\n  git:github.com/earendil-works/pi-chat@abc\n";

        let commands = pi_package_commands(&catalog, listed, false)
            .into_iter()
            .map(|command| command.display())
            .collect::<Vec<_>>();

        assert!(commands
            .iter()
            .any(|command| command == "pi install npm:pi-subagents@0.58.0"));
        assert!(commands
            .iter()
            .any(|command| command == "pi install -l npm:pi-subagents@0.58.0"));
        assert!(commands.iter().any(|command| {
            command.starts_with("pi install -l git:github.com/earendil-works/pi-chat@")
        }));
        assert!(!commands.iter().any(|command| {
            command.starts_with("pi install git:github.com/earendil-works/pi-chat@")
        }));

        let windows_commands = pi_package_commands(&catalog, listed, true)
            .into_iter()
            .map(|command| command.display())
            .collect::<Vec<_>>();
        assert!(!windows_commands
            .iter()
            .any(|command| command.contains("pi-chat")));
    }
}
