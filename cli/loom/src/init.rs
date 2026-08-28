//! `loom init` — scaffold a project's AGENTS.md and CLAUDE.md from the
//! published templates (manifest/init/ in the repo), with sections chosen by
//! what the project is and what the machine has.
//!
//! Templates are configuration: editing them in the repo and merging to main
//! changes what every future init writes, like the tool manifest. Sections
//! land wrapped in `<!-- loom:section:<name> -->` markers so a re-run
//! appends missing sections and never touches anything else.

use crate::ui::{tidy_path, Mark, Out};
use crate::{skills, CommandSpec, System};
use anyhow::{Context, Result};
use inquire::Select;
use std::fs;
use std::path::Path;

pub struct InitOptions {
    pub python: Option<bool>,
    pub rust: Option<bool>,
    pub adhd: Option<bool>,
    pub codegraph: Option<bool>,
    pub yes: bool,
    pub force: bool,
}

struct Section {
    name: &'static str,
    template: &'static str,
}

const SECTIONS: [Section; 3] = [
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
];

const RETIRED_SECTIONS: [&str; 1] = ["beads"];

const BASE_TEMPLATE: &str = "manifest/init/AGENTS.base.md";

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

#[derive(Debug, Eq, PartialEq)]
struct InitFeatures {
    project_instructions: bool,
    python: bool,
    rust: bool,
    adhd: bool,
    codegraph: bool,
}

fn choose_features(
    project: &Path,
    codegraph_installed: bool,
    options: &InitOptions,
    mut ask: impl FnMut(&'static str, &'static str, bool) -> Result<bool>,
) -> Result<InitFeatures> {
    let (python_default, rust_default) = detect(project);
    let project_instructions = ask(
        "Set up project agent instructions?",
        "Creates or updates AGENTS.md and CLAUDE.md for this project.",
        true,
    )?;
    let (python, rust, adhd) = if project_instructions {
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
        )
    } else {
        (false, false, false)
    };
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
        codegraph,
    })
}

fn setup_codegraph(system: &dyn System) -> Result<()> {
    let result = system
        .run(&CommandSpec::new(
            "codegraph",
            ["install", "--yes", "--init"],
        ))
        .context("could not start CodeGraph setup")?;
    if !result.success {
        anyhow::bail!(
            "CodeGraph setup failed: {}",
            crate::install::command_failure_message(&result)
        );
    }
    Ok(())
}

pub fn run_init(system: &dyn System, options: &InitOptions) -> Result<bool> {
    let project = std::env::current_dir().context("no current directory")?;
    let features = choose_features(
        &project,
        system.command_exists("codegraph"),
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
    let mut chosen: Vec<(&'static str, String)> = Vec::new();
    for section in &SECTIONS {
        let wanted = match section.name {
            "python" => features.python,
            "rust" => features.rust,
            "i-have-adhd" => features.adhd,
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
    fn codegraph_setup_delegates_to_upstreams_one_shot_command() {
        let system = RecordingSystem {
            commands: Mutex::new(Vec::new()),
        };

        setup_codegraph(&system).unwrap();

        assert_eq!(
            *system.commands.lock().unwrap(),
            [CommandSpec::new(
                "codegraph",
                ["install", "--yes", "--init"]
            )]
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
            codegraph: None,
            yes: false,
            force: false,
        };
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut asked = Vec::new();
        let selected = choose_features(project, true, &options, |prompt, help, default| {
            asked.push((prompt.to_string(), help.to_string(), default));
            Ok(default)
        })
        .unwrap();

        assert_eq!(
            selected,
            InitFeatures {
                project_instructions: true,
                python: false,
                rust: true,
                adhd: false,
                codegraph: true,
            }
        );
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
                    "Set up CodeGraph?".into(),
                    "Wires installed agents and indexes this project.".into(),
                    true,
                ),
            ]
        );

        let mut asked = Vec::new();
        let selected = choose_features(project, true, &options, |prompt, _, _| {
            asked.push(prompt.to_string());
            Ok(prompt != "Set up project agent instructions?")
        })
        .unwrap();

        assert_eq!(
            asked,
            ["Set up project agent instructions?", "Set up CodeGraph?"]
        );
        assert!(!selected.project_instructions);
        assert!(selected.codegraph);
    }
}
