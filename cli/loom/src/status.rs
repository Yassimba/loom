use crate::ui::{tidy_path, Mark, Out};
use crate::{skills, CommandSpec, System};

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
    style.section("Agent skills");
    print_skill_trees(system, &style);
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
    let healthy = checks.iter().all(|check| check.health != Health::Bad);
    if healthy {
        style.verdict(true, "All checks passed");
        style.hint("optional managers are installed on demand by `loom setup`");
    } else {
        style.verdict(false, "Some checks need attention");
        style.next("repair the failed managers, then run `loom status` again");
    }
    healthy
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
fn print_skill_trees(system: &dyn System, style: &Out) {
    let Some(home) = system.home_dir() else {
        style.row(Mark::Bad, "skills", "home directory is unavailable");
        return;
    };
    let mut trees = crate::SkillAgent::ALL
        .into_iter()
        .map(|agent| (agent, agent.global_skill_tree(&home)))
        .filter(|(_, tree)| tree.parent().is_some_and(|parent| parent.is_dir()))
        .collect::<Vec<_>>();
    if let Some(current) = system.current_dir() {
        let project = skills::project_root(&current);
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
        return;
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
    match system.run(&command) {
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
