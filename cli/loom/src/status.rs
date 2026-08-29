use crate::ui::{tidy_path, Mark, Out};
use crate::{skills, CommandSpec, InstallReport, System};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Health {
    Good,
    Optional,
    Bad,
}

impl Health {
    fn mark(self) -> Mark {
        match self {
            Self::Good => Mark::Ok,
            Self::Optional => Mark::Off,
            Self::Bad => Mark::Bad,
        }
    }
}

struct RuntimeCheck {
    name: &'static str,
    health: Health,
    detail: String,
}

pub fn run_status(system: &(dyn System + Sync)) -> bool {
    let style = Out::detect();
    style.title("status", concat!("v", env!("CARGO_PKG_VERSION")));
    style.section("Core");
    style.row(
        Mark::Ok,
        "CLI",
        style.muted(concat!("loom ", env!("CARGO_PKG_VERSION"))),
    );
    print_manifest(system, &style);
    style.blank();
    style.section("Selected resources");
    let resources_healthy = print_managed_resources(system, &style);
    style.blank();
    style.section("Agent skills");
    let skills_healthy = print_skill_trees(system, &style);
    style.blank();
    style.section("Integrations");
    print_opencode_adapter(system, &style);
    style.blank();
    style.section("Runtimes");
    // Every probe is an independent `<tool> --version`; run them all at once.
    let probes: [(&'static str, &[&str]); 5] = [
        ("mise", &["--version"]),
        ("node", &["--version"]),
        ("npm", &["--version"]),
        ("pi", &["--version"]),
        ("herdr", &["--version"]),
    ];
    let mut checks = std::thread::scope(|scope| {
        probes
            .map(|(name, args)| scope.spawn(move || check_command(system, name, args)))
            .map(|handle| handle.join().expect("status probe thread"))
    });
    // Node is detect-and-instruct only: flag a version below Pi's floor, but
    // never install or update it.
    if let Some(node) = checks.iter_mut().find(|check| check.name == "node") {
        if node.health == Health::Good {
            if let Some(warning) = crate::NodeStatus::detect(system).warning() {
                node.health = Health::Bad;
                node.detail = format!("{} — {warning}", node.detail);
            }
        }
    }
    for check in &checks {
        style.row(check.health.mark(), check.name, style.muted(&check.detail));
    }
    let healthy = resources_healthy
        && skills_healthy
        && checks.iter().all(|check| check.health != Health::Bad);
    if healthy {
        style.verdict(true, "Selected resources and runtimes verified");
        style.hint("optional managers are installed on demand by `loom setup`");
    } else {
        style.verdict(false, "Some checks need attention");
        style.next("repair the failed managers, then run `loom status` again");
    }
    healthy
}

const RESOURCE_REGISTRY: &str = ".config/loom/resources.json";

fn resource_registry(home: &Path) -> PathBuf {
    home.join(RESOURCE_REGISTRY)
}

fn read_selected_resources(home: &Path) -> Result<HashSet<String>, String> {
    let path = resource_registry(home);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashSet::new()),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };
    serde_json::from_str::<Vec<String>>(&text)
        .map(|items| items.into_iter().collect())
        .map_err(|error| format!("could not parse {}: {error}", path.display()))
}

