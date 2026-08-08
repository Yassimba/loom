use crate::settings::{curated_settings, setting_state, SettingsPaths};
use crate::ui::{confirm_plan, print_plan};
use crate::wizard::{run_wizard, Model, WizardOutcome};
use crate::{
    build_install_plan, execute_install_plan, expand_skill_dependencies, Catalog, CommandSpec,
    InstallReport, NodeStatus, Platform, PrerequisiteStatus, Resource, ResourceKind, SkillAgent,
    SkillDestination, SkillScope, System,
};
use anyhow::{bail, Context, Result};

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

pub fn install_selected(
    catalog: &Catalog,
    selectors: &Selectors,
    requested_agents: &[SkillAgent],
    scope: SkillScope,
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
        return run_interactive(catalog, status, platform, dry_run, destination, system);
    }

    let resources =
        expand_skill_dependencies(&catalog.resources, resolve_selectors(catalog, selectors)?);
    if resources.is_empty() {
        println!("Nothing selected; no changes made.");
        return Ok(true);
    }
    let plan = build_install_plan(&resources, &[], status, platform, &destination)?;
    print_plan(&plan);
    if dry_run {
        println!("\nDry run; no changes made.");
        return Ok(true);
    }
    if !assume_yes && !confirm_plan()? {
        println!("Cancelled; no changes made.");
        return Ok(true);
    }

    let report = execute_install_plan(&plan, system);
    print_report(&report);
    if let Some(next) = resources
        .iter()
        .find(|resource| {
            report.installed.contains(&resource.id)
                || (resource.kind == ResourceKind::Skill
                    && report.installed.iter().any(|target| target == "skills"))
        })
        .map(|resource| resource.next_action.as_str())
    {
        println!("\nNext: {next}");
    }
    Ok(report.failures.is_empty())
}

fn run_interactive(
    catalog: &Catalog,
    status: PrerequisiteStatus,
    platform: Platform,
    dry_run: bool,
    skill_destination: SkillDestination,
    system: &(dyn System + Sync),
) -> Result<bool> {
    let settings_paths = SettingsPaths::detect()?;
    let settings = curated_settings();
    let setting_states = settings
        .iter()
        .map(|spec| setting_state(spec, &settings_paths))
        .collect();
    let zed_present = settings_paths.zed_settings.exists();
    // Installed marks arrive from a background probe once the wizard is on
    // screen; starting all-false keeps the first frame instant.
    let installed = vec![false; catalog.resources.len()];
    let model = Model {
        resources: catalog.resources.clone(),
        presets: catalog.presets.clone(),
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
            print_plan(&plan);
            for change in &setting_changes {
                println!("  Configure {change}");
            }
            println!("\nDry run; no changes made.");
            Ok(true)
        }
        WizardOutcome::Installed(report) => {
            print_report(&report);
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
            .run(&CommandSpec::new(program, args.iter().copied()))
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
            ResourceKind::Skill => skill_trees
                .iter()
                .any(|tree| crate::skills::skill_present_in(tree, &resource.install_target)),
        })
        .collect()
}

fn print_report(report: &InstallReport) {
    if !report.installed.is_empty() {
        println!("\nInstalled:");
        for target in &report.installed {
            println!("  ✓ {target}");
        }
    }
    if !report.failures.is_empty() {
        eprintln!("\nCould not install:");
        for failure in &report.failures {
            eprintln!("  ! {}: {}", failure.target, failure.message);
        }
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
