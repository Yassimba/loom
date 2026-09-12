use crate::session::{InstallOwnership, InstallSession};
use crate::settings::{curated_settings, setting_state, SettingSpec, SettingsPaths};
use crate::ui::{confirm_plan, print_plan, Mark, Out};
use crate::wizard::{run_wizard, Model, WizardOutcome};
use crate::{
    build_install_plan, expand_skill_dependencies, Catalog, CommandSpec, InstallFailure,
    InstallReport, Platform, PrerequisiteStatus, Resource, ResourceKind, SkillAgent,
    SkillDestination, SkillScope, System,
};
use anyhow::{bail, Context, Result};
use inquire::Confirm;
use std::collections::{BTreeMap, HashSet};

pub(crate) const SETUP_NEXT_ACTION: &str = "run `loom status` to verify the setup";

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
    pub mcp_servers: Vec<String>,
}

impl Selectors {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
            && self.pi_packages.is_empty()
            && self.herdr_plugins.is_empty()
            && self.tools.is_empty()
            && self.mcp_servers.is_empty()
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

    let selected = resolve_selectors(catalog, selectors)?;
    let mut resources =
        expand_skill_dependencies(&catalog.resources, selected, &destination.agents);
    include_automatic_pi_package(
        catalog,
        &mut resources,
        mode == SelectionMode::Setup && status.pi,
    );
    if platform == Platform::Windows {
        if let Some(resource) = resources.iter().find(|resource| resource.windows_wsl) {
            bail!(
                "{} requires WSL2 on Windows. Open Ubuntu and run this loom command there.",
                resource.label
            );
        }
    }
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
    if resources.is_empty() {
        Out::detect().verdict(true, "Nothing selected; no changes made");
        return Ok(true);
    }
    // Preflight before installed filtering: conflicts must not become silent no-ops.
    if resources.iter().any(|r| r.kind == ResourceKind::McpServer) {
        build_install_plan(&resources, status, platform, &destination)?;
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
    let ownership = InstallOwnership::capture(
        &resources,
        &related_settings,
        &settings_paths,
        &destination,
        status,
    );
    let mut session = InstallSession::new(plan, related_settings, settings_paths);
    session.ownership = Some(ownership);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let mut report = session.run_attempt(system, &cancelled, &mut |_, _| {});
    if let Err(message) = session.record_ownership(system) {
        report.failures.push(InstallFailure {
            target: "ownership ledger".into(),
            message,
        });
    }
    print_report(&out, catalog, &report, true);
    out.next(install_next_action(mode, &resources, &report));
    Ok(report.failures.is_empty())
}

fn include_automatic_pi_package(
    catalog: &Catalog,
    selected: &mut Vec<Resource>,
    pi_installed: bool,
) {
    let needs_pi = pi_installed
        || selected.iter().any(|resource| {
            resource.id == "tool:pi"
                || (resource.kind == ResourceKind::PiPackage && !resource.is_automatic_pi_package())
        });
    if needs_pi && !selected.iter().any(Resource::is_automatic_pi_package) {
        if let Some(resource) = catalog
            .resources
            .iter()
            .find(|resource| resource.is_automatic_pi_package())
        {
            selected.push(resource.clone());
        }
    }
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
        Out::detect().verdict(true, "Dry run; would prepare WSL2, no changes made");
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
    let model = Model {
        mode,
        purpose: crate::wizard::WizardPurpose::Install,
        uninstall_dependencies: BTreeMap::new(),
        resources,
        profiles: catalog.profiles.clone(),
        installed,
        settings,
        setting_states,
        zed_present,
        settings_paths,
        status,
        platform,
        dry_run,
        skill_destination,
    };
    match run_wizard(model, system)? {
        WizardOutcome::Cancelled => {
            Out::detect().verdict(true, "Cancelled; no changes made");
            Ok(true)
        }
        WizardOutcome::NothingSelected => {
            Out::detect().verdict(true, "Nothing selected; no changes made");
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
        WizardOutcome::Installed {
            report, resources, ..
        } => {
            let out = Out::detect();
            out.blank();
            // The wizard already showed every task; repeat only failures.
            print_report(&out, catalog, &report, false);
            out.next(install_next_action(mode, &resources, &report));
            Ok(report.failures.is_empty())
        }
    }
}

/// Which catalog resources are already on this machine. Uses the same
/// probes as post-install verification: manager list output for plugins and
/// packages, and the currently selected destination trees for skills.
fn pi_packages_from_settings(home: &std::path::Path) -> Option<String> {
    pi_packages_listing(
        &crate::settings::pi_agent_dir(home).join("settings.json"),
        "User packages:",
    )
}

/// Prefer Herdr's registry file so the wizard probe does not boot `herdr`.
fn herdr_plugins_from_registry(home: &std::path::Path) -> Option<String> {
    let content =
        std::fs::read_to_string(crate::settings::herdr_dir(home).join("plugins.json")).ok()?;
    let plugins = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let plugins = plugins.as_array()?;
    let mut listed = String::new();
    for plugin in plugins {
        if let Some(id) = plugin.get("plugin_id").and_then(serde_json::Value::as_str) {
            listed.push_str(id);
            listed.push('\n');
        }
    }
    Some(listed)
}

/// A `pi list`-shaped listing read straight from a Pi settings file, so the
/// installed-state probe never has to boot the Node CLI. `None` when the file
/// cannot be read safely; a missing file lists nothing.
pub(crate) fn pi_packages_listing(
    settings_path: &std::path::Path,
    heading: &str,
) -> Option<String> {
    let content = match std::fs::read_to_string(settings_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(format!("{heading}\n"));
        }
        Err(_) => return None,
    };
    let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
    let mut listed = format!("{heading}\n");
    let Some(packages) = settings.get("packages") else {
        return Some(listed);
    };
    let packages = packages.as_array()?;
    for package in packages {
        if let Some(source) = package
            .as_str()
            .or_else(|| package.get("source").and_then(serde_json::Value::as_str))
        {
            let path = std::path::Path::new(source);
            let resolved = path
                .is_relative()
                .then(|| {
                    settings_path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new(""))
                        .join(path)
                })
                .filter(|path| path.is_dir())
                .map(|path| path.canonicalize().unwrap_or(path));
            listed.push_str("  ");
            listed.push_str(
                resolved
                    .as_deref()
                    .and_then(std::path::Path::to_str)
                    .unwrap_or(source),
            );
            listed.push('\n');
        }
    }
    Some(listed)
}

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
            .map(|result| result.stdout)
    };
    let home = system.home_dir();
    // Prefer on-disk registries: `pi list` and `herdr plugin list` boot CLIs.
    let pi_packages = home
        .as_deref()
        .and_then(pi_packages_from_settings)
        .or_else(|| list_output(status.pi, "pi", &["list"]));
    let herdr_plugins = home
        .as_deref()
        .and_then(herdr_plugins_from_registry)
        .or_else(|| list_output(status.herdr, "herdr", &["plugin", "list"]));
    let skill_trees = destination.trees();
    let skill_names = skill_trees
        .iter()
        .map(|tree| skill_names_in(tree))
        .collect::<Vec<_>>();
    let selected_tools = home
        .map(|home| crate::manifest::selected_keys(&home))
        .unwrap_or_default();

    resources
        .iter()
        .map(|resource| {
            // Wiki rows are checked only after choosing a Vault, never against
            // this shell's global or project installation.
            if resource.group == "Wiki" {
                return false;
            }
            match resource.kind {
                ResourceKind::McpServer => crate::mcp::Server::from_name(&resource.install_target)
                    .is_ok_and(|server| crate::mcp::configured(server, destination, system)),
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
                    crate::install::pi_package_installed(output, &resource.install_target, false)
                }),
                ResourceKind::Skill => {
                    !skill_trees.is_empty()
                        && skill_trees.iter().zip(&skill_names).all(|(tree, names)| {
                            names.contains(&resource.install_target)
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

fn skill_names_in(tree: &std::path::Path) -> HashSet<String> {
    let Ok(entries) = std::fs::read_dir(tree) else {
        return HashSet::new();
    };
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let name = name.to_str()?.to_owned();
            tree.join(&name).join("SKILL.md").is_file().then_some(name)
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

pub(crate) fn install_next_action(
    mode: SelectionMode,
    resources: &[Resource],
    report: &InstallReport,
) -> String {
    if !report.failures.is_empty() {
        format!(
            "run `loom {}` to retry; completed work stays installed",
            mode.command()
        )
    } else if mode == SelectionMode::Setup {
        SETUP_NEXT_ACTION.into()
    } else {
        next_actions(resources, report)
            .into_iter()
            .next()
            .unwrap_or_else(|| SETUP_NEXT_ACTION.into())
    }
}

fn print_report(out: &Out, catalog: &Catalog, report: &InstallReport, list_installed: bool) {
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
    for target in report.installed.iter().filter(|_| list_installed) {
        out.row(
            Mark::Ok,
            &label(target),
            if target.starts_with("mcp-server:") {
                "configured; live health not checked"
            } else {
                "installed"
            },
        );
    }
    for failure in &report.failures {
        out.row(
            Mark::Bad,
            &label(&failure.target),
            crate::ui::failure_text(&failure.message),
        );
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
        (ResourceKind::McpServer, &selectors.mcp_servers),
    ] {
        for value in values {
            let mut matches = catalog.resources.iter().filter(|resource| {
                resource.kind == kind
                    && (resource.id == *value
                        || resource.install_target == *value
                        || resource.label.eq_ignore_ascii_case(value)
                        || resource.id.ends_with(&format!(":{value}"))
                        || resource.install_target.ends_with(&format!("/{value}")))
            });
            match (matches.next(), matches.next()) {
                (Some(resource), None) => {
                    if !selected.contains(resource) {
                        selected.push(resource.clone());
                    }
                }
                (None, _) => bail!("unknown {kind}: {value}"),
                _ => bail!("ambiguous {kind}: {value}"),
            }
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::adapter_existed;

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

    #[test]
    fn pi_package_paths_are_relative_to_the_settings_file() {
        let root = temp_root("relative-pi-package");
        let settings = root.join("home/projects/wiki/.pi/settings.json");
        let package = root.join("home/.local/package");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(&settings, r#"{"packages":["../../../.local/package"]}"#).unwrap();

        let listed = pi_packages_listing(&settings, "Project packages:").unwrap();
        let listed_path = listed
            .lines()
            .find_map(|line| line.strip_prefix("  "))
            .expect("listing includes a package path");
        assert_eq!(
            std::path::Path::new(listed_path).canonicalize().unwrap(),
            package.canonicalize().unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
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
    fn pi_loom_is_automatically_added_when_pi_is_present_or_selected() {
        let catalog = Catalog::embedded().unwrap();
        let pi = catalog
            .resources
            .iter()
            .find(|resource| resource.id == "tool:pi")
            .unwrap()
            .clone();

        let mut selected = Vec::new();
        include_automatic_pi_package(&catalog, &mut selected, true);
        assert!(selected.iter().any(Resource::is_automatic_pi_package));

        let mut selected = vec![pi];
        include_automatic_pi_package(&catalog, &mut selected, false);
        assert!(selected.iter().any(Resource::is_automatic_pi_package));

        let mut implement = catalog
            .resources
            .iter()
            .find(|resource| resource.id == "skill:implement")
            .unwrap()
            .clone();
        implement.dependencies = vec!["pi-package:@yassimba/pi-loom-mermaid".into()];
        let mut expanded =
            expand_skill_dependencies(&catalog.resources, vec![implement], &[SkillAgent::Pi]);
        assert!(expanded.iter().any(|resource| {
            resource.kind == ResourceKind::PiPackage && !resource.is_automatic_pi_package()
        }));
        include_automatic_pi_package(&catalog, &mut expanded, false);
        assert!(expanded.iter().any(Resource::is_automatic_pi_package));
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
                    "User packages:\n  npm:@example/already-there\n".into()
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

    struct SettingsOnlySystem {
        home: std::path::PathBuf,
    }

    impl System for SettingsOnlySystem {
        fn command_exists(&self, name: &str) -> bool {
            name == "pi"
        }

        fn refresh_path(&self) {}

        fn run(&self, command: &CommandSpec) -> Result<crate::CommandResult> {
            panic!(
                "installed-state detection shelled out to {}",
                command.display()
            )
        }

        fn home_dir(&self) -> Option<std::path::PathBuf> {
            Some(self.home.clone())
        }
    }

    #[test]
    fn installed_pi_packages_are_read_directly_from_settings() {
        let root = temp_root("direct-pi-settings");
        std::fs::create_dir_all(root.join(".pi/agent")).unwrap();
        std::fs::create_dir_all(root.join(".pi/agent/plugins/skill-autocomplete")).unwrap();
        std::fs::write(
            root.join(".pi/agent/plugins/skill-autocomplete/package.json"),
            r#"{"name":"@yassimba/pi-skill-autocomplete"}"#,
        )
        .unwrap();
        std::fs::write(
            root.join(".pi/agent/settings.json"),
            r#"{"packages":["npm:pi-subagents@0.66.0",{"source":"git:github.com/ayghri/i-have-adhd@abc"},"plugins/skill-autocomplete"]}"#,
        )
        .unwrap();
        let catalog = Catalog::embedded().unwrap();
        let resources = [
            "pi-subagents",
            "i-have-adhd",
            "@yassimba/pi-skill-autocomplete",
            "pi-web-access",
        ]
        .into_iter()
        .map(|target| {
            catalog
                .resources
                .iter()
                .find(|resource| resource.install_target == target)
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();
        let destination = SkillDestination::new(Vec::new(), SkillScope::Global, &root, &root);

        assert_eq!(
            detect_installed(
                &resources,
                PrerequisiteStatus {
                    pi: true,
                    herdr: false,
                    mise: true,
                },
                &SettingsOnlySystem { home: root.clone() },
                &destination,
            ),
            [true, true, true, false]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_herdr_plugins_are_read_directly_from_the_registry() {
        const MARKER: &str = "LOOM_TEST_HERDR_REGISTRY";
        let Some(root) = std::env::var_os(MARKER) else {
            let root = temp_root("direct-herdr-registry");
            std::fs::create_dir_all(&root).unwrap();
            // Isolate XDG in the child; never mutate the parallel test harness's environment.
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "app::tests::installed_herdr_plugins_are_read_directly_from_the_registry",
                ])
                .env(MARKER, &root)
                .env("HOME", &root)
                .env("USERPROFILE", &root)
                .env("XDG_CONFIG_HOME", root.join(".config"))
                .status()
                .unwrap();
            std::fs::remove_dir_all(root).unwrap();
            assert!(status.success());
            return;
        };
        let root = std::path::PathBuf::from(root);
        let registry = crate::settings::herdr_dir(&root);
        assert!(registry.starts_with(&root), "registry escaped fixture home");
        std::fs::create_dir_all(&registry).unwrap();
        std::fs::write(
            registry.join("plugins.json"),
            r#"[{"plugin_id":"annotate","name":"Annotate"}]"#,
        )
        .unwrap();
        let catalog = Catalog::embedded().unwrap();
        let annotate = catalog
            .resources
            .iter()
            .find(|resource| resource.id == "herdr-plugin:annotate")
            .unwrap()
            .clone();
        let other = Resource {
            id: "herdr-plugin:other".into(),
            kind: ResourceKind::HerdrPlugin,
            group: annotate.group.clone(),
            label: "other".into(),
            description: String::new(),
            install_target: "other".into(),
            next_action: String::new(),
            dependencies: Vec::new(),
            bin: None,
            version: None,
            source: None,
            windows_wsl: false,
            companions: Vec::new(),
            bundled_skills: Vec::new(),
        };
        let resources = vec![annotate, other];
        let destination = SkillDestination::new(Vec::new(), SkillScope::Global, &root, &root);

        assert_eq!(
            detect_installed(
                &resources,
                PrerequisiteStatus {
                    pi: false,
                    herdr: true,
                    mise: false,
                },
                &SettingsOnlySystem { home: root.clone() },
                &destination,
            ),
            [true, false]
        );
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

        InstallOwnership::capture(
            &resources,
            &[],
            &SettingsPaths {
                herdr_config: root.join("herdr.toml"),
                zed_settings: root.join("zed.json"),
                zed_keymap: root.join("keymap.json"),
                pi_fff_config: root.join("fff.json"),
                pi_adhd_flag: root.join(".i-have-adhd-always"),
            },
            &destination,
            PrerequisiteStatus {
                pi: false,
                herdr: true,
                mise: true,
            },
        )
        .record(&system, &report.installed)
        .unwrap();

        let state = crate::ownership::InstallState::load(&root).unwrap();
        assert_eq!(state.resources["tool:search"].receipts.len(), 2);
        assert!(state.resources.contains_key("tool:pi"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_op_package_selection_does_not_claim_preexisting_ownership() {
        let root = temp_root("record-noop");
        std::fs::create_dir_all(root.join(".pi/agent")).unwrap();
        std::fs::write(
            root.join(".pi/agent/settings.json"),
            r#"{"packages":["npm:@example/already-there"]}"#,
        )
        .unwrap();
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
        let report = InstallReport {
            installed: vec!["skills".into()],
            failures: Vec::new(),
        };

        InstallOwnership::capture(
            &[resource],
            &[],
            &SettingsPaths {
                herdr_config: root.join("herdr.toml"),
                zed_settings: root.join("zed.json"),
                zed_keymap: root.join("keymap.json"),
                pi_fff_config: root.join("fff.json"),
                pi_adhd_flag: root.join(".i-have-adhd-always"),
            },
            &destination,
            PrerequisiteStatus {
                pi: false,
                herdr: false,
                mise: false,
            },
        )
        .record(
            &InstalledSkillSystem {
                home: root.clone(),
                commands: std::sync::Mutex::new(Vec::new()),
            },
            &report.installed,
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