/// Remember successfully installed manager-owned resources so status can
/// distinguish an optional manager from one that disappeared after setup.
pub fn record_managed_resources(system: &dyn System, report: &InstallReport) -> Result<(), String> {
    let installed = report
        .installed
        .iter()
        .filter(|id| id.starts_with("pi-package:") || id.starts_with("herdr-plugin:"))
        .cloned()
        .collect::<Vec<_>>();
    if installed.is_empty() {
        return Ok(());
    }
    let Some(home) = system.home_dir() else {
        return Err("home directory is unavailable".into());
    };
    let mut selected = read_selected_resources(&home)?;
    let before = selected.len();
    selected.extend(installed);
    if selected.len() == before {
        return Ok(());
    }
    let path = resource_registry(&home);
    std::fs::create_dir_all(path.parent().expect("registry has a parent"))
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    let mut selected = selected.into_iter().collect::<Vec<_>>();
    selected.sort();
    let text = serde_json::to_string_pretty(&selected).expect("resource ids serialize");
    std::fs::write(&path, format!("{text}\n"))
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn print_managed_resources(system: &dyn System, style: &Out) -> bool {
    let Some(home) = system.home_dir() else {
        style.row(Mark::Bad, "resources", "home directory is unavailable");
        return false;
    };
    let Ok(catalog) = crate::Catalog::embedded() else {
        style.row(Mark::Bad, "catalog", "embedded catalog is unavailable");
        return false;
    };
    let selected = crate::manifest::selected_keys(&home);
    let (managed, mut healthy) = match read_selected_resources(&home) {
        Ok(selected) => (selected, true),
        Err(error) => {
            style.row(Mark::Bad, "resource registry", error);
            (HashSet::new(), false)
        }
    };
    for resource in catalog.resources.iter().filter(|resource| {
        resource.kind == crate::ResourceKind::Tool && selected.contains(&resource.install_target)
    }) {
        let present = resource
            .bin
            .as_deref()
            .is_some_and(|binary| system.command_exists(binary));
        style.row(
            if present { Mark::Ok } else { Mark::Bad },
            &resource.label,
            if present {
                "selected tool available"
            } else {
                "selected tool missing from PATH"
            },
        );
        healthy &= present;
    }
    healthy &= print_manager_inventory(
        system,
        style,
        &catalog,
        "pi",
        &["list"],
        crate::ResourceKind::PiPackage,
        &managed,
    );
    healthy &= print_manager_inventory(
        system,
        style,
        &catalog,
        "herdr",
        &["plugin", "list"],
        crate::ResourceKind::HerdrPlugin,
        &managed,
    );
    healthy
}

fn print_manager_inventory(
    system: &dyn System,
    style: &Out,
    catalog: &crate::Catalog,
    manager: &'static str,
    args: &[&str],
    kind: crate::ResourceKind,
    selected: &HashSet<String>,
) -> bool {
    let selected_for_manager = catalog
        .resources
        .iter()
        .any(|resource| resource.kind == kind && selected.contains(&resource.id));
    if !system.command_exists(manager) {
        style.row(
            if selected_for_manager {
                Mark::Bad
            } else {
                Mark::Off
            },
            manager,
            if selected_for_manager {
                "selected manager missing from PATH"
            } else {
                "not selected"
            },
        );
        return !selected_for_manager;
    }
    match system.run_probe(&CommandSpec::new(manager, args.iter().copied())) {
        Ok(result) if result.success => {
            let output = format!("{}\n{}", result.stdout, result.stderr);
            let mut healthy = true;
            for resource in catalog
                .resources
                .iter()
                .filter(|resource| resource.kind == kind)
            {
                let installed = output.contains(&resource.install_target)
                    || output.contains(resource.id.trim_start_matches("herdr-plugin:"));
                let expected = selected.contains(&resource.id);
                healthy &= installed || !expected;
                style.row(
                    if installed {
                        Mark::Ok
                    } else if expected {
                        Mark::Bad
                    } else {
                        Mark::Off
                    },
                    &resource.label,
                    if installed {
                        "catalog item installed"
                    } else if expected {
                        "selected catalog item missing"
                    } else {
                        "catalog item not installed"
                    },
                );
            }
            healthy
        }
        Ok(result) => {
            style.row(
                Mark::Bad,
                manager,
                crate::install::command_failure_message(&result),
            );
            false
        }
        Err(error) => {
            style.row(Mark::Bad, manager, error.to_string());
            false
        }
    }
}

fn print_opencode_adapter(system: &dyn System, style: &Out) {
    let Some(home) = system.home_dir() else {
        return;
    };
    let mut adapters = vec![(
        home.join(".config").join("opencode"),
        skills::opencode_adapter_path(&home),
    )];
    if let Some(current) = system.current_dir() {
        let project = skills::project_root(&current);
        adapters.push((
            project.join(".opencode"),
            skills::project_opencode_adapter_path(&project),
        ));
    }
    let mut found = false;
    for (config, adapter) in adapters {
        if !config.is_dir() {
            continue;
        }
        found = true;
        if adapter.is_file() {
            style.row(Mark::Ok, "OpenCode", tidy_path(&adapter, &home));
        } else {
            style.row(
                Mark::Off,
                "OpenCode",
                style.muted(format!(
                    "{}  session adapter not installed",
                    tidy_path(&adapter, &home)
                )),
            );
        }
    }
    if !found {
        style.row(Mark::Off, "OpenCode", style.muted("not configured"));
    }
}

/// Whether the published tool manifest has been synced into mise's conf.d.
fn print_manifest(system: &dyn System, style: &Out) {
    let Some(home) = system.home_dir() else {
        return;
    };
    let target = crate::manifest::conf_d_target(&home);
    if target.is_file() {
        style.row(
            Mark::Ok,
            "manifest",
            style.muted(format!("synced  {}", tidy_path(&target, &home))),
        );
    } else {
        style.row(
            Mark::Off,
            "manifest",
            style.muted("not synced yet — run `loom setup` or `loom update`"),
        );
    }
}

/// The agent skill trees the native installer would write into, with how
/// many catalog skills each already holds.
fn print_skill_trees(system: &dyn System, style: &Out) -> bool {
    let Some(home) = system.home_dir() else {
        style.row(Mark::Bad, "skills", "home directory is unavailable");
        return false;
    };
    let mut trees = crate::SkillAgent::ALL
        .into_iter()
        .map(|agent| (agent, agent.global_skill_tree(&home)))
        .filter(|(_, tree)| tree.parent().is_some_and(|parent| parent.is_dir()))
        .collect::<Vec<_>>();
    let mut projects = match skills::prune_registered_skill_projects(&home) {
        Ok(projects) => projects,
        Err(error) => {
            style.row(Mark::Bad, "skill projects", error);
            return false;
        }
    };
    if let Some(current) = system.current_dir() {
        projects.push(skills::project_root(&current));
    }
    projects.sort();
    projects.dedup();
    for project in projects {
        trees.extend(
            crate::SkillAgent::ALL
                .into_iter()
                .map(|agent| (agent, agent.project_skill_tree(&project)))
                .filter(|(_, tree)| tree.is_dir()),
        );
    }
    trees.sort_by(|left, right| left.1.cmp(&right.1));
    trees.dedup_by(|left, right| left.1 == right.1);
    if trees.is_empty() {
        style.row(
            Mark::Off,
            "skills",
            style.muted(format!(
                "no agent directory found — {}",
                skills::agent_dirs_display()
            )),
        );
        return true;
    }
    let catalog_skills = crate::Catalog::embedded()
        .map(|catalog| {
            catalog
                .resources
                .into_iter()
                .filter(|resource| resource.kind == crate::ResourceKind::Skill)
                .map(|resource| resource.install_target)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for (agent, tree) in trees {
        let installed = catalog_skills
            .iter()
            .filter(|name| tree.join(name.as_str()).join("SKILL.md").is_file())
            .count();
        let health = if installed == catalog_skills.len() {
            Health::Good
        } else {
            Health::Optional
        };
        let coverage = format!("{:<6}", format!("{installed}/{}", catalog_skills.len()));
        let coverage = match health {
            Health::Good => style.good(coverage),
            Health::Optional => style.warn(coverage),
            Health::Bad => unreachable!(),
        };
        style.row(
            health.mark(),
            agent.status_label(),
            format!("{}  {}", coverage, style.muted(tidy_path(&tree, &home))),
        );
    }
    true
}

fn check_command(system: &dyn System, name: &'static str, args: &[&str]) -> RuntimeCheck {
    if !system.command_exists(name) {
        return RuntimeCheck {
            name,
            health: Health::Optional,
            detail: "not installed (optional until selected)".into(),
        };
    }
    let command = CommandSpec::new(name, args.iter().copied());
    match system.run_probe(&command) {
        Ok(result) if result.success => RuntimeCheck {
            name,
            health: Health::Good,
            detail: result
                .stdout
                .lines()
                .next()
                .unwrap_or("available")
                .trim()
                .into(),
        },
        Ok(result) => RuntimeCheck {
            name,
            health: Health::Bad,
            detail: result.stderr.trim().into(),
        },
        Err(error) => RuntimeCheck {
            name,
            health: Health::Bad,
            detail: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HomeSystem(PathBuf);

    impl System for HomeSystem {
        fn command_exists(&self, _name: &str) -> bool {
            false
        }

        fn refresh_path(&self) {}

        fn run(&self, _command: &CommandSpec) -> anyhow::Result<crate::CommandResult> {
            unreachable!()
        }

        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.0.clone())
        }
    }

    #[test]
    fn a_missing_selected_manager_makes_status_unhealthy() {
        let root = std::env::temp_dir().join(format!(
            "loom-missing-manager-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package = crate::Catalog::embedded()
            .unwrap()
            .resources
            .into_iter()
            .find(|resource| resource.kind == crate::ResourceKind::PiPackage)
            .unwrap();
        let system = HomeSystem(root.clone());
        record_managed_resources(
            &system,
            &InstallReport {
                installed: vec![package.id],
                failures: Vec::new(),
            },
        )
        .unwrap();

        assert!(!print_managed_resources(&system, &Out::plain()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_manager_resources_are_recorded_without_clobbering_errors() {
        let root = std::env::temp_dir().join(format!(
            "loom-resource-registry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let system = HomeSystem(root.clone());
        record_managed_resources(
            &system,
            &InstallReport {
                installed: vec!["pi-package:one".into(), "skill:ignored".into()],
                failures: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(
            read_selected_resources(&root).unwrap(),
            HashSet::from(["pi-package:one".into()])
        );

        let path = resource_registry(&root);
        std::fs::write(&path, "not json").unwrap();
        assert!(record_managed_resources(
            &system,
            &InstallReport {
                installed: vec!["herdr-plugin:two".into()],
                failures: Vec::new(),
            },
        )
        .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
        std::fs::remove_dir_all(root).unwrap();
    }
}
