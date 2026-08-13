use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand};
use inquire::Confirm;
use loom::app::{install_selected, load_catalog, Selectors};
use loom::init::{run_init, sync_projects, InitOptions};
use loom::status::run_status;
use loom::update::run_updates;
use loom::{Catalog, RealSystem, ResourceKind, SkillAgent, SkillScope};

#[derive(Parser)]
#[command(
    name = "loom",
    version,
    about = "Set up Yassimba's curated skills, Pi packages, and Herdr plugins"
)]
struct Cli {
    /// With no subcommand, runs the guided setup.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// First-time guided setup
    Setup(SelectionArgs),
    /// Add one or more capabilities
    Add(SelectionArgs),
    /// Update installed managers, packages, plugins, and this CLI
    Update {
        /// Apply updates without confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Show installed agents, integrations, and runtime health
    Status,
    /// Scaffold this project's AGENTS.md and CLAUDE.md from the templates
    Init {
        /// Include the Python section (--no-python to exclude)
        #[arg(long, overrides_with = "no_python")]
        python: bool,
        #[arg(long, hide = true)]
        no_python: bool,
        /// Include the Rust section (--no-rust to exclude)
        #[arg(long, overrides_with = "no_rust")]
        rust: bool,
        #[arg(long, hide = true)]
        no_rust: bool,
        /// Accept detection defaults without prompting
        #[arg(long)]
        yes: bool,
        /// Rewrite AGENTS.md and CLAUDE.md from scratch
        #[arg(long)]
        force: bool,
    },
    /// Refresh every registered project's AGENTS.md from the templates
    Sync,
    /// Print shell completions (skill/tool/package names included)
    Completions { shell: clap_complete::Shell },
}

#[derive(Args, Default)]
struct SelectionArgs {
    /// Install a named shared skill; repeat for multiple skills
    #[arg(long = "skill")]
    skills: Vec<String>,
    /// Install a Pi npm package from this catalog; repeat for multiple packages
    #[arg(long = "pi-package")]
    pi_packages: Vec<String>,
    /// Install a Herdr plugin from this catalog; repeat for multiple plugins
    #[arg(long = "herdr-plugin")]
    herdr_plugins: Vec<String>,
    /// Install a tool from the pinned manifest; repeat for multiple tools
    #[arg(long = "tool")]
    tools: Vec<String>,
    /// Install skills for this agent; repeat for multiple agents
    #[arg(long = "agent", value_enum)]
    agents: Vec<SkillAgent>,
    /// Install skills globally or in the current project
    #[arg(long, value_enum, default_value_t)]
    scope: SkillScope,
    /// Show the plan without making changes
    #[arg(long)]
    dry_run: bool,
    /// Apply the displayed plan without confirmation
    #[arg(long)]
    yes: bool,
}

/// Add each catalog-backed selector value to the CLI command.
fn completion_command(catalog: &Catalog) -> clap::Command {
    let values = |kind: ResourceKind| {
        clap::builder::PossibleValuesParser::new(
            catalog
                .resources
                .iter()
                .filter(|resource| resource.kind == kind)
                .map(|resource| resource.label.clone())
                .collect::<Vec<_>>(),
        )
    };
    let mut command = Cli::command();
    for name in ["setup", "add"] {
        command = command.mut_subcommand(name, |sub| {
            sub.mut_arg("skills", |arg| {
                arg.value_parser(values(ResourceKind::Skill))
            })
            .mut_arg("pi_packages", |arg| {
                arg.value_parser(values(ResourceKind::PiPackage))
            })
            .mut_arg("herdr_plugins", |arg| {
                arg.value_parser(values(ResourceKind::HerdrPlugin))
            })
            .mut_arg("tools", |arg| arg.value_parser(values(ResourceKind::Tool)))
        });
    }
    command
}

/// Print shell completions with each catalog selector value.
fn print_completions(shell: clap_complete::Shell) -> Result<()> {
    let catalog = load_catalog()?;
    let mut command = completion_command(&catalog);
    clap_complete::generate(shell, &mut command, "loom", &mut std::io::stdout());
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let system = RealSystem::default();
    // Bare `loom` is the guided setup — one less word to teach.
    let command = cli
        .command
        .unwrap_or_else(|| Command::Setup(SelectionArgs::default()));
    let success = match command {
        Command::Setup(args) | Command::Add(args) => {
            let catalog = load_catalog()?;
            let selectors = Selectors {
                skills: args.skills,
                pi_packages: args.pi_packages,
                herdr_plugins: args.herdr_plugins,
                tools: args.tools,
            };
            install_selected(
                &catalog,
                &selectors,
                &args.agents,
                args.scope,
                args.yes,
                args.dry_run,
                &system,
            )?
        }
        Command::Status => run_status(&system),
        Command::Sync => {
            let (ok, report) = sync_projects(&system);
            if ok {
                println!("{report}");
            } else {
                eprintln!("{report}");
            }
            ok
        }
        Command::Init {
            python,
            no_python,
            rust,
            no_rust,
            yes,
            force,
        } => {
            let flag = |on: bool, off: bool| match (on, off) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            };
            run_init(
                &system,
                &InitOptions {
                    python: flag(python, no_python),
                    rust: flag(rust, no_rust),
                    yes,
                    force,
                },
            )?
        }
        Command::Completions { shell } => {
            print_completions(shell)?;
            true
        }
        Command::Update { yes } => {
            if !yes
                && !Confirm::new("Update installed Yassimba tooling and resources?")
                    .with_default(false)
                    .prompt()?
            {
                println!("Cancelled; no changes made.");
                true
            } else {
                run_updates(&system, &load_catalog()?)
            }
        }
    };
    if !success {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom::Resource;

    #[test]
    fn selection_completion_includes_herdr_plugin_catalog_values() {
        let catalog = Catalog {
            schema_version: 1,
            presets: vec![],
            resources: vec![Resource {
                id: "herdr-plugin:reviewr".to_string(),
                kind: ResourceKind::HerdrPlugin,
                group: "Herdr plugins".to_string(),
                label: "reviewr".to_string(),
                description: "Review agent".to_string(),
                install_target: "reviewr".to_string(),
                next_action: "Run reviewr".to_string(),
                dependencies: vec![],
                bin: None,
                version: None,
                companions: vec![],
            }],
        };

        let command = completion_command(&catalog);
        for name in ["setup", "add"] {
            let subcommand = command
                .get_subcommands()
                .find(|subcommand| subcommand.get_name() == name)
                .expect("selection subcommand");
            let argument = subcommand
                .get_arguments()
                .find(|argument| argument.get_id() == "herdr_plugins")
                .expect("Herdr plugin argument");
            let values = argument
                .get_value_parser()
                .possible_values()
                .expect("catalog-backed values")
                .map(|value| value.get_name().to_string())
                .collect::<Vec<_>>();

            assert_eq!(
                values,
                ["reviewr"],
                "a missing Herdr value parser hides valid plugins from shell completion"
            );
        }
    }
}
