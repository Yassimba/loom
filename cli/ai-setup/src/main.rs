use ai_setup::app::{install_selected, load_catalog, Selectors};
use ai_setup::doctor::run_doctor;
use ai_setup::init::{run_init, InitOptions};
use ai_setup::update::run_updates;
use ai_setup::{RealSystem, ResourceKind};
use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand};
use inquire::Confirm;

#[derive(Parser)]
#[command(
    name = "ai-setup",
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
    /// Check the setup and print actionable repairs
    Doctor,
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
        /// Include the Beads issue-triage section (--no-beads to exclude)
        #[arg(long, overrides_with = "no_beads")]
        beads: bool,
        #[arg(long, hide = true)]
        no_beads: bool,
        /// Accept detection defaults without prompting
        #[arg(long)]
        yes: bool,
        /// Rewrite AGENTS.md and CLAUDE.md from scratch
        #[arg(long)]
        force: bool,
    },
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
    /// Show the plan without making changes
    #[arg(long)]
    dry_run: bool,
    /// Apply the displayed plan without confirmation
    #[arg(long)]
    yes: bool,
}

/// Completions that know the catalog: the value lists for --skill,
/// --pi-package, and --tool are baked into the generated script.
fn print_completions(shell: clap_complete::Shell) -> Result<()> {
    let catalog = load_catalog()?;
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
            .mut_arg("tools", |arg| arg.value_parser(values(ResourceKind::Tool)))
        });
    }
    clap_complete::generate(shell, &mut command, "ai-setup", &mut std::io::stdout());
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let system = RealSystem::default();
    // Bare `ai-setup` is the guided setup — one less word to teach.
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
            install_selected(&catalog, &selectors, args.yes, args.dry_run, &system)?
        }
        Command::Doctor => run_doctor(&system),
        Command::Init {
            python,
            no_python,
            rust,
            no_rust,
            beads,
            no_beads,
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
                &load_catalog()?,
                &InitOptions {
                    python: flag(python, no_python),
                    rust: flag(rust, no_rust),
                    beads: flag(beads, no_beads),
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
