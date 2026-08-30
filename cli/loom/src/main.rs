use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand};
use inquire::Confirm;
use loom::app::{install_selected, load_catalog, SelectionMode, Selectors};
use loom::init::{run_init, sync_projects, DomainLayout, Editor, InitOptions, Tracker};
use loom::status::run_status;
use loom::ui::{Mark, Out};
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
    /// Make this repository ready; feature flags select only the named capabilities
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
        /// Keep ADHD-friendly output on for this project
        #[arg(long, overrides_with = "no_adhd")]
        adhd: bool,
        #[arg(long, hide = true)]
        no_adhd: bool,
        /// Issue tracker: Beads or local Markdown
        #[arg(long, value_enum)]
        tracker: Option<Tracker>,
        /// Domain documentation layout
        #[arg(long, value_enum)]
        domain: Option<DomainLayout>,
        /// Editor used for clickable source links
        #[arg(long, value_enum)]
        editor: Option<Editor>,
        /// Add the project's CODING_STANDARDS.md review checklist
        #[arg(long, overrides_with = "no_coding_standards")]
        coding_standards: bool,
        #[arg(long, hide = true)]
        no_coding_standards: bool,
        /// Wire CodeGraph into installed agents and index this project
        #[arg(long, overrides_with = "no_codegraph")]
        codegraph: bool,
        #[arg(long, hide = true)]
        no_codegraph: bool,
        /// Run without prompts; with no feature flags, accept detection defaults
        #[arg(long)]
        yes: bool,
        /// Rewrite Loom-managed project files from scratch
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
                .filter(|resource| !cfg!(windows) || !resource.windows_wsl)
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

fn run_selection(
    mode: SelectionMode,
    args: SelectionArgs,
    offer_wsl: bool,
    system: &RealSystem,
) -> Result<bool> {
    let catalog = load_catalog()?;
    let selectors = Selectors {
        skills: args.skills,
        pi_packages: args.pi_packages,
        herdr_plugins: args.herdr_plugins,
        tools: args.tools,
    };
    install_selected(
        mode,
        &catalog,
        &selectors,
        &args.agents,
        args.scope,
        offer_wsl,
        args.yes,
        args.dry_run,
        system,
    )
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let system = RealSystem::default();
    // Bare `loom` is the guided setup — one less word to teach.
    let command = cli
        .command
        .unwrap_or_else(|| Command::Setup(SelectionArgs::default()));
    let success = match command {
        Command::Setup(args) => run_selection(SelectionMode::Setup, args, true, &system)?,
        Command::Add(args) => run_selection(SelectionMode::Add, args, false, &system)?,
        Command::Status => run_status(&system),
        Command::Sync => {
            let out = Out::detect();
            out.title("sync", "project AGENTS.md files");
            let sync = sync_projects(&system);
            let mark = if sync.ok { Mark::Ok } else { Mark::Bad };
            out.row(mark, "Projects", &sync.summary);
            for note in &sync.notes {
                out.note(note);
            }
            out.verdict(
                sync.ok,
                if sync.ok {
                    "Up to date"
                } else {
                    "Some projects failed"
                },
            );
            sync.ok
        }
        Command::Init {
            python,
            no_python,
            rust,
            no_rust,
            adhd,
            no_adhd,
            tracker,
            domain,
            editor,
            coding_standards,
            no_coding_standards,
            codegraph,
            no_codegraph,
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
                    adhd: flag(adhd, no_adhd),
                    tracker,
                    domain,
                    editor,
                    coding_standards: flag(coding_standards, no_coding_standards),
                    codegraph: flag(codegraph, no_codegraph),
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
                && !Confirm::new("Update skills, tools, Pi packages, Herdr, and project AGENTS.md?")
                    .with_default(true)
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
                source: None,
                windows_wsl: false,
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

    #[test]
    fn init_accepts_adhd_permanent_mode_flag() {
        // Capability/seam: scripted permanent ADHD mode. This fails if the
        // public flag stops reaching the init workflow. No expiry.
        let cli = Cli::try_parse_from(["loom", "init", "--adhd"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Init { adhd: true, .. })
        ));
    }

    #[test]
    fn init_accepts_project_setup_flags() {
        // Capability/seam: scripted project setup. This fails if automation
        // can no longer select Beads and coding standards. No expiry.
        let cli = Cli::try_parse_from([
            "loom",
            "init",
            "--tracker",
            "beads",
            "--domain",
            "multi",
            "--editor",
            "cursor",
            "--coding-standards",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Init {
                tracker: Some(Tracker::Beads),
                domain: Some(DomainLayout::Multi),
                editor: Some(Editor::Cursor),
                coding_standards: true,
                ..
            })
        ));
    }

    #[test]
    fn init_accepts_codegraph_flag() {
        let cli = Cli::try_parse_from(["loom", "init", "--codegraph"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Init {
                codegraph: true,
                ..
            })
        ));
    }
}
