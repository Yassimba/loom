//! Sync tool selections from the published manifest into mise.
//!
//! The repo's `manifest/ai-setup.toml` is the MENU: every tool this setup
//! can provide, exact-pinned. What lands on a machine is the SELECTION —
//! `~/.config/mise/conf.d/ai-setup.toml` holds the core block plus the
//! tools the user chose. Syncing rebuilds the selection from the fresh
//! manifest (so pins update) filtered to the previously selected keys plus
//! any newly requested ones; mise merges the file without touching the
//! user's own config.toml. Tools therefore change version only when a new
//! manifest lands on main, and change *set* only when the user asks.

use crate::{skills, CommandSpec, System};
use std::fs;
use std::path::PathBuf;

const MANIFEST_IN_REPO: &str = "manifest/ai-setup.toml";
const CORE_BEGIN: &str = "# core:begin";
const CORE_END: &str = "# core:end";

/// The manifest key that provides Pi; a selected Pi package pulls it in.
pub const PI_TOOL_KEY: &str = "npm:@mariozechner/pi-coding-agent";

pub fn conf_d_target(home: &std::path::Path) -> PathBuf {
    home.join(".config")
        .join("mise")
        .join("conf.d")
        .join("ai-setup.toml")
}

pub fn mise_available(system: &dyn System) -> bool {
    system.command_exists("mise")
}

/// The tool key a manifest/selection line defines, if any:
/// `"quoted:key" = ...` or `bare-key = ...`.
fn line_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with('[') {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        let (key, rest) = rest.split_once('"')?;
        return rest.trim_start().starts_with('=').then_some(key);
    }
    let (key, _) = trimmed.split_once('=')?;
    let key = key.trim();
    (!key.is_empty() && !key.contains(char::is_whitespace)).then_some(key)
}

/// The keys already selected on this machine (empty when nothing is synced
/// yet). Core keys are implicit and excluded.
pub fn selected_keys(home: &std::path::Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(conf_d_target(home)) else {
        return Vec::new();
    };
    let core = core_section(&content).unwrap_or_default();
    content
        .lines()
        .filter(|line| !core.contains(*line))
        .filter_map(line_key)
        .map(str::to_string)
        .collect()
}

fn core_section(manifest: &str) -> Option<String> {
    let begin = manifest.find(CORE_BEGIN)?;
    let end = manifest[begin..].find(CORE_END)? + begin + CORE_END.len();
    Some(manifest[begin..end].to_string())
}

/// Build the selection file: core block + the manifest's lines for the
/// selected keys, all carrying the manifest's current pins.
fn render_selection(manifest: &str, keys: &[String]) -> Result<String, String> {
    let core = core_section(manifest)
        .ok_or_else(|| format!("{MANIFEST_IN_REPO} is missing its core:begin/core:end block"))?;
    let mut out = String::from(
        "# Managed by ai-setup: the selected tools from the published manifest.\n\
         # Change the selection with `ai-setup setup`; refresh pins with\n\
         # `ai-setup update`. Personal tools belong in your own config.toml.\n\n\
         [tools]\n",
    );
    out.push_str(&core);
    out.push('\n');
    let mut missing = Vec::new();
    for key in keys {
        match manifest.lines().find(|line| line_key(line) == Some(key)) {
            Some(line) => {
                out.push_str(line);
                out.push('\n');
            }
            None => missing.push(key.as_str()),
        }
    }
    if !missing.is_empty() {
        // A tool that left the published manifest silently leaves the
        // selection too — its pin has no source of truth anymore.
        eprintln!(
            "  ! tools no longer in the published manifest, dropped: {}",
            missing.join(", ")
        );
    }
    Ok(out)
}

/// Fetch the manifest, rebuild the selection (previous keys + `extra`),
/// write it to conf.d, and `mise install` the result.
pub fn sync_selected(system: &dyn System, extra: &[String]) -> Result<PathBuf, String> {
    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let staging = home
        .join(".cache")
        .join("ai-setup")
        .join("manifest-staging");
    let result = skills::fetch_repo(system, &staging).and_then(|repo_root| {
        let source = repo_root.join(MANIFEST_IN_REPO);
        let manifest = fs::read_to_string(&source)
            .map_err(|error| format!("downloaded repo has no {MANIFEST_IN_REPO}: {error}"))?;
        let mut keys = selected_keys(&home);
        for key in extra {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        let content = render_selection(&manifest, &keys)?;
        let target = conf_d_target(&home);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        fs::write(&target, content)
            .map_err(|error| format!("could not write {}: {error}", target.display()))?;
        Ok(target)
    });
    let _ = fs::remove_dir_all(&staging);
    let target = result?;
    mise_install(system).map_err(|error| format!("mise install failed: {error}"))?;
    system.refresh_path();
    Ok(target)
}

/// Refresh pins for the existing selection without changing it.
pub fn sync_and_install(system: &dyn System) -> Result<PathBuf, String> {
    sync_selected(system, &[])
}

fn mise_install(system: &dyn System) -> Result<(), String> {
    let spec = CommandSpec::new("mise", ["install", "--yes"]);
    match system.run(&spec) {
        Ok(result) if result.success => Ok(()),
        Ok(result) => Err(crate::install::command_failure_message(&result)),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "\
[tools]
# core:begin
node = \"24.19.0\"
\"github:Yassimba/ai-setup[exe=ai-setup]\" = { version = \"ai-setup-v0.6.2\" }
# core:end

gh = \"2.97.0\"
\"npm:@mariozechner/pi-coding-agent\" = \"0.73.1\"
\"github:zdyxry/tokui\" = \"0.12.0\"
";

    #[test]
    fn selection_carries_core_plus_chosen_lines() {
        let rendered =
            render_selection(MANIFEST, &["gh".into(), "github:zdyxry/tokui".into()]).unwrap();
        assert!(rendered.contains("node = \"24.19.0\""));
        assert!(rendered.contains("gh = \"2.97.0\""));
        assert!(rendered.contains("\"github:zdyxry/tokui\" = \"0.12.0\""));
        assert!(!rendered.contains("pi-coding-agent"));
    }

    #[test]
    fn vanished_tools_drop_from_the_selection() {
        let rendered = render_selection(MANIFEST, &["gone".into()]).unwrap();
        assert!(!rendered.contains("gone"));
    }

    #[test]
    fn line_keys_parse_quoted_and_bare() {
        assert_eq!(line_key("gh = \"2.97.0\""), Some("gh"));
        assert_eq!(
            line_key("\"npm:agent-browser\" = \"0.33.2\""),
            Some("npm:agent-browser")
        );
        assert_eq!(line_key("# comment"), None);
        assert_eq!(line_key("[tools]"), None);
    }
}
