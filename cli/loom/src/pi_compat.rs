use crate::System;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const FEYNMAN: &str = "pi-package:@companion-ai/feynman";
pub(crate) const AUTORESEARCH: &str = "pi-package:pi-autoresearch";
const AUTORESEARCH_SEARCH_SHORTCUT: &str = "ctrl+shift+f";
const AUTORESEARCH_DASHBOARD_SHORTCUT: &str = "ctrl+shift+y";
const FEYNMAN_THINKING_IMPORT: &str =
    "import { registerThinkingCommand } from \"./research-tools/thinking.js\";\n";
const FEYNMAN_THINKING_REGISTRATION: &str = "\tregisterThinkingCommand(pi);\n";

pub(crate) fn is_managed(target: &str) -> bool {
    matches!(target, FEYNMAN | AUTORESEARCH)
}

pub(crate) fn apply_for_package(target: &str, system: &dyn System) -> Result<bool> {
    if !is_managed(target) {
        return Ok(false);
    }
    let agent_dir = std::env::var_os("PI_CODING_AGENT_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| system.home_dir().map(|home| home.join(".pi/agent")))
        .context("home directory is unavailable")?;
    if target == AUTORESEARCH {
        return configure_autoresearch_shortcut(&agent_dir);
    }

    let mut package_roots = vec![agent_dir];
    if let Some(project_root) = system.current_dir().map(|cwd| cwd.join(".pi")) {
        if !package_roots.contains(&project_root) {
            package_roots.push(project_root);
        }
    }
    let installed = package_roots
        .into_iter()
        .filter(|root| feynman_source(root).is_file())
        .collect::<Vec<_>>();
    if installed.is_empty() {
        bail!("installed Feynman package could not be found");
    }
    let mut changed = false;
    for root in installed {
        changed |= disable_feynman_thinking_command(&root)?;
    }
    Ok(changed)
}

fn configure_autoresearch_shortcut(agent_dir: &Path) -> Result<bool> {
    let path = agent_dir.join("extensions/pi-autoresearch.json");
    let mut config = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Value>(&content)
            .with_context(|| format!("could not parse {}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()))
        }
    };
    let root = config
        .as_object_mut()
        .with_context(|| format!("{} must contain a JSON object", path.display()))?;
    let shortcuts = root.entry("shortcuts").or_insert_with(|| json!({}));
    if !shortcuts.is_object() {
        bail!("{}.shortcuts must contain a JSON object", path.display());
    }
    let shortcuts = shortcuts.as_object_mut().expect("checked object");
    match shortcuts.get("fullscreenDashboard") {
        Some(Value::String(value)) if value != AUTORESEARCH_SEARCH_SHORTCUT => return Ok(false),
        Some(Value::Null) => return Ok(false),
        _ => {}
    }
    shortcuts.insert(
        "fullscreenDashboard".into(),
        Value::String(AUTORESEARCH_DASHBOARD_SHORTCUT.into()),
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&config)?),
    )
    .with_context(|| format!("could not write {}", path.display()))?;
    Ok(true)
}

fn feynman_source(agent_dir: &Path) -> PathBuf {
    agent_dir.join("npm/node_modules/@companion-ai/feynman/extensions/research-tools.ts")
}

fn disable_feynman_thinking_command(agent_dir: &Path) -> Result<bool> {
    let path = feynman_source(agent_dir);
    let content =
        fs::read_to_string(&path).with_context(|| format!("could not read {}", path.display()))?;
    if !content.contains(FEYNMAN_THINKING_REGISTRATION) {
        return Ok(false);
    }
    if !content.contains(FEYNMAN_THINKING_IMPORT) {
        bail!(
            "{} changed upstream; cannot safely disable its duplicate /thinking command",
            path.display()
        );
    }
    let patched = content
        .replace(FEYNMAN_THINKING_IMPORT, "")
        .replace(FEYNMAN_THINKING_REGISTRATION, "");
    fs::write(&path, patched).with_context(|| format!("could not write {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_agent(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "loom-pi-compat-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn autoresearch_moves_only_the_conflicting_shortcut() {
        let agent = temp_agent("autoresearch");
        let path = agent.join("extensions/pi-autoresearch.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"other":true,"shortcuts":{"fullscreenDashboard":"ctrl+shift+f"}}"#,
        )
        .unwrap();

        assert!(configure_autoresearch_shortcut(&agent).unwrap());
        let config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["other"], true);
        assert_eq!(
            config["shortcuts"]["fullscreenDashboard"],
            AUTORESEARCH_DASHBOARD_SHORTCUT
        );
        assert!(!configure_autoresearch_shortcut(&agent).unwrap());
        fs::remove_dir_all(agent).unwrap();
    }

    #[test]
    fn autoresearch_preserves_a_custom_shortcut() {
        let agent = temp_agent("custom-shortcut");
        let path = agent.join("extensions/pi-autoresearch.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"shortcuts":{"fullscreenDashboard":"ctrl+shift+u"}}"#,
        )
        .unwrap();

        assert!(!configure_autoresearch_shortcut(&agent).unwrap());
        assert!(fs::read_to_string(&path).unwrap().contains("ctrl+shift+u"));
        fs::remove_dir_all(agent).unwrap();
    }

    #[test]
    fn feynman_layout_changes_fail_loudly() {
        let agent = temp_agent("feynman-layout");
        let path = feynman_source(&agent);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!("export default function researchTools(pi: unknown) {{\n{FEYNMAN_THINKING_REGISTRATION}}}\n"),
        )
        .unwrap();

        assert!(disable_feynman_thinking_command(&agent)
            .unwrap_err()
            .to_string()
            .contains("changed upstream"));
        fs::remove_dir_all(agent).unwrap();
    }

    #[test]
    fn feynman_patch_is_idempotent() {
        let agent = temp_agent("feynman");
        let path =
            agent.join("npm/node_modules/@companion-ai/feynman/extensions/research-tools.ts");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(
                "{FEYNMAN_THINKING_IMPORT}export default function researchTools(pi: unknown) {{\n{FEYNMAN_THINKING_REGISTRATION}}}\n"
            ),
        )
        .unwrap();

        assert!(disable_feynman_thinking_command(&agent).unwrap());
        let patched = fs::read_to_string(&path).unwrap();
        assert!(!patched.contains("registerThinkingCommand"));
        assert!(!disable_feynman_thinking_command(&agent).unwrap());
        fs::remove_dir_all(agent).unwrap();
    }
}
