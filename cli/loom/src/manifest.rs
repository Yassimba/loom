//! Sync tool selections from the published manifest into mise.
//!
//! The repo's `manifest/loom.toml` is the MENU: every tool this setup
//! can provide, exact-pinned. What lands on a machine is the SELECTION —
//! `~/.config/mise/conf.d/loom.toml` holds the core block plus the
//! tools the user chose. Syncing rebuilds the selection from the fresh
//! manifest (so pins update) filtered to the previously selected keys plus
//! any newly requested ones; mise merges the file without touching the
//! user's own config.toml. Tools therefore change version only when a new
//! manifest lands on main, and change *set* only when the user asks.

use crate::{skills, CommandSpec, System};
use std::fs;
use std::path::PathBuf;

const MANIFEST_IN_REPO: &str = "manifest/loom.toml";
const CORE_BEGIN: &str = "# core:begin";
const CORE_END: &str = "# core:end";

/// The manifest key that provides Pi; a selected Pi package pulls it in.
pub const PI_TOOL_KEY: &str = "npm:@earendil-works/pi-coding-agent";

pub fn conf_d_target(home: &std::path::Path) -> PathBuf {
    home.join(".config")
        .join("mise")
        .join("conf.d")
        .join("loom.toml")
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

/// Manifest keys that moved: a selection written under the old key follows
/// the tool to its new key instead of being dropped as "no longer published".
const RENAMED_KEYS: &[(&str, &str)] = &[
    // loom-teams left the shared `github:` backend it collided with loom on.
    ("github:Yassimba/loom[exe=loom-teams]", "ubi:Yassimba/loom"),
    // Pi moved npm scopes; extensions built for the new scope cannot load
    // under the old package.
    (
        "npm:@mariozechner/pi-coding-agent",
        "npm:@earendil-works/pi-coding-agent",
    ),
];

fn current_key(key: &str) -> &str {
    RENAMED_KEYS
        .iter()
        .find(|(old, _)| *old == key)
        .map_or(key, |(_, new)| new)
}

/// The keys already selected on this machine (empty when nothing is synced
/// yet). Core keys are implicit and excluded; renamed keys come back current.
pub fn selected_keys(home: &std::path::Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(conf_d_target(home)) else {
        return Vec::new();
    };
    let core = core_section(&content).unwrap_or_default();
    let mut keys = content
        .lines()
        .filter(|line| !core.contains(*line))
        .filter_map(line_key)
        .map(|key| current_key(key).to_string())
        .collect::<Vec<_>>();
    keys.dedup();
    keys
}

pub fn selection_contains(home: &std::path::Path, key: &str) -> bool {
    fs::read_to_string(conf_d_target(home)).is_ok_and(|content| {
        content
            .lines()
            .filter_map(line_key)
            .any(|selected| current_key(selected) == current_key(key))
    })
}

fn core_section(manifest: &str) -> Option<String> {
    let begin = manifest.find(CORE_BEGIN)?;
    let end = manifest[begin..].find(CORE_END)? + begin + CORE_END.len();
    Some(manifest[begin..end].to_string())
}

/// Build the selection file: core block + the manifest's lines for the
/// selected keys, all carrying the manifest's current pins.
fn render_selection(manifest: &str, current: &str, keys: &[String]) -> Result<String, String> {
    let core = core_section(manifest)
        .ok_or_else(|| format!("{MANIFEST_IN_REPO} is missing its core:begin/core:end block"))?;
    let mut out = String::from(
        "# Managed by Loom: the selected tools from the published manifest.\n\
         # Change the selection with `loom setup`; refresh pins with\n\
         # `loom update`. Personal tools belong in your own config.toml.\n\n\
         [tools]\n",
    );
    out.push_str(&core);
    out.push('\n');
    let mut kept = Vec::new();
    for key in keys {
        match manifest.lines().find(|line| line_key(line) == Some(key)) {
            Some(line) => {
                out.push_str(line);
                out.push('\n');
            }
            None => {
                // A key the published manifest no longer carries keeps its
                // current line: this binary may simply predate a rename
                // (a newer loom maps it), and dropping it would uninstall
                // the tool on the next prune.
                if let Some(line) = current.lines().find(|line| line_key(line) == Some(key)) {
                    out.push_str(line);
                    out.push('\n');
                    kept.push(key.as_str());
                }
            }
        }
    }
    if !kept.is_empty() {
        eprintln!(
            "  ! not in the published manifest, kept as pinned: {} — a newer loom may know where it moved",
            kept.join(", ")
        );
    }
    Ok(out)
}

/// Fetch the manifest, rebuild the selection (previous keys + `extra`),
/// write it to conf.d, and `mise install` the result.
pub fn sync_selected(system: &dyn System, extra: &[String]) -> Result<PathBuf, String> {
    sync_selected_controlled(system, extra, &std::sync::atomic::AtomicBool::new(false))
}

pub fn sync_selected_controlled(
    system: &dyn System,
    extra: &[String],
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<PathBuf, String> {
    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let staging = home.join(".cache").join("loom").join("manifest-staging");
    let result = skills::fetch_repo_controlled(system, &staging, cancelled).and_then(|repo_root| {
        let source = repo_root.join(MANIFEST_IN_REPO);
        let manifest = fs::read_to_string(&source)
            .map_err(|error| format!("downloaded repo has no {MANIFEST_IN_REPO}: {error}"))?;
        let mut keys = selected_keys(&home);
        for key in extra {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        let target = conf_d_target(&home);
        let current = fs::read_to_string(&target).unwrap_or_default();
        let content = render_selection(&manifest, &current, &keys)?;
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
    mise_install(system, cancelled).map_err(|error| format!("mise install failed: {error}"))?;
    // Moved pins leave their old versions in the store; prune is best-effort
    // cleanup and only removes versions no config references anymore.
    let _ = system.run_controlled(
        &CommandSpec::new("mise", ["prune", "--yes"]),
        crate::system::MANAGER_COMMAND_TIMEOUT,
        cancelled,
    );
    system.refresh_path();
    Ok(target)
}

/// Remove keys from Loom's selection without touching the user's mise files.
pub fn remove_selected(
    system: &dyn System,
    removed: &[String],
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let target = conf_d_target(&home);
    let current = fs::read_to_string(&target)
        .map_err(|error| format!("could not read {}: {error}", target.display()))?;
    let keys = selected_keys(&home)
        .into_iter()
        .filter(|key| !removed.contains(key))
        .collect::<Vec<_>>();
    let content = render_selection(&current, &current, &keys)?;
    fs::write(&target, content)
        .map_err(|error| format!("could not write {}: {error}", target.display()))?;
    let result = system
        .run_controlled(
            &CommandSpec::new("mise", ["prune", "--yes"]),
            crate::system::MANAGER_COMMAND_TIMEOUT,
            cancelled,
        )
        .map_err(|error| error.to_string())?;
    if !result.success {
        return Err(crate::install::command_failure_message(&result));
    }
    system.refresh_path();
    Ok(())
}

/// Refresh pins for the existing selection without changing it.
pub fn sync_and_install(system: &dyn System) -> Result<PathBuf, String> {
    sync_selected(system, &[])
}

fn mise_install(
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    let spec = CommandSpec::new("mise", ["install", "--yes"]);
    match system.run_controlled(&spec, crate::system::MANAGER_COMMAND_TIMEOUT, cancelled) {
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
\"github:Yassimba/loom[exe=loom]\" = { version = \"loom-v0.6.2\" }
# core:end

gh = \"2.97.0\"
\"npm:@earendil-works/pi-coding-agent\" = \"0.84.4\"
\"github:zdyxry/tokui\" = \"0.12.0\"
";

    #[test]
    fn selection_carries_core_plus_chosen_lines() {
        let rendered =
            render_selection(MANIFEST, "", &["gh".into(), "github:zdyxry/tokui".into()]).unwrap();
        assert!(rendered.contains("node = \"24.19.0\""));
        assert!(rendered.contains("gh = \"2.97.0\""));
        assert!(rendered.contains("\"github:zdyxry/tokui\" = \"0.12.0\""));
        assert!(!rendered.contains("pi-coding-agent"));
    }

    #[test]
    fn renamed_keys_follow_the_tool() {
        assert_eq!(
            current_key("github:Yassimba/loom[exe=loom-teams]"),
            "ubi:Yassimba/loom"
        );
        assert_eq!(current_key("gh"), "gh");
    }

    #[test]
    fn vanished_tools_keep_their_current_pin() {
        let current = "[tools]\ngone = \"1.0.0\"\n";
        let rendered = render_selection(MANIFEST, current, &["gone".into()]).unwrap();
        assert!(rendered.contains("gone = \"1.0.0\""));
        let rendered = render_selection(MANIFEST, "", &["gone".into()]).unwrap();
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
