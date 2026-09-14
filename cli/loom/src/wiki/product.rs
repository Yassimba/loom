use super::registry::WikiRegistry;
use super::{WikiOperation, CONFLUENCE_KEY, CONFLUENCE_SKILL, PRODUCT_KEY, PYTHON_KEY, QMD_KEY};
use crate::{CommandSpec, System};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub(super) struct ReviewedPlan {
    schema: String,
    status: String,
    #[serde(default)]
    changed_paths: Vec<String>,
    approved_plan_sha256: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct InspectedTransaction {
    schema: String,
    valid: bool,
    approval_sha256: String,
    #[serde(default)]
    changed_paths: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct DoctorReport {
    schema: String,
    ok: bool,
}

pub(super) fn run_checked(system: &dyn System, command: &CommandSpec) -> Result<String> {
    let result = system.run(command)?;
    if !result.success {
        bail!(
            "{} failed: {}",
            command.display(),
            crate::install::command_failure_message(&result)
        );
    }
    Ok(result.stdout)
}

pub(super) fn doctor_ok(system: &dyn System, product: &Path, vault: &Path) -> bool {
    system
        .run_probe(&python_command(
            product,
            vec![
                "doctor".into(),
                "--vault".into(),
                vault.display().to_string(),
            ],
        ))
        .ok()
        .filter(|result| result.success)
        .and_then(|result| serde_json::from_str::<DoctorReport>(&result.stdout).ok())
        .is_some_and(|report| report.schema == "claude-obsidian.doctor.v1" && report.ok)
}

pub(super) fn product_root(system: &dyn System) -> Result<PathBuf> {
    let output = run_checked(system, &CommandSpec::new("mise", ["where", PRODUCT_KEY]))?;
    let path = PathBuf::from(output.trim());
    anyhow::ensure!(
        path.is_absolute(),
        "mise returned an invalid claude-obsidian root"
    );
    Ok(path)
}

pub(super) fn operation_stamp(prefix: &str) -> (String, String) {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = epoch.div_euclid(86_400);
    let second = epoch.rem_euclid(86_400);
    // Howard Hinnant's civil-from-days conversion, with Unix epoch offset.
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    let hour = second / 3_600;
    let minute = second % 3_600 / 60;
    let second = second % 60;
    let generated = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z");
    let operation_id = format!("loom-{prefix}-{}", generated.replace([':', '-'], ""));
    (generated, operation_id)
}

pub(super) fn python_command(
    product: &Path,
    args: impl IntoIterator<Item = String>,
) -> CommandSpec {
    let mut argv = vec![
        "exec".into(),
        PYTHON_KEY.into(),
        "--".into(),
        "python".into(),
        product
            .join("scripts/claude-obsidian.py")
            .display()
            .to_string(),
    ];
    argv.extend(args);
    CommandSpec::new("mise", argv)
}

pub(super) type Confirm<'a> = &'a mut dyn FnMut(&str, &[String]) -> Result<bool>;

pub(super) fn approve_plan(plan: &ReviewedPlan, confirm: Confirm<'_>) -> Result<bool> {
    if plan.status == "noop" {
        return Ok(true);
    }
    confirm("Apply this exact reviewed Vault plan?", &plan.changed_paths)
}

pub(super) fn initialize_vault(
    system: &dyn System,
    product: &Path,
    operation: &WikiOperation,
    vault: &Path,
    confirm: Confirm<'_>,
) -> Result<bool> {
    match operation {
        WikiOperation::Create if vault.exists() => {
            anyhow::ensure!(
                vault.is_dir()
                    && vault.join(".obsidian").is_dir()
                    && vault.join(".claude-obsidian.json").is_file(),
                "Create requires a new path; an existing path is resumed only when it is already a claude-obsidian Vault"
            );
            anyhow::ensure!(
                doctor_ok(system, product, vault),
                "existing partial Vault failed claude-obsidian doctor; use Adopt or inspect the Vault before repair"
            );
            return Ok(true);
        }
        WikiOperation::Create => {}
        WikiOperation::Adopt => anyhow::ensure!(
            vault.is_dir() && vault.join(".obsidian").is_dir(),
            "Adopt requires an existing Obsidian Vault with .obsidian/"
        ),
        _ => unreachable!(),
    }
    let verb = if *operation == WikiOperation::Create {
        "init"
    } else {
        "adopt"
    };
    let (generated_at, operation_id) = operation_stamp(verb);
    let common = vec![
        verb.to_string(),
        vault.display().to_string(),
        "--generated-at".into(),
        generated_at,
        "--operation-id".into(),
        operation_id,
    ];
    let output = run_checked(system, &python_command(product, common.clone()))?;
    let plan: ReviewedPlan =
        serde_json::from_str(&output).context("invalid claude-obsidian plan JSON")?;
    let expected_schema = if *operation == WikiOperation::Create {
        "claude-obsidian.initialization-plan.v1"
    } else {
        "claude-obsidian.adoption-plan.v1"
    };
    anyhow::ensure!(
        plan.schema == expected_schema,
        "unsupported claude-obsidian plan schema: {}",
        plan.schema
    );
    anyhow::ensure!(
        matches!(plan.status.as_str(), "dry-run" | "noop"),
        "unsupported claude-obsidian plan status: {}",
        plan.status
    );
    if !approve_plan(&plan, confirm)? {
        return Ok(false);
    }
    if plan.status != "noop" {
        let approval = plan
            .approved_plan_sha256
            .context("claude-obsidian plan did not include an approval hash")?;
        let mut apply = common;
        apply.extend(["--approved-plan-sha256".into(), approval, "--apply".into()]);
        run_checked(system, &python_command(product, apply))?;
    }
    Ok(true)
}

pub(super) fn sha256(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

pub(super) fn ensure_pi_ignored(
    system: &dyn System,
    home: &Path,
    product: &Path,
    vault: &Path,
    confirm: Confirm<'_>,
) -> Result<bool> {
    let path = vault.join(".gitignore");
    let (before, existed) = match fs::read(&path) {
        Ok(content) => (content, true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Vec::new(), false),
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", path.display())),
    };
    let before_text = std::str::from_utf8(&before)
        .with_context(|| format!("{} is not UTF-8; refusing to replace it", path.display()))?;
    if before_text.lines().any(|line| line.trim() == ".pi/") {
        return Ok(true);
    }
    let mut after = before_text.to_owned();
    if !after.is_empty() && !after.ends_with('\n') {
        after.push('\n');
    }
    after.push_str("\n# Pi project packages are machine-local.\n.pi/\n");
    let operation = serde_json::json!({
        "schema": "claude-obsidian.transaction.v1",
        "operation_id": format!("loom-pi-ignore-{}", std::process::id()),
        "operation_type": "setup",
        "expected_hashes": {".gitignore": if existed { serde_json::Value::String(sha256(&before)) } else { serde_json::Value::Null }},
        "writes": [{"path": ".gitignore", "mode": if existed {"replace"} else {"create"}, "content": after, "sha256": sha256(after.as_bytes())}],
        "address_requests": [],
        "source_manifest_updates": {}
    });
    let bundle = home
        .join(".cache")
        .join("loom")
        .join(format!("wiki-pi-ignore-{}.json", std::process::id()));
    fs::create_dir_all(bundle.parent().context("bundle parent")?)?;
    fs::write(&bundle, serde_json::to_vec_pretty(&operation)?)?;
    let args = vec![
        "transaction".into(),
        "inspect".into(),
        bundle.display().to_string(),
        "--vault".into(),
        vault.display().to_string(),
    ];
    let result = (|| {
        let output = run_checked(system, &python_command(product, args))?;
        let inspected: InspectedTransaction =
            serde_json::from_str(&output).context("invalid transaction inspection JSON")?;
        anyhow::ensure!(
            inspected.schema == "claude-obsidian.transaction-plan.v1" && inspected.valid,
            "unsupported or invalid transaction inspection"
        );
        anyhow::ensure!(
            inspected.changed_paths == [".gitignore"],
            "ignore transaction changed unexpected paths"
        );
        let approval = inspected.approval_sha256;
        if !confirm(
            "Apply this reviewed machine-local ignore rule?",
            &[".gitignore: add .pi/".into()],
        )? {
            return Ok(false);
        }
        run_checked(
            system,
            &python_command(
                product,
                vec![
                    "transaction".into(),
                    "apply".into(),
                    bundle.display().to_string(),
                    "--vault".into(),
                    vault.display().to_string(),
                    "--approved-plan-sha256".into(),
                    approval,
                ],
            ),
        )?;
        Ok(true)
    })();
    let _ = fs::remove_file(bundle);
    result
}

pub(super) fn project_package_lines(listed: &str) -> impl Iterator<Item = &str> {
    listed
        .split_once("Project packages:")
        .map(|(_, project)| project)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
}

pub(super) fn has_project_packages(listed: &str, product: &Path, feynman: bool) -> bool {
    let lines = project_package_lines(listed).collect::<Vec<_>>();
    let core = product
        .canonicalize()
        .unwrap_or_else(|_| product.to_path_buf())
        .display()
        .to_string();
    lines.iter().any(|line| *line == core)
        && (!feynman
            || lines
                .iter()
                .any(|line| line.starts_with("npm:@companion-ai/feynman@")))
}

pub(super) fn stale_core_sources(vault: &Path, product: &Path) -> Result<Vec<String>> {
    let settings_path = vault.join(".pi/settings.json");
    let content = match fs::read_to_string(&settings_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot read {}", settings_path.display()));
        }
    };
    let settings: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("invalid Pi settings: {}", settings_path.display()))?;
    let current = product
        .canonicalize()
        .unwrap_or_else(|_| product.to_path_buf());
    Ok(settings["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|source| source.as_str())
        .filter(|source| source.contains("github-agrici-daniel-claude-obsidian"))
        .filter(|source| {
            let path = Path::new(source);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                vault.join(".pi").join(path)
            };
            resolved.canonicalize().unwrap_or(resolved) != current
        })
        .map(str::to_owned)
        .collect())
}

pub(super) fn prune_stale_core_sources(vault: &Path, stale: &[String]) -> Result<()> {
    if stale.is_empty() {
        return Ok(());
    }
    let path = vault.join(".pi/settings.json");
    let mut settings: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path)?)
        .with_context(|| format!("invalid Pi settings: {}", path.display()))?;
    let packages = settings["packages"]
        .as_array_mut()
        .context("Pi settings packages must be an array")?;
    packages.retain(|source| {
        source
            .as_str()
            .is_none_or(|source| !stale.iter().any(|stale| stale == source))
    });
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let mut content = serde_json::to_vec_pretty(&settings)?;
    content.push(b'\n');
    fs::write(&temporary, content)?;
    fs::rename(&temporary, &path)?;
    Ok(())
}

