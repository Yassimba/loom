use crate::settings::{apply_setting, curated_settings, setting_state, SettingSpec, SettingsPaths};
use crate::ui::{confirm_plan, print_plan, Mark, Out};
use crate::wizard::{run_wizard, Model, WizardOutcome};
use crate::{
    build_install_plan, execute_install_plan, expand_skill_dependencies, Catalog, CommandSpec,
    InstallFailure, InstallReport, Platform, PrerequisiteStatus, Resource, ResourceKind,
    SkillAgent, SkillDestination, SkillScope, System,
};
use anyhow::{bail, Context, Result};
use inquire::Confirm;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

pub(crate) const SETUP_NEXT_ACTIONS: [&str; 3] = [
    "if a newly installed command is missing, open a new shell",
    "run `loom status` to verify the setup",
    "run `loom init` inside your first project",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionMode {
    Setup,
    Add,
}

impl SelectionMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Add => "add",
        }
    }
}

#[derive(Default)]
pub struct Selectors {
    pub skills: Vec<String>,
    pub pi_packages: Vec<String>,
    pub herdr_plugins: Vec<String>,
    pub tools: Vec<String>,
}

impl Selectors {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
            && self.pi_packages.is_empty()
            && self.herdr_plugins.is_empty()
            && self.tools.is_empty()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn install_selected(
    mode: SelectionMode,
    catalog: &Catalog,
    selectors: &Selectors,
    requested_agents: &[SkillAgent],
    scope: SkillScope,
    offer_wsl: bool,
    assume_yes: bool,
    dry_run: bool,
    system: &(dyn System + Sync),
) -> Result<bool> {
    let status = PrerequisiteStatus {
        pi: system.command_exists("pi"),
        herdr: system.command_exists("herdr"),
        mise: system.command_exists("mise"),
    };
    let platform = if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Unix
    };
    if platform == Platform::Windows && offer_wsl && selectors.is_empty() {
        let mut labels = catalog
            .resources
            .iter()
            .filter(|resource| resource.windows_wsl)
            .map(|resource| resource.label.as_str())
            .collect::<Vec<_>>();
        labels.sort_unstable();
        labels.dedup();
        if !labels.is_empty() {
            println!(
                "{} are not available on native Windows. WSL2 lets Loom offer the complete setup.\n",
                labels.join(", ")
            );
            if Confirm::new("Use WSL2 for the complete Loom setup?")
                .with_default(true)
                .prompt()?
            {
                return prepare_wsl(system, dry_run);
            }
        }
    }
    let home = system.home_dir().context("home directory is unavailable")?;
    let current_dir = system
        .current_dir()
        .context("current directory is unavailable")?;
    let agents = if requested_agents.is_empty() {
        crate::detect_skill_agents(&home)
    } else {
        requested_agents.to_vec()
    };
    let destination = SkillDestination::new(agents, scope, &home, &current_dir);
    if selectors.is_empty() {
        let resources = if platform == Platform::Windows {
            native_windows_resources(catalog)
        } else {
            catalog.resources.clone()
        };
        return run_interactive(
            mode,
            catalog,
            resources,
            status,
            platform,
            dry_run,
            destination,
            system,
        );
    }

