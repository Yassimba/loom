//! `loom init` — make a repository ready for coding agents: instructions,
//! issue tracking, domain docs, editor links, coding standards, and optional
//! integrations selected from project and machine evidence.
//!
//! Templates are configuration: editing them in the repo and merging to main
//! changes what every future init writes, like the tool manifest. Sections
//! land wrapped in `<!-- loom:section:<name> -->` markers so a re-run
//! appends missing sections and never touches anything else.

use crate::ui::{tidy_path, Mark, Out};
use crate::{skills, CommandSpec, System};
use anyhow::{Context, Result};
use inquire::Select;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum Tracker {
    Beads,
    Local,
}

impl fmt::Display for Tracker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Beads => "Beads (br + bv)",
            Self::Local => "Local Markdown",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum DomainLayout {
    Single,
    Multi,
}

impl fmt::Display for DomainLayout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Single => "Single context",
            Self::Multi => "Multiple contexts",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum Editor {
    Vscode,
    Zed,
    Cursor,
    Jetbrains,
    None,
}

impl Editor {
    fn deep_link(self) -> Option<&'static str> {
        match self {
            Self::Vscode => Some("vscode://file/{path}:{line}"),
            Self::Zed => Some("zed://file/{path}:{line}"),
            Self::Cursor => Some("cursor://file/{path}:{line}"),
            Self::Jetbrains => Some("idea://open?file={path}&line={line}"),
            Self::None => None,
        }
    }
}

impl fmt::Display for Editor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Vscode => "VS Code",
            Self::Zed => "Zed",
            Self::Cursor => "Cursor",
            Self::Jetbrains => "JetBrains",
            Self::None => "None",
        })
    }
}

pub struct InitOptions {
    pub python: Option<bool>,
    pub rust: Option<bool>,
    pub adhd: Option<bool>,
    pub tracker: Option<Tracker>,
    pub domain: Option<DomainLayout>,
    pub editor: Option<Editor>,
    pub coding_standards: Option<bool>,
    pub codegraph: Option<bool>,
    pub yes: bool,
    pub force: bool,
}

struct Section {
    name: &'static str,
    template: &'static str,
}

const SECTIONS: [Section; 5] = [
    Section {
        name: "python",
        template: "manifest/init/sections/python.md",
    },
    Section {
        name: "rust",
        template: "manifest/init/sections/rust.md",
    },
    Section {
        name: "i-have-adhd",
        template: "manifest/init/sections/i-have-adhd.md",
    },
    Section {
        name: "coding-standards",
        template: "manifest/init/sections/coding-standards.md",
    },
    Section {
        name: "project-setup",
        template: "manifest/init/sections/project-setup.md",
    },
];

const RETIRED_SECTIONS: [&str; 1] = ["beads"];

const BASE_TEMPLATE: &str = "manifest/init/AGENTS.base.md";
const CODING_STANDARDS_TEMPLATE: &str = "manifest/init/CODING_STANDARDS.md";
const BEADS_WORKFLOW_TEMPLATE: &str = "skills/loom/references/issue-tracker-beads.md";
const LOCAL_WORKFLOW_TEMPLATE: &str = "skills/loom/references/issue-tracker-local.md";
const DOMAIN_TEMPLATE: &str = "skills/loom/references/domain.md";

fn marker_open(name: &str) -> String {
    format!("<!-- loom:section:{name}")
}

/// FNV-1a over the trimmed template: enough to tell "pristine" from
/// "hand-edited inside the fence"; not a security boundary.
fn stamp(content: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in content.trim().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn wrap_section(name: &str, content: &str) -> String {
    format!(
        "\n<!-- loom:section:{name} hash:{} -->\n\n{}\n<!-- /loom:section:{name} -->\n",
        stamp(content),
        content.trim_end()
    )
}

/// One managed fence found in an existing AGENTS.md.
struct Fence {
    start: usize,
    end: usize,
    stamped: Option<String>,
    body: String,
}

/// Locate `name`'s fence: from its opening marker line through the closing
/// marker (inclusive). Returns None when the file has no such fence.
fn find_fence(content: &str, name: &str) -> Option<Fence> {
    let open_prefix = marker_open(name);
    let close = format!("<!-- /loom:section:{name} -->");
    let start = content.find(&open_prefix)?;
    let open_end = start + content[start..].find("-->")? + 3;
    let close_start = content[open_end..].find(&close)? + open_end;
    let end = close_start + close.len();
    let header = &content[start..open_end];
    let stamped = header
        .split_whitespace()
        .find_map(|part| part.strip_prefix("hash:"))
        .map(|hash| hash.trim_end_matches("-->").to_string());
    let body = content[open_end..close_start].to_string();
    Some(Fence {
        start,
        end,
        stamped,
        body,
    })
}

/// Detection defaults from evidence in the project.
fn detect(project: &Path) -> (bool, bool) {
    let python = ["pyproject.toml", "setup.py", "requirements.txt"]
        .iter()
        .any(|file| project.join(file).exists());
    let rust = project.join("Cargo.toml").exists();
    (python, rust)
}

fn select_yes_no(prompt: &str, help: &str, default: bool, assume_yes: bool) -> Result<bool> {
    if assume_yes || !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(default);
    }
    Ok(Select::new(prompt, vec!["Yes", "No"])
        .with_help_message(help)
        .with_starting_cursor(usize::from(!default))
        .without_filtering()
        .prompt()?
        == "Yes")
}

