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

use crate::{skills::Repository, CommandSpec, System};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

const MANIFEST_IN_REPO: &str = "manifest/loom.toml";
const BUNDLED_MANIFEST: &str = include_str!("../../../manifest/loom.toml");
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
    // The clean fork carries no patches, so install upstream's release.
    (
        "github:Yassimba/plannotator",
        "github:backnotprop/plannotator",
    ),
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
    let target = conf_d_target(home);
    let _ = crate::fs_tx::recover(&target);
    let Ok(content) = fs::read_to_string(target) else {
        return Vec::new();
    };
    let core = core_section(&content).unwrap_or_default();
    let mut seen = HashSet::new();
    let keys = content
        .lines()
        .filter(|line| !core.contains(*line))
        .filter_map(line_key)
        .map(|key| current_key(key).to_string())
        .filter(|key| seen.insert(key.clone()))
        .collect();
    keys
}

pub fn selection_contains(home: &std::path::Path, key: &str) -> bool {
    let target = conf_d_target(home);
    let _ = crate::fs_tx::recover(&target);
    fs::read_to_string(target).is_ok_and(|content| {
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
                // the tool on the next prune. A newly requested key can come
                // from this binary's reviewed manifest when main is briefly
                // behind the release or a local build.
                if let Some(line) = current.lines().find(|line| line_key(line) == Some(key)) {
                    out.push_str(line);
                    out.push('\n');
                    kept.push(key.as_str());
                } else if let Some(line) = BUNDLED_MANIFEST
                    .lines()
                    .find(|line| line_key(line) == Some(key))
                {
                    out.push_str(line);
                    out.push('\n');
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

// Wiki setup and update sync keys directly, bypassing install-plan expansion.
fn with_tool_companions(mut keys: Vec<String>) -> Result<Vec<String>, String> {
    let catalog = crate::Catalog::embedded().map_err(|error| error.to_string())?;
    for tool in &catalog.resources {
        if keys.contains(&tool.install_target) {
            for companion in &tool.companions {
                if !keys.contains(companion) {
                    keys.push(companion.clone());
                }
            }
        }
    }
    Ok(keys)
}

/// Fetch the manifest, rebuild the selection (previous keys + `extra`),
/// write it to conf.d, and `mise install` the result.
pub fn sync_selected(system: &dyn System, extra: &[String]) -> Result<PathBuf, String> {
    sync_selected_from(
        system,
        extra,
        &std::sync::atomic::AtomicBool::new(false),
        &Repository::default(),
    )
}

pub(crate) fn sync_selected_from(
    system: &dyn System,
    extra: &[String],
    cancelled: &std::sync::atomic::AtomicBool,
    repository: &Repository,
) -> Result<PathBuf, String> {
    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let target = conf_d_target(&home);
    crate::fs_tx::recover(&target)?;
    let target = repository.get(system, cancelled).and_then(|repo_root| {
        let source = repo_root.join(MANIFEST_IN_REPO);
        let manifest = fs::read_to_string(&source)
            .map_err(|error| format!("downloaded repo has no {MANIFEST_IN_REPO}: {error}"))?;
        let mut keys = selected_keys(&home);
        for key in extra {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        let keys = with_tool_companions(keys)?;
        let current = fs::read_to_string(&target).unwrap_or_default();
        let content = render_selection(&manifest, &current, &keys)?;
        crate::fs_tx::atomic_write(&target, content.as_bytes())?;
        Ok(target)
    })?;
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
    crate::fs_tx::recover(&target)?;
    let current = fs::read_to_string(&target)
        .map_err(|error| format!("could not read {}: {error}", target.display()))?;
    let keys = selected_keys(&home)
        .into_iter()
        .filter(|key| !removed.contains(key))
        .collect::<Vec<_>>();
    let content = render_selection(&current, &current, &keys)?;
    crate::fs_tx::atomic_write(&target, content.as_bytes())?;
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

    #[test]
    fn selected_confluence_includes_its_pinned_installer_once() {
        let keys = with_tool_companions(vec!["pipx:confluence-markdown-exporter".into()]).unwrap();
        assert!(keys.contains(&"uv".into()));
        let content = render_selection(BUNDLED_MANIFEST, "", &keys).unwrap();
        let pinned_uv = BUNDLED_MANIFEST
            .lines()
            .find(|line| line_key(line) == Some("uv"))
            .unwrap();
        assert!(content.contains(pinned_uv));
        assert_eq!(with_tool_companions(keys.clone()).unwrap(), keys);
        assert!(!with_tool_companions(vec!["npm:@tobilu/qmd".into()])
            .unwrap()
            .contains(&"uv".into()));
    }

    #[test]
    fn published_manifest_is_valid_toml() {
        include_str!("../../../manifest/loom.toml")
            .parse::<toml_edit::DocumentMut>()
            .unwrap();
    }

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
    fn selected_keys_recovers_an_interrupted_selection() {
        let home = std::env::temp_dir().join(format!(
            "loom-manifest-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = conf_d_target(&home);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(
            &target,
            "[tools]\n\"github:Yassimba/loom[exe=loom-teams]\" = \"old\"\ngh = \"2.97.0\"\n\"ubi:Yassimba/loom\" = \"new\"\n",
        )
        .unwrap();
        let backup = target.with_file_name(".loom.toml.loom-old");
        std::fs::rename(&target, &backup).unwrap();

        assert_eq!(
            selected_keys(&home),
            vec!["ubi:Yassimba/loom".to_string(), "gh".to_string()]
        );
        assert!(target.is_file());
        assert!(!backup.exists());
        std::fs::remove_dir_all(home).unwrap();
    }

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
    fn newly_requested_key_falls_back_to_the_bundled_exact_pin() {
        let rendered = render_selection(MANIFEST, "", &["python".into()]).unwrap();
        assert!(rendered.contains("python = \"3.13.7\""));
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
    fn plannotator_selection_moves_to_upstream() {
        let old_key = "github:Yassimba/plannotator";
        let current = format!("[tools]\n\"{old_key}\" = \"v0.27.9-loom.1\"\n");
        let rendered =
            render_selection(BUNDLED_MANIFEST, &current, &[current_key(old_key).into()]).unwrap();
        assert!(rendered.contains("\"github:backnotprop/plannotator\" ="));
        assert!(!rendered.contains(old_key));
        assert!(!rendered.contains("v0.27.9-loom.1"));
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
