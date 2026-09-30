//! Pi's built-in MCP support reads `mcp.json`; servers use its default codemode exposure.
use crate::{SkillAgent, SkillDestination, SkillScope, System};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const EXPOSURE_NOTE: &str = "Pi built-in MCP, codemode exposure: tools are reached through the codemode tool. Run /reload in Pi and use /mcp to check live health.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Server {
    Context7,
    CodebaseMemory,
}

impl Server {
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "context7" => Ok(Self::Context7),
            "codebase-memory-mcp" => Ok(Self::CodebaseMemory),
            _ => bail!("unverified MCP server"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Context7 => "context7",
            Self::CodebaseMemory => "codebase-memory-mcp",
        }
    }

    fn executable(self) -> Option<&'static str> {
        match self {
            Self::Context7 => None,
            Self::CodebaseMemory => Some("codebase-memory-mcp"),
        }
    }

    fn tool_dependency(self) -> Option<&'static str> {
        match self {
            Self::Context7 => None,
            Self::CodebaseMemory => Some("tool:codebase-memory-mcp"),
        }
    }

    fn entry(self) -> Value {
        match self {
            Self::Context7 => json!({"url": "https://mcp.context7.com/mcp"}),
            Self::CodebaseMemory => json!({
                "command": "codebase-memory-mcp",
                "args": ["--tool-profile=analysis"]
            }),
        }
    }

    fn compatible_entry(self, entry: &Value) -> bool {
        let expected = self.entry();
        expected.as_object().unwrap().iter().all(|(key, value)| {
            entry.get(key) == Some(value)
                || (key == "command"
                    && self.executable().is_some_and(|binary| {
                        entry
                            .get(key)
                            .and_then(Value::as_str)
                            .is_some_and(|command| {
                                let path = Path::new(command);
                                path.is_absolute() && path.file_name() == Some(binary.as_ref())
                            })
                    }))
        }) && entry.get("enabled").is_none_or(|v| v == true)
            && entry.get("exposure").is_none_or(|v| v != "hidden")
            && match self.executable() {
                Some(_) => entry.get("url").is_none(),
                None => entry.get("command").is_none() && entry.get("args").is_none(),
            }
    }

    fn prerequisites_present(self, system: &dyn System) -> bool {
        system.command_exists("pi")
            && self
                .executable()
                .is_none_or(|binary| system.command_exists(binary))
    }
}

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

pub(crate) fn read_object(path: &Path) -> Result<(String, Value)> {
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

const SERVERS_KEY: &str = "mcpServers";

fn validate_config(value: &Value, path: &Path) -> Result<()> {
    ensure!(
        value
            .get(SERVERS_KEY)
            .is_none_or(|v| v.is_null() || v.is_object()),
        "{}: {SERVERS_KEY} must be an object",
        path.display()
    );
    ensure!(
        value.get("autoEnableCodemode") != Some(&json!(false)),
        "{} sets autoEnableCodemode to false, so codemode MCP tools cannot be called; remove it before installing an MCP server",
        path.display()
    );
    Ok(())
}

fn config_paths(destination: &SkillDestination) -> [PathBuf; 2] {
    [
        destination.home.join(".pi/agent/mcp.json"),
        destination.project_root.join(".pi/mcp.json"),
    ]
}

fn validate_entries(server: Server, destination: &SkillDestination, target: &Path) -> Result<()> {
    let name = server.name();
    for path in config_paths(destination) {
        let (_, value) = read_object(&path)?;
        validate_config(&value, &path)?;
        if let Some(entry) = value.get(SERVERS_KEY).and_then(|v| v.get(name)) {
            ensure!(server.compatible_entry(entry), "{} has a conflicting, disabled, or hidden {name} entry; preserve it and fix it in /mcp", path.display());
            ensure!(
                path.is_file() || path == target,
                "{} has a pending MCP recovery; repair that file before adding another scope",
                path.display()
            );
        }
    }
    Ok(())
}

/// Read-only preflight, used both before review and again before any install lane.
pub fn preflight(server: Server, destination: &SkillDestination) -> Result<()> {
    ensure!(
        destination.agents.contains(&SkillAgent::Pi),
        "MCP setup needs Pi selected; other agent adapters are not yet verified (use --agent pi)"
    );
    let global = destination.home.join(".pi/agent");
    ensure!(std::env::var_os("PI_CODING_AGENT_DIR").is_none_or(|value| value.is_empty() || Path::new(&value) == global),
        "custom PI_CODING_AGENT_DIR is not yet supported for MCP setup; use the default Pi agent directory");
    let target = config_path(destination);
    validate_entries(server, destination, &target)?;
    builtin_mcp_enabled(destination)?;
    crate::ownership::InstallState::inspect(&destination.home).map_err(anyhow::Error::msg)?;
    Ok(())
}

/// Pi's built-in MCP support is off when an extension such as pi-mcp-adapter
/// registers `/mcp`, or when settings disable `builtin:mcp`.
pub fn builtin_mcp_enabled(destination: &SkillDestination) -> Result<()> {
    for root in [
        destination.home.join(".pi/agent"),
        destination.project_root.join(".pi"),
    ] {
        let path = root.join("settings.json");
        let (_, value) = read_object(&path)?;
        ensure!(
            value.get("packages").is_none_or(Value::is_array),
            "{}: packages must be an array",
            path.display()
        );
        let strings = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|entry| {
                    entry
                        .as_str()
                        .or_else(|| entry.get("source").and_then(Value::as_str))
                })
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        ensure!(
            !strings("packages")
                .iter()
                .chain(&strings("extensions"))
                .any(|s| s.contains("mcp-adapter")),
            "{} installs pi-mcp-adapter, which replaces Pi's built-in MCP support; run `loom update` to migrate to it",
            path.display()
        );
        ensure!(
            !strings("extensions").iter().any(|s| s == "-builtin:mcp"),
            "{} disables Pi's built-in MCP support (-builtin:mcp); enable it with pi config",
            path.display()
        );
    }
    Ok(())
}