    let resources = expand_skill_dependencies(
        &catalog.resources,
        resolve_selectors(catalog, selectors)?,
        &destination.agents,
    );
    if resources.iter().any(|resource| resource.group == "Wiki") {
        let feynman = resources
            .iter()
            .any(|resource| resource.install_target == "@companion-ai/feynman");
        let generic = resources
            .iter()
            .filter(|resource| resource.group != "Wiki")
            .cloned()
            .collect::<Vec<_>>();
        if dry_run {
            let out = Out::detect();
            out.title(mode.command(), "dry run");
            if !generic.is_empty() {
                let plan = build_install_plan(&generic, status, platform, &destination)?;
                print_plan(&out, &plan);
            }
            out.row(
                Mark::Off,
                "Wiki",
                "would enter the Vault-scoped setup; no Vault changes made",
            );
            out.verdict(true, "Dry run; no changes made");
            return Ok(true);
        }
        anyhow::ensure!(
            generic.is_empty(),
            "scripted Wiki selection cannot be mixed with global resources; run the selections separately or use the interactive wizard"
        );
        return crate::wiki::run_interactive_with_default(system, feynman);
    }
    if platform == Platform::Windows {
        if let Some(resource) = resources.iter().find(|resource| resource.windows_wsl) {
            bail!(
                "{} requires WSL2 on Windows. Open Ubuntu and run this loom command there.",
                resource.label
            );
        }
    }
    if resources.is_empty() {
        println!("Nothing selected; no changes made.");
        return Ok(true);
    }
    let installed = detect_installed(&resources, status, system, &destination);
    let resources = resources
        .into_iter()
        .zip(installed)
        .filter_map(|(resource, installed)| (!installed).then_some(resource))
        .collect::<Vec<_>>();
    if resources.is_empty() {
        let out = Out::detect();
        out.title(mode.command(), "already configured");
        out.verdict(
            true,
            "Everything selected is already set up; no changes made",
        );
        return Ok(true);
    }
    let plan = build_install_plan(&resources, status, platform, &destination)?;
    let settings_paths = SettingsPaths::detect()?;
    let related_settings = unapplied_related_settings(&resources, &settings_paths);
    let out = Out::detect();
    out.title(mode.command(), format!("{} item(s)", resources.len()));
    print_plan(&out, &plan);
    print_settings_plan(&out, &related_settings, &settings_paths);
    if dry_run {
        out.verdict(true, "Dry run; no changes made");
        return Ok(true);
    }
    if !assume_yes && !confirm_plan()? {
        out.verdict(true, "Cancelled; no changes made");
        return Ok(true);
    }
    let setting_before = setting_snapshots(&related_settings, &settings_paths);
    let adapter_existed = adapter_existed(&destination);
    let skills_before = existing_skill_paths(&resources, &destination);
    let mut report = execute_install_plan(&plan, system);
    apply_related_settings(&related_settings, &settings_paths, &mut report);
    if let Err(message) = record_install_ownership(
        system,
        &resources,
        &destination,
        &related_settings,
        &setting_before,
        &settings_paths,
        adapter_existed,
        &skills_before,
        status,
        &report,
    ) {
        report.failures.push(InstallFailure {
            target: "ownership ledger".into(),
            message,
        });
    }
    print_report(&out, catalog, &report);
    for action in next_actions(&resources, &report) {
        out.next(action);
    }
    if mode == SelectionMode::Setup && report.failures.is_empty() {
        for action in SETUP_NEXT_ACTIONS {
            out.next(action);
        }
    }
    Ok(report.failures.is_empty())
}

fn native_windows_resources(catalog: &Catalog) -> Vec<Resource> {
    catalog
        .resources
        .iter()
        .filter(|resource| !resource.windows_wsl)
        .cloned()
        .collect()
}

fn first_wsl2_distribution(output: &str) -> Option<String> {
    output.replace('\0', "").lines().find_map(|line| {
        let columns = line
            .trim()
            .trim_start_matches('*')
            .split_whitespace()
            .collect::<Vec<_>>();
        let name = columns.first()?;
        (columns.last() == Some(&"2")
            && !name.eq_ignore_ascii_case("NAME")
            && !name.starts_with("docker-desktop"))
        .then(|| (*name).to_owned())
    })
}

