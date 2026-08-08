//! `loom init` — scaffold a project's AGENTS.md and CLAUDE.md from the
//! published templates (manifest/init/ in the repo), with sections chosen by
//! what the project is and what the machine has.
//!
//! Templates are configuration: editing them in the repo and merging to main
//! changes what every future init writes, like the tool manifest. Sections
//! land wrapped in `<!-- loom:section:<name> -->` markers so a re-run
//! appends missing sections and never touches anything else.

use crate::{skills, System};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub struct InitOptions {
    pub python: Option<bool>,
    pub rust: Option<bool>,
    pub yes: bool,
    pub force: bool,
}

struct Section {
    name: &'static str,
    template: &'static str,
}

const SECTIONS: [Section; 2] = [
    Section {
        name: "python",
        template: "manifest/init/sections/python.md",
    },
    Section {
        name: "rust",
        template: "manifest/init/sections/rust.md",
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

fn confirm(prompt: &str, default: bool, assume_yes: bool) -> Result<bool> {
    if assume_yes || !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(default);
    }
    Ok(inquire::Confirm::new(prompt)
        .with_default(default)
        .prompt()?)
}

pub fn run_init(system: &dyn System, options: &InitOptions) -> Result<bool> {
    let project = std::env::current_dir().context("no current directory")?;
    let (python_default, rust_default) = detect(&project);

    let python = match options.python {
        Some(explicit) => explicit,
        None => confirm(
            "Include the Python section? (project is / will be Python)",
            python_default,
            options.yes,
        )?,
    };
    let rust = match options.rust {
        Some(explicit) => explicit,
        None => confirm(
            "Include the Rust section? (project is / will be Rust)",
            rust_default,
            options.yes,
        )?,
    };
    // Templates come from the published repo, so init output is publish-gated.
    let home = system.home_dir().context("home directory is unavailable")?;
    let staging = home.join(".cache").join("loom").join("init-staging");
    let repo_root = skills::fetch_repo(system, &staging)
        .map_err(anyhow::Error::msg)
        .context("could not fetch the templates")?;
    let base = fs::read_to_string(repo_root.join(BASE_TEMPLATE))
        .with_context(|| format!("template missing: {BASE_TEMPLATE}"))?;
    let mut chosen: Vec<(&'static str, String)> = Vec::new();
    for section in &SECTIONS {
        let wanted = match section.name {
            "python" => python,
            "rust" => rust,
            _ => false,
        };
        if wanted {
            let content = fs::read_to_string(repo_root.join(section.template))
                .with_context(|| format!("template missing: {}", section.template))?;
            chosen.push((section.name, content));
        }
    }
    let _ = fs::remove_dir_all(&staging);

    let agents_path = project.join("AGENTS.md");
    let existing = fs::read_to_string(&agents_path).ok();
    let (agents, outcome) = render_agents(existing.as_deref(), &base, &chosen, options.force);
    match agents {
        Some(content) => {
            fs::write(&agents_path, content)
                .with_context(|| format!("could not write {}", agents_path.display()))?;
            if existing.is_none() {
                println!("  ✓ AGENTS.md created");
            } else {
                if !outcome.appended.is_empty() {
                    println!("  ✓ AGENTS.md: appended {}", outcome.appended.join(", "));
                }
                if !outcome.refreshed.is_empty() {
                    println!(
                        "  ✓ AGENTS.md: refreshed {} from the published templates",
                        outcome.refreshed.join(", ")
                    );
                }
            }
        }
        None => println!("  ✓ AGENTS.md is current"),
    }
    for name in &outcome.kept_edited {
        println!(
            "  ! AGENTS.md: section {name} was edited inside its fence — kept your version \
             (the published template has moved; --force rewrites everything)"
        );
    }
    if !outcome.removed.is_empty() {
        println!(
            "  ✓ AGENTS.md: removed retired {} section",
            outcome.removed.join(", ")
        );
    }

    let claude_path = project.join("CLAUDE.md");
    match fs::read_to_string(&claude_path) {
        Err(_) => {
            fs::write(&claude_path, "@AGENTS.md\n")
                .with_context(|| format!("could not write {}", claude_path.display()))?;
            println!("  ✓ CLAUDE.md (points at AGENTS.md)");
        }
        Ok(content) if content.trim() == "@AGENTS.md" => {
            println!("  ✓ CLAUDE.md already points at AGENTS.md");
        }
        Ok(_) if options.force => {
            fs::write(&claude_path, "@AGENTS.md\n")?;
            println!("  ✓ CLAUDE.md rewritten to point at AGENTS.md (--force)");
        }
        Ok(_) => {
            println!(
                "  ! CLAUDE.md exists with its own content — leaving it; consider moving it into AGENTS.md and keeping just `@AGENTS.md` (or rerun with --force)"
            );
        }
    }

    if let Err(error) = register_project(&home, &project) {
        eprintln!("  ! could not register the project for sync: {error}");
    } else {
        println!("  ✓ registered for `loom sync`");
    }

    Ok(true)
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

/// Refresh every registered project's AGENTS.md from the published
/// templates: pristine fences update, edited fences are kept and named,
/// nothing outside a fence is touched, and no new sections are added —
/// section choices stay with `init` in the project. Returns (ok, report).
pub fn sync_projects(system: &dyn System) -> (bool, String) {
    let Some(home) = system.home_dir() else {
        return (
            false,
            "  ! Project AGENTS.md: home directory is unavailable".into(),
        );
    };
    let projects = read_registry(&home);
    if projects.is_empty() {
        return (true, "  ✓ Project AGENTS.md (none registered)".into());
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
        Err(message) => return (false, format!("  ! Project AGENTS.md: {message}")),
    };

    let mut report = String::from("  ✓ Project AGENTS.md files");
    let mut ok = true;
    let mut surviving = Vec::new();
    for project in &projects {
        let agents_path = Path::new(project).join("AGENTS.md");
        let Ok(existing) = fs::read_to_string(&agents_path) else {
            report.push_str(&format!("\n      {project}: gone, unregistered"));
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
                report.push_str(&format!("\n      {project}: write failed: {error}"));
                ok = false;
                continue;
            }
        }
        let mut notes = Vec::new();
        if !outcome.refreshed.is_empty() {
            notes.push(format!("refreshed {}", outcome.refreshed.join(", ")));
        }
        if !outcome.kept_edited.is_empty() {
            notes.push(format!("kept edited {}", outcome.kept_edited.join(", ")));
        }
        if !outcome.removed.is_empty() {
            notes.push(format!("removed retired {}", outcome.removed.join(", ")));
        }
        if notes.is_empty() {
            notes.push("current".into());
        }
        report.push_str(&format!("\n      {project}: {}", notes.join(" · ")));
    }
    if surviving.len() != projects.len() {
        let _ = write_registry(&home, &surviving);
    }
    (ok, report)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