pub(super) fn feynman_spec() -> Result<String> {
    crate::Catalog::embedded()?
        .resources
        .into_iter()
        .find(|resource| resource.install_target == "@companion-ai/feynman")
        .map(|resource| resource.pi_install_spec())
        .context("embedded catalog has no Feynman package")
}

pub(super) fn install_packages(
    system: &dyn System,
    product: &Path,
    vault: &Path,
    feynman: bool,
) -> Result<()> {
    let core = product.display().to_string();
    run_checked(
        system,
        &CommandSpec::new("pi", ["install", "-l", "--approve", &core]).in_dir(vault),
    )?;
    if feynman {
        let spec = feynman_spec()?;
        run_checked(
            system,
            &CommandSpec::new("pi", ["install", "-l", "--approve", &spec]).in_dir(vault),
        )?;
    }
    let listed = run_checked(
        system,
        &CommandSpec::new("pi", ["list", "--approve"]).in_dir(vault),
    )?;
    anyhow::ensure!(
        has_project_packages(&listed, product, feynman),
        "Pi did not report the exact selected packages under Project packages; rerun `loom wiki` and choose Repair"
    );
    let stale = stale_core_sources(vault, product)?;
    prune_stale_core_sources(vault, &stale)?;
    let listed = run_checked(
        system,
        &CommandSpec::new("pi", ["list", "--approve"]).in_dir(vault),
    )?;
    anyhow::ensure!(
        has_project_packages(&listed, product, feynman),
        "removing a stale core reference disturbed the current Vault packages; rerun `loom wiki` and choose Repair"
    );
    Ok(())
}

