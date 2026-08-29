use crate::settings::{apply_setting, curated_settings, setting_state, SettingSpec, SettingsPaths};
use crate::ui::{confirm_plan, print_plan, Mark, Out};
use crate::wizard::{run_wizard, Model, WizardOutcome};
use crate::{
    build_install_plan, execute_install_plan, expand_skill_dependencies, Catalog, CommandSpec,
    InstallFailure, InstallReport, NodeStatus, Platform, PrerequisiteStatus, Resource,
    ResourceKind, SkillAgent, SkillDestination, SkillScope, System,
};
use anyhow::{bail, Context, Result};
use inquire::Confirm;

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
        npm: system.command_exists("npm"),
        mise: crate::manifest::mise_available(system),
        node: NodeStatus::detect(system),
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

    let resources =
        expand_skill_dependencies(&catalog.resources, resolve_selectors(catalog, selectors)?);
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
    let already_installed = InstallReport {
        installed: resources
            .iter()
            .zip(&installed)
            .filter(|(resource, installed)| {
                **installed
                    && matches!(
                        resource.kind,
                        ResourceKind::PiPackage | ResourceKind::HerdrPlugin
                    )
            })
            .map(|(resource, _)| resource.id.clone())
            .collect(),
        failures: Vec::new(),
    };
    let resources = resources
        .into_iter()
        .zip(installed)
        .filter_map(|(resource, installed)| (!installed).then_some(resource))
        .collect::<Vec<_>>();
    if resources.is_empty() {
        if !dry_run {
            crate::status::record_managed_resources(system, &already_installed)
                .map_err(anyhow::Error::msg)?;
        }
        let out = Out::detect();
        out.title(mode.command(), "already configured");
        out.verdict(
            true,
            "Everything selected is already set up; no changes made",
        );
        return Ok(true);
    }
    let plan = build_install_plan(&resources, &[], status, platform, &destination)?;
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
    crate::status::record_managed_resources(system, &already_installed)
        .map_err(anyhow::Error::msg)?;

    let mut report = execute_install_plan(&plan, system);
    apply_related_settings(&related_settings, &settings_paths, &mut report);
    if let Err(message) = crate::status::record_managed_resources(system, &report) {
        report.failures.push(InstallFailure {
            target: "resource registry".into(),
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
    let model = Model {
        mode,
        resources,
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
        WizardOutcome::Installed(mut report, actions) => {
            if let Err(message) = crate::status::record_managed_resources(system, &report) {
                report.failures.push(InstallFailure {
                    target: "resource registry".into(),
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
        .map(|resource| match resource.kind {
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
                    && skill_trees
                        .iter()
                        .all(|tree| crate::skills::skill_present_in(tree, &resource.install_target))
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

fn resolve_selectors(catalog: &Catalog, selectors: &Selectors) -> Result<Vec<Resource>> {
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

pub fn load_catalog() -> Result<Catalog> {
    Catalog::embedded().context("could not load the curated setup catalog")
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
        };
        let tree = SkillAgent::AgentsStandard.global_skill_tree(&root);
        std::fs::create_dir_all(tree.join("already-there")).unwrap();
        std::fs::write(tree.join("already-there/SKILL.md"), "installed").unwrap();
        let catalog = Catalog {
            schema_version: 1,
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
        assert_eq!(system.commands.into_inner().unwrap(), ["node --version"]);
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

    #[test]
    fn no_op_package_selection_is_recorded_for_status() {
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
        };
        let catalog = Catalog {
            schema_version: 1,
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
        let recorded: Vec<String> = serde_json::from_str(
            &std::fs::read_to_string(root.join(".config/loom/resources.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(recorded, ["pi-package:already-there"]);
        std::fs::remove_dir_all(root).unwrap();
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
        assert!(!visible.iter().any(|resource| resource.label == "sandbox"));
        assert!(visible.iter().any(|resource| resource.label == "pi"));
    }

    #[test]
    fn sandbox_install_applies_its_defaults_after_package_success() {
        let root = temp_root("sandbox");
        let paths = SettingsPaths {
            herdr_config: root.join("herdr.toml"),
            zed_settings: root.join("zed-settings.json"),
            zed_keymap: root.join("zed-keymap.json"),
            pi_fff_config: root.join("agent/pi-fff.json"),
            pi_sandbox_config: root.join("agent/sandbox.json"),
        };
        let sandbox = Catalog::embedded()
            .unwrap()
            .resources
            .into_iter()
            .find(|resource| resource.id == "pi-package:pi-sandbox")
            .unwrap();
        let settings = unapplied_related_settings(&[sandbox], &paths);
        let mut failed_report = InstallReport::default();
        apply_related_settings(&settings, &paths, &mut failed_report);
        assert!(!paths.pi_sandbox_config.exists());

        let mut report = InstallReport {
            installed: vec!["pi-package:pi-sandbox".into()],
            failures: vec![],
        };
        apply_related_settings(&settings, &paths, &mut report);

        assert!(paths.pi_sandbox_config.is_file());
        assert!(report.installed.contains(&"pi:sandbox-defaults".into()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