/// Configuration presence is not live health. No MCP processes are launched.
pub fn configured(server: Server, destination: &SkillDestination, system: &dyn System) -> bool {
    let path = config_path(destination);
    preflight(server, destination).is_ok()
        && server.prerequisites_present(system)
        && path.is_file()
        && read_object(&path).is_ok_and(|(_, value)| {
            value
                .get(SERVERS_KEY)
                .and_then(|servers| servers.get(server.name()))
                .is_some_and(|entry| server.compatible_entry(entry))
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

pub(crate) fn write_config(path: &Path, before: &str, after: &str) -> Result<()> {
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

/// Pi parses mcp.json as strict JSON: splice an existing `mcpServers` value in
/// place, otherwise rewrite the whole object (jsonc insertion leaves a trailing comma).
fn render_config(before: &str, value: &Value) -> Result<String> {
    if crate::jsonc::get(before, SERVERS_KEY).is_some() {
        return crate::jsonc::set(before, SERVERS_KEY, &value[SERVERS_KEY]);
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}

pub fn entry_digest(value: &Value) -> String {
    crate::ownership::hex(&Sha256::digest(
        serde_json::to_vec(value).expect("JSON serializes"),
    ))
}

pub fn install(server: Server, destination: &SkillDestination, system: &dyn System) -> Result<()> {
    preflight(server, destination)?;
    let name = server.name();
    ensure!(
        server.prerequisites_present(system),
        "{name} MCP prerequisites missing; retry setup to install Pi and the server's prerequisites"
    );
    let path = config_path(destination);
    recover_config(&path)?;
    let (before, mut value) = read_object(&path)?;
    let key = SERVERS_KEY;
    if value
        .get(key)
        .and_then(|servers| servers.get(name))
        .is_some()
    {
        return Ok(());
    }
    let entry = server.entry();
    if value.get(key).is_none_or(Value::is_null) {
        value[key] = json!({});
    }
    value[key][name] = entry.clone();
    let after = render_config(&before, &value)?;
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
        crate::ownership::OwnershipScope::Global => format!("mcp-server:{name}"),
        crate::ownership::OwnershipScope::Project { root } => {
            format!("project:{}:mcp-server:{name}", root.display())
        }
    };
    let mut depends_on = vec!["core:loom".into(), "core:mise".into(), "tool:pi".into()];
    if let Some(tool) = server.tool_dependency() {
        depends_on.push(tool.into());
    }
    state.record(crate::ownership::OwnedResource {
        id,
        scope,
        depends_on,
        receipts: vec![crate::ownership::Receipt::McpEntry {
            path: path.clone(),
            name: name.into(),
            digest: entry_digest(&entry),
        }],
    });
    write_config(&path, &before, &after)?;
    if let Err(error) = state.save(&destination.home) {
        // Do not leave a new, unowned entry after an ordinary ledger write failure.
        remove_entry(&path, name, &entry_digest(&entry))?;
        bail!("MCP ownership could not be saved: {error}");
    }
    Ok(())
}

pub fn entry_status(path: &Path, name: &str, digest: &str) -> crate::uninstall::ReceiptStatus {
    use crate::uninstall::ReceiptStatus;
    match read_object(path) {
        Ok((_, value))
            if value
                .get(SERVERS_KEY)
                .is_some_and(|v| !v.is_null() && !v.is_object()) =>
        {
            ReceiptStatus::Modified
        }
        Ok((_, value)) => match value.get(SERVERS_KEY).and_then(|v| v.get(name)) {
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
    let key = SERVERS_KEY;
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
    let after = render_config(&before, &value)?;
    write_config(path, &before, &after)
}