fn select_choice<T>(
    prompt: &str,
    help: &str,
    choices: Vec<T>,
    default: T,
    assume_yes: bool,
) -> Result<T>
where
    T: Clone + Eq + fmt::Display,
{
    if assume_yes || !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(default);
    }
    let cursor = choices
        .iter()
        .position(|choice| choice == &default)
        .unwrap_or_default();
    Ok(Select::new(prompt, choices)
        .with_help_message(help)
        .with_starting_cursor(cursor)
        .without_filtering()
        .prompt()?)
}

fn detect_editor(system: &dyn System) -> Editor {
    [
        ("cursor", Editor::Cursor),
        ("zed", Editor::Zed),
        ("code", Editor::Vscode),
        ("idea", Editor::Jetbrains),
    ]
    .into_iter()
    .find_map(|(command, editor)| system.command_exists(command).then_some(editor))
    .unwrap_or(Editor::None)
}

#[derive(Debug, Eq, PartialEq)]
struct InitFeatures {
    project_instructions: bool,
    python: bool,
    rust: bool,
    adhd: bool,
    tracker: Option<Tracker>,
    domain: Option<DomainLayout>,
    editor: Option<Editor>,
    coding_standards: bool,
    codegraph: bool,
}

fn has_project_selection(options: &InitOptions) -> bool {
    options.python == Some(true)
        || options.rust == Some(true)
        || options.adhd == Some(true)
        || options.tracker.is_some()
        || options.domain.is_some()
        || options.editor.is_some()
        || options.coding_standards == Some(true)
}

fn has_explicit_selection(options: &InitOptions) -> bool {
    has_project_selection(options) || options.codegraph == Some(true)
}

fn choose_features(
    project: &Path,
    beads_tools_installed: bool,
    codegraph_installed: bool,
    default_editor: Editor,
    options: &InitOptions,
    mut ask: impl FnMut(&'static str, &'static str, bool) -> Result<bool>,
) -> Result<InitFeatures> {
    if has_explicit_selection(options) {
        if options.tracker == Some(Tracker::Beads) && !beads_tools_installed {
            anyhow::bail!("Beads needs br and bv; run `loom add --tool beads --tool beads-viewer`");
        }
        if options.codegraph == Some(true) && !codegraph_installed {
            anyhow::bail!("CodeGraph is not installed; run `loom add --tool codegraph`");
        }
        return Ok(InitFeatures {
            project_instructions: has_project_selection(options),
            python: options.python.unwrap_or(false),
            rust: options.rust.unwrap_or(false),
            adhd: options.adhd.unwrap_or(false),
            tracker: options.tracker,
            domain: options.domain,
            editor: options.editor,
            coding_standards: options.coding_standards.unwrap_or(false),
            codegraph: options.codegraph.unwrap_or(false),
        });
    }

    let (python_default, rust_default) = detect(project);
    let project_instructions = ask(
        "Set up project agent instructions?",
        "Creates or updates AGENTS.md and CLAUDE.md for this project.",
        true,
    )?;
    let (python, rust, adhd, tracker, domain, editor, coding_standards) = if project_instructions {
        (
            match options.python {
                Some(explicit) => explicit,
                None => ask(
                    "Add Python instructions?",
                    "Adds typing, uv, and Python quality commands to AGENTS.md.",
                    python_default,
                )?,
            },
            match options.rust {
                Some(explicit) => explicit,
                None => ask(
                    "Add Rust instructions?",
                    "Adds Rust conventions, Clippy, and test commands to AGENTS.md.",
                    rust_default,
                )?,
            },
            match options.adhd {
                Some(explicit) => explicit,
                None => ask(
                    "Use ADHD-friendly agent output?",
                    "Requests short, scannable progress updates for this project.",
                    false,
                )?,
            },
            Some(match options.tracker {
                Some(explicit) => explicit,
                None => select_choice(
                    "Choose the issue tracker",
                    "Beads provides a dependency graph; local Markdown stores issues under ai-docs/plans/.",
                    vec![Tracker::Beads, Tracker::Local],
                    if beads_tools_installed || project.join(".beads").is_dir() {
                        Tracker::Beads
                    } else {
                        Tracker::Local
                    },
                    options.yes,
                )?,
            }),
            Some(match options.domain {
                Some(explicit) => explicit,
                None => select_choice(
                    "Choose the domain-doc layout",
                    "Most repositories have one context; monorepos may map several contexts.",
                    vec![DomainLayout::Single, DomainLayout::Multi],
                    if project.join("CONTEXT-MAP.md").exists() {
                        DomainLayout::Multi
                    } else {
                        DomainLayout::Single
                    },
                    options.yes,
                )?,
            }),
            Some(match options.editor {
                Some(explicit) => explicit,
                None => select_choice(
                    "Choose the editor for source links",
                    "Agents use this URL scheme for clickable file and line links.",
                    vec![
                        Editor::Vscode,
                        Editor::Zed,
                        Editor::Cursor,
                        Editor::Jetbrains,
                        Editor::None,
                    ],
                    default_editor,
                    options.yes,
                )?,
            }),
            match options.coding_standards {
                Some(explicit) => explicit,
                None => ask(
                    "Add coding standards?",
                    "Creates CODING_STANDARDS.md with type, design, and simplicity checks.",
                    true,
                )?,
            },
        )
    } else {
        (false, false, false, None, None, None, false)
    };
    if tracker == Some(Tracker::Beads) && !beads_tools_installed {
        anyhow::bail!("Beads needs br and bv; run `loom add --tool beads --tool beads-viewer`");
    }
    let codegraph = match options.codegraph {
        Some(true) if !codegraph_installed => {
            anyhow::bail!("CodeGraph is not installed; run `loom add --tool codegraph`");
        }
        Some(explicit) => explicit,
        None if codegraph_installed => ask(
            "Set up CodeGraph?",
            "Wires installed agents and indexes this project.",
            true,
        )?,
        None => false,
    };
    Ok(InitFeatures {
        project_instructions,
        python,
        rust,
        adhd,
        tracker,
        domain,
        editor,
        coding_standards,
        codegraph,
    })
}

