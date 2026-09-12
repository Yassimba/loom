use super::product::{
    doctor_ok, has_project_packages, install_packages, product_root, project_package_lines,
    setup_qmd,
};
use super::registry::{VaultRecord, WikiRegistry};
use super::{CONFLUENCE_KEY, CONFLUENCE_SKILL, PRODUCT_KEY, PYTHON_KEY, QMD_KEY};
use crate::ui::{Mark, Out};
use crate::{manifest, CommandSpec, System};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn status_registered(system: &(dyn System + Sync)) -> bool {
    use crate::ui::tidy_path;

    let style = Out::detect();
    style.section("Wiki");
    let Some(home) = system.home_dir() else {
        style.row(Mark::Bad, "Vaults", "home directory is unavailable");
        return false;
    };
    let registry = match WikiRegistry::load(&home) {
        Ok(registry) => registry,
        Err(error) => {
            style.row(Mark::Bad, "registry", error.to_string());
            return false;
        }
    };
    if registry.vaults.is_empty() {
        style.hint("no registered Vaults — run `loom wiki` to set one up");
        return true;
    }
    let mut healthy = true;
    let mut shared = Vec::new();
    for record in registry.vaults {
        let health = inspect_vault(system, &record);
        style.row(
            if health.healthy { Mark::Ok } else { Mark::Bad },
            &tidy_path(&record.path, &home),
            "",
        );
        for (mark, label, detail) in health.rows {
            if SHARED_WIKI_ROWS.contains(&label) {
                if !shared.iter().any(|(_, name, _)| *name == label) {
                    shared.push((mark, label, detail));
                }
                continue;
            }
            style.row(mark, label, detail);
        }
        healthy &= health.healthy;
    }
    for (mark, label, detail) in shared {
        style.row(mark, label, detail);
    }
    healthy
}

pub(super) const SHARED_WIKI_ROWS: [&str; 3] = ["QMD (shared)", "Confluence (shared)", "Obsidian"];

