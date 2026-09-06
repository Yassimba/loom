//! Pi's gateway defers tool exposure, not necessarily server startup.
use crate::{SkillAgent, SkillDestination, SkillScope, System};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SEM_TOOL_KEY: &str = "github:Ataraxy-Labs/sem[exe=sem]";
pub const ADAPTER_SPEC: &str = "npm:pi-mcp-adapter@2.32.1";
pub const EXPOSURE_NOTE: &str = "Pi gateway only (directTools=false); lifecycle unchanged. First launch may discover server metadata. Restart Pi and use /mcp to check live health.";

pub fn config_path(destination: &SkillDestination) -> PathBuf {
    match destination.scope {
        SkillScope::Global => destination.home.join(".pi/agent/mcp.json"),
        SkillScope::Project => destination.project_root.join(".pi/mcp.json"),
    }
}

fn safe_path(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match ancestor.symlink_metadata() {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "{} is symlinked; configure MCP manually",
                ancestor.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("cannot inspect {}", ancestor.display()),
        }
    }
    Ok(())
}

fn transaction_backup(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .context("MCP path has no file name")?
        .to_string_lossy();
    Ok(path.with_file_name(format!(".{name}.loom-old")))
}

fn recover_config(path: &Path) -> Result<()> {
    safe_path(path)?;
    safe_path(&transaction_backup(path)?)?;
    crate::fs_tx::recover(path).map_err(anyhow::Error::msg)
}

fn read_object(path: &Path) -> Result<(String, Value)> {
    safe_path(path)?;
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Preview recoverable state without mutating it during plan/status.
            let backup = transaction_backup(path)?;
            safe_path(&backup)?;
            match fs::read_to_string(&backup) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => "{}\n".into(),
                Err(_) => bail!("cannot read {}", backup.display()),
            }
        }
        Err(_) => bail!("cannot read {}", path.display()),
    };
    // Never include parser input/errors: MCP and Pi settings can contain secrets.
    let value = crate::jsonc::parse_document(&text)
        .ok()
        .filter(Value::is_object)
        .with_context(|| {
            format!(
                "{} must contain a valid JSON object; no changes made",
                path.display()
            )
        })?;
    Ok((text, value))
}

fn sem_entry() -> Value {
    json!({"command": "sem", "args": ["mcp"], "directTools": false})
}

fn compatible_entry(entry: &Value) -> bool {
    entry.is_object()
        && entry.get("command") == Some(&json!("sem"))
        && entry.get("args") == Some(&json!(["mcp"]))
        && entry.get("directTools") == Some(&json!(false))
        && entry.get("disabled").is_none_or(|value| value == false)
        && entry.get("url").is_none()
        && entry.get("socket").is_none()
}

// Match the adapter's `mcpServers ?? raw["mcp-servers"]` precedence.
fn servers_key(value: &Value) -> &'static str {
    if value.get("mcpServers").is_none_or(Value::is_null)
        && value.get("mcp-servers").is_some_and(|v| !v.is_null())
    {
        "mcp-servers"
    } else {
        "mcpServers"
    }
}

fn validate_config(value: &Value, path: &Path) -> Result<()> {
    let key = servers_key(value);
    ensure!(
        value.get(key).is_none_or(|v| v.is_null() || v.is_object()),
        "{}: {key} must be an object",
        path.display()
    );
    ensure!(
        value.get("settings").is_none_or(Value::is_object),
        "{}: settings must be an object",
        path.display()
    );
    ensure!(
        value.pointer("/settings/disableProxyTool") != Some(&json!(true)),
        "{} disables the MCP gateway; enable the proxy before installing Sem",
        path.display()
    );
    ensure!(
        value
            .get("imports")
            .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
            && value
                .pointer("/settings/hostConfigDiscovery")
                .is_none_or(|v| v == "off"),
        "{} uses host imports/discovery; resolve Sem manually in /mcp setup before using Loom",
        path.display()
    );
    Ok(())
}

