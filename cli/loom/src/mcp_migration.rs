//! One-time move from pi-mcp-adapter to Pi's built-in MCP support (Pi 0.99+).
//!
//! The adapter read `mcp.json` (2.x) or `mcp-adapter.json` (3.x), and while it
//! is installed it replaces Pi's built-in MCP. `loom update` migrates each
//! config directory, rewrites Loom's ownership receipts to match, then removes
//! the adapter package. Every rewritten file keeps a backup.
use crate::mcp::{entry_digest, lists_adapter, read_object, write_config};
use crate::ownership::{InstallState, Receipt};
use crate::{CommandSpec, System};
use anyhow::{bail, ensure, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MIN_PI: (u32, u32, u32) = (0, 99, 0);
const ADAPTER: &str = "npm:pi-mcp-adapter";
const LEGACY_FILE: &str = "mcp-adapter.json";

/// Migrates when there is adapter state to move. `Ok(None)` means nothing to do.
pub fn migrate_adapter(system: &dyn System, home: &Path, project: &Path) -> Result<Option<String>> {
    let global = home.join(".pi/agent");
    let mut state = InstallState::load(home).map_err(anyhow::Error::msg)?;
    let mut dirs = BTreeSet::from([global.clone(), project.join(".pi")]);
    for receipt in state.resources.values().flat_map(|r| &r.receipts) {
        if let Receipt::McpEntry { path, .. } = receipt {
            dirs.extend(path.parent().map(Path::to_path_buf));
        }
    }
    let installed: Vec<bool> = [(global, false), (project.join(".pi"), true)]
        .into_iter()
        .filter(|(dir, _)| adapter_listed(dir))
        .map(|(_, local)| local)
        .collect();
    let pending: Vec<&PathBuf> = dirs.iter().filter(|dir| needs_migration(dir)).collect();
    if installed.is_empty() && pending.is_empty() {
        return Ok(None);
    }

    system.refresh_path();
    let Some(version) = pi_version(system).filter(|v| *v >= MIN_PI) else {
        bail!("Pi 0.99 or newer is needed for built-in MCP; pi-mcp-adapter kept. Rerun `loom update` after Pi updates");
    };

    let mut notes = Vec::new();
    for dir in &pending {
        migrate_dir(dir, &mut state, &mut notes)?;
    }
    // Loom recorded the adapter as an owned prerequisite of its MCP servers.
    state
        .resources
        .retain(|id, _| !id.ends_with("pi-package:pi-mcp-adapter"));
    for resource in state.resources.values_mut() {
        resource
            .depends_on
            .retain(|dependency| !dependency.ends_with("pi-package:pi-mcp-adapter"));
    }
    state.save(home).map_err(anyhow::Error::msg)?;

    for local in installed {
        let args = if local {
            vec!["remove", "-l", ADAPTER]
        } else {
            vec!["remove", ADAPTER]
        };
        let command = CommandSpec::new("pi", args).in_dir(project);
        let result = system.run(&command)?;
        ensure!(
            result.success,
            "MCP config migrated, but `{}` failed: {}",
            command.display(),
            crate::install::command_failure_message(&result)
        );
    }
    let (major, minor, patch) = version;
    notes.insert(
        0,
        format!("moved to Pi {major}.{minor}.{patch} built-in MCP · {} config(s) migrated · pi-mcp-adapter removed", pending.len()),
    );
    Ok(Some(notes.join("; ")))
}

fn pi_version(system: &dyn System) -> Option<(u32, u32, u32)> {
    let result = system
        .run_probe(&CommandSpec::new("pi", ["--version"]))
        .ok()?;
    crate::install::parse_version(result.stdout.trim()).filter(|_| result.success)
}

fn adapter_listed(dir: &Path) -> bool {
    read_object(&dir.join("settings.json")).is_ok_and(|(_, value)| lists_adapter(&value))
}

fn needs_migration(dir: &Path) -> bool {
    dir.join(LEGACY_FILE).is_file()
        || read_object(&dir.join("mcp.json"))
            .is_ok_and(|(_, mut value)| normalize(&mut value, &mut Vec::new()))
}

/// Rewrites adapter-only settings into built-in MCP settings. Returns whether
/// anything changed; unknown keys stay, since Pi ignores them.
fn normalize(value: &mut Value, notes: &mut Vec<String>) -> bool {
    let Some(root) = value.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    if root.get("mcpServers").is_none_or(Value::is_null) {
        if let Some(servers) = root.remove("mcp-servers") {
            root.insert("mcpServers".into(), servers);
            changed = true;
        }
    }
    if root
        .get("imports")
        .and_then(Value::as_array)
        .is_some_and(|imports| !imports.is_empty())
    {
        notes.push("built-in MCP does not read `imports`; copy those servers into mcp.json".into());
    }
    let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        return changed;
    };
    for entry in servers.values_mut().filter_map(Value::as_object_mut) {
        changed |= normalize_entry(entry);
    }
    changed
}

