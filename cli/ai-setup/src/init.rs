//! `ai-setup init` — scaffold a project's AGENTS.md and CLAUDE.md from the
//! published templates (manifest/init/ in the repo), with sections chosen by
//! what the project is and what the machine has.
//!
//! Templates are configuration: editing them in the repo and merging to main
//! changes what every future init writes, like the tool manifest. Sections
//! land wrapped in `<!-- ai-setup:section:<name> -->` markers so a re-run
//! appends missing sections and never touches anything else.

use crate::{skills, Catalog, System};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub struct InitOptions {
    pub python: Option<bool>,
    pub rust: Option<bool>,
    pub beads: Option<bool>,
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
        name: "beads",
        template: "manifest/init/sections/beads.md",
    },
];

const BASE_TEMPLATE: &str = "manifest/init/AGENTS.base.md";

fn marker_open(name: &str) -> String {
    format!("<!-- ai-setup:section:{name}")
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
        "\n<!-- ai-setup:section:{name} hash:{} -->\n\n{}\n<!-- /ai-setup:section:{name} -->\n",
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
    let close = format!("<!-- /ai-setup:section:{name} -->");
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

/// Detection defaults: evidence in the project, or (for beads) the machine.
fn detect(project: &Path, system: &dyn System) -> (bool, bool, bool) {
    let python = ["pyproject.toml", "setup.py", "requirements.txt"]
        .iter()
        .any(|file| project.join(file).exists());
    let rust = project.join("Cargo.toml").exists();
    let beads = project.join(".beads").is_dir()
        || (system.command_exists("br") && system.command_exists("bv"));
    (python, rust, beads)
}

fn confirm(prompt: &str, default: bool, assume_yes: bool) -> Result<bool> {
    if assume_yes || !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(default);
    }
    Ok(inquire::Confirm::new(prompt)
        .with_default(default)
        .prompt()?)
}

/// The beads pair from the catalog (labels beads/beads-viewer): both or
/// nothing — the tracker and its viewer are designed to travel together.
fn beads_tool_keys(catalog: &Catalog) -> Vec<String> {
    catalog
        .resources
        .iter()
        .filter(|resource| {
            resource.kind == crate::ResourceKind::Tool
                && (resource.label == "beads" || resource.label == "beads-viewer")
        })
        .map(|resource| resource.install_target.clone())
        .collect()
}

pub fn run_init(system: &dyn System, catalog: &Catalog, options: &InitOptions) -> Result<bool> {
    let project = std::env::current_dir().context("no current directory")?;
    let (python_default, rust_default, beads_default) = detect(&project, system);

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
    let beads = match options.beads {
        Some(explicit) => explicit,
        None => confirm(
            "Include the Beads issue-triage section? (br + bv)",
            beads_default,
            options.yes,
        )?,
    };

    // Beads works as a pair: the section assumes both the tracker (br) and
    // the viewer (bv). Offer to install both through the tool selection.
    if beads && !(system.command_exists("br") && system.command_exists("bv")) {
        println!("The Beads section needs br and bv — they are designed to be installed together.");
        if crate::manifest::mise_available(system)
            && confirm("Install beads and beads-viewer now?", true, options.yes)?
        {
            let keys = beads_tool_keys(catalog);
            match crate::manifest::sync_selected(system, &keys) {
                Ok(_) => println!("  ✓ beads + beads-viewer added to the tool selection"),
                Err(message) => eprintln!("  ! could not install the beads pair: {message}"),
            }
        } else {
            println!("  → later: ai-setup add --tool beads --tool beads-viewer");
        }
    }

    // Templates come from the published repo, so init output is publish-gated.
    let home = system.home_dir().context("home directory is unavailable")?;
    let staging = home.join(".cache").join("ai-setup").join("init-staging");
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
            "beads" => beads,
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

    Ok(true)
}

/// What happened to each managed fence during a render.
#[derive(Debug, Default)]
pub(crate) struct RenderOutcome {
    pub refreshed: Vec<&'static str>,
    pub appended: Vec<&'static str>,
    /// Edited inside the fence: preserved, listed so the user knows the
    /// published template moved past their copy.
    pub kept_edited: Vec<&'static str>,
}

/// Build the new AGENTS.md content (None when nothing needs writing) plus a
/// report of what moved. The contract: text inside ai-setup:section fences
/// belongs to ai-setup and refreshes when templates change — but only while
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
    if updated == current {
        (None, outcome)
    } else {
        (Some(updated), outcome)
    }
}

const OWNERSHIP_NOTE: &str = "<!-- Managed by ai-setup init: text inside ai-setup:section fences is refreshed\n     from the published templates while unedited; your own content is safe anywhere\n     outside the fences (and edited fences are always left alone). -->\n";

fn render_fresh(base: &str, chosen: &[(&'static str, String)]) -> String {
    let mut out = String::from(OWNERSHIP_NOTE);
    out.push_str(wrap_section("base", base).trim_start());
    for (name, content) in chosen {
        out.push_str(&wrap_section(name, content));
    }
    out
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
        assert!(rendered.contains("Managed by ai-setup init"));
        assert!(rendered.contains("<!-- ai-setup:section:base hash:"));
        assert!(rendered.contains("<!-- ai-setup:section:python hash:"));
        assert!(rendered.contains("- uv add"));
        assert!(rendered.contains("<!-- /ai-setup:section:python -->"));
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
            second.matches("<!-- /ai-setup:section:python -->").count(),
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