pub(crate) struct VaultHealth {
    pub healthy: bool,
    pub rows: Vec<(crate::ui::Mark, &'static str, String)>,
}

/// Inspect only the chosen Vault. Shared by the status report and Wiki picker.
pub(crate) fn inspect_vault(system: &(dyn System + Sync), record: &VaultRecord) -> VaultHealth {
    use crate::ui::Mark;
    if !record.path.is_dir() {
        return VaultHealth {
            healthy: false,
            rows: vec![(Mark::Bad, "Vault", "missing; not recreated".into())],
        };
    }
    let product = product_root(system).ok();
    let selected = system
        .home_dir()
        .map(|home| manifest::selected_keys(&home))
        .unwrap_or_default();
    let pins_ready = selected.iter().any(|key| key == PRODUCT_KEY)
        && selected.iter().any(|key| key == PYTHON_KEY)
        && selected
            .iter()
            .any(|key| key == crate::manifest::PI_TOOL_KEY);
    let mut rows = Vec::new();
    let marker = record.path.join(".claude-obsidian.json").is_file();
    let doctor = product
        .as_ref()
        .is_some_and(|root| doctor_ok(system, root, &record.path));
    // Read the Vault's own Pi settings first; `pi list` boots Node and was
    // the slow part of every health check.
    let packages = crate::app::pi_packages_listing(
        &record.path.join(".pi/settings.json"),
        "Project packages:",
    )
    .or_else(|| {
        system
            .run_probe(&CommandSpec::new("pi", ["list", "--approve"]).in_dir(&record.path))
            .ok()
            .filter(|result| result.success)
            .map(|result| result.stdout)
    })
    .unwrap_or_default();
    let core = product
        .as_ref()
        .is_some_and(|path| has_project_packages(&packages, path, false));
    let feynman =
        project_package_lines(&packages).any(|line| line.starts_with("npm:@companion-ai/feynman@"));
    let vault_skills = record.path.join(".agents/skills");
    let qmd_tool = system.command_exists("qmd");
    let qmd = selected.iter().any(|key| key == QMD_KEY)
        && qmd_tool
        && vault_skills.join("qmd/SKILL.md").is_file();
    let confluence_tool = system.command_exists("cme");
    let confluence = selected.iter().any(|key| key == CONFLUENCE_KEY)
        && confluence_tool
        && vault_skills
            .join(CONFLUENCE_SKILL)
            .join("SKILL.md")
            .is_file();
    let core_ready = marker && doctor && pins_ready && core;
    let ok = core_ready
        && (!record.qmd || qmd)
        && (!record.feynman || feynman)
        && (!record.confluence || confluence);
    rows.push((
        if core_ready { Mark::Ok } else { Mark::Bad },
        "claude-obsidian",
        if core_ready {
            "ready".into()
        } else {
            format!(
                "repair needed — Vault package {}; marker {}; prerequisites {}; doctor {}",
                if core { "installed" } else { "missing" },
                if marker { "present" } else { "missing" },
                if pins_ready { "ready" } else { "missing" },
                if doctor { "ok" } else { "failed" },
            )
        },
    ));
    for (label, installed, required, detail) in [
        ("Feynman", feynman, record.feynman, "Vault Pi package"),
        ("qmd", qmd, record.qmd, "tool + Vault skill"),
        (
            "Confluence",
            confluence,
            record.confluence,
            "tool + Vault skill",
        ),
    ] {
        rows.push((
            if required && installed {
                Mark::Ok
            } else if required {
                Mark::Bad
            } else {
                Mark::Off
            },
            label,
            if required && installed {
                "ready".into()
            } else if required {
                format!("missing — {detail}; run `loom wiki` to repair")
            } else {
                "not selected for this Vault".into()
            },
        ));
    }
    for (label, installed) in [
        ("QMD (shared)", qmd_tool),
        ("Confluence (shared)", confluence_tool),
    ] {
        rows.push((
            if installed { Mark::Ok } else { Mark::Off },
            label,
            if installed {
                "installed on this machine; does not imply Vault configuration"
            } else {
                "not installed on this machine"
            }
            .into(),
        ));
    }
    let obsidian = obsidian_installed(system);
    rows.push((
        if obsidian { Mark::Ok } else { Mark::Off },
        "Obsidian",
        if obsidian {
            "shared desktop app available"
        } else {
            "optional; run `loom wiki` for install guidance"
        }
        .into(),
    ));
    VaultHealth { healthy: ok, rows }
}

pub fn update_registered(system: &(dyn System + Sync), interactive: bool, out: &Out) -> bool {
    let Some(home) = system.home_dir() else {
        out.row(Mark::Bad, "Wiki", "home directory is unavailable");
        return false;
    };
    let registry = match WikiRegistry::load(&home) {
        Ok(registry) => registry,
        Err(error) => {
            out.row(Mark::Bad, "Wiki registry", error.to_string());
            return false;
        }
    };
    let product = match product_root(system) {
        Ok(product) => product,
        Err(_) if registry.vaults.is_empty() => return true,
        Err(error) => {
            out.row(Mark::Bad, "Wiki product", error.to_string());
            out.note("rerun `loom wiki` to repair prerequisites");
            return false;
        }
    };
    let mut missing_tools = Vec::new();
    if registry.vaults.iter().any(|vault| vault.qmd) && !system.command_exists("qmd") {
        missing_tools.push(QMD_KEY.into());
    }
    if registry.vaults.iter().any(|vault| vault.confluence) && !system.command_exists("cme") {
        missing_tools.push(CONFLUENCE_KEY.into());
    }
    if !missing_tools.is_empty() {
        if let Err(error) = manifest::sync_selected(system, &missing_tools) {
            out.row(Mark::Bad, "Wiki tools", &error);
            return false;
        }
    }
    let mut healthy = true;
    let vault_count = registry.vaults.len();
    for (index, record) in registry.vaults.into_iter().enumerate() {
        let label = crate::ui::tidy_path(&record.path, &home);
        if !record.path.is_dir() {
            out.row(
                Mark::Bad,
                "Wiki",
                format!("{label} · missing; not recreated"),
            );
            healthy = false;
            continue;
        }
        let vault_name = record
            .path
            .file_name()
            .unwrap_or(record.path.as_os_str())
            .to_string_lossy();
        let activity = format!("Vault {}/{} · {vault_name}", index + 1, vault_count);
        let refreshed = crate::wiki_progress::run(system, interactive, &activity, |system, _| {
            install_packages(system, &product, &record.path, record.feynman)?;
            if record.qmd {
                setup_qmd(system, &record.path)
            } else {
                Ok(String::new())
            }
        });
        match refreshed {
            Ok(note) => {
                out.row(Mark::Ok, "Wiki", format!("{label} · refreshed"));
                if !note.is_empty() {
                    out.note(note);
                }
            }
            Err(error) => {
                out.row(Mark::Bad, "Wiki", format!("{label} · {error}"));
                healthy = false;
            }
        }
    }
    healthy
}

pub(crate) fn obsidian_installed(system: &dyn System) -> bool {
    if system.command_exists("obsidian") {
        return true;
    }
    if std::env::var_os("WSL_DISTRO_NAME").is_some()
        && system.command_exists("cmd.exe")
        && system
            .run_probe(&CommandSpec::new(
                "cmd.exe",
                ["/C", "where", "Obsidian.exe"],
            ))
            .is_ok_and(|result| result.success)
    {
        return true;
    }
    system.home_dir().is_some_and(|home| {
        [
            PathBuf::from("/Applications/Obsidian.app"),
            home.join("Applications/Obsidian.app"),
            home.join("AppData/Local/Obsidian/Obsidian.exe"),
        ]
        .iter()
        .any(|path| path.exists())
    })
}

pub(super) fn percent_encode_path(path: &Path) -> String {
    path.display()
        .to_string()
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

pub(super) fn open_url(system: &dyn System, url: String) -> Result<()> {
    let command = if cfg!(target_os = "macos") {
        CommandSpec::new("open", [url])
    } else if cfg!(windows) {
        CommandSpec::new("cmd", ["/C".into(), "start".into(), "".into(), url])
    } else if std::env::var_os("WSL_DISTRO_NAME").is_some() && system.command_exists("wslview") {
        CommandSpec::new("wslview", [url])
    } else {
        CommandSpec::new("xdg-open", [url])
    };
    system.spawn_detached(&command)
}

pub(super) fn open_obsidian(system: &dyn System, vault: &Path) -> Result<bool> {
    let url = format!("obsidian://open?path={}", percent_encode_path(vault));
    open_url(system, url)?;
    Ok(true)
}

pub(super) fn offer_finish_actions(system: &dyn System, vault: &Path) -> Result<()> {
    let choices = ["Done", "Open in Obsidian", "Launch Pi in the Vault"];
    loop {
        match crate::wiki_tui::select("Vault ready", &choices)? {
            None | Some(0) => return Ok(()),
            Some(1) => {
                open_obsidian(system, vault)?;
            }
            Some(2) => system.spawn_detached(
                &CommandSpec::new("pi", std::iter::empty::<&str>()).in_dir(vault),
            )?,
            _ => unreachable!(),
        }
    }
}