fn normalize_entry(entry: &mut Map<String, Value>) -> bool {
    let mut changed = false;
    if let Some(direct) = entry.remove("directTools") {
        changed = true;
        match direct {
            Value::Bool(true) => {
                entry.entry("exposure").or_insert(json!("direct"));
            }
            Value::Array(tools) => {
                let exposure = entry.entry("toolExposure").or_insert(json!({}));
                if let Some(exposure) = exposure.as_object_mut() {
                    for tool in tools.iter().filter_map(Value::as_str) {
                        exposure.entry(tool).or_insert(json!("direct"));
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(disabled) = entry.remove("disabled") {
        changed = true;
        if disabled == true {
            entry.insert("enabled".into(), json!(false));
        }
    }
    // Absolute paths into a mise install break when the pin moves; the mise
    // shim on PATH resolves the bare name to the current version.
    let pinned = entry
        .get("command")
        .and_then(Value::as_str)
        .and_then(|command| {
            let path = Path::new(command);
            let parts: Vec<_> = path.components().map(|c| c.as_os_str()).collect();
            let in_mise = parts
                .windows(2)
                .any(|w| w[0] == "mise" && w[1] == "installs");
            (path.is_absolute() && in_mise)
                .then(|| path.file_name()?.to_str().map(str::to_owned))
                .flatten()
        });
    if let Some(name) = pinned {
        entry.insert("command".into(), json!(name));
        changed = true;
    }
    changed
}

fn migrate_dir(dir: &Path, state: &mut InstallState, notes: &mut Vec<String>) -> Result<()> {
    let target = dir.join("mcp.json");
    let legacy = dir.join(LEGACY_FILE);
    let (before, original) = read_object(&target)?;
    let mut value = original.clone();
    normalize(&mut value, notes);
    let legacy_original = if legacy.is_file() {
        let (_, legacy_value) = read_object(&legacy)?;
        let mut incoming = legacy_value.clone();
        normalize(&mut incoming, notes);
        if value.get("mcpServers").is_none_or(Value::is_null) {
            value["mcpServers"] = json!({});
        }
        let Some(servers) = value["mcpServers"].as_object_mut() else {
            bail!(
                "{}: mcpServers must be an object; no changes made",
                target.display()
            );
        };
        for (name, entry) in incoming["mcpServers"].as_object().into_iter().flatten() {
            if servers.contains_key(name) {
                notes.push(format!("kept the {name} entry from mcp.json; {LEGACY_FILE}'s copy is in {LEGACY_FILE}.loom-migrated"));
            } else {
                servers.insert(name.clone(), entry.clone());
            }
        }
        Some(legacy_value)
    } else {
        None
    };

    // Clean Loom receipts follow their entry; modified ones stay modified.
    for receipt in state
        .resources
        .values_mut()
        .flat_map(|resource| &mut resource.receipts)
    {
        let Receipt::McpEntry { path, name, digest } = receipt else {
            continue;
        };
        let old = if *path == target {
            original
                .pointer(&format!("/mcpServers/{name}"))
                .or_else(|| original.pointer(&format!("/mcp-servers/{name}")))
        } else if *path == legacy {
            legacy_original
                .as_ref()
                .and_then(|v| v.pointer(&format!("/mcpServers/{name}")))
        } else {
            continue;
        };
        let new = value.pointer(&format!("/mcpServers/{name}"));
        if let (Some(old), Some(new)) = (old, new) {
            if entry_digest(old) == *digest {
                *digest = entry_digest(new);
                *path = target.clone();
            }
        }
    }

    write_config(
        &target,
        &before,
        &format!("{}\n", serde_json::to_string_pretty(&value)?),
    )?;
    if legacy_original.is_some() {
        std::fs::rename(&legacy, dir.join(format!("{LEGACY_FILE}.loom-migrated")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_entries_become_built_in_settings() {
        let mut value = json!({"mcp-servers": {
            "memory": {"command": "/home/u/.local/share/mise/installs/npm-codebase-memory-mcp/0.10.8/node_modules/.bin/codebase-memory-mcp", "directTools": ["trace_path"]},
            "docs": {"url": "https://mcp.context7.com/mcp", "directTools": false},
            "all": {"url": "https://x.invalid/mcp", "directTools": true, "disabled": true},
            "own": {"command": "/opt/bin/tool"}
        }});
        assert!(normalize(&mut value, &mut Vec::new()));
        assert_eq!(
            value,
            json!({"mcpServers": {
                "memory": {"command": "codebase-memory-mcp", "toolExposure": {"trace_path": "direct"}},
                "docs": {"url": "https://mcp.context7.com/mcp"},
                "all": {"url": "https://x.invalid/mcp", "exposure": "direct", "enabled": false},
                "own": {"command": "/opt/bin/tool"}
            }})
        );
        assert!(!normalize(&mut value, &mut Vec::new()));
    }
}
