use crate::ownership::{digest_path, InstallState, OwnedPathKind, OwnershipScope, Receipt};
use crate::ui::{Mark, Out};
use crate::{CommandSpec, System};
use anyhow::{bail, Result};
use inquire::Confirm;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::IsTerminal;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptStatus {
    Clean,
    Missing,
    Modified,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallRequest {
    /// None means every visible owned resource. Some means only these IDs.
    pub selected: Option<Vec<String>>,
    pub force_modified: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallStep {
    pub resource_id: String,
    pub receipts: Vec<Receipt>,
    pub missing_only: bool,
    pub force_modified: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallPlan {
    pub visible: Vec<String>,
    pub hidden: Vec<String>,
    pub locked: BTreeMap<String, String>,
    pub modified_preserved: Vec<String>,
    pub steps: Vec<UninstallStep>,
}

impl UninstallPlan {
    pub fn remove_ids(&self) -> Vec<&str> {
        self.steps
            .iter()
            .map(|step| step.resource_id.as_str())
            .collect()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallOptions {
    pub selected: Vec<String>,
    pub all: bool,
    pub yes: bool,
    pub dry_run: bool,
    pub force_modified: bool,
}

pub fn run_uninstall(system: &(dyn System + Sync), options: &UninstallOptions) -> Result<bool> {
    let home = system
        .home_dir()
        .ok_or_else(|| anyhow::anyhow!("home directory is unavailable"))?;
    let cwd = system
        .current_dir()
        .ok_or_else(|| anyhow::anyhow!("current directory is unavailable"))?;
    let project = crate::skills::project_root(&cwd);
    let project = project.canonicalize().unwrap_or(project);
    let mut state = InstallState::load(&home).map_err(anyhow::Error::msg)?;
    if state.resources.is_empty() {
        let out = Out::detect();
        out.title("uninstall", "nothing owned");
        out.verdict(true, "Loom has no recorded resources to remove");
        return Ok(true);
    }

    let visible = visible_ids(&state, &project);
    let scripted = options.all || !options.selected.is_empty();
    if !std::io::stdin().is_terminal() && !scripted {
        bail!("non-interactive uninstall needs selectors or --all --yes");
    }
    if !std::io::stdin().is_terminal() && !options.dry_run && !options.yes {
        bail!("non-interactive uninstall needs --yes");
    }

    let selected = if options.all {
        None
    } else if !options.selected.is_empty() {
        Some(resolve_owned_ids(&state, &visible, &options.selected)?)
    } else {
        match uninstall_wizard_selection(&state, &visible, &home, &project, system)? {
            crate::wizard::WizardOutcome::UninstallSelection(chosen) => Some(chosen),
            crate::wizard::WizardOutcome::Cancelled
            | crate::wizard::WizardOutcome::NothingSelected => {
                println!("Cancelled; no changes made.");
                return Ok(true);
            }
            _ => bail!("uninstall wizard returned an install result"),
        }
    };

    let mut request = UninstallRequest {
        selected,
        force_modified: options.force_modified,
    };
    let probe = |receipt: &Receipt| receipt_status_on_system(receipt, system, &home);
    let mut plan = build_uninstall_plan(&state, &request, &project, probe)?;
    if !scripted && !request.force_modified && !plan.modified_preserved.is_empty() {
        let names = plan.modified_preserved.join(", ");
        if Confirm::new(&format!("Delete modified Loom content: {names}?"))
            .with_default(false)
            .prompt()?
        {
            request.force_modified = true;
            plan = build_uninstall_plan(&state, &request, &project, |receipt| {
                receipt_status_on_system(receipt, system, &home)
            })?;
        }
    }

    print_uninstall_plan(&plan);
    if options.dry_run {
        Out::detect().verdict(true, "Dry run; no changes made");
        return Ok(true);
    }
    if plan.steps.is_empty() {
        Out::detect().verdict(true, "Nothing selected can be removed");
        return Ok(true);
    }
    if !options.yes
        && !Confirm::new("Remove the reviewed resources?")
            .with_default(false)
            .prompt()?
    {
        Out::detect().verdict(true, "Cancelled; no changes made");
        return Ok(true);
    }

    let report = execute_uninstall_plan(
        &plan,
        &mut state,
        &home,
        system,
        &std::sync::atomic::AtomicBool::new(false),
    );
    print_uninstall_report(&report);
    Ok(report.failures.is_empty() && !report.cancelled)
}

fn uninstall_wizard_selection(
    state: &InstallState,
    visible: &[String],
    home: &Path,
    project: &Path,
    system: &(dyn System + Sync),
) -> Result<crate::wizard::WizardOutcome> {
    let resources = visible
        .iter()
        .map(|id| {
            let owned = &state.resources[id];
            let kind = if id.contains("skill:") {
                crate::ResourceKind::Skill
            } else if id.contains("pi-package:") {
                crate::ResourceKind::PiPackage
            } else if id.contains("mcp-server:") {
                crate::ResourceKind::McpServer
            } else if id.contains("herdr-plugin:") {
                crate::ResourceKind::HerdrPlugin
            } else {
                crate::ResourceKind::Tool
            };
            let statuses = owned
                .receipts
                .iter()
                .map(receipt_status)
                .collect::<Vec<_>>();
            let content = if statuses.contains(&ReceiptStatus::Modified) {
                "modified; preserved unless separately confirmed"
            } else if statuses
                .iter()
                .all(|status| *status == ReceiptStatus::Missing)
            {
                "already missing; remove the stale ownership record"
            } else {
                "owned by Loom"
            };
            crate::Resource {
                id: id.clone(),
                kind,
                group: match &owned.scope {
                    OwnershipScope::Global => "Global".into(),
                    OwnershipScope::Project { .. } => "Current project".into(),
                },
                label: id.clone(),
                description: content.into(),
                install_target: id.clone(),
                next_action: String::new(),
                dependencies: Vec::new(),
                bin: None,
                version: None,
                source: None,
                windows_wsl: false,
                companions: Vec::new(),
                bundled_skills: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    let dependencies = visible
        .iter()
        .map(|id| (id.clone(), state.resources[id].depends_on.clone()))
        .collect();
    crate::wizard::run_wizard(
        crate::wizard::Model {
            mode: crate::app::SelectionMode::Add,
            purpose: crate::wizard::WizardPurpose::Uninstall,
            uninstall_dependencies: dependencies,
            installed: vec![false; resources.len()],
            resources,
            profiles: Vec::new(),
            settings: Vec::new(),
            setting_states: Vec::new(),
            zed_present: false,
            settings_paths: crate::settings::SettingsPaths {
                herdr_config: home.join(".config/herdr/config.toml"),
                zed_settings: home.join(".config/zed/settings.json"),
                zed_keymap: home.join(".config/zed/keymap.json"),
                pi_fff_config: home.join(".pi/agent/pi-fff.json"),
                pi_adhd_flag: home.join(".pi/agent/.i-have-adhd-always"),
                diagrams: home.join(".config/loom/diagrams.json"),
            },
            status: crate::PrerequisiteStatus {
                pi: true,
                herdr: true,
                mise: true,
            },
            platform: if cfg!(windows) {
                crate::Platform::Windows
            } else {
                crate::Platform::Unix
            },
            dry_run: false,
            skill_destination: crate::SkillDestination::new(
                Vec::new(),
                crate::SkillScope::Global,
                home,
                project,
            ),
        },
        system,
    )
}

fn visible_ids(state: &InstallState, project: &Path) -> Vec<String> {
    state
        .resources
        .iter()
        .filter(|(_, resource)| match &resource.scope {
            OwnershipScope::Global => true,
            OwnershipScope::Project { root } => root == project,
        })
        .map(|(id, _)| id.clone())
        .collect()
}

fn resolve_owned_ids(
    state: &InstallState,
    visible: &[String],
    requested: &[String],
) -> Result<Vec<String>> {
    let mut resolved = Vec::new();
    for requested_id in requested {
        let matches = visible
            .iter()
            .filter(|key| {
                let resource = &state.resources[*key];
                *key == requested_id
                    || resource.id == *requested_id
                    || key.ends_with(&format!(":{requested_id}"))
            })
            .cloned()
            .collect::<Vec<_>>();
        if matches.is_empty() {
            bail!("{requested_id} is not owned by Loom here");
        }
        resolved.extend(matches);
    }
    resolved.sort();
    resolved.dedup();
    Ok(resolved)
}

fn print_uninstall_plan(plan: &UninstallPlan) {
    let out = Out::detect();
    out.title("uninstall", format!("{} item(s)", plan.steps.len()));
    for step in &plan.steps {
        out.row(
            if step.missing_only {
                Mark::Off
            } else {
                Mark::Ok
            },
            &step.resource_id,
            if step.missing_only {
                "already missing; prune ownership record"
            } else {
                "remove"
            },
        );
    }
    for (id, reason) in &plan.locked {
        out.row(Mark::Off, id, reason);
    }
    for id in &plan.modified_preserved {
        out.row(Mark::Off, id, "modified; preserved");
    }
}

fn print_uninstall_report(report: &UninstallReport) {
    let out = Out::detect();
    for id in &report.removed {
        out.row(Mark::Ok, id, "removed");
    }
    for id in &report.missing_pruned {
        out.row(Mark::Off, id, "ownership record pruned");
    }
    for failure in &report.failures {
        out.row(Mark::Bad, &failure.target, &failure.message);
    }
    out.verdict(
        report.failures.is_empty() && !report.cancelled,
        if report.cancelled {
            "Cancelled; remaining work is still recorded".into()
        } else if report.failures.is_empty() {
            format!(
                "{} removed",
                report.removed.len() + report.missing_pruned.len()
            )
        } else {
            format!(
                "{} removed · {} failed",
                report.removed.len(),
                report.failures.len()
            )
        },
    );
}

pub fn build_uninstall_plan(
    state: &InstallState,
    request: &UninstallRequest,
    current_project: &Path,
    mut receipt_status: impl FnMut(&Receipt) -> ReceiptStatus,
) -> Result<UninstallPlan> {
    let mut visible = Vec::new();
    let mut hidden = Vec::new();
    for (id, resource) in &state.resources {
        match &resource.scope {
            OwnershipScope::Global => visible.push(id.clone()),
            OwnershipScope::Project { root } if root == current_project => visible.push(id.clone()),
            OwnershipScope::Project { .. } => hidden.push(id.clone()),
        }
    }

    let visible_set = visible.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = match &request.selected {
        Some(ids) => {
            for id in ids {
                if !visible_set.contains(id) {
                    bail!("{id} is not owned by Loom here");
                }
            }
            ids.iter().cloned().collect::<BTreeSet<_>>()
        }
        None => visible_set.clone(),
    };

    let statuses = selected
        .iter()
        .map(|id| {
            let statuses = state.resources[id]
                .receipts
                .iter()
                .map(&mut receipt_status)
                .collect::<Vec<_>>();
            (id.clone(), statuses)
        })
        .collect::<BTreeMap<_, _>>();
    let mut modified_preserved = Vec::new();
    if !request.force_modified {
        for (id, receipt_statuses) in &statuses {
            if receipt_statuses.contains(&ReceiptStatus::Modified) {
                selected.remove(id);
                modified_preserved.push(id.clone());
            }
        }
    }

    let mut keep_roots = state
        .resources
        .keys()
        .filter(|id| !selected.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    keep_roots.extend(hidden.iter().cloned());

    let mut required_by = BTreeMap::new();
    for root in keep_roots.clone() {
        let mut pending = vec![root.clone()];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let Some(resource) = state.resources.get(&id) else {
                continue;
            };
            for dependency in &resource.depends_on {
                if selected.remove(dependency) {
                    required_by
                        .entry(dependency.clone())
                        .or_insert_with(|| format!("required by kept {root}"));
                }
                pending.push(dependency.clone());
            }
        }
    }

    let mut steps = Vec::new();
    let mut ordered = selected.into_iter().collect::<Vec<_>>();
    order_dependents_first(&mut ordered, state);
    for id in ordered {
        let resource = &state.resources[&id];
        let receipt_statuses = &statuses[&id];
        steps.push(UninstallStep {
            resource_id: id,
            receipts: resource.receipts.clone(),
            missing_only: receipt_statuses
                .iter()
                .all(|status| *status == ReceiptStatus::Missing),
            force_modified: request.force_modified,
        });
    }

    Ok(UninstallPlan {
        visible,
        hidden,
        locked: required_by,
        modified_preserved,
        steps,
    })
}

fn order_dependents_first(ids: &mut [String], state: &InstallState) {
    ids.sort_by_key(|id| std::cmp::Reverse(dependency_depth(id, state, &mut BTreeSet::new())));
}

fn dependency_depth(id: &str, state: &InstallState, seen: &mut BTreeSet<String>) -> usize {
    if !seen.insert(id.to_owned()) {
        return 0;
    }
    state.resources.get(id).map_or(0, |resource| {
        1 + resource
            .depends_on
            .iter()
            .map(|dependency| dependency_depth(dependency, state, seen))
            .max()
            .unwrap_or(0)
    })
}

fn receipt_status_on_system(receipt: &Receipt, system: &dyn System, home: &Path) -> ReceiptStatus {
    match receipt {
        Receipt::Manager { manager, target } if manager == "pi" || manager == "herdr" => {
            if !system.command_exists(manager) {
                return ReceiptStatus::Missing;
            }
            let command = if manager == "pi" {
                CommandSpec::new("pi", ["list"])
            } else {
                CommandSpec::new("herdr", ["plugin", "list"])
            };
            match system.run_probe(&command) {
                Ok(result)
                    if result.success
                        && (result.stdout.contains(target)
                            || result.stderr.contains(target)
                            || target
                                .split(['@', '#'])
                                .find(|part| !part.is_empty())
                                .is_some_and(|part| result.stdout.contains(part))) =>
                {
                    ReceiptStatus::Clean
                }
                Ok(_) => ReceiptStatus::Missing,
                Err(_) => ReceiptStatus::Modified,
            }
        }
        Receipt::MiseTool { key } => {
            if crate::manifest::selection_contains(home, key) {
                ReceiptStatus::Clean
            } else {
                ReceiptStatus::Missing
            }
        }
        Receipt::MiseInstallation { root, .. } if !root.exists() => ReceiptStatus::Missing,
        _ => receipt_status(receipt),
    }
}

pub fn receipt_status(receipt: &Receipt) -> ReceiptStatus {
    match receipt {
        Receipt::McpEntry { path, name, digest } => crate::mcp::entry_status(path, name, digest),
        Receipt::Path { path, digest, .. } => {
            if !path.exists() {
                ReceiptStatus::Missing
            } else if digest_path(path).is_ok_and(|current| current == *digest) {
                ReceiptStatus::Clean
            } else {
                ReceiptStatus::Modified
            }
        }
        Receipt::PiSkillExclusion { path, entry } => {
            match crate::bundled_skills::exclusion_present(path, entry) {
                Ok(true) => ReceiptStatus::Clean,
                Ok(false) => ReceiptStatus::Missing,
                Err(_) => ReceiptStatus::Modified,
            }
        }
        Receipt::ActivationLine { path, line } => match fs::read_to_string(path) {
            Ok(content) if content.lines().any(|current| current == line) => ReceiptStatus::Clean,
            Ok(_) => ReceiptStatus::Missing,
            Err(_) if !path.exists() => ReceiptStatus::Missing,
            Err(_) => ReceiptStatus::Modified,
        },
        Receipt::Manager { .. }
        | Receipt::Command { .. }
        | Receipt::MiseTool { .. }
        | Receipt::MiseInstallation { .. } => ReceiptStatus::Clean,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallFailure {
    pub target: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallReport {
    pub removed: Vec<String>,
    pub missing_pruned: Vec<String>,
    pub failures: Vec<UninstallFailure>,
    pub cancelled: bool,
}

pub fn execute_uninstall_plan(
    plan: &UninstallPlan,
    state: &mut InstallState,
    home: &Path,
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
) -> UninstallReport {
    let mut report = UninstallReport::default();
    let (final_steps, ordinary_steps): (Vec<_>, Vec<_>) = plan
        .steps
        .iter()
        .partition(|step| step.resource_id.starts_with("core:"));
    'steps: for step in ordinary_steps {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        for receipt in &step.receipts {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                report.cancelled = true;
                break 'steps;
            }
            let current_status = receipt_status_on_system(receipt, system, home);
            if !step.force_modified && current_status == ReceiptStatus::Modified {
                report.failures.push(UninstallFailure {
                    target: step.resource_id.clone(),
                    message: "content changed after review; rerun and confirm modified removal"
                        .into(),
                });
                continue 'steps;
            }
            if current_status != ReceiptStatus::Missing {
                if let Err(message) = remove_receipt_and_selection(receipt, system, cancelled) {
                    report.failures.push(UninstallFailure {
                        target: step.resource_id.clone(),
                        message,
                    });
                    continue 'steps;
                }
            }
            let mut next = state.clone();
            if let Some(resource) = next.resources.get_mut(&step.resource_id) {
                if let Some(index) = resource.receipts.iter().position(|item| item == receipt) {
                    resource.receipts.remove(index);
                }
                if resource.receipts.is_empty() {
                    next.resources.remove(&step.resource_id);
                }
            }
            if let Err(message) = next.save(home) {
                report.failures.push(UninstallFailure {
                    target: step.resource_id.clone(),
                    message,
                });
                break 'steps;
            }
            *state = next;
        }
        if state.resources.contains_key(&step.resource_id) {
            continue;
        }
        if step.missing_only {
            report.missing_pruned.push(step.resource_id.clone());
        } else {
            report.removed.push(step.resource_id.clone());
        }
    }
    if report.failures.is_empty()
        && !report.cancelled
        && !final_steps.is_empty()
        && state
            .resources
            .keys()
            .all(|id| final_steps.iter().any(|step| &step.resource_id == id))
    {
        match schedule_final_cleanup(&final_steps, home, system) {
            Ok(()) => report
                .removed
                .extend(final_steps.iter().map(|step| step.resource_id.clone())),
            Err(message) => report.failures.push(UninstallFailure {
                target: "final cleanup".into(),
                message,
            }),
        }
    }
    report
}

fn schedule_final_cleanup(
    steps: &[&UninstallStep],
    home: &Path,
    system: &dyn System,
) -> Result<(), String> {
    let mise_installation = steps
        .iter()
        .flat_map(|step| &step.receipts)
        .find_map(|receipt| match receipt {
            Receipt::MiseInstallation {
                root,
                executable,
                manager,
                path_entry_added,
            } => Some((
                root.clone(),
                executable.clone(),
                manager.clone(),
                path_entry_added.clone(),
            )),
            _ => None,
        })
        .filter(|_| !mise_has_foreign_use(home));
    let cache = home.join(".cache").join("loom");
    fs::create_dir_all(&cache)
        .map_err(|error| format!("could not create {}: {error}", cache.display()))?;
    let state_path = home.join(crate::ownership::STATE_PATH);
    let selection = crate::manifest::conf_d_target(home);
    let pid = std::process::id().to_string();
    if cfg!(windows) {
        let script = cache.join("uninstall-final.ps1");
        let quote = |path: &Path| path.display().to_string().replace('\'', "''");
        let remove_mise = mise_installation.as_ref().map_or_else(
            || "mise prune --yes 2>$null | Out-Null".into(),
            |(root, executable, manager, path_entry)| {
                let uninstall = match manager.as_deref() {
                    Some("winget") => "winget uninstall --id jdx.mise --silent 2>$null | Out-Null".into(),
                    Some("scoop") => "scoop uninstall mise 2>$null | Out-Null".into(),
                    _ => executable.as_ref().map_or_else(String::new, |path| {
                        let shim = path.with_file_name("mise-shim.exe");
                        format!(
                            "Remove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue\nRemove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue",
                            quote(path), quote(&shim)
                        )
                    }),
                };
                let remove_path = path_entry.as_ref().map_or_else(String::new, |entry| {
                    format!(
                        "$ownedPath='{}'; $userPath=[Environment]::GetEnvironmentVariable('Path',[System.EnvironmentVariableTarget]::User); $kept=@($userPath -split ';' | Where-Object {{ $_ -and $_ -ne $ownedPath }}); [Environment]::SetEnvironmentVariable('Path',($kept -join ';'),[System.EnvironmentVariableTarget]::User)",
                        quote(entry)
                    )
                });
                format!(
                    "{}\n{}\nRemove-Item -LiteralPath '{}' -Recurse -Force -ErrorAction SilentlyContinue",
                    uninstall, remove_path, quote(root)
                )
            },
        );
        let activation_cleanup = steps
            .iter()
            .flat_map(|step| &step.receipts)
            .filter_map(|receipt| match receipt {
                Receipt::ActivationLine { path, line } => Some(format!(
                    "$p='{}'; $l='{}'; if (Test-Path -LiteralPath $p) {{ @((Get-Content -LiteralPath $p) | Where-Object {{ $_ -ne $l }}) | Set-Content -LiteralPath $p }}",
                    quote(path),
                    line.replace('\'', "''")
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let body = format!(
            "$parentPid = [int]$args[0]\nwhile (Get-Process -Id $parentPid -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 100 }}\n{}\nRemove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue\nRemove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue\n{}\nRemove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force -ErrorAction SilentlyContinue\n",
            activation_cleanup,
            quote(&selection),
            quote(&state_path),
            remove_mise,
        );
        fs::write(&script, body)
            .map_err(|error| format!("could not write {}: {error}", script.display()))?;
        system
            .spawn_detached(&CommandSpec::new(
                "powershell",
                [
                    "-NoProfile".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-File".into(),
                    script.display().to_string(),
                    pid,
                ],
            ))
            .map_err(|error| error.to_string())
    } else {
        let script = cache.join("uninstall-final.sh");
        let quote_value = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
        let quote = |path: &Path| quote_value(&path.display().to_string());
        let remove_mise = mise_installation.as_ref().map_or_else(
            || "mise prune --yes >/dev/null 2>&1 || true".into(),
            |(root, executable, _, _)| {
                format!(
                    "rm -rf -- {}\n{}",
                    quote(root),
                    executable
                        .as_ref()
                        .map_or_else(String::new, |path| format!("rm -f -- {}", quote(path)))
                )
            },
        );
        let activation_cleanup = steps
            .iter()
            .flat_map(|step| &step.receipts)
            .filter_map(|receipt| match receipt {
                Receipt::ActivationLine { path, line } => Some(format!(
                    "p={}; l={}; if [ -f \"$p\" ]; then t=\"$p.loom-uninstall.$$\"; grep -Fvx -- \"$l\" \"$p\" >\"$t\" || true; mv \"$t\" \"$p\"; fi",
                    quote(path),
                    quote_value(line)
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let body = format!(
            "#!/bin/sh\nwhile kill -0 \"$1\" 2>/dev/null; do sleep 0.1; done\n{}\nrm -f -- {} {}\n{}\nrm -f -- \"$0\"\n",
            activation_cleanup,
            quote(&selection),
            quote(&state_path),
            remove_mise,
        );
        fs::write(&script, body)
            .map_err(|error| format!("could not write {}: {error}", script.display()))?;
        system
            .spawn_detached(&CommandSpec::new("sh", [script.display().to_string(), pid]))
            .map_err(|error| error.to_string())
    }
}

fn mise_has_foreign_use(home: &Path) -> bool {
    let config = home.join(".config").join("mise");
    let loom = crate::manifest::conf_d_target(home);
    let mut pending = vec![config];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path == loom {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                return true;
            }
        }
    }
    false
}

fn remove_receipt_and_selection(
    receipt: &Receipt,
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    remove_receipt(receipt, system, cancelled)?;
    if let Receipt::MiseTool { key } = receipt {
        crate::manifest::remove_selected(system, std::slice::from_ref(key), cancelled)?;
    }
    Ok(())
}

fn remove_receipt(
    receipt: &Receipt,
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    match receipt {
        Receipt::McpEntry { path, name, digest } => {
            crate::mcp::remove_entry(path, name, digest).map_err(|e| e.to_string())?
        }
        Receipt::Manager { manager, target } => {
            let args = if manager == "herdr" {
                vec![
                    "plugin".into(),
                    "uninstall".into(),
                    target.clone(),
                    "--yes".into(),
                ]
            } else if manager == "pi" {
                let source = if target.contains(':') || target.starts_with(['.', '/']) {
                    target.clone()
                } else {
                    format!("npm:{target}")
                };
                vec!["uninstall".into(), source]
            } else {
                vec!["uninstall".into(), target.clone()]
            };
            let result = system
                .run_controlled(
                    &CommandSpec::new(manager, args),
                    crate::system::MANAGER_COMMAND_TIMEOUT,
                    cancelled,
                )
                .map_err(|error| error.to_string())?;
            if !result.success {
                return Err(crate::install::command_failure_message(&result));
            }
        }
        Receipt::Command { program, args } => {
            let result = system
                .run_controlled(
                    &CommandSpec::new(program, args.clone()),
                    crate::system::MANAGER_COMMAND_TIMEOUT,
                    cancelled,
                )
                .map_err(|error| error.to_string())?;
            if !result.success {
                return Err(crate::install::command_failure_message(&result));
            }
        }
        Receipt::MiseTool { .. } => {}
        Receipt::Path {
            path,
            path_kind,
            before,
            ..
        } => {
            if !path.exists() {
                return Ok(());
            }
            match (path_kind, before) {
                (OwnedPathKind::File, Some(content)) => fs::write(path, content)
                    .map_err(|error| format!("could not restore {}: {error}", path.display()))?,
                (OwnedPathKind::File, None) => fs::remove_file(path)
                    .map_err(|error| format!("could not remove {}: {error}", path.display()))?,
                (OwnedPathKind::Tree, _) => fs::remove_dir_all(path)
                    .map_err(|error| format!("could not remove {}: {error}", path.display()))?,
            }
        }
        Receipt::PiSkillExclusion { path, entry } => {
            crate::bundled_skills::remove_exclusion(path, entry)?;
        }
        Receipt::ActivationLine { path, line } => {
            let content = match fs::read_to_string(path) {
                Ok(content) => content,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(format!("could not read {}: {error}", path.display())),
            };
            let kept = content
                .lines()
                .filter(|current| *current != line)
                .collect::<Vec<_>>();
            let updated = if kept.is_empty() {
                String::new()
            } else {
                format!("{}\n", kept.join("\n"))
            };
            fs::write(path, updated)
                .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        }
        Receipt::MiseInstallation { .. } => {
            return Err("mise self-removal must run as the final detached cleanup".into())
        }
    }
    Ok(())
}