fn setup_beads(system: &dyn System, project: &Path) -> Result<bool> {
    if project.join(".beads").is_dir() {
        return Ok(false);
    }
    let result = system
        .run(&CommandSpec::new("br", ["init", "--quiet"]))
        .context("could not initialize Beads")?;
    if !result.success {
        anyhow::bail!(
            "Beads setup failed: {}",
            crate::install::command_failure_message(&result)
        );
    }
    Ok(true)
}

fn write_seed_file(path: &Path, content: &str, force: bool) -> Result<&'static str> {
    if path.exists() && !force {
        return Ok("has its own content; left alone");
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(if force { "written" } else { "created" })
}

fn editor_document(editor: Editor) -> String {
    match editor.deep_link() {
        Some(template) => format!(
            "# Editor links\n\nEditor: {editor}\n\nUse `{template}` for clickable source links. `{{path}}` is absolute.\n"
        ),
        None => "# Editor links\n\nEditor: None\n\nUse plain `path:line` source references.\n".into(),
    }
}

fn domain_document(layout: DomainLayout, template: &str) -> String {
    let layout = match layout {
        DomainLayout::Single => "single-context",
        DomainLayout::Multi => "multi-context",
    };
    format!("{template}\n\nSelected layout: **{layout}**.\n")
}

fn render_gitignore(existing: &str) -> Option<String> {
    let covered = existing
        .lines()
        .map(str::trim)
        .any(|line| line.trim_start_matches('/').trim_end_matches('/') == "ai-docs");
    if covered {
        return None;
    }
    let separator = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    Some(format!("{existing}{separator}ai-docs/\n"))
}

fn ignore_agent_docs(project: &Path) -> Result<&'static str> {
    let path = project.join(".gitignore");
    let existing = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let Some(updated) = render_gitignore(&existing) else {
        return Ok("already ignores ai-docs/");
    };
    fs::write(&path, updated)?;
    Ok("ignores ai-docs/")
}

fn setup_codegraph(system: &dyn System) -> Result<()> {
    for command in [
        CommandSpec::new("codegraph", ["install", "--yes", "--location", "local"]),
        CommandSpec::new("codegraph", ["init"]),
    ] {
        let result = system
            .run(&command)
            .context("could not start CodeGraph setup")?;
        if !result.success {
            anyhow::bail!(
                "CodeGraph setup failed: {}",
                crate::install::command_failure_message(&result)
            );
        }
    }
    Ok(())
}

fn init_has_consent(options: &InitOptions, interactive: bool) -> bool {
    interactive
        || options.yes
        || options.python.is_some()
        || options.rust.is_some()
        || options.adhd.is_some()
        || options.tracker.is_some()
        || options.domain.is_some()
        || options.editor.is_some()
        || options.coding_standards.is_some()
        || options.codegraph.is_some()
}

