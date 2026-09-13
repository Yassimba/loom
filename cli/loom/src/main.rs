use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand};
use inquire::Confirm;
use loom::app::{install_selected, SelectionMode, Selectors};
use loom::init::{run_init, sync_projects, DomainLayout, Editor, InitOptions, Tracker};
use loom::status::run_status;
use loom::ui::{columns, ellipsize, Mark, Out};
use loom::update::{
    herdr_gate, probe_herdr_server_running, run_updates, HerdrGate, HerdrLane, HERDR_SKIP_INSIDE,
    HERDR_SKIP_SERVER,
};
use loom::wiki::{WikiOperation, WikiRequest};
use loom::{
    Catalog, CommandSpec, RealSystem, ResourceKind, SkillAgent, SkillScope, UninstallOptions,
};
use std::io::Write;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

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
    /// Validate code contract structure without modifying files
    Contracts {
        #[arg(long, global = true)]
        json: bool,
        #[command(subcommand)]
        command: ContractsCommand,
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
enum ContractsCommand {
    /// Check CONTRACTS files in a directory or one explicit CONTRACTS file
    Check { path: Option<PathBuf> },
    /// Show the contracts governing PATH:LINE
    At {
        location: String,
        /// Leave out directory contracts from ancestor CONTRACTS files
        #[arg(long)]
        no_global: bool,
    },
    /// List contracts in a file, a directory, or by contract ID
    List {
        target: String,
        /// Leave out directory contracts from ancestor CONTRACTS files
        #[arg(long)]
        no_global: bool,
    },
    /// List contracts whose governed lines a diff touches
    Affected {
        /// Compare against this git revision
        #[arg(long, default_value = "HEAD")]
        base: String,
        /// Only attached contracts whose code changed but whose text did not
        #[arg(long)]
        stale: bool,
    },
    /// Changed declarations that carry no contract yet
    Propose {
        /// Compare against this git revision
        #[arg(long, default_value = "HEAD")]
        base: String,
    },
    /// Contracts added, reworded, or removed since a git revision
    Diff {
        /// Compare against this git revision
        #[arg(long, default_value = "HEAD")]
        base: String,
    },
    /// Show code that depends on the declaration at PATH:LINE.
    /// Uses the language server on PATH; rust-analyzer may run build
    /// scripts and proc macros while analysing the project.
    Related {
        location: String,
        /// Callers of the declaration, or every reference to it
        #[arg(long, value_enum, default_value_t = RelatedKind::References)]
        kind: RelatedKind,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum RelatedKind {
    Callers,
    References,
}

#[derive(Subcommand)]
enum WikiCommand {
    /// Create a new Vault at an absent path
    Create(WikiSetupArgs),
    /// Adopt an existing Obsidian Vault
    Adopt(WikiSetupArgs),
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
struct WikiSetupArgs {
    path: PathBuf,
    #[arg(long)]
    feynman: bool,
    #[arg(long)]
    confluence: bool,
    #[arg(long)]
    qmd: bool,
    #[arg(long)]
    yes: bool,
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
    /// Select a reviewed MCP server through Pi's gateway (use --agent pi)
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
    let values = |kind: ResourceKind, include_automatic: bool| {
        clap::builder::PossibleValuesParser::new(
            catalog
                .resources
                .iter()
                .filter(|resource| resource.kind == kind)
                .filter(|resource| include_automatic || !resource.is_automatic_pi_package())
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
                    arg.value_parser(values(ResourceKind::McpServer, false))
                })
            };
            sub.mut_arg("skills", |arg| {
                arg.value_parser(values(ResourceKind::Skill, name == "uninstall"))
            })
            .mut_arg("pi_packages", |arg| {
                arg.value_parser(values(ResourceKind::PiPackage, name == "uninstall"))
            })
            .mut_arg("herdr_plugins", |arg| {
                arg.value_parser(values(ResourceKind::HerdrPlugin, name == "uninstall"))
            })
            .mut_arg("tools", |arg| {
                arg.value_parser(values(ResourceKind::Tool, name == "uninstall"))
            })
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
    let selected = loom::app::resolve_selectors(&catalog, &selectors)?
        .into_iter()
        .map(|resource| resource.id)
        .collect();
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

/// `PATH:LINE` with an optional trailing `:COL`, which is accepted and ignored.
fn contracts_location(location: &str) -> Result<(PathBuf, usize)> {
    let segments: Vec<_> = location.split(':').collect();
    let numeric = |index: usize| {
        segments
            .get(index)
            .and_then(|part| part.parse::<usize>().ok())
    };
    let last = segments.len().saturating_sub(1);
    // A trailing column is accepted, so the line is the last-but-one number.
    let (line, end) = match (numeric(last), numeric(last.wrapping_sub(1))) {
        (Some(_), Some(line)) if segments.len() > 2 => (line, last - 1),
        (Some(line), _) => (line, last),
        _ => anyhow::bail!("expected PATH:LINE, got {location}"),
    };
    let path = segments[..end].join(":");
    anyhow::ensure!(!path.is_empty(), "expected PATH:LINE, got {location}");
    Ok((PathBuf::from(path), line))
}

fn run_contracts(command: ContractsCommand, json: bool) -> i32 {
    match command {
        ContractsCommand::Check { path } => run_contracts_check(path, json),
        ContractsCommand::At {
            location,
            no_global,
        } => {
            let found = contracts_location(&location)
                .and_then(|(path, line)| loom::contracts::governing(&path, line));
            run_contracts_query(
                found.map(|contracts| (without_global(contracts, no_global), false)),
                "at",
                &location,
                json,
            )
        }
        ContractsCommand::List { target, no_global } => run_contracts_query(
            loom::contracts::list(&target)
                .map(|report| (without_global(report.contracts, no_global), report.by_id)),
            "list",
            &target,
            json,
        ),
        ContractsCommand::Affected { base, stale } => run_contracts_query(
            loom::contracts::affected(&base, stale).map(|contracts| (contracts, false)),
            if stale { "stale" } else { "affected" },
            &base,
            json,
        ),
        ContractsCommand::Diff { base } => run_contracts_diff(&base, json),
        ContractsCommand::Propose { base } => run_contracts_propose(&base, json),
        ContractsCommand::Related { location, kind } => {
            run_contracts_related(&location, kind, json)
        }
    }
}

/// Render `propose`: the obligation sweep as a checklist. Loom lists the
/// declarations; deciding whether an obligation exists stays with the reader.
fn run_contracts_propose(base: &str, json: bool) -> i32 {
    let result = (|| {
        let proposals = match loom::contracts::propose(base) {
            Ok(proposals) => proposals,
            Err(error) => {
                report_contracts_error("propose", base, &error, json)?;
                return Ok(2);
            }
        };
        if json {
            write_contracts_json(&serde_json::json!({ "proposals": proposals }))?;
        } else {
            let out = Out::detect();
            let mut stdout = std::io::stdout().lock();
            out.write_title(&mut stdout, "contracts propose", base)?;
            if proposals.is_empty() {
                writeln!(
                    stdout,
                    "  {}",
                    out.muted("every changed declaration is contracted")
                )?;
            }
            let columns = columns();
            for proposal in &proposals {
                let place = format!("{}:{}", proposal.path.display(), proposal.line);
                let room = columns.saturating_sub(place.width() + 7).max(12);
                writeln!(
                    stdout,
                    "  {} · {}",
                    out.bold(&place),
                    ellipsize(proposal.name.trim_end_matches(['{', ' ']), room)
                )?;
                let ids: Vec<_> = proposal.governed_by.iter().map(|c| c.id.as_str()).collect();
                writeln!(
                    stdout,
                    "    {}",
                    out.muted(if ids.is_empty() {
                        "governed by: (none)".to_string()
                    } else {
                        format!("governed by: {}", ids.join(", "))
                    })
                )?;
            }
            out.write_verdict(
                &mut stdout,
                proposals.is_empty(),
                match proposals.len() {
                    0 => "Every changed declaration carries a contract".to_string(),
                    1 => "1 changed declaration carries no contract".to_string(),
                    count => format!("{count} changed declarations carry no contract"),
                },
            )?;
            out.write_next(
                &mut stdout,
                if proposals.is_empty() {
                    "run `loom contracts affected --stale` for contracts whose code moved"
                } else {
                    "for each: write a contract, or note why no obligation survives a rewrite"
                },
            )?;
        }
        Ok(i32::from(!proposals.is_empty()))
    })();
    contracts_output_status(result)
}

/// Render `diff`: what a change did to the contracts themselves. A reworded or
/// removed obligation is a finding, because its owners must agree to it.
fn run_contracts_diff(base: &str, json: bool) -> i32 {
    let result = (|| {
        let report = match loom::contracts::diff(base) {
            Ok(report) => report,
            Err(error) => {
                report_contracts_error("diff", base, &error, json)?;
                return Ok(2);
            }
        };
        let findings = report.changed.len() + report.removed.len();
        if json {
            write_contracts_json(&report)?;
        } else {
            let out = Out::detect();
            let mut stdout = std::io::stdout().lock();
            out.write_title(&mut stdout, "contracts diff", base)?;
            let after: Vec<_> = report.changed.iter().map(|c| c.after.clone()).collect();
            for (label, contracts) in [
                ("added", &report.added),
                ("changed", &after),
                ("removed", &report.removed),
            ] {
                if contracts.is_empty() {
                    continue;
                }
                writeln!(stdout, "  {}", out.bold(label))?;
                write_contract_entries(&out, &mut stdout, contracts, false)?;
            }
            if findings == 0 && report.added.is_empty() {
                writeln!(stdout, "  {}", out.muted("no contract changed"))?;
            }
            out.write_verdict(
                &mut stdout,
                findings == 0,
                match findings {
                    0 => "No obligation was weakened or removed".to_string(),
                    1 => "1 obligation changed or went away".to_string(),
                    count => format!("{count} obligations changed or went away"),
                },
            )?;
            out.write_next(
                &mut stdout,
                if findings == 0 {
                    "run `loom contracts affected --stale` for contracts whose code moved"
                } else {
                    "confirm each change with its owners; never weaken a contract to pass"
                },
            )?;
        }
        Ok(i32::from(findings > 0))
    })();
    contracts_output_status(result)
}

/// Render `related`: the governing contracts, then every place the declaration
/// is used. No usage is a finding, because a contracted declaration nobody
/// calls is worth knowing about.
fn run_contracts_related(location: &str, kind: RelatedKind, json: bool) -> i32 {
    let result = (|| {
        let report = match contracts_location(location).and_then(|(path, line)| {
            loom::contracts::related(&path, line, matches!(kind, RelatedKind::Callers))
        }) {
            Ok(report) => report,
            Err(error) => {
                report_contracts_error("related", location, &error, json)?;
                return Ok(2);
            }
        };
        if json {
            write_contracts_json(&serde_json::json!({
                "contracts": report.contracts,
                "related": report.related,
                "server": report.server,
                "fellBack": report.fell_back,
            }))?;
        } else {
            let out = Out::detect();
            let mut stdout = std::io::stdout().lock();
            out.write_title(&mut stdout, "contracts related", location)?;
            write_contract_entries(&out, &mut stdout, &report.contracts, false)?;
            let width = columns();
            for used in &report.related {
                let place = format!("{}:{}", used.path.display(), used.line);
                // A reference has no declaration name, so it ends at the location.
                if used.name.is_empty() {
                    writeln!(stdout, "  {}", out.muted(ellipsize(&place, width - 2)))?;
                    continue;
                }
                writeln!(
                    stdout,
                    "  {:<CONTRACT_LABEL_WIDTH$}  {}",
                    out.muted(ellipsize(&place, CONTRACT_LABEL_WIDTH)),
                    out.muted(ellipsize(&used.name, width.saturating_sub(26))),
                )?;
            }
            if report.fell_back {
                out.write_row(
                    &mut stdout,
                    Mark::Off,
                    report.server.as_str(),
                    "has no call hierarchy · showing references instead",
                )?;
            }
            let verdict = match report.related.len() {
                0 => format!("Nothing uses this, according to {}", report.server),
                1 => "1 place uses this".to_string(),
                count => format!("{count} places use this"),
            };
            out.write_verdict(&mut stdout, !report.related.is_empty(), verdict)?;
            out.write_next(
                &mut stdout,
                "read each use against the obligations above before changing them",
            )?;
        }
        Ok(i32::from(report.related.is_empty()))
    })();
    contracts_output_status(result)
}

/// `--no-global` keeps only the contracts attached to declarations, leaving out
/// the directory rules inherited from ancestor `CONTRACTS` files.
fn without_global(
    contracts: Vec<loom::contracts::Contract>,
    no_global: bool,
) -> Vec<loom::contracts::Contract> {
    if !no_global {
        return contracts;
    }
    contracts
        .into_iter()
        .filter(|contract| contract.scope != loom::contracts::Scope::Directory)
        .collect()
}

/// Render contracts found by `at` or `list`: a lookup by identifier that finds
/// nothing is a finding, an empty path listing is not.
fn run_contracts_query(
    found: Result<(Vec<loom::contracts::Contract>, bool)>,
    command: &str,
    target: &str,
    json: bool,
) -> i32 {
    let result = (|| {
        let (contracts, by_id) = match found {
            Ok(found) => found,
            Err(error) => {
                report_contracts_error(command, target, &error, json)?;
                return Ok(2);
            }
        };
        if json {
            write_contracts_json(&serde_json::json!({ "contracts": contracts }))?;
        } else {
            let out = Out::detect();
            let mut stdout = std::io::stdout().lock();
            out.write_title(&mut stdout, &format!("contracts {command}"), target)?;
            write_contract_entries(&out, &mut stdout, &contracts, by_id)?;
            // `affected` offers candidates for review; it never judges compliance.
            let verdict = match (contracts.len(), by_id, command) {
                (0, true, _) => "No contract carries that identifier".to_string(),
                (0, _, "affected") => "This diff touches no contract".to_string(),
                (0, _, "stale") => "No contract was left behind by this diff".to_string(),
                (0, false, _) => "No contract applies here".to_string(),
                (1, _, "affected") => "1 candidate obligation for this diff".to_string(),
                (count, _, "affected") => format!("{count} candidate obligations for this diff"),
                (1, _, "stale") => "1 contract's code changed without its text".to_string(),
                (count, _, "stale") => {
                    format!("{count} contracts' code changed without their text")
                }
                (1, _, _) => "1 contract applies".to_string(),
                (count, _, _) => format!("{count} contracts apply"),
            };
            out.write_verdict(&mut stdout, !(by_id && contracts.is_empty()), verdict)?;
            out.write_next(
                &mut stdout,
                if command == "stale" && !contracts.is_empty() {
                    "reread each obligation against the new code; update it or confirm it holds"
                } else if command == "affected" && !contracts.is_empty() {
                    "judge each candidate against the diff; Loom does not decide compliance"
                } else if contracts.is_empty() {
                    "run `loom contracts list .` to see every contract in this repository"
                } else if by_id {
                    "run `loom contracts at PATH:LINE` to see what governs a line"
                } else {
                    "run `loom contracts list ID` to read one obligation in full"
                },
            )?;
        }
        Ok(i32::from(by_id && contracts.is_empty()))
    })();
    contracts_output_status(result)
}

/// One entry per contract: identifier, where it lives, what it governs, and its
/// obligation underneath. Listing is information, so entries carry no status mark.
fn write_contract_entries(
    out: &Out,
    writer: &mut impl std::io::Write,
    contracts: &[loom::contracts::Contract],
    by_id: bool,
) -> std::io::Result<()> {
    if contracts.is_empty() {
        // The body always says something, so the header never abuts the verdict.
        return writeln!(
            writer,
            "  {}",
            out.muted(if by_id {
                "no match in this repository"
            } else {
                "nothing here yet · write one beside the code it governs"
            })
        );
    }
    // One line per entry: the declaration is trimmed before the obligation is.
    let columns = columns();
    for contract in contracts {
        let mut place = format!("{}:{}", contract.path.display(), contract.line);
        if let loom::contracts::Scope::Declaration { name, .. } = &contract.scope {
            let room = columns
                .saturating_sub(CONTRACT_LABEL_WIDTH + place.width() + 9)
                .max(12);
            place.push_str(" · ");
            place.push_str(&ellipsize(name.trim_end_matches(['{', ' ']), room));
        }
        writeln!(
            writer,
            "  {:<width$}  {}",
            out.bold(&contract.id),
            out.muted(place),
            // Padding counts the identifier, not the escape codes around it.
            width = CONTRACT_LABEL_WIDTH + (out.bold(&contract.id).len() - contract.id.len())
        )?;
        for line in contract.prose.lines() {
            writeln!(
                writer,
                "    {}",
                out.muted(ellipsize(line, columns.saturating_sub(4)))
            )?;
        }
        if let Some(owners) = contract
            .metadata
            .iter()
            .find(|(key, _)| key == "owner")
            .map(|(_, values)| values.join(", "))
        {
            writeln!(writer, "    {}", out.muted(format!("ask {owners}")))?;
        }
    }
    Ok(())
}

/// Identifiers are short; the obligation deserves the rest of the line. Entry
/// text starts where a marked row's detail does, so both align in one report.
const CONTRACT_LABEL_WIDTH: usize = 22;

/// Failures keep the report's shape: header, one cause, verdict, next action.
fn report_contracts_error(
    command: &str,
    target: &str,
    error: &anyhow::Error,
    json: bool,
) -> Result<()> {
    if json {
        write_contracts_json(
            &serde_json::json!({"errors": [{"path": target, "message": format!("{error:#}")}]}),
        )?;
    } else {
        let out = Out::detect();
        let mut stdout = std::io::stdout().lock();
        out.write_title(&mut stdout, &format!("contracts {command}"), target)?;
        out.write_row(&mut stdout, Mark::Bad, target, format!("{error:#}"))?;
        out.write_verdict(&mut stdout, false, "Nothing was read")?;
        out.write_next(&mut stdout, "check the path, then run the command again")?;
    }
    Ok(())
}

fn run_contracts_check(path: Option<PathBuf>, json: bool) -> i32 {
    let path = path.unwrap_or_else(|| PathBuf::from("."));
    let result = (|| {
        let report = match loom::contracts::check(&path) {
            Ok(report) => report,
            Err(error) => {
                report_contracts_error("check", &path.display().to_string(), &error, json)?;
                return Ok(2);
            }
        };
        if json {
            write_contracts_json(&report)?;
        } else {
            let out = Out::detect();
            let mut stdout = std::io::stdout().lock();
            let files = match report.checked {
                1 => "1 file".to_string(),
                count => format!("{count} files"),
            };
            out.write_title(&mut stdout, "contracts check", format!("{files} read"))?;
            if report.diagnostics.is_empty() {
                writeln!(
                    &mut stdout,
                    "  {}",
                    out.muted("no findings · structure only, prose is never judged")
                )?;
            }
            for diagnostic in &report.diagnostics {
                out.write_row(
                    &mut stdout,
                    Mark::Bad,
                    &format!("{}:{}", diagnostic.path.display(), diagnostic.line),
                    &diagnostic.message,
                )?;
            }
            out.write_verdict(
                &mut stdout,
                report.diagnostics.is_empty(),
                match report.diagnostics.len() {
                    0 => "Every contract is well formed".to_string(),
                    1 => "1 contract needs attention".to_string(),
                    count => format!("{count} contracts need attention"),
                },
            )?;
            out.write_next(
                &mut stdout,
                if report.diagnostics.is_empty() {
                    "run `loom contracts at PATH:LINE` before editing contracted code"
                } else {
                    "fix the listed lines, then run `loom contracts check` again"
                },
            )?;
        }
        Ok(i32::from(!report.diagnostics.is_empty()))
    })();
    contracts_output_status(result)
}

fn write_contracts_json(value: &impl serde::Serialize) -> Result<()> {
    let text = serde_json::to_string(value)?;
    writeln!(std::io::stdout().lock(), "{text}")?;
    Ok(())
}

fn contracts_output_status(result: Result<i32>) -> i32 {
    result
        .and_then(|code| {
            std::io::stdout().flush()?;
            Ok(code)
        })
        .unwrap_or_else(|error| {
            // Stdout may contain a partial report. Never append a second response.
            let _ = writeln!(std::io::stderr(), "cannot write contracts output: {error}");
            2
        })
}

/**
@cc [owner:Yassimba] contracts-json-single-object
When --json is selected for contracts and stdout remains writable, every successful, finding, usage-error, or filesystem-error result writes exactly one JSON object to stdout and no other stdout text.
*/
/**
@cc [owner:Yassimba] contracts-output-io-error
If writing a contracts check result or JSON usage error fails, exit with code 2 without panicking or retrying output on stdout, including after a partial write.
*/
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let cli = Cli::try_parse_from(&args).unwrap_or_else(|error| {
        // Clap errors occur before typed dispatch. Help/version keep Clap's normal output.
        let contracts_json = args.get(1).is_some_and(|arg| arg == "contracts")
            && args
                .iter()
                .skip(2)
                .take_while(|arg| *arg != "--")
                .any(|arg| arg == "--json");
        if error.use_stderr() && contracts_json {
            let result = write_contracts_json(
                &serde_json::json!({"errors": [{"message": error.to_string()}]}),
            )
            .map(|()| 2);
            std::process::exit(contracts_output_status(result));
        }
        error.exit()
    });
    let system = RealSystem::default();
    // Validation must bypass setup's PATH probes and bootstrap ownership writes.
    if !matches!(cli.command, Some(Command::Contracts { .. })) {
        loom::System::refresh_path(&system);
        if let Some(home) = loom::System::home_dir(&system) {
            loom::ownership::record_bootstrap_from_env(&home).map_err(anyhow::Error::msg)?;
        }
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
                let existing = |path| WikiSetupArgs {
                    path,
                    yes: true,
                    ..Default::default()
                };
                let (operation, args) = match command {
                    WikiCommand::Create(args) => (WikiOperation::Create, args),
                    WikiCommand::Adopt(args) => (WikiOperation::Adopt, args),
                    WikiCommand::Status => (WikiOperation::Status, existing(PathBuf::new())),
                    WikiCommand::Repair { path } => (WikiOperation::Repair, existing(path)),
                    WikiCommand::Unregister { path } => (WikiOperation::Unregister, existing(path)),
                    WikiCommand::Open { path } => (WikiOperation::Open, existing(path)),
                    WikiCommand::Launch { path } => (WikiOperation::Launch, existing(path)),
                };
                let request = WikiRequest {
                    operation,
                    vault: args.path,
                    feynman: args.feynman,
                    confluence: args.confluence,
                    qmd: args.qmd,
                    yes: args.yes,
                };
                loom::wiki::run_wiki(&request, &system)?
            }
        },
        Command::Contracts { json, command } => std::process::exit(run_contracts(command, json)),
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
                    coding_standards: flag(coding_standards, no_coding_standards),
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
                Out::detect().verdict(true, "Cancelled; no changes made");
                true
            } else {
                let out = Out::detect();
                let inside = std::env::var("HERDR_ENV").ok().as_deref() == Some("1");
                let herdr_present = loom::System::command_exists(&system, "herdr");
                let server_running = herdr_present && probe_herdr_server_running(&system);
                let herdr = match herdr_gate(herdr_present, inside, server_running) {
                    HerdrGate::None => HerdrLane::Run,
                    HerdrGate::Ready => HerdrLane::Run,
                    HerdrGate::Inside => {
                        if yes
                            || Confirm::new(
                                "You're inside Herdr. Please run `loom update` from a regular terminal. Continue without updating Herdr?",
                            )
                            .with_default(true)
                            .prompt()?
                        {
                            HerdrLane::Skip(HERDR_SKIP_INSIDE)
                        } else {
                            Out::detect().verdict(true, "Cancelled; no changes made");
                            return Ok(());
                        }
                    }
                    HerdrGate::StopServer if yes => HerdrLane::Run,
                    HerdrGate::StopServer => {
                        if Confirm::new("Herdr's server is running. Close it so Herdr can update?")
                            .with_default(true)
                            .prompt()?
                        {
                            match loom::System::run(
                                &system,
                                &CommandSpec::new("herdr", ["server", "stop"]),
                            ) {
                                Ok(result) if result.success => HerdrLane::Run,
                                _ => HerdrLane::Skip(HERDR_SKIP_SERVER),
                            }
                        } else {
                            HerdrLane::Skip(HERDR_SKIP_SERVER)
                        }
                    }
                };
                let updated = run_updates(&system, &Catalog::embedded()?, herdr);
                let wikis_updated = loom::wiki::update_registered(&system, !yes, &out);
                let success = updated && wikis_updated;
                out.verdict(
                    success,
                    if success {
                        "Update complete"
                    } else {
                        "Update incomplete · completed work stays"
                    },
                );
                out.next(if !wikis_updated {
                    "run `loom wiki` to repair the flagged Vault"
                } else if herdr == HerdrLane::Skip(HERDR_SKIP_INSIDE) {
                    "run `loom update` from a regular terminal to update Herdr"
                } else if herdr == HerdrLane::Skip(HERDR_SKIP_SERVER) {
                    "run `herdr server stop`, then `loom update` to update Herdr"
                } else if !updated {
                    "resolve the reported cause, then run `loom update --yes` again"
                } else {
                    "run `loom status` to verify the updated setup"
                });
                success
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
    fn pi_loom_is_not_an_install_selector_but_can_be_uninstalled() {
        let catalog = Catalog::embedded().unwrap();

        assert!(completion_command(&catalog)
            .try_get_matches_from(["loom", "setup", "--pi-package", "Loom"])
            .is_err());
        assert!(completion_command(&catalog)
            .try_get_matches_from(["loom", "uninstall", "--pi-package", "Loom", "--yes"])
            .is_ok());
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
                command: Some(WikiCommand::Create(WikiSetupArgs {
                    feynman: true,
                    confluence: true,
                    yes: true,
                    ..
                }))
            })
        ));
    }

    #[test]
    fn wiki_has_unregister_but_no_delete_command() {
        assert!(Cli::try_parse_from(["loom", "wiki", "unregister", "/tmp/vault"]).is_ok());
        assert!(Cli::try_parse_from(["loom", "wiki", "delete", "/tmp/vault"]).is_err());
    }
}