fn prepare_wsl(system: &(dyn System + Sync), dry_run: bool) -> Result<bool> {
    if dry_run {
        println!("\nWould prepare WSL2; no changes made.");
        return Ok(true);
    }
    let mut distribution = system
        .run(&CommandSpec::new("wsl", ["--list", "--verbose"]))
        .ok()
        .filter(|result| result.success)
        .and_then(|result| first_wsl2_distribution(&result.stdout));
    if distribution.is_none() {
        if !Confirm::new("Install WSL2 with Ubuntu now? This may require elevation and a reboot.")
            .with_default(true)
            .prompt()?
        {
            println!("\nWhen ready, run: wsl --install -d Ubuntu");
            return Ok(true);
        }
        let result = system.run(&CommandSpec::new("wsl", ["--install", "-d", "Ubuntu"]))?;
        if !result.success {
            bail!("WSL2 installation failed: {}", result.stderr.trim());
        }
        distribution = Some("Ubuntu".into());
    }
    let distribution = distribution.expect("a distribution was found or installed");
    println!(
        "\nOpen it with `wsl -d \"{distribution}\"`, then run:\n\n  curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh\n"
    );
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn run_interactive(
    mode: SelectionMode,
    catalog: &Catalog,
    resources: Vec<Resource>,
    status: PrerequisiteStatus,
    platform: Platform,
    dry_run: bool,
    skill_destination: SkillDestination,
    system: &(dyn System + Sync),
) -> Result<bool> {
    let settings_paths = SettingsPaths::detect()?;
    let settings = curated_settings()
        .into_iter()
        .filter(|setting| {
            setting
                .related_resource
                .as_ref()
                .is_none_or(|related| resources.iter().any(|resource| &resource.id == related))
        })
        .collect::<Vec<_>>();
    let setting_states = settings
        .iter()
        .map(|spec| setting_state(spec, &settings_paths))
        .collect();
    let zed_present = settings_paths.zed_settings.exists();
    // Installed marks arrive from a background probe once the wizard is on
    // screen; starting all-false keeps the first frame instant.
    let installed = vec![false; resources.len()];
    let ownership_destination = skill_destination.clone();
    let setting_before = setting_snapshots(&settings, &settings_paths);
    let adapter_existed = adapter_existed(&ownership_destination);
    let skills_before = existing_skill_paths(&resources, &ownership_destination);
    let model = Model {
        mode,
        purpose: crate::wizard::WizardPurpose::Install,
        uninstall_dependencies: BTreeMap::new(),
        resources,
        profiles: catalog.profiles.clone(),
        installed,
        settings: settings.clone(),
        setting_states,
        zed_present,
        settings_paths: settings_paths.clone(),
        status,
        platform,
        dry_run,
        skill_destination,
    };
    match run_wizard(model, system)? {
        WizardOutcome::Cancelled => {
            println!("Cancelled; no changes made.");
            Ok(true)
        }
        WizardOutcome::NothingSelected => {
            println!("Nothing selected; no changes made.");
            Ok(true)
        }
        WizardOutcome::DryRun(plan, setting_changes) => {
            let out = Out::detect();
            out.title(mode.command(), "dry run");
            print_plan(&out, &plan);
            for change in &setting_changes {
                out.row(Mark::Off, "setting", change);
            }
            out.verdict(true, "Dry run; no changes made");
            Ok(true)
        }
        WizardOutcome::UninstallSelection(_) => {
            anyhow::bail!("install wizard returned an uninstall selection")
        }
        WizardOutcome::WikiSelection { feynman } => {
            crate::wiki::run_interactive_with_default(system, feynman)
        }
        WizardOutcome::Installed(mut report, actions, selected_resources) => {
            let wiki_feynman = selected_resources
                .iter()
                .any(|resource| resource.install_target == "@companion-ai/feynman");
            let has_wiki = selected_resources
                .iter()
                .any(|resource| resource.group == "Wiki");
            let generic_resources = selected_resources
                .into_iter()
                .filter(|resource| resource.group != "Wiki")
                .collect::<Vec<_>>();
            if let Err(message) = record_install_ownership(
                system,
                &generic_resources,
                &ownership_destination,
                &settings,
                &setting_before,
                &settings_paths,
                adapter_existed,
                &skills_before,
                status,
                &report,
            ) {
                report.failures.push(InstallFailure {
                    target: "ownership ledger".into(),
                    message,
                });
            }
            let out = Out::detect();
            out.blank();
            print_report(&out, catalog, &report);
            for action in actions {
                out.next(action);
            }
            if mode == SelectionMode::Setup && report.failures.is_empty() {
                for action in SETUP_NEXT_ACTIONS {
                    out.next(action);
                }
            }
            if has_wiki && report.failures.is_empty() {
                return crate::wiki::run_interactive_with_default(system, wiki_feynman);
            }
            Ok(report.failures.is_empty())
        }
    }
}

/// Which catalog resources are already on this machine. Uses the same
/// probes as post-install verification: manager list output for plugins and
/// packages, and the currently selected destination trees for skills.
pub(crate) fn detect_installed(
    resources: &[Resource],
    status: PrerequisiteStatus,
    system: &(dyn System + Sync),
    destination: &SkillDestination,
) -> Vec<bool> {
    let list_output = |present: bool, program: &str, args: &[&str]| {
        if !present {
            return None;
        }
        system
            .run_probe(&CommandSpec::new(program, args.iter().copied()))
            .ok()
            .filter(|result| result.success)
            .map(|result| format!("{}\n{}", result.stdout, result.stderr))
    };
    // Both list commands shell out to their manager; probe them concurrently.
    let (herdr_plugins, pi_packages) = std::thread::scope(|scope| {
        let herdr = scope.spawn(|| list_output(status.herdr, "herdr", &["plugin", "list"]));
        let pi = scope.spawn(|| list_output(status.pi, "pi", &["list"]));
        (
            herdr.join().expect("herdr probe thread"),
            pi.join().expect("pi probe thread"),
        )
    });
    let skill_trees = destination.trees();
    let selected_tools = system
        .home_dir()
        .map(|home| crate::manifest::selected_keys(&home))
        .unwrap_or_default();

    resources
        .iter()
        .map(|resource| {
            // This catalog row means “set up Feynman inside a chosen Vault.”
            // A user-level package cannot satisfy a destination that has not
            // been chosen yet, so keep the row actionable in the setup wizard.
            if resource.id == "pi-package:@companion-ai/feynman" && resource.group == "Wiki" {
                return false;
            }
            match resource.kind {
                // A tool is installed when mise manages it (it is in the
                // selection) or its binary is on PATH from any other installer
                // (brew, cargo, ...): both are honestly "installed".
                ResourceKind::Tool => {
                    selected_tools.contains(&resource.install_target)
                        || resource
                            .bin
                            .as_deref()
                            .is_some_and(|bin| system.command_exists(bin))
                }
                ResourceKind::HerdrPlugin => herdr_plugins.as_ref().is_some_and(|output| {
                    output.contains(resource.id.trim_start_matches("herdr-plugin:"))
                }),
                ResourceKind::PiPackage => pi_packages.as_ref().is_some_and(|output| {
                    // `pi list` prints npm specs for registry installs and
                    // directory paths for local ones; accept either shape.
                    let unscoped = resource
                        .install_target
                        .rsplit('/')
                        .next()
                        .unwrap_or(&resource.install_target);
                    let plain = unscoped.strip_prefix("pi-").unwrap_or(unscoped);
                    let last_component_is = |line: &str, name: &str| {
                        line.ends_with(&format!("/{name}")) || line.ends_with(&format!("\\{name}"))
                    };
                    output.lines().map(str::trim).any(|line| {
                        line.contains(&resource.install_target)
                            || last_component_is(line, unscoped)
                            || last_component_is(line, plain)
                    })
                }),
                ResourceKind::Skill => {
                    !skill_trees.is_empty()
                        && skill_trees.iter().all(|tree| {
                            crate::skills::skill_present_in(tree, &resource.install_target)
                                || crate::bundled_skills::provided_in_tree(
                                    &destination.home,
                                    tree,
                                    &resource.install_target,
                                )
                        })
                }
            }
        })
        .collect()
}

fn unapplied_related_settings(resources: &[Resource], paths: &SettingsPaths) -> Vec<SettingSpec> {
    curated_settings()
        .into_iter()
        .filter(|setting| {
            setting
                .related_resource
                .as_ref()
                .is_some_and(|related| resources.iter().any(|resource| resource.id == *related))
                && setting_state(setting, paths) == crate::settings::SettingState::NotApplied
        })
        .collect()
}

fn print_settings_plan(out: &Out, settings: &[SettingSpec], paths: &SettingsPaths) {
    for setting in settings {
        out.row(
            Mark::Off,
            "setting",
            format!(
                "{}: {}",
                setting.target_path(paths).display(),
                setting.change_summary().join(", ")
            ),
        );
    }
}

fn setting_snapshots(
    settings: &[SettingSpec],
    paths: &SettingsPaths,
) -> BTreeMap<String, Option<String>> {
    settings
        .iter()
        .map(|setting| {
            (
                setting.id.clone(),
                fs::read_to_string(setting.target_path(paths)).ok(),
            )
        })
        .collect()
}

fn existing_skill_paths(
    resources: &[Resource],
    destination: &SkillDestination,
) -> BTreeSet<std::path::PathBuf> {
    destination
        .trees()
        .into_iter()
        .flat_map(|tree| {
            resources
                .iter()
                .filter(|resource| resource.kind == ResourceKind::Skill)
                .map(move |resource| tree.join(&resource.install_target))
        })
        .filter(|path| path.symlink_metadata().is_ok())
        .collect()
}

fn adapter_existed(destination: &SkillDestination) -> bool {
    destination.agents.contains(&SkillAgent::OpenCode)
        && destination
            .opencode_adapter_path()
            .symlink_metadata()
            .is_ok()
}

#[allow(clippy::too_many_arguments)]
fn record_install_ownership(
    system: &dyn System,
    resources: &[Resource],
    destination: &SkillDestination,
    settings: &[SettingSpec],
    setting_before: &BTreeMap<String, Option<String>>,
    settings_paths: &SettingsPaths,
    adapter_existed: bool,
    skills_before: &BTreeSet<std::path::PathBuf>,
    prerequisite_status: PrerequisiteStatus,
    report: &InstallReport,
) -> Result<(), String> {
    use crate::ownership::{
        digest_path, InstallState, OwnedPathKind, OwnedResource, OwnershipScope, Receipt,
    };

    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let scope = match destination.scope {
        SkillScope::Global => OwnershipScope::Global,
        SkillScope::Project => OwnershipScope::Project {
            root: destination
                .project_root
                .canonicalize()
                .unwrap_or_else(|_| destination.project_root.clone()),
        },
    };
    let owned_id = |scope: &OwnershipScope, id: &str| match scope {
        OwnershipScope::Global => id.to_owned(),
        OwnershipScope::Project { root } => format!("project:{}:{id}", root.display()),
    };
    let succeeded = |resource: &Resource| {
        report.installed.contains(&resource.id)
            || (resource.kind == ResourceKind::Skill
                && report.installed.iter().any(|target| target == "skills"))
            || (resource.kind == ResourceKind::Tool
                && report.installed.iter().any(|target| target == "tools"))
    };
    let mut state = InstallState::load(&home)?;
    for resource in resources.iter().filter(|resource| succeeded(resource)) {
        let resource_scope = if resource.kind == ResourceKind::Skill {
            scope.clone()
        } else {
            OwnershipScope::Global
        };
        let id = owned_id(&resource_scope, &resource.id);
        let mut dependencies = resource
            .dependencies
            .iter()
            .map(|dependency| owned_id(&resource_scope, &format!("skill:{dependency}")))
            .collect::<Vec<_>>();
        match resource.kind {
            ResourceKind::PiPackage => dependencies.push("tool:pi".into()),
            ResourceKind::HerdrPlugin => dependencies.push("tool:herdr".into()),
            ResourceKind::Tool => dependencies.push("core:mise".into()),
            ResourceKind::Skill => {}
        }
        dependencies.push("core:loom".into());
        dependencies.sort();
        dependencies.dedup();
        let receipts = match resource.kind {
            ResourceKind::Skill => destination
                .trees()
                .into_iter()
                .map(|tree| tree.join(&resource.install_target))
                .filter(|path| path.is_dir() && !skills_before.contains(path))
                .map(|path| path.canonicalize().unwrap_or(path))
                .map(|path| {
                    Ok(Receipt::Path {
                        digest: digest_path(&path)?,
                        path,
                        path_kind: OwnedPathKind::Tree,
                        before: None,
                    })
                })
                .collect::<std::result::Result<Vec<_>, String>>()?,
            ResourceKind::PiPackage => vec![Receipt::Manager {
                manager: "pi".into(),
                target: resource.install_target.clone(),
            }],
            ResourceKind::HerdrPlugin => vec![Receipt::Manager {
                manager: "herdr".into(),
                target: resource.id.trim_start_matches("herdr-plugin:").into(),
            }],
            ResourceKind::Tool => std::iter::once(resource.install_target.clone())
                .chain(resource.companions.iter().cloned())
                .map(|key| Receipt::MiseTool { key })
                .collect(),
        };
        if !receipts.is_empty() {
            state.record(OwnedResource {
                id,
                scope: resource_scope,
                depends_on: dependencies,
                receipts,
            });
        }
    }
    if report.installed.iter().any(|target| target == "tools") {
        for (needed, id, key) in [
            (
                !prerequisite_status.pi
                    && resources
                        .iter()
                        .any(|resource| resource.kind == ResourceKind::PiPackage),
                "tool:pi",
                crate::manifest::PI_TOOL_KEY,
            ),
            (
                !prerequisite_status.herdr
                    && resources
                        .iter()
                        .any(|resource| resource.kind == ResourceKind::HerdrPlugin),
                "tool:herdr",
                "herdr",
            ),
        ] {
            if needed {
                state.record(OwnedResource {
                    id: id.into(),
                    scope: OwnershipScope::Global,
                    depends_on: vec!["core:loom".into(), "core:mise".into()],
                    receipts: vec![Receipt::MiseTool { key: key.into() }],
                });
            }
        }
    }
    for setting in settings
        .iter()
        .filter(|setting| report.installed.contains(&setting.id))
    {
        let path = setting.target_path(settings_paths).to_path_buf();
        if !path.is_file() {
            continue;
        }
        state.record(OwnedResource {
            id: owned_id(&OwnershipScope::Global, &format!("setting:{}", setting.id)),
            scope: OwnershipScope::Global,
            depends_on: setting
                .related_resource
                .iter()
                .map(|id| id.to_owned())
                .collect(),
            receipts: vec![Receipt::Path {
                digest: digest_path(&path)?,
                path,
                path_kind: OwnedPathKind::File,
                before: setting_before.get(&setting.id).cloned().flatten(),
            }],
        });
    }
    if !adapter_existed
        && destination.agents.contains(&SkillAgent::OpenCode)
        && report.installed.iter().any(|target| target == "skills")
    {
        let path = destination.opencode_adapter_path();
        if path.is_file() {
            let path = path.canonicalize().unwrap_or(path);
            state.record(OwnedResource {
                id: owned_id(&scope, "adapter:opencode"),
                scope: scope.clone(),
                depends_on: vec!["core:loom".into()],
                receipts: vec![Receipt::Path {
                    digest: digest_path(&path)?,
                    path,
                    path_kind: OwnedPathKind::File,
                    before: None,
                }],
            });
        }
    }
    state.save(&home)
}

fn apply_related_settings(
    settings: &[SettingSpec],
    paths: &SettingsPaths,
    report: &mut InstallReport,
) {
    for setting in settings {
        if !setting
            .related_resource
            .as_ref()
            .is_some_and(|resource| report.installed.contains(resource))
        {
            continue;
        }
        match apply_setting(setting, paths) {
            Ok(_) => report.installed.push(setting.id.clone()),
            Err(error) => report.failures.push(crate::InstallFailure {
                target: setting.id.clone(),
                message: error.to_string(),
            }),
        }
    }
}

pub(crate) fn next_actions(resources: &[Resource], report: &InstallReport) -> Vec<String> {
    let mut actions = Vec::new();
    for resource in resources {
        let installed = report.installed.contains(&resource.id)
            || (resource.kind == ResourceKind::Skill
                && report.installed.iter().any(|target| target == "skills"));
        if installed && !actions.contains(&resource.next_action) {
            actions.push(resource.next_action.clone());
        }
    }
    actions
}

fn print_report(out: &Out, catalog: &Catalog, report: &InstallReport) {
    let settings = curated_settings();
    let label = |target: &str| {
        catalog
            .resources
            .iter()
            .find(|resource| resource.id == target)
            .map(|resource| resource.label.clone())
            .or_else(|| {
                settings
                    .iter()
                    .find(|setting| setting.id == target)
                    .map(|setting| setting.label.clone())
            })
            .unwrap_or_else(|| target.to_owned())
    };
    for target in &report.installed {
        out.row(Mark::Ok, &label(target), "installed");
    }
    for failure in &report.failures {
        out.row(Mark::Bad, &label(&failure.target), &failure.message);
    }
    let installed = report.installed.len();
    let failed = report.failures.len();
    if failed == 0 {
        out.verdict(true, format!("{installed} installed"));
    } else {
        out.verdict(false, format!("{installed} installed · {failed} failed"));
    }
}

pub fn resolve_selectors(catalog: &Catalog, selectors: &Selectors) -> Result<Vec<Resource>> {
    let mut selected = Vec::new();
    for (kind, values) in [
        (ResourceKind::Skill, &selectors.skills),
        (ResourceKind::PiPackage, &selectors.pi_packages),
        (ResourceKind::HerdrPlugin, &selectors.herdr_plugins),
        (ResourceKind::Tool, &selectors.tools),
    ] {
        for value in values {
            let matches = catalog
                .resources
                .iter()
                .filter(|resource| {
                    resource.kind == kind
                        && (resource.id == *value
                            || resource.install_target == *value
                            || resource.label.eq_ignore_ascii_case(value)
                            || resource.id.ends_with(&format!(":{value}"))
                            || resource.install_target.ends_with(&format!("/{value}")))
                })
                .cloned()
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [resource] => {
                    if !selected.contains(resource) {
                        selected.push(resource.clone());
                    }
                }
                [] => bail!("unknown {kind}: {value}"),
                _ => bail!("ambiguous {kind}: {value}"),
            }
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "loom-app-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    struct NoCommands;

    impl System for NoCommands {
        fn command_exists(&self, _name: &str) -> bool {
            false
        }

        fn refresh_path(&self) {}

        fn run(&self, command: &CommandSpec) -> Result<crate::CommandResult> {
            panic!("dry run executed {}", command.display())
        }
    }

    struct InstalledSkillSystem {
        home: std::path::PathBuf,
        commands: std::sync::Mutex<Vec<String>>,
    }

    impl System for InstalledSkillSystem {
        fn command_exists(&self, _name: &str) -> bool {
            false
        }

        fn refresh_path(&self) {}

        fn run(&self, command: &CommandSpec) -> Result<crate::CommandResult> {
            self.commands.lock().unwrap().push(command.display());
            Ok(crate::CommandResult {
                success: false,
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn home_dir(&self) -> Option<std::path::PathBuf> {
            Some(self.home.clone())
        }

        fn current_dir(&self) -> Option<std::path::PathBuf> {
            Some(self.home.clone())
        }
    }

    #[test]
    fn scripted_setup_skips_resources_that_are_already_installed() {
        let root = temp_root("noop");
        let skill = Resource {
            id: "skill:already-there".into(),
            kind: ResourceKind::Skill,
            group: "test".into(),
            label: "Already there".into(),
            description: String::new(),
            install_target: "already-there".into(),
            next_action: String::new(),
            dependencies: Vec::new(),
            bin: None,
            version: None,
            source: None,
            windows_wsl: false,
            companions: Vec::new(),
            bundled_skills: Vec::new(),
        };
        let tree = SkillAgent::AgentsStandard.global_skill_tree(&root);
        std::fs::create_dir_all(tree.join("already-there")).unwrap();
        std::fs::write(tree.join("already-there/SKILL.md"), "installed").unwrap();
        let catalog = Catalog {
            schema_version: 1,
            profiles: Vec::new(),
            resources: vec![skill],
        };
        let selectors = Selectors {
            skills: vec!["already-there".into()],
            ..Selectors::default()
        };

        let system = InstalledSkillSystem {
            home: root.clone(),
            commands: std::sync::Mutex::new(Vec::new()),
        };
        assert!(install_selected(
            SelectionMode::Setup,
            &catalog,
            &selectors,
            &[SkillAgent::AgentsStandard],
            SkillScope::Global,
            false,
            true,
            false,
            &system,
        )
        .unwrap());
        assert!(system.commands.into_inner().unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    struct InstalledPackageSystem {
        home: std::path::PathBuf,
    }

    impl System for InstalledPackageSystem {
        fn command_exists(&self, name: &str) -> bool {
            name == "pi"
        }

        fn refresh_path(&self) {}

        fn run(&self, command: &CommandSpec) -> Result<crate::CommandResult> {
            Ok(crate::CommandResult {
                success: command.program == "pi",
                stdout: if command.program == "pi" {
                    "@example/already-there\n".into()
                } else {
                    String::new()
                },
                stderr: String::new(),
            })
        }

        fn home_dir(&self) -> Option<std::path::PathBuf> {
            Some(self.home.clone())
        }

        fn current_dir(&self) -> Option<std::path::PathBuf> {
            Some(self.home.clone())
        }
    }

    struct GlobalFeynmanSystem;

    impl System for GlobalFeynmanSystem {
        fn command_exists(&self, name: &str) -> bool {
            name == "pi"
        }

        fn refresh_path(&self) {}

        fn run(&self, command: &CommandSpec) -> Result<crate::CommandResult> {
            Ok(crate::CommandResult {
                success: command.program == "pi",
                stdout: "User packages:\n  npm:@companion-ai/feynman@0.3.47\n".into(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn global_feynman_does_not_satisfy_vault_local_setup() {
        let root = temp_root("vault-feynman");
        let system = GlobalFeynmanSystem;
        let destination = SkillDestination::new(Vec::new(), SkillScope::Global, &root, &root);
        let resource = Resource {
            id: "pi-package:@companion-ai/feynman".into(),
            kind: ResourceKind::PiPackage,
            group: "Wiki".into(),
            label: "feynman".into(),
            description: String::new(),
            install_target: "@companion-ai/feynman".into(),
            next_action: String::new(),
            dependencies: Vec::new(),
            bin: None,
            version: Some("0.3.47".into()),
            source: None,
            windows_wsl: false,
            companions: Vec::new(),
            bundled_skills: Vec::new(),
        };
        let status = PrerequisiteStatus {
            pi: true,
            herdr: false,
            mise: true,
        };

        assert_eq!(
            detect_installed(&[resource], status, &system, &destination),
            [false]
        );
    }

    #[test]
    fn successful_tool_sync_records_companions_and_required_runtimes() {
        let root = temp_root("record-tools");
        std::fs::create_dir_all(&root).unwrap();
        let system = InstalledPackageSystem { home: root.clone() };
        let destination = SkillDestination::new(Vec::new(), SkillScope::Global, &root, &root);
        let resources = vec![
            Resource {
                id: "tool:search".into(),
                kind: ResourceKind::Tool,
                group: "test".into(),
                label: "Search".into(),
                description: String::new(),
                install_target: "search".into(),
                next_action: String::new(),
                dependencies: Vec::new(),
                bin: Some("search".into()),
                version: None,
                source: None,
                windows_wsl: false,
                companions: vec!["search-helper".into()],
                bundled_skills: Vec::new(),
            },
            Resource {
                id: "pi-package:chat".into(),
                kind: ResourceKind::PiPackage,
                group: "test".into(),
                label: "Chat".into(),
                description: String::new(),
                install_target: "@example/chat".into(),
                next_action: String::new(),
                dependencies: Vec::new(),
                bin: None,
                version: Some("1.0.0".into()),
                source: None,
                windows_wsl: false,
                companions: Vec::new(),
                bundled_skills: Vec::new(),
            },
        ];
        let report = InstallReport {
            installed: vec!["tools".into(), "pi-package:chat".into()],
            failures: Vec::new(),
        };

        record_install_ownership(
            &system,
            &resources,
            &destination,
            &[],
            &BTreeMap::new(),
            &SettingsPaths {
                herdr_config: root.join("herdr.toml"),
                zed_settings: root.join("zed.json"),
                zed_keymap: root.join("keymap.json"),
                pi_fff_config: root.join("fff.json"),
                diagrams: root.join("diagrams.json"),
            },
            false,
            &BTreeSet::new(),
            PrerequisiteStatus {
                pi: false,
                herdr: true,
                mise: true,
            },
            &report,
        )
        .unwrap();

        let state = crate::ownership::InstallState::load(&root).unwrap();
        assert_eq!(state.resources["tool:search"].receipts.len(), 2);
        assert!(state.resources.contains_key("tool:pi"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_op_package_selection_does_not_claim_preexisting_ownership() {
        let root = temp_root("record-noop");
        let package = Resource {
            id: "pi-package:already-there".into(),
            kind: ResourceKind::PiPackage,
            group: "test".into(),
            label: "Already there".into(),
            description: String::new(),
            install_target: "@example/already-there".into(),
            next_action: String::new(),
            dependencies: Vec::new(),
            bin: None,
            version: Some("1.0.0".into()),
            source: None,
            windows_wsl: false,
            companions: Vec::new(),
            bundled_skills: Vec::new(),
        };
        let catalog = Catalog {
            schema_version: 1,
            profiles: Vec::new(),
            resources: vec![package],
        };
        let selectors = Selectors {
            pi_packages: vec!["already-there".into()],
            ..Selectors::default()
        };

        assert!(install_selected(
            SelectionMode::Setup,
            &catalog,
            &selectors,
            &[],
            SkillScope::Global,
            false,
            true,
            false,
            &InstalledPackageSystem { home: root.clone() },
        )
        .unwrap());
        assert!(crate::ownership::InstallState::load(&root)
            .unwrap()
            .resources
            .is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn adapter_presence_does_not_require_utf8_content() {
        let root = temp_root("non-utf8-adapter");
        let destination =
            SkillDestination::new(vec![SkillAgent::OpenCode], SkillScope::Global, &root, &root);
        let adapter = destination.opencode_adapter_path();
        std::fs::create_dir_all(adapter.parent().unwrap()).unwrap();
        std::fs::write(&adapter, [0xff]).unwrap();

        assert!(adapter_existed(&destination));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn ownership_does_not_adopt_a_preexisting_skill_dependency() {
        let root = temp_root("preexisting-skill");
        let tree = root.join(".agents/skills");
        std::fs::create_dir_all(tree.join("dependency")).unwrap();
        std::fs::write(tree.join("dependency/SKILL.md"), "custom").unwrap();
        let resource = Resource {
            id: "skill:dependency".into(),
            kind: ResourceKind::Skill,
            group: "test".into(),
            label: "dependency".into(),
            description: String::new(),
            install_target: "dependency".into(),
            next_action: String::new(),
            dependencies: Vec::new(),
            bin: None,
            version: None,
            source: None,
            windows_wsl: false,
            companions: Vec::new(),
            bundled_skills: Vec::new(),
        };
        let destination = SkillDestination::new(
            vec![SkillAgent::AgentsStandard],
            SkillScope::Global,
            &root,
            &root,
        );
        let skills_before = existing_skill_paths(std::slice::from_ref(&resource), &destination);
        let report = InstallReport {
            installed: vec!["skills".into()],
            failures: Vec::new(),
        };

        record_install_ownership(
            &InstalledSkillSystem {
                home: root.clone(),
                commands: std::sync::Mutex::new(Vec::new()),
            },
            &[resource],
            &destination,
            &[],
            &BTreeMap::new(),
            &SettingsPaths {
                herdr_config: root.join("herdr.toml"),
                zed_settings: root.join("zed.json"),
                zed_keymap: root.join("keymap.json"),
                pi_fff_config: root.join("fff.json"),
                diagrams: root.join("diagrams.json"),
            },
            false,
            &skills_before,
            PrerequisiteStatus {
                pi: false,
                herdr: false,
                mise: false,
            },
            &report,
        )
        .unwrap();

        assert!(crate::ownership::InstallState::load(&root)
            .unwrap()
            .resources
            .is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn wsl_dry_run_executes_nothing() {
        assert!(prepare_wsl(&NoCommands, true).unwrap());
    }

    #[test]
    fn wsl_distribution_parser_requires_a_user_wsl2_distro() {
        let output = "  N\0A\0M\0E\0  S\0T\0A\0T\0E\0  V\0E\0R\0S\0I\0O\0N\0\r\0\n\0* U\0b\0u\0n\0t\0u\0  R\0u\0n\0n\0i\0n\0g\0  2\0\r\0\n\0";
        assert_eq!(first_wsl2_distribution(output), Some("Ubuntu".into()));
        assert_eq!(
            first_wsl2_distribution(
                "NAME STATE VERSION\ndocker-desktop Running 2\nDebian Stopped 1\nUbuntu Stopped 2\n"
            ),
            Some("Ubuntu".into())
        );
        assert_eq!(
            first_wsl2_distribution(
                "NAME STATE VERSION\ndocker-desktop Running 2\nDebian Stopped 1\n"
            ),
            None
        );
    }

    #[test]
    fn native_windows_hides_wsl_resources() {
        let catalog = Catalog::embedded().unwrap();

        let visible = native_windows_resources(&catalog);

        assert!(!visible.iter().any(|resource| resource.label == "chat"));
        assert!(!visible.iter().any(|resource| resource.label == "herdr"));
        assert!(visible.iter().any(|resource| resource.label == "pi"));
    }
}