pub(super) fn wiki_tool_keys(qmd: bool, confluence: bool) -> Vec<String> {
    let mut tools = vec![
        PYTHON_KEY.into(),
        crate::manifest::PI_TOOL_KEY.into(),
        PRODUCT_KEY.into(),
    ];
    if qmd {
        tools.push(QMD_KEY.into());
    }
    if confluence {
        tools.push(CONFLUENCE_KEY.into());
    }
    tools
}

pub(super) fn wiki_skill_names(confluence: bool) -> Vec<String> {
    confluence
        .then(|| CONFLUENCE_SKILL.into())
        .into_iter()
        .collect()
}

pub(super) fn qmd_index(vault: &Path) -> String {
    format!(
        "loom-wiki-{}",
        &sha256(vault.as_os_str().as_encoded_bytes())[..16]
    )
}

pub(super) fn setup_qmd(system: &dyn System, vault: &Path) -> Result<String> {
    let skill = vault.join(".agents/skills/qmd/SKILL.md");
    if !skill.is_file() {
        run_checked(
            system,
            &CommandSpec::new("qmd", ["skill", "install"]).in_dir(vault),
        )?;
    }

    let index = qmd_index(vault);
    let command = |args: &[&str]| {
        CommandSpec::new(
            "qmd",
            ["--index", index.as_str()]
                .into_iter()
                .chain(args.iter().copied()),
        )
        .in_dir(vault)
    };
    let exists = system.run(&command(&["collection", "show", "vault"]))?;
    if !exists.success {
        run_checked(
            system,
            &command(&[
                "collection",
                "add",
                ".",
                "--name",
                "vault",
                "--mask",
                "**/*.md",
            ]),
        )?;
    }
    run_checked(system, &command(&["update"]))?;
    let status = run_checked(system, &command(&["status"]))?;
    let total = qmd_document_count(&status)?;
    if total == 0 {
        return Ok("Search check skipped: the Vault index is empty.".into());
    }
    run_checked(system, &command(&["pull", "--progress"]))?;
    let embed_command = command(&["embed"]);
    let embedded = system.run(&embed_command)?;
    if !embedded.success {
        bail!(
            "{} failed: {}",
            embed_command.display(),
            crate::install::command_failure_message(&embedded)
        );
    }
    let embed_output = format!("{}\n{}", embedded.stdout, embedded.stderr);
    anyhow::ensure!(
        !embed_output.contains("Another embed process is already running")
            && !embed_output.contains("chunks still failed after retries"),
        "QMD embeddings are incomplete or busy; rerun Wiki repair"
    );
    let status = run_checked(system, &command(&["status"]))?;
    qmd_document_count(&status)?;
    anyhow::ensure!(
        !status.contains("Pending:"),
        "QMD still has pending embeddings; rerun Wiki repair"
    );
    let checked = system.run_controlled(
        &command(&[
            "query",
            "What knowledge and topics are documented in this wiki?",
            "-c",
            "vault",
            "-C",
            "4",
            "-n",
            "1",
            "--format",
            "files",
        ]),
        std::time::Duration::from_secs(120),
        &std::sync::atomic::AtomicBool::new(false),
    );
    // Never print search results or tool errors: they may contain private notes.
    Ok(if checked.is_ok_and(|result| result.success) {
        "Search check completed. Disk caches are prepared; models reload for later CLI searches."
    } else {
        "Search index prepared. Warning: first-search check failed or timed out; try a search later."
    }.into())
}