pub fn run_init(system: &dyn System, options: &InitOptions) -> Result<bool> {
    if !init_has_consent(options, std::io::IsTerminal::is_terminal(&std::io::stdin())) {
        anyhow::bail!(
            "non-interactive `loom init` needs --yes or explicit feature flags; no files changed"
        );
    }
    let project = std::env::current_dir().context("no current directory")?;
    let features = choose_features(
        &project,
        system.command_exists("br") && system.command_exists("bv"),
        system.command_exists("codegraph"),
        detect_editor(system),
        options,
        |prompt, help, default| select_yes_no(prompt, help, default, options.yes),
    )?;
    let out = Out::detect();
    let home = system.home_dir().context("home directory is unavailable")?;
    out.title("init", tidy_path(&project, &home));
    if !features.project_instructions {
        if features.codegraph {
            setup_codegraph(system)?;
            out.row(Mark::Ok, "CodeGraph", "agents wired, project indexed");
            out.verdict(true, "Done");
        } else {
            out.verdict(true, "Nothing selected; no changes made");
        }
        return Ok(true);
    }
    // Templates come from the published repo, so init output is publish-gated.
    let staging = home.join(".cache").join("loom").join("init-staging");
    let repo_root = skills::fetch_repo(system, &staging)
        .map_err(anyhow::Error::msg)
        .context("could not fetch the templates")?;
    let base = fs::read_to_string(repo_root.join(BASE_TEMPLATE))
        .with_context(|| format!("template missing: {BASE_TEMPLATE}"))?;
    let coding_standards = features
        .coding_standards
        .then(|| fs::read_to_string(repo_root.join(CODING_STANDARDS_TEMPLATE)))
        .transpose()
        .with_context(|| format!("template missing: {CODING_STANDARDS_TEMPLATE}"))?;
    let issue_tracker_template = match features.tracker {
        Some(Tracker::Beads) => Some(BEADS_WORKFLOW_TEMPLATE),
        Some(Tracker::Local) => Some(LOCAL_WORKFLOW_TEMPLATE),
        None => None,
    };
    let issue_tracker = issue_tracker_template
        .map(|template| fs::read_to_string(repo_root.join(template)))
        .transpose()
        .with_context(|| {
            format!(
                "template missing: {}",
                issue_tracker_template.unwrap_or_default()
            )
        })?;
    let domain = features
        .domain
        .map(|_| fs::read_to_string(repo_root.join(DOMAIN_TEMPLATE)))
        .transpose()
        .with_context(|| format!("template missing: {DOMAIN_TEMPLATE}"))?;
    let mut chosen: Vec<(&'static str, String)> = Vec::new();
    for section in &SECTIONS {
        let wanted = match section.name {
            "python" => features.python,
            "rust" => features.rust,
            "i-have-adhd" => features.adhd,
            "coding-standards" => features.coding_standards,
            "project-setup" => features.tracker.is_some(),
            _ => false,
        };
        if wanted {
            let content = fs::read_to_string(repo_root.join(section.template))
                .with_context(|| format!("template missing: {}", section.template))?;
            chosen.push((section.name, content));
        }
    }
    let _ = fs::remove_dir_all(&staging);

    let mut ok = true;
    let agents_path = project.join("AGENTS.md");
    let existing = fs::read_to_string(&agents_path).ok();
    let (agents, outcome) = render_agents(existing.as_deref(), &base, &chosen, options.force);
    let sections = chosen
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ");
    match agents {
        Some(content) => {
            fs::write(&agents_path, content)
                .with_context(|| format!("could not write {}", agents_path.display()))?;
            if existing.is_none() {
                let detail = if sections.is_empty() {
                    "created".to_owned()
                } else {
                    format!("created with {sections}")
                };
                out.row(Mark::Ok, "AGENTS.md", detail);
            } else {
                let mut notes = Vec::new();
                if !outcome.appended.is_empty() {
                    notes.push(format!("added {}", outcome.appended.join(", ")));
                }
                if !outcome.refreshed.is_empty() {
                    notes.push(format!("refreshed {}", outcome.refreshed.join(", ")));
                }
                if !outcome.removed.is_empty() {
                    notes.push(format!("removed retired {}", outcome.removed.join(", ")));
                }
                out.row(Mark::Ok, "AGENTS.md", notes.join(" · "));
            }
        }
        None => out.row(Mark::Ok, "AGENTS.md", "already current"),
    }
    for name in &outcome.kept_edited {
        out.row(
            Mark::Off,
            "AGENTS.md",
            format!("kept your edited {name} section (the template moved; --force rewrites)"),
        );
    }

    let claude_path = project.join("CLAUDE.md");
    match fs::read_to_string(&claude_path) {
        Err(_) => {
            fs::write(&claude_path, "@AGENTS.md\n")
                .with_context(|| format!("could not write {}", claude_path.display()))?;
            out.row(Mark::Ok, "CLAUDE.md", "created, points at AGENTS.md");
        }
        Ok(content) if content.trim() == "@AGENTS.md" => {
            out.row(Mark::Ok, "CLAUDE.md", "already points at AGENTS.md");
        }
        Ok(_) if options.force => {
            fs::write(&claude_path, "@AGENTS.md\n")?;
            out.row(Mark::Ok, "CLAUDE.md", "rewritten to point at AGENTS.md");
        }
        Ok(_) => {
            out.row(Mark::Off, "CLAUDE.md", "has its own content; left alone");
            out.note("move it into AGENTS.md and keep just `@AGENTS.md`, or rerun with --force");
        }
    }

    if let Some(content) = coding_standards {
        let path = project.join("CODING_STANDARDS.md");
        let detail = write_seed_file(&path, &content, options.force)
            .with_context(|| format!("could not write {}", path.display()))?;
        out.row(Mark::Ok, "CODING_STANDARDS.md", detail);
    }
    if let (Some(tracker), Some(content)) = (features.tracker, issue_tracker) {
        if tracker == Tracker::Beads {
            let initialized = setup_beads(system, &project)?;
            out.row(
                Mark::Ok,
                "Beads",
                if initialized {
                    "br initialized; bv workflow configured"
                } else {
                    "already initialized; bv workflow configured"
                },
            );
        }
        let path = project.join("ai-docs/agents/issue-tracker.md");
        let detail = write_seed_file(&path, &content, options.force)
            .with_context(|| format!("could not write {}", path.display()))?;
        out.row(Mark::Ok, "Issue tracker", format!("{tracker}: {detail}"));
    }
    if let (Some(layout), Some(content)) = (features.domain, domain) {
        let path = project.join("ai-docs/agents/domain.md");
        let content = domain_document(layout, &content);
        let detail = write_seed_file(&path, &content, options.force)
            .with_context(|| format!("could not write {}", path.display()))?;
        out.row(Mark::Ok, "Domain docs", format!("{layout}: {detail}"));
    }
    if let Some(editor) = features.editor {
        let path = project.join("ai-docs/agents/editor.md");
        let detail = write_seed_file(&path, &editor_document(editor), options.force)
            .with_context(|| format!("could not write {}", path.display()))?;
        out.row(Mark::Ok, "Editor", format!("{editor}: {detail}"));
    }
    let ignore_detail = ignore_agent_docs(&project).context("could not update .gitignore")?;
    out.row(Mark::Ok, ".gitignore", ignore_detail);
    if features.codegraph {
        setup_codegraph(system)?;
        out.row(Mark::Ok, "CodeGraph", "agents wired, project indexed");
    }

    match register_project(&home, &project) {
        Ok(()) => out.row(
            Mark::Ok,
            "Sync",
            "registered; `loom update` refreshes the templates",
        ),
        Err(error) => {
            ok = false;
            out.row(
                Mark::Bad,
                "Sync",
                format!("could not register the project: {error}"),
            );
        }
    }
    out.verdict(ok, if ok { "Done" } else { "Done with problems" });
    if existing.is_none() {
        out.next("open AGENTS.md and fill in the project section");
    }
    Ok(ok)
}

/// What happened to each managed fence during a render.
#[derive(Debug, Default)]
pub(crate) struct RenderOutcome {
    pub refreshed: Vec<&'static str>,
    pub appended: Vec<&'static str>,
    pub removed: Vec<&'static str>,
    /// Edited inside the fence: preserved, listed so the user knows the
    /// published template moved past their copy.
    pub kept_edited: Vec<&'static str>,
}

/// Build the new AGENTS.md content (None when nothing needs writing) plus a
/// report of what moved. The contract: text inside loom:section fences
/// belongs to Loom and refreshes when templates change — but only while
/// pristine (its body still matches the stamped template hash). Hand-edited
/// fences are kept and reported; everything outside fences is never touched.
fn render_agents(
    existing: Option<&str>,
    base: &str,
    chosen: &[(&'static str, String)],
    force: bool,
) -> (Option<String>, RenderOutcome) {
    let mut outcome = RenderOutcome::default();
    let Some(current) = existing else {
        return (Some(render_fresh(base, chosen)), outcome);
    };
    if force {
        return (Some(render_fresh(base, chosen)), outcome);
    }

    let mut updated = current.to_string();
    let mut managed: Vec<(&'static str, &str)> = vec![("base", base)];
    for (name, content) in chosen {
        managed.push((name, content));
    }
    for (name, template) in &managed {
        match find_fence(&updated, name) {
            Some(fence) => {
                let pristine = fence
                    .stamped
                    .as_deref()
                    .map(|stamped| stamped == stamp(&fence.body))
                    // Unstamped fences (pre-hash inits): pristine when the
                    // body equals the current template.
                    .unwrap_or_else(|| fence.body.trim() == template.trim());
                let fresh = wrap_section(name, template);
                let current_slice = &updated[fence.start..fence.end];
                if pristine && current_slice.trim() != fresh.trim() {
                    updated.replace_range(fence.start..fence.end, fresh.trim_end());
                    outcome.refreshed.push(name);
                } else if !pristine && stamp(&fence.body) != stamp(template) {
                    outcome.kept_edited.push(name);
                }
            }
            None if *name != "base" => {
                updated = format!("{}\n{}", updated.trim_end(), wrap_section(name, template));
                outcome.appended.push(name);
            }
            // A file without a base fence keeps its own top; init never
            // rewrites what it cannot prove it owns.
            None => {}
        }
    }
    for name in RETIRED_SECTIONS {
        if let Some(fence) = find_fence(&updated, name) {
            let pristine = fence
                .stamped
                .as_deref()
                .is_some_and(|stamped| stamped == stamp(&fence.body));
            if pristine {
                updated.replace_range(fence.start..fence.end, "");
                outcome.removed.push(name);
            } else {
                outcome.kept_edited.push(name);
            }
        }
    }
    if updated == current {
        (None, outcome)
    } else {
        (Some(updated), outcome)
    }
}

const OWNERSHIP_NOTE: &str = "<!-- Managed by loom init: text inside loom:section fences is refreshed\n     from the published templates while unedited; your own content is safe anywhere\n     outside the fences (and edited fences are always left alone). -->\n";

fn render_fresh(base: &str, chosen: &[(&'static str, String)]) -> String {
    let mut out = String::from(OWNERSHIP_NOTE);
    out.push_str(wrap_section("base", base).trim_start());
    for (name, content) in chosen {
        out.push_str(&wrap_section(name, content));
    }
    out
}

/// Machine-local list of projects init has scaffolded, so sync can walk
/// them. Absolute paths, deduped; entries whose AGENTS.md vanished prune
/// themselves on the next sync.
fn registry_path(home: &Path) -> std::path::PathBuf {
    home.join(".config").join("loom").join("projects.json")
}

fn read_registry(home: &Path) -> Vec<String> {
    fs::read_to_string(registry_path(home))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_registry(home: &Path, projects: &[String]) -> Result<()> {
    let path = registry_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(projects)?),
    )?;
    Ok(())
}

fn register_project(home: &Path, project: &Path) -> Result<()> {
    let entry = project.display().to_string();
    let mut projects = read_registry(home);
    if !projects.contains(&entry) {
        projects.push(entry);
        write_registry(home, &projects)?;
    }
    Ok(())
}

/// The outcome of a sync: one summary line plus one note per project.
pub struct SyncReport {
    pub ok: bool,
    pub summary: String,
    pub notes: Vec<String>,
}

impl SyncReport {
    fn failed(summary: impl Into<String>) -> Self {
        Self {
            ok: false,
            summary: summary.into(),
            notes: Vec::new(),
        }
    }
}

/// Refresh every registered project's AGENTS.md from the published
/// templates: pristine fences update, edited fences are kept and named,
/// nothing outside a fence is touched, and no new sections are added —
/// section choices stay with `init` in the project.
pub fn sync_projects(system: &dyn System) -> SyncReport {
    let Some(home) = system.home_dir() else {
        return SyncReport::failed("home directory is unavailable");
    };
    let projects = read_registry(&home);
    if projects.is_empty() {
        return SyncReport {
            ok: true,
            summary: "no projects registered; `loom init` adds one".into(),
            notes: Vec::new(),
        };
    }

    let staging = home.join(".cache").join("loom").join("sync-staging");
    let templates = skills::fetch_repo(system, &staging).and_then(|repo_root| {
        let base = fs::read_to_string(repo_root.join(BASE_TEMPLATE))
            .map_err(|error| format!("template missing: {BASE_TEMPLATE}: {error}"))?;
        let mut sections = Vec::new();
        for section in &SECTIONS {
            let content = fs::read_to_string(repo_root.join(section.template))
                .map_err(|error| format!("template missing: {}: {error}", section.template))?;
            sections.push((section.name, content));
        }
        Ok((base, sections))
    });
    let _ = fs::remove_dir_all(&staging);
    let (base, sections) = match templates {
        Ok(templates) => templates,
        Err(message) => return SyncReport::failed(message),
    };

    let mut notes = Vec::new();
    let mut ok = true;
    let mut surviving = Vec::new();
    for project in &projects {
        let agents_path = Path::new(project).join("AGENTS.md");
        let shown = tidy_path(Path::new(project), &home);
        let Ok(existing) = fs::read_to_string(&agents_path) else {
            notes.push(format!("{shown}  gone, unregistered"));
            continue;
        };
        surviving.push(project.clone());
        // Only the sections this project already carries.
        let chosen: Vec<(&'static str, String)> = sections
            .iter()
            .filter(|(name, _)| find_fence(&existing, name).is_some())
            .map(|(name, content)| (*name, content.clone()))
            .collect();
        let (updated, outcome) = render_agents(Some(&existing), &base, &chosen, false);
        if let Some(content) = updated {
            if let Err(error) = fs::write(&agents_path, content) {
                notes.push(format!("{shown}  write failed: {error}"));
                ok = false;
                continue;
            }
        }
        let mut changes = Vec::new();
        if !outcome.refreshed.is_empty() {
            changes.push(format!("refreshed {}", outcome.refreshed.join(", ")));
        }
        if !outcome.kept_edited.is_empty() {
            changes.push(format!("kept edited {}", outcome.kept_edited.join(", ")));
        }
        if !outcome.removed.is_empty() {
            changes.push(format!("removed retired {}", outcome.removed.join(", ")));
        }
        if changes.is_empty() {
            changes.push("current".into());
        }
        notes.push(format!("{shown}  {}", changes.join(" · ")));
    }
    if surviving.len() != projects.len() {
        let _ = write_registry(&home, &surviving);
    }
    SyncReport {
        ok,
        summary: format!("{} project AGENTS.md files", surviving.len()),
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandResult, CommandSpec};
    use std::sync::Mutex;

    const BASE: &str = "# AGENTS.md\n\n## Style\n- be kind\n";

    fn sections(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
        pairs
            .iter()
            .map(|(name, content)| (*name, content.to_string()))
            .collect()
    }

    #[test]
    fn fresh_file_is_fenced_base_plus_sections() {
        let chosen = sections(&[("python", "## Python\n- uv add")]);
        let (rendered, _) = render_agents(None, BASE, &chosen, false);
        let rendered = rendered.unwrap();
        assert!(rendered.contains("Managed by loom init"));
        assert!(rendered.contains("<!-- loom:section:base hash:"));
        assert!(rendered.contains("<!-- loom:section:python hash:"));
        assert!(rendered.contains("- uv add"));
        assert!(rendered.contains("<!-- /loom:section:python -->"));
    }

    #[test]
    fn sync_removes_only_pristine_retired_sections() {
        // Capability/seam: managed-fence retirement. This fails if sync keeps
        // obsolete context or deletes a user's edit. No expiry: permanent safety contract.
        let pristine = format!("prefix{}suffix", wrap_section("beads", "old template"));
        let (rendered, outcome) = render_agents(Some(&pristine), BASE, &[], false);
        let rendered = rendered.unwrap();
        assert!(!rendered.contains("loom:section:beads"));
        assert_eq!(outcome.removed, vec!["beads"]);

        let edited = pristine.replace("old template", "user edit");
        let (rendered, outcome) = render_agents(Some(&edited), BASE, &[], false);
        assert!(rendered.is_none());
        assert_eq!(outcome.kept_edited, vec!["beads"]);
    }

    #[test]
    fn rerun_appends_missing_sections_and_preserves_outside_edits() {
        let python = sections(&[("python", "## Python")]);
        let both = sections(&[("python", "## Python"), ("rust", "## Rust")]);
        let (first, _) = render_agents(None, BASE, &python, false);
        let edited = format!("{}\n## My own notes\n- hands off\n", first.unwrap());
        let (second, outcome) = render_agents(Some(&edited), BASE, &both, false);
        let second = second.unwrap();
        assert!(second.contains("## My own notes"));
        assert_eq!(outcome.appended, vec!["rust"]);
        assert_eq!(
            second.matches("<!-- /loom:section:python -->").count(),
            1,
            "existing sections are not duplicated"
        );
        // A third run with the same templates changes nothing.
        let (third, _) = render_agents(Some(&second), BASE, &both, false);
        assert!(third.is_none());
    }

    #[test]
    fn pristine_fences_refresh_when_the_template_moves() {
        let old = sections(&[("python", "## Python\n- old rule")]);
        let new = sections(&[("python", "## Python\n- new rule")]);
        let (first, _) = render_agents(None, BASE, &old, false);
        let first = first.unwrap();
        let (second, outcome) = render_agents(Some(&first), BASE, &new, false);
        let second = second.unwrap();
        assert!(second.contains("- new rule"));
        assert!(!second.contains("- old rule"));
        assert_eq!(outcome.refreshed, vec!["python"]);
    }

    #[test]
    fn edited_fences_are_kept_and_reported() {
        let old = sections(&[("python", "## Python\n- old rule")]);
        let new = sections(&[("python", "## Python\n- new rule")]);
        let (first, _) = render_agents(None, BASE, &old, false);
        let customized = first.unwrap().replace("- old rule", "- my custom rule");
        let (second, outcome) = render_agents(Some(&customized), BASE, &new, false);
        assert!(second.is_none(), "the hand-edited fence is left untouched");
        assert_eq!(outcome.kept_edited, vec!["python"]);
    }

    struct RecordingSystem {
        commands: Mutex<Vec<CommandSpec>>,
    }

    impl System for RecordingSystem {
        fn command_exists(&self, name: &str) -> bool {
            name == "codegraph"
        }

        fn refresh_path(&self) {}

        fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
            self.commands.lock().unwrap().push(command.clone());
            Ok(CommandResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn codegraph_setup_uses_current_install_and_init_commands() {
        let system = RecordingSystem {
            commands: Mutex::new(Vec::new()),
        };

        setup_codegraph(&system).unwrap();

        assert_eq!(
            *system.commands.lock().unwrap(),
            [
                CommandSpec::new("codegraph", ["install", "--yes", "--location", "local"]),
                CommandSpec::new("codegraph", ["init"]),
            ]
        );
    }

    #[test]
    fn noninteractive_init_requires_explicit_consent() {
        let mut options = InitOptions {
            python: None,
            rust: None,
            adhd: None,
            tracker: None,
            domain: None,
            editor: None,
            coding_standards: None,
            codegraph: None,
            yes: false,
            force: false,
        };
        assert!(!init_has_consent(&options, false));
        options.force = true;
        assert!(!init_has_consent(&options, false));
        options.force = false;
        assert!(init_has_consent(&options, true));
        options.yes = true;
        assert!(init_has_consent(&options, false));
        options.yes = false;
        options.python = Some(false);
        assert!(init_has_consent(&options, false));
    }

    #[test]
    fn explicit_tracker_selects_only_tracker_setup() {
        let options = InitOptions {
            python: None,
            rust: None,
            adhd: None,
            tracker: Some(Tracker::Beads),
            domain: None,
            editor: None,
            coding_standards: None,
            codegraph: None,
            yes: false,
            force: false,
        };

        let selected = choose_features(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            true,
            true,
            Editor::Cursor,
            &options,
            |prompt, _, _| panic!("explicit init should not prompt for {prompt}"),
        )
        .unwrap();

        assert_eq!(
            selected,
            InitFeatures {
                project_instructions: true,
                python: false,
                rust: false,
                adhd: false,
                tracker: Some(Tracker::Beads),
                domain: None,
                editor: None,
                coding_standards: false,
                codegraph: false,
            }
        );
    }

    #[test]
    fn init_questions_show_context_defaults_and_skip_irrelevant_sections() {
        // Capability/seam: interactive `loom init` questions. This fails if
        // defaults, context, or the project-instructions dependency drift.
        let options = InitOptions {
            python: None,
            rust: None,
            adhd: None,
            tracker: None,
            domain: None,
            editor: None,
            coding_standards: None,
            codegraph: None,
            yes: true,
            force: false,
        };
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut asked = Vec::new();
        let selected = choose_features(
            project,
            true,
            true,
            Editor::Cursor,
            &options,
            |prompt, help, default| {
                asked.push((prompt.to_string(), help.to_string(), default));
                Ok(default)
            },
        )
        .unwrap();

        assert_eq!(
            selected,
            InitFeatures {
                project_instructions: true,
                python: false,
                rust: true,
                adhd: false,
                tracker: Some(Tracker::Beads),
                domain: Some(DomainLayout::Single),
                editor: Some(Editor::Cursor),
                coding_standards: true,
                codegraph: true,
            }
        );
        let local_defaults = choose_features(
            project,
            false,
            false,
            Editor::None,
            &options,
            |_, _, default| Ok(default),
        )
        .unwrap();
        assert_eq!(local_defaults.tracker, Some(Tracker::Local));
        assert_eq!(local_defaults.editor, Some(Editor::None));

        assert_eq!(
            asked,
            [
                (
                    "Set up project agent instructions?".into(),
                    "Creates or updates AGENTS.md and CLAUDE.md for this project.".into(),
                    true,
                ),
                (
                    "Add Python instructions?".into(),
                    "Adds typing, uv, and Python quality commands to AGENTS.md.".into(),
                    false,
                ),
                (
                    "Add Rust instructions?".into(),
                    "Adds Rust conventions, Clippy, and test commands to AGENTS.md.".into(),
                    true,
                ),
                (
                    "Use ADHD-friendly agent output?".into(),
                    "Requests short, scannable progress updates for this project.".into(),
                    false,
                ),
                (
                    "Add coding standards?".into(),
                    "Creates CODING_STANDARDS.md with type, design, and simplicity checks.".into(),
                    true,
                ),
                (
                    "Set up CodeGraph?".into(),
                    "Wires installed agents and indexes this project.".into(),
                    true,
                ),
            ]
        );

        let mut asked = Vec::new();
        let selected = choose_features(
            project,
            false,
            true,
            Editor::None,
            &options,
            |prompt, _, _| {
                asked.push(prompt.to_string());
                Ok(prompt != "Set up project agent instructions?")
            },
        )
        .unwrap();

        assert_eq!(
            asked,
            ["Set up project agent instructions?", "Set up CodeGraph?"]
        );
        assert!(!selected.project_instructions);
        assert!(selected.codegraph);
    }

    #[test]
    fn gitignore_setup_is_idempotent_and_keeps_existing_content() {
        // Capability/seam: project-local agent-doc exclusion. This fails if
        // init duplicates the rule or damages an existing final line. No expiry.
        assert_eq!(
            render_gitignore("target\n.env"),
            Some("target\n.env\nai-docs/\n".into())
        );
        assert_eq!(render_gitignore("target\n/ai-docs/\n"), None);
        assert_eq!(render_gitignore("ai-docs\n"), None);
    }

    #[test]
    fn beads_setup_uses_noninteractive_init() {
        // Capability/seam: Beads repository initialization. This fails if
        // loom invokes the instructional quickstart command instead. No expiry.
        let system = RecordingSystem {
            commands: Mutex::new(Vec::new()),
        };
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));

        assert!(setup_beads(&system, project).unwrap());

        assert_eq!(
            *system.commands.lock().unwrap(),
            [CommandSpec::new("br", ["init", "--quiet"])]
        );
    }
}