/// Read-only preflight, used both before review and again before any install lane.
/// Existing same-name definitions in other layers require explicit user resolution.
pub fn preflight(destination: &SkillDestination) -> Result<()> {
    ensure!(
        destination.agents.contains(&SkillAgent::Pi),
        "Sem MCP needs Pi selected; other agent adapters are not yet verified (use --agent pi)"
    );
    ensure!(
        destination.scope != SkillScope::Project
            || !std::env::var("PI_MCP_CONFIG_MODE")
                .is_ok_and(|mode| mode.trim().eq_ignore_ascii_case("exclusive")),
        "PI_MCP_CONFIG_MODE=exclusive ignores project MCP configuration; unset it or use --scope global"
    );
    let global = destination.home.join(".pi/agent");
    ensure!(std::env::var_os("PI_CODING_AGENT_DIR").is_none_or(|value| value.is_empty() || Path::new(&value) == global),
        "custom PI_CODING_AGENT_DIR is not yet supported for MCP setup; use the default Pi agent directory");
    let target = config_path(destination);
    for path in [
        destination.home.join(".config/mcp/mcp.json"),
        destination.home.join(".agents/mcp.json"),
        destination.home.join(".agents/mcp/mcp.json"),
        global.join("mcp.json"),
        destination.project_root.join(".mcp.json"),
        destination.project_root.join(".pi/mcp.json"),
    ] {
        let (_, value) = read_object(&path)?;
        validate_config(&value, &path)?;
        if let Some(entry) = value.get(servers_key(&value)).and_then(|v| v.get("sem")) {
            ensure!(path == target, "Sem already has a definition in {}; resolve that entry before adding another scope", path.display());
            ensure!(compatible_entry(entry), "{} has a conflicting or disabled Sem entry; preserve it and configure gateway exposure manually in /mcp", path.display());
        }
    }
    adapter_needed(destination)?;
    crate::ownership::InstallState::inspect(&destination.home).map_err(anyhow::Error::msg)?;
    Ok(())
}

fn supported_version(version: &str) -> bool {
    let parts = version
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>();
    // Preserve compatible stable 2.x releases; unknown sources/majors are not downgraded.
    parts.is_ok_and(|p| p.len() == 3 && p[0] == 2 && (p[1], p[2]) >= (32, 1))
}

/// Missing global registry install may be added; filtered, local, or unknown
/// adapter installations are never silently replaced or duplicated.
pub fn adapter_needed(destination: &SkillDestination) -> Result<bool> {
    let global = destination.home.join(".pi/agent");
    let mut found = false;
    for root in [global.clone(), destination.project_root.join(".pi")] {
        let path = root.join("settings.json");
        let (_, value) = read_object(&path)?;
        ensure!(
            value.get("packages").is_none_or(Value::is_array),
            "{}: packages must be an array",
            path.display()
        );
        ensure!(
            value
                .get("extensions")
                .is_none_or(|v| v.as_array().is_some_and(|entries| entries
                    .iter()
                    .all(|entry| entry.as_str().is_some_and(|s| !s.contains("mcp-adapter"))))),
            "{} has an unverified MCP extension path/filter; use pi config to resolve it",
            path.display()
        );
        for entry in value
            .get("packages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let source = entry
                .as_str()
                .or_else(|| entry.get("source").and_then(Value::as_str));
            let Some(source) = source else {
                bail!("{} has an invalid Pi package entry", path.display())
            };
            if !source.contains("mcp-adapter") {
                continue;
            }
            ensure!(root == global && !found, "{} has a project or duplicate MCP adapter; preserve it and resolve the shared prerequisite with pi config", path.display());
            ensure!(
                source == "npm:pi-mcp-adapter"
                    || source
                        .strip_prefix("npm:pi-mcp-adapter@")
                        .is_some_and(|v| v == "latest" || supported_version(v)),
                "{} has an unverified MCP adapter source/version; no replacement made",
                path.display()
            );
            ensure!(
                entry.get("extensions").is_none()
                    && entry.get("autoload").is_none_or(|v| v == true),
                "{} filters or disables the MCP adapter; enable it with pi config",
                path.display()
            );
            let package = global.join("npm/node_modules/pi-mcp-adapter");
            let (_, manifest) = read_object(&package.join("package.json"))?;
            ensure!(manifest.get("name") == Some(&json!("pi-mcp-adapter"))
                && manifest.get("version").and_then(Value::as_str).is_some_and(supported_version)
                && manifest.pointer("/pi/extensions").and_then(Value::as_array).is_some_and(|paths| !paths.is_empty() && paths.iter().all(|p| p.as_str().is_some_and(|s| package.join(s).is_file()))),
                "MCP adapter files are missing or unverified; repair with pi install {ADAPTER_SPEC} before retrying");
            found = true;
        }
    }
    if !found {
        ensure!(!global.join("npm/node_modules/pi-mcp-adapter").exists(), "unregistered MCP adapter files exist; register the existing package with Pi instead of replacing it");
    }
    Ok(!found)
}

/// Configuration presence is not live health. No MCP processes are launched.
pub fn configured(destination: &SkillDestination, system: &dyn System) -> bool {
    preflight(destination).is_ok()
        && config_path(destination).is_file()
        && adapter_needed(destination).is_ok_and(|needed| !needed)
        && system.command_exists("pi")
        && system.command_exists("sem")
        && read_object(&config_path(destination)).is_ok_and(|(_, v)| {
            v.get(servers_key(&v))
                .and_then(|servers| servers.get("sem"))
                .is_some_and(compatible_entry)
        })
}

fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("cannot create private MCP file {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn write_config(path: &Path, before: &str, after: &str) -> Result<()> {
    recover_config(path)?;
    ensure!(
        read_object(path)?.0 == before,
        "MCP configuration changed during installation; retry"
    );
    fs::create_dir_all(path.parent().context("MCP path has no parent")?)?;
    // Unique create_new files avoid following links or overwriting earlier backups.
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let staged = path.with_file_name(format!(".mcp.json.loom-new-{suffix}"));
    if path.exists() {
        private_file(
            &path.with_file_name(format!(".mcp.json.loom-backup-{suffix}")),
            before.as_bytes(),
        )?;
    }
    private_file(&staged, after.as_bytes())?;
    // Recheck after staging as well: never replace an edit observed since review.
    if read_object(path)?.0 != before {
        let _ = fs::remove_file(&staged);
        bail!("MCP configuration changed while staging; retry");
    }
    let result = crate::fs_tx::replace_staged(path, &staged).map_err(anyhow::Error::msg);
    if result.is_err() {
        let _ = fs::remove_file(staged);
    }
    result
}

fn entry_digest(value: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("JSON serializes"))
    )
}

pub fn install(destination: &SkillDestination, system: &dyn System) -> Result<()> {
    preflight(destination)?;
    ensure!(
        !adapter_needed(destination)?
            && system.command_exists("pi")
            && system.command_exists("sem"),
        "Sem MCP prerequisites missing; retry setup to install Sem and the Pi adapter"
    );
    let path = config_path(destination);
    recover_config(&path)?;
    let (before, mut value) = read_object(&path)?;
    let key = servers_key(&value);
    if value.get(key).and_then(|v| v.get("sem")).is_some() {
        return Ok(());
    }
    let entry = sem_entry();
    if value.get(key).is_none_or(Value::is_null) {
        value[key] = json!({});
    }
    value[key]["sem"] = entry.clone();
    let after = crate::jsonc::set(&before, key, &value[key])?;
    let mut state =
        crate::ownership::InstallState::load(&destination.home).map_err(anyhow::Error::msg)?;
    let scope = match destination.scope {
        SkillScope::Global => crate::ownership::OwnershipScope::Global,
        SkillScope::Project => crate::ownership::OwnershipScope::Project {
            root: destination
                .project_root
                .canonicalize()
                .unwrap_or_else(|_| destination.project_root.clone()),
        },
    };
    let id = match &scope {
        crate::ownership::OwnershipScope::Global => "mcp-server:sem".into(),
        crate::ownership::OwnershipScope::Project { root } => {
            format!("project:{}:mcp-server:sem", root.display())
        }
    };
    state.record(crate::ownership::OwnedResource {
        id,
        scope,
        depends_on: vec![
            "core:loom".into(),
            "core:mise".into(),
            "tool:pi".into(),
            "pi-package:pi-mcp-adapter".into(),
            "tool:sem".into(),
        ],
        receipts: vec![crate::ownership::Receipt::McpEntry {
            path: path.clone(),
            name: "sem".into(),
            digest: entry_digest(&entry),
        }],
    });
    write_config(&path, &before, &after)?;
    if let Err(error) = state.save(&destination.home) {
        // Do not leave a new, unowned entry after an ordinary ledger write failure.
        remove_entry(&path, "sem", &entry_digest(&entry))?;
        bail!("MCP ownership could not be saved: {error}");
    }
    Ok(())
}

pub fn entry_status(path: &Path, name: &str, digest: &str) -> crate::uninstall::ReceiptStatus {
    use crate::uninstall::ReceiptStatus;
    match read_object(path) {
        Ok((_, value))
            if value
                .get(servers_key(&value))
                .is_some_and(|v| !v.is_null() && !v.is_object()) =>
        {
            ReceiptStatus::Modified
        }
        Ok((_, value)) => match value.get(servers_key(&value)).and_then(|v| v.get(name)) {
            None => ReceiptStatus::Missing,
            Some(entry) if entry_digest(entry) == digest => ReceiptStatus::Clean,
            Some(_) => ReceiptStatus::Modified,
        },
        Err(_) => ReceiptStatus::Modified,
    }
}

pub fn remove_entry(path: &Path, name: &str, digest: &str) -> Result<()> {
    recover_config(path)?;
    let (before, mut value) = read_object(path)?;
    let key = servers_key(&value);
    ensure!(
        value.get(key).is_none_or(|v| v.is_null() || v.is_object()),
        "{key} must be an object; preserved"
    );
    let Some(entry) = value.get(key).and_then(|v| v.get(name)) else {
        return Ok(());
    };
    ensure!(
        entry_digest(entry) == digest,
        "MCP entry changed; preserved even with --force-modified"
    );
    value[key]
        .as_object_mut()
        .context("MCP servers must be an object")?
        .remove(name);
    let after = crate::jsonc::set(&before, key, &value[key])?;
    write_config(path, &before, &after)
}