pub(super) fn qmd_document_count(status: &str) -> Result<usize> {
    status
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("Total:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
        .context("QMD status did not report an indexed document count; search readiness is unknown")
}

pub(super) fn offer_global_feynman_migration(
    system: &dyn System,
    vault: &Path,
    yes: bool,
) -> Result<()> {
    if yes {
        return Ok(()); // scripted setup never removes an existing global package
    }
    let listed = run_checked(
        system,
        &CommandSpec::new("pi", ["list", "--approve"]).in_dir(vault),
    )?;
    let global = listed
        .split("Project packages:")
        .next()
        .is_some_and(|user| {
            user.lines()
                .map(str::trim)
                .any(|line| line.starts_with("npm:@companion-ai/feynman@"))
        });
    if global
        && crate::wiki_tui::confirm(
            "Remove the global Feynman package?",
            &["Vault-local Feynman is verified.".into()],
        )?
    {
        run_checked(
            system,
            &CommandSpec::new("pi", ["remove", "npm:@companion-ai/feynman"]),
        )?;
    }
    Ok(())
}

pub(super) fn canonical_vault(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("Vault is unavailable: {}", path.display()))
}

pub(crate) fn absolute_vault_target(
    system: &dyn System,
    path: &Path,
    create: bool,
) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        system
            .current_dir()
            .context("current directory is unavailable")?
            .join(path)
    };
    if !create {
        return canonical_vault(&candidate);
    }
    let name = candidate
        .file_name()
        .context("Create requires a Vault folder name")?;
    let parent = candidate
        .parent()
        .context("Create requires a parent directory")?
        .canonicalize()
        .with_context(|| format!("Vault parent is unavailable: {}", candidate.display()))?;
    Ok(parent.join(name))
}

pub(super) fn canonicalize_with_missing_tail(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut missing = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name().map(ToOwned::to_owned) else {
            return path.to_path_buf();
        };
        missing.push(name);
        if !existing.pop() {
            return path.to_path_buf();
        }
    }
    let Ok(mut canonical) = existing.canonicalize() else {
        return path.to_path_buf();
    };
    for name in missing.into_iter().rev() {
        canonical.push(name);
    }
    canonical
}

pub(super) fn registry_match_path(
    system: &dyn System,
    registry: &WikiRegistry,
    path: &Path,
) -> PathBuf {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        system
            .current_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(path)
    };
    let candidate = canonicalize_with_missing_tail(&candidate);
    registry
        .vaults
        .iter()
        .find(|record| {
            record.path == candidate
                || record
                    .path
                    .canonicalize()
                    .is_ok_and(|registered| registered == candidate)
        })
        .map(|record| record.path.clone())
        .unwrap_or(candidate)
}
