//! Sync the published tool manifest into mise and install it.
//!
//! The repo's `manifest/ai-setup.toml` is the single source of truth for the
//! tools this setup provides (node, pi, herdr, gh, ...), exact-pinned. This
//! module copies it to `~/.config/mise/conf.d/ai-setup.toml` — merged by mise
//! into the user's global config without touching their own config.toml — and
//! runs `mise install`. Tools therefore change only when a new manifest lands
//! on main.

use crate::{skills, CommandSpec, System};
use std::fs;
use std::path::PathBuf;

const MANIFEST_IN_REPO: &str = "manifest/ai-setup.toml";

pub fn conf_d_target(home: &std::path::Path) -> PathBuf {
    home.join(".config")
        .join("mise")
        .join("conf.d")
        .join("ai-setup.toml")
}

pub fn mise_available(system: &dyn System) -> bool {
    system.command_exists("mise")
}

/// Fetch the repo tarball and copy the manifest into mise's conf.d.
/// Returns the target path written.
pub fn sync_manifest(system: &dyn System) -> Result<PathBuf, String> {
    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let staging = home
        .join(".cache")
        .join("ai-setup")
        .join("manifest-staging");
    let result = skills::fetch_repo(system, &staging).and_then(|repo_root| {
        let source = repo_root.join(MANIFEST_IN_REPO);
        let content = fs::read_to_string(&source)
            .map_err(|error| format!("downloaded repo has no {MANIFEST_IN_REPO}: {error}"))?;
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
    result
}

/// Run `mise install` so the synced manifest's pins are present.
pub fn mise_install(system: &dyn System) -> Result<(), String> {
    let spec = CommandSpec::new("mise", ["install", "--yes"]);
    match system.run(&spec) {
        Ok(result) if result.success => Ok(()),
        Ok(result) => Err(crate::install::command_failure_message(&result)),
        Err(error) => Err(error.to_string()),
    }
}

/// The whole lane: sync the manifest, then install its pins. Used by both
/// `ai-setup update` and the setup flow. Errors describe the failing half.
pub fn sync_and_install(system: &dyn System) -> Result<PathBuf, String> {
    let target = sync_manifest(system)?;
    mise_install(system).map_err(|error| format!("mise install failed: {error}"))?;
    Ok(target)
}
