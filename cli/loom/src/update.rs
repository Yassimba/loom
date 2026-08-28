use crate::{skills, Catalog, CommandSpec, NodeStatus, ResourceKind, System};

/// One independent update lane; lanes run concurrently and report whole
/// blocks of output so nothing interleaves.
struct UpdateTask {
    label: &'static str,
    commands: Vec<CommandSpec>,
}

pub fn run_updates(system: &(dyn System + Sync), catalog: &Catalog) -> bool {
    // Warn-only: loom never installs or updates Node itself, but a Node
    // below Pi's floor is worth flagging before Pi's own update runs. A
    // missing Node stays silent here — there is nothing installed to age.
    let node = NodeStatus::detect(system);
    if matches!(node, NodeStatus::TooOld(..)) {
        if let Some(warning) = node.warning() {
            eprintln!("  ! {warning}");
        }
    }

    let mut tasks = Vec::new();
    if system.command_exists("pi") {
        // Pinned reinstalls instead of `pi update --all`: cataloged packages
        // move only when their pin (or a first-party publish) does, and Pi
        // itself is the mise manifest's job. Packages outside the catalog
        // are left alone.
        let listed = system
            .run(&CommandSpec::new("pi", ["list"]))
            .ok()
            .filter(|result| result.success)
            .map(|result| format!("{}\n{}", result.stdout, result.stderr))
            .unwrap_or_default();
        let commands = catalog
            .resources
            .iter()
            .filter(|resource| resource.kind == ResourceKind::PiPackage)
            .filter(|resource| listed.contains(&resource.install_target))
            .map(|resource| {
                let spec = match &resource.version {
                    Some(version) => format!("npm:{}@{version}", resource.install_target),
                    None => format!("npm:{}", resource.install_target),
                };
                CommandSpec::new("pi", ["install", &spec])
            })
            .collect::<Vec<_>>();
        if !commands.is_empty() {
            tasks.push(UpdateTask {
                label: "Pi packages (pinned)",
                commands,
            });
        }
    }
    if system.command_exists("herdr") {
        tasks.push(UpdateTask {
            label: "Herdr and Herdr plugins",
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
    if !mise {
        eprintln!("  ! mise is not on PATH; rerun the installer from the README to restore it");
    }

    // Skills, Pi, Herdr, and the tool manifest touch
    // disjoint state, so all lanes run concurrently; each lane prints once,
    // when it finishes.
    let lanes = ["Shared skills", "Project AGENTS.md files"]
        .into_iter()
        .chain(mise.then_some("Tool manifest (mise)"))
        .chain(tasks.iter().map(|task| task.label))
        .collect::<Vec<_>>()
        .join(", ");
    println!("Updating in parallel: {lanes}...");
    let mut ready = true;
    let (line_sender, lines) = std::sync::mpsc::channel::<(bool, String)>();
    std::thread::scope(|scope| {
        {
            let line_sender = line_sender.clone();
            scope.spawn(move || {
                let _ = line_sender.send(update_installed_skills(system, catalog));
            });
        }
        {
            let line_sender = line_sender.clone();
            scope.spawn(move || {
                let _ = line_sender.send(crate::init::sync_projects(system));
            });
        }
        if mise {
            let line_sender = line_sender.clone();
            scope.spawn(move || {
                let _ = line_sender.send(sync_tool_manifest(system));
            });
        }
        for task in &tasks {
            let line_sender = line_sender.clone();
            scope.spawn(move || {
                let _ = line_sender.send(run_update_task(system, task));
            });
        }
        drop(line_sender);
        for (ok, block) in lines {
            ready &= ok;
            if ok {
                println!("{block}");
            } else {
                eprintln!("{block}");
            }
        }
    });
    ready
}

/// Refresh mise's conf.d copy of the published manifest and install its pins.
/// Tools move only when a new manifest landed on main since the last sync.
fn sync_tool_manifest(system: &dyn System) -> (bool, String) {
    match crate::manifest::sync_and_install(system) {
        Ok(target) => (
            true,
            format!("  ✓ Tool manifest (mise)\n      {}", target.display()),
        ),
        Err(message) => (false, format!("  ! Tool manifest (mise): {message}")),
    }
}

fn run_update_task(system: &dyn System, task: &UpdateTask) -> (bool, String) {
    for command in &task.commands {
        match system.run(command) {
            Ok(result) if result.success => {}
            Ok(result) => {
                return (
                    false,
                    format!(
                        "  ! {}: {}",
                        task.label,
                        crate::install::command_failure_message(&result)
                    ),
                );
            }
            Err(error) => return (false, format!("  ! {}: {error}", task.label)),
        }
    }
    (true, format!("  ✓ {}", task.label))
}

/// Refresh catalog skills in the exact global and current-project trees where
/// they already exist. Agent and scope choices remain stable across updates.
fn update_installed_skills(system: &dyn System, catalog: &Catalog) -> (bool, String) {
    match skills::refresh_installed_skills(system, &catalog.resources) {
        Ok(reports) => {
            if reports.is_empty() {
                return (true, "  ✓ Shared skills (none installed)".to_string());
            }
            let mut block = String::from("  ✓ Shared skills");
            for report in reports {
                let skipped = if report.skipped_symlinks.is_empty() {
                    String::new()
                } else {
                    format!(" ({} symlinked, left alone)", report.skipped_symlinks.len())
                };
                block.push_str(&format!(
                    "\n      {}: {} skills{skipped}",
                    report.tree.display(),
                    report.installed
                ));
            }
            (true, block)
        }
        Err(message) => (false, format!("  ! Shared skills: {message}")),
    }
}
