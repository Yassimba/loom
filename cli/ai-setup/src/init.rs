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
    format!("<!-- ai-setup:section:{name} -->")
}

fn wrap_section(name: &str, content: &str) -> String {
    format!(
        "\n{}\n\n{}\n<!-- /ai-setup:section:{name} -->\n",
        marker_open(name),
        content.trim_end()
    )
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
    let agents = render_agents(existing.as_deref(), &base, &chosen, options.force);
    match agents {
        Some(content) => {
            fs::write(&agents_path, content)
                .with_context(|| format!("could not write {}", agents_path.display()))?;
            println!(
                "  ✓ AGENTS.md ({})",
                if existing.is_some() {
                    "sections appended"
                } else {
                    "created"
                }
            );
        }
        None => println!("  ✓ AGENTS.md already has every selected section"),
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

/// Build the new AGENTS.md content, or None when nothing needs writing.
/// Fresh file: base + every chosen section. Existing file: append only the
/// chosen sections whose markers are absent; everything else stays byte-same.
fn render_agents(
    existing: Option<&str>,
    base: &str,
    chosen: &[(&'static str, String)],
    force: bool,
) -> Option<String> {
    match existing {
        None => Some(render_fresh(base, chosen)),
        Some(_) if force => Some(render_fresh(base, chosen)),
        Some(current) => {
            let missing: Vec<&(&'static str, String)> = chosen
                .iter()
                .filter(|(name, _)| !current.contains(&marker_open(name)))
                .collect();
            if missing.is_empty() {
                return None;
            }
            let mut updated = current.trim_end().to_string();
            updated.push('\n');
            for (name, content) in missing {
                updated.push_str(&wrap_section(name, content));
            }
            Some(updated)
        }
    }
}

fn render_fresh(base: &str, chosen: &[(&'static str, String)]) -> String {
    let mut out = base.trim_end().to_string();
    out.push('\n');
    for (name, content) in chosen {
        out.push_str(&wrap_section(name, content));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "# AGENTS.md\n\n## Style\n- be kind\n";

    #[test]
    fn fresh_file_is_base_plus_marked_sections() {
        let sections = vec![("python", "## Python\n- uv add".to_string())];
        let rendered = render_agents(None, BASE, &sections, false).unwrap();
        assert!(rendered.starts_with("# AGENTS.md"));
        assert!(rendered.contains("<!-- ai-setup:section:python -->"));
        assert!(rendered.contains("- uv add"));
        assert!(rendered.contains("<!-- /ai-setup:section:python -->"));
    }

    #[test]
    fn rerun_appends_only_missing_sections_and_preserves_edits() {
        let sections = vec![
            ("python", "## Python".to_string()),
            ("rust", "## Rust".to_string()),
        ];
        let first = render_agents(None, BASE, &sections[..1], false).unwrap();
        let edited = format!("{first}\n## My own notes\n- hands off\n");
        let second = render_agents(Some(&edited), BASE, &sections, false).unwrap();
        assert!(second.contains("## My own notes"));
        assert!(second.contains("<!-- ai-setup:section:rust -->"));
        assert_eq!(
            second.matches("<!-- ai-setup:section:python -->").count(),
            1,
            "existing sections are not duplicated"
        );
        // A third run with the same choices changes nothing.
        assert!(render_agents(Some(&second), BASE, &sections, false).is_none());
    }
}
