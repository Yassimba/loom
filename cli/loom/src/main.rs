use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand};
use inquire::Confirm;
use loom::app::{install_selected, SelectionMode, Selectors};
use loom::init::{run_init, sync_projects, DomainLayout, Editor, InitOptions, Tracker};
use loom::status::run_status;
use loom::ui::{Mark, Out};
use loom::update::run_updates;
use loom::wiki::{WikiOperation, WikiRequest};
use loom::{Catalog, RealSystem, ResourceKind, SkillAgent, SkillScope, UninstallOptions};
use std::path::PathBuf;

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
    /// Remove Loom-owned resources
    Uninstall(UninstallArgs),
    /// Update installed managers, packages, plugins, and this CLI
    Update {
        /// Apply updates without confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Create, adopt, and manage Pi Wiki Vaults
    Wiki {
        #[command(subcommand)]
        command: Option<WikiCommand>,
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
        /// Diagram style for this project; inherit uses your Loom setup default
        #[arg(long, value_enum)]
        diagrams: Option<loom::diagrams::DiagramStyle>,
        /// Add the project's CODING_STANDARDS.md review checklist
        #[arg(long, overrides_with = "no_coding_standards")]
        coding_standards: bool,
        #[arg(long, hide = true)]
        no_coding_standards: bool,
        /// Set up Gortex for Pi and Zed and track this repository
        #[arg(long, overrides_with = "no_gortex")]
        gortex: bool,
        #[arg(long, hide = true)]
        no_gortex: bool,
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
    /// List or reconcile Pi package-provided skills (`scripts/sync-skills.sh`)
    #[command(hide = true)]
    BundledSkills {
        /// Write Pi shared-skill exclusions and drop unchanged owned copies
        #[arg(long)]
        reconcile: bool,
    },
}

#[derive(Subcommand)]
enum WikiCommand {
    /// Create a new Vault at an absent path
    Create {
        path: PathBuf,
        #[arg(long)]
        feynman: bool,
        #[arg(long)]
        confluence: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Adopt an existing Obsidian Vault
    Adopt {
        path: PathBuf,
        #[arg(long)]
        feynman: bool,
        #[arg(long)]
        confluence: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Check every registered Vault
    Status,
    /// Restore project-local Pi wiring without changing knowledge
    Repair { path: PathBuf },
    /// Stop managing a Vault without deleting it
    Unregister { path: PathBuf },
    /// Open a Vault in Obsidian
    Open { path: PathBuf },
    /// Launch Pi with the Vault as its working directory
    Launch { path: PathBuf },
}

#[derive(Args, Default)]
struct UninstallArgs {
    /// Remove a named owned skill; repeat for multiple skills
    #[arg(long = "skill", conflicts_with = "all")]
    skills: Vec<String>,
    /// Remove a named owned Pi package; repeat for multiple packages
    #[arg(long = "pi-package", conflicts_with = "all")]
    pi_packages: Vec<String>,
    /// Remove a named owned Herdr plugin; repeat for multiple plugins
    #[arg(long = "herdr-plugin", conflicts_with = "all")]
    herdr_plugins: Vec<String>,
    /// Remove a named owned tool; repeat for multiple tools
    #[arg(long = "tool", conflicts_with = "all")]
    tools: Vec<String>,
    /// Select every visible owned resource
    #[arg(long)]
    all: bool,
    /// Apply the displayed removal plan without confirmation
    #[arg(long)]
    yes: bool,
    /// Show the removal plan without making changes
    #[arg(long)]
    dry_run: bool,
    /// Delete modified Loom-owned content; requires --yes in scripts
    #[arg(long, requires = "yes")]
    force_modified: bool,
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
    /// Select a reviewed MCP server (Sem via Pi gateway; use --agent pi)
    #[arg(long = "mcp-server")]
    mcp_servers: Vec<String>,
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
    for name in ["setup", "add", "uninstall"] {
        command = command.mut_subcommand(name, |sub| {
            let sub = if name == "uninstall" {
                sub
            } else {
                sub.mut_arg("mcp_servers", |arg| {
                    arg.value_parser(values(ResourceKind::McpServer))
                })
            };
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
    let catalog = Catalog::embedded()?;
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
    let catalog = Catalog::embedded()?;
    let selectors = Selectors {
        skills: args.skills,
        pi_packages: args.pi_packages,
        herdr_plugins: args.herdr_plugins,
        tools: args.tools,
        mcp_servers: args.mcp_servers,
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

fn run_uninstall(args: UninstallArgs, system: &RealSystem) -> Result<bool> {
    let catalog = Catalog::embedded()?;
    let selectors = Selectors {
        skills: args.skills,
        pi_packages: args.pi_packages,
        herdr_plugins: args.herdr_plugins,
        tools: args.tools,
        mcp_servers: Vec::new(),
    };
    let selected = if selectors.is_empty() {
        Vec::new()
    } else {
        loom::app::resolve_selectors(&catalog, &selectors)?
            .into_iter()
            .map(|resource| resource.id)
            .collect()
    };
    loom::uninstall::run_uninstall(
        system,
        &UninstallOptions {
            selected,
            all: args.all,
            yes: args.yes,
            dry_run: args.dry_run,
            force_modified: args.force_modified,
        },
    )
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let system = RealSystem::default();
    loom::System::refresh_path(&system);
    if let Some(home) = loom::System::home_dir(&system) {
        loom::ownership::record_bootstrap_from_env(&home).map_err(anyhow::Error::msg)?;
    }
    // Bare `loom` is the guided setup — one less word to teach.
    let command = cli
        .command
        .unwrap_or_else(|| Command::Setup(SelectionArgs::default()));
    let success = match command {
        Command::Setup(args) => run_selection(SelectionMode::Setup, args, true, &system)?,
        Command::Add(args) => run_selection(SelectionMode::Add, args, false, &system)?,
        Command::Uninstall(args) => run_uninstall(args, &system)?,
        Command::Wiki { command } => match command {
            None => loom::wiki::run_interactive(&system)?,
            Some(command) => {
                let request = match command {
                    WikiCommand::Create {
                        path,
                        feynman,
                        confluence,
                        yes,
                    } => WikiRequest {
                        operation: WikiOperation::Create,
                        vault: path,
                        feynman,
                        confluence,
                        yes,
                    },
                    WikiCommand::Adopt {
                        path,
                        feynman,
                        confluence,
                        yes,
                    } => WikiRequest {
                        operation: WikiOperation::Adopt,
                        vault: path,
                        feynman,
                        confluence,
                        yes,
                    },
                    WikiCommand::Status => WikiRequest {
                        operation: WikiOperation::Status,
                        vault: PathBuf::new(),
                        feynman: false,
                        confluence: false,
                        yes: true,
                    },
                    WikiCommand::Repair { path } => WikiRequest {
                        operation: WikiOperation::Repair,
                        vault: path,
                        feynman: false,
                        confluence: false,
                        yes: true,
                    },
                    WikiCommand::Unregister { path } => WikiRequest {
                        operation: WikiOperation::Unregister,
                        vault: path,
                        feynman: false,
                        confluence: false,
                        yes: true,
                    },
                    WikiCommand::Open { path } => WikiRequest {
                        operation: WikiOperation::Open,
                        vault: path,
                        feynman: false,
                        confluence: false,
                        yes: true,
                    },
                    WikiCommand::Launch { path } => WikiRequest {
                        operation: WikiOperation::Launch,
                        vault: path,
                        feynman: false,
                        confluence: false,
                        yes: true,
                    },
                };
                loom::wiki::run_wiki(&request, &system)?
            }
        },
        Command::Status => {
            let core = run_status(&system);
            let wiki = loom::wiki::status_registered(&system);
            core && wiki
        }
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
            diagrams,
            coding_standards,
            no_coding_standards,
            gortex,
            no_gortex,
            yes,
            force,
        } => {
            let flag = |on: bool, off: bool| match (on, off) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            };
            let home = loom::System::home_dir(&system)
                .ok_or_else(|| anyhow::anyhow!("home directory is unavailable"))?;
            let current = loom::System::current_dir(&system)
                .ok_or_else(|| anyhow::anyhow!("current directory is unavailable"))?;
            let project = loom::project_root(&current);
            let before = loom::ownership::snapshot_project(&project);
            let init = run_init(
                &system,
                &InitOptions {
                    python: flag(python, no_python),
                    rust: flag(rust, no_rust),
                    adhd: flag(adhd, no_adhd),
                    tracker,
                    domain,
                    editor,
                    diagrams,
                    coding_standards: flag(coding_standards, no_coding_standards),
                    gortex: flag(gortex, no_gortex),
                    yes,
                    force,
                },
            );
            let ownership = loom::ownership::record_project_changes(&home, &before);
            if let Err(error) = ownership {
                let message = match loom::ownership::restore_project(&system, before) {
                    Ok(()) => error,
                    Err(rollback) => format!("{error}; rollback failed: {rollback}"),
                };
                return Err(anyhow::anyhow!(message));
            }
            init?
        }
        Command::Completions { shell } => {
            print_completions(shell)?;
            true
        }
        Command::BundledSkills { reconcile } => {
            let home = loom::System::home_dir(&system)
                .ok_or_else(|| anyhow::anyhow!("home directory is unavailable"))?;
            if reconcile {
                for note in loom::reconcile_bundled_skills(&home).map_err(anyhow::Error::msg)? {
                    eprintln!("{note}");
                }
            } else {
                for name in loom::provided_bundled_skills(&home) {
                    println!("{name}");
                }
            }
            true
        }
        Command::Update { yes } => {
            if !yes
                && !Confirm::new(
                    "Update skills, tools, Pi packages, Herdr, Wiki Vaults, and project AGENTS.md?",
                )
                .with_default(true)
                .prompt()?
            {
                println!("Cancelled; no changes made.");
                true
            } else {
                let updated = run_updates(&system, &Catalog::embedded()?);
                let wikis_updated = loom::wiki::update_registered(&system);
                updated && wikis_updated
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
            profiles: Vec::new(),
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
                bundled_skills: Vec::new(),
            }],
        };

        let command = completion_command(&catalog);
        for name in ["setup", "add", "uninstall"] {
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
    fn mcp_documented_uninstall_command_parses() {
        let command = include_str!("../MCP.md")
            .split('`')
            .find(|text| text.starts_with("loom uninstall"))
            .expect("MCP removal instructions");
        let cli = Cli::try_parse_from(command.split_whitespace())
            .expect("documented MCP removal command must be accepted");
        assert!(matches!(cli.command, Some(Command::Uninstall(_))));
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
    fn wiki_create_is_pi_only_and_scriptable() {
        let cli = Cli::try_parse_from([
            "loom",
            "wiki",
            "create",
            "/tmp/knowledge",
            "--feynman",
            "--confluence",
            "--yes",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Wiki {
                command: Some(WikiCommand::Create {
                    feynman: true,
                    confluence: true,
                    yes: true,
                    ..
                })
            })
        ));
    }

    #[test]
    fn init_accepts_gortex_flag() {
        let cli = Cli::try_parse_from(["loom", "init", "--gortex"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Init { gortex: true, .. })
        ));
    }

    #[test]
    fn wiki_has_unregister_but_no_delete_command() {
        assert!(Cli::try_parse_from(["loom", "wiki", "unregister", "/tmp/vault"]).is_ok());
        assert!(Cli::try_parse_from(["loom", "wiki", "delete", "/tmp/vault"]).is_err());
    }
}
