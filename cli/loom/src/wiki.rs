use crate::ui::{Mark, Out};
use crate::{manifest, CommandSpec, System};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const PRODUCT_KEY: &str = "github:AgriciDaniel/claude-obsidian";
pub const PYTHON_KEY: &str = "python";
const QMD_KEY: &str = "npm:@tobilu/qmd";
const CONFLUENCE_KEY: &str = "pipx:confluence-markdown-exporter";
const CONFLUENCE_SKILL: &str = "confluence-export";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WikiOperation {
    Create,
    Adopt,
    Repair,
    Status,
    Unregister,
    Open,
    Launch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WikiRequest {
    pub operation: WikiOperation,
    pub vault: PathBuf,
    pub feynman: bool,
    pub confluence: bool,
    pub yes: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultRecord {
    pub path: PathBuf,
    pub feynman: bool,
    #[serde(default)]
    pub confluence: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiRegistry {
    schema_version: u32,
    pub vaults: Vec<VaultRecord>,
}

impl Default for WikiRegistry {
    fn default() -> Self {
        Self {
            schema_version: 1,
            vaults: Vec::new(),
        }
    }
}

impl WikiRegistry {
    fn path(home: &Path) -> PathBuf {
        home.join(".config").join("loom").join("wiki-vaults.json")
    }

    pub fn load(home: &Path) -> Result<Self> {
        let path = Self::path(home);
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        };
        let registry: Self = serde_json::from_str(&content)
            .with_context(|| format!("invalid Wiki registry: {}", path.display()))?;
        anyhow::ensure!(
            registry.schema_version == 1,
            "unsupported Wiki registry schema"
        );
        anyhow::ensure!(
            registry.vaults.iter().all(|vault| vault.path.is_absolute()),
            "Wiki registry paths must be absolute"
        );
        Ok(registry)
    }

    pub fn save(&mut self, home: &Path) -> Result<()> {
        self.vaults.sort_by(|a, b| a.path.cmp(&b.path));
        self.vaults.dedup_by(|a, b| {
            if a.path == b.path {
                a.feynman = b.feynman;
                a.confluence = b.confluence;
                true
            } else {
                false
            }
        });
        let path = Self::path(home);
        let parent = path.parent().context("Wiki registry has no parent")?;
        fs::create_dir_all(parent)?;
        let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn unregister(&mut self, path: &Path) -> bool {
        let before = self.vaults.len();
        self.vaults.retain(|record| record.path != path);
        self.vaults.len() != before
    }

    fn register(&mut self, path: PathBuf, feynman: bool, confluence: bool) {
        if let Some(record) = self.vaults.iter_mut().find(|record| record.path == path) {
            record.feynman = feynman;
            record.confluence = confluence;
        } else {
            self.vaults.push(VaultRecord {
                path,
                feynman,
                confluence,
            });
        }
    }
}

#[derive(Deserialize)]
struct ReviewedPlan {
    schema: String,
    status: String,
    #[serde(default)]
    changed_paths: Vec<String>,
    approved_plan_sha256: Option<String>,
}

#[derive(Deserialize)]
struct InspectedTransaction {
    schema: String,
    valid: bool,
    approval_sha256: String,
    #[serde(default)]
    changed_paths: Vec<String>,
}

#[derive(Deserialize)]
struct DoctorReport {
    schema: String,
    ok: bool,
}

fn run_checked(system: &dyn System, command: &CommandSpec) -> Result<String> {
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

fn doctor_ok(system: &dyn System, product: &Path, vault: &Path) -> bool {
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

fn product_root(system: &dyn System) -> Result<PathBuf> {
    let output = run_checked(system, &CommandSpec::new("mise", ["where", PRODUCT_KEY]))?;
    let path = PathBuf::from(output.trim());
    anyhow::ensure!(
        path.is_absolute(),
        "mise returned an invalid claude-obsidian root"
    );
    Ok(path)
}

fn operation_stamp(prefix: &str) -> (String, String) {
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

fn python_command(product: &Path, args: impl IntoIterator<Item = String>) -> CommandSpec {
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

type Confirm<'a> = &'a mut dyn FnMut(&str, &[String]) -> Result<bool>;

fn approve_plan(plan: &ReviewedPlan, confirm: Confirm<'_>) -> Result<bool> {
    if plan.status == "noop" {
        return Ok(true);
    }
    confirm("Apply this exact reviewed Vault plan?", &plan.changed_paths)
}

fn initialize_vault(
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

fn sha256(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

fn ensure_pi_ignored(
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

fn project_package_lines(listed: &str) -> impl Iterator<Item = &str> {
    listed
        .split_once("Project packages:")
        .map(|(_, project)| project)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
}

fn has_project_packages(listed: &str, product: &Path, feynman: bool) -> bool {
    let lines = project_package_lines(listed).collect::<Vec<_>>();
    let core = product.display().to_string();
    lines.iter().any(|line| *line == core)
        && (!feynman
            || lines
                .iter()
                .any(|line| line.starts_with("npm:@companion-ai/feynman@")))
}

fn stale_core_sources(vault: &Path, product: &Path) -> Result<Vec<String>> {
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

fn prune_stale_core_sources(vault: &Path, stale: &[String]) -> Result<()> {
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

fn feynman_spec() -> Result<String> {
    crate::Catalog::embedded()?
        .resources
        .into_iter()
        .find(|resource| resource.install_target == "@companion-ai/feynman")
        .map(|resource| resource.pi_install_spec())
        .context("embedded catalog has no Feynman package")
}

fn install_packages(
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

fn wiki_tool_keys(confluence: bool) -> Vec<String> {
    let mut tools = vec![
        PYTHON_KEY.into(),
        crate::manifest::PI_TOOL_KEY.into(),
        PRODUCT_KEY.into(),
        QMD_KEY.into(),
    ];
    if confluence {
        tools.push(CONFLUENCE_KEY.into());
    }
    tools
}

fn wiki_skill_names(confluence: bool) -> Vec<String> {
    confluence
        .then(|| CONFLUENCE_SKILL.into())
        .into_iter()
        .collect()
}

fn qmd_index(vault: &Path) -> String {
    format!(
        "loom-wiki-{}",
        &sha256(vault.as_os_str().as_encoded_bytes())[..16]
    )
}

fn setup_qmd(system: &dyn System, vault: &Path) -> Result<String> {
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

fn qmd_document_count(status: &str) -> Result<usize> {
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

fn offer_global_feynman_migration(system: &dyn System, vault: &Path, yes: bool) -> Result<()> {
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

fn canonical_vault(path: &Path) -> Result<PathBuf> {
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

fn canonicalize_with_missing_tail(path: &Path) -> PathBuf {
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

fn registry_match_path(system: &dyn System, registry: &WikiRegistry, path: &Path) -> PathBuf {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WikiOutcome {
    Finished(bool),
    Incomplete,
}

impl WikiOutcome {
    fn successful_exit(self) -> bool {
        self != Self::Finished(false)
    }
}

pub fn run_wiki(request: &WikiRequest, system: &(dyn System + Sync)) -> Result<bool> {
    run_wiki_outcome(request, system).map(WikiOutcome::successful_exit)
}

fn run_wiki_outcome(request: &WikiRequest, system: &(dyn System + Sync)) -> Result<WikiOutcome> {
    setup_with_confirmation(
        request,
        system,
        None,
        &std::sync::atomic::AtomicBool::new(false),
        &mut Vec::new(),
    )
}

/// The setup chooser supplies its own file-approval dialog and cancellation token.
/// No nested terminal, authentication, launch, or global-package removal in this mode.
pub(crate) fn setup_with_confirmation(
    request: &WikiRequest,
    system: &(dyn System + Sync),
    confirm: Option<Confirm<'_>>,
    cancelled: &std::sync::atomic::AtomicBool,
    notes: &mut Vec<String>,
) -> Result<WikiOutcome> {
    let inline = confirm.is_some();
    let interactive = !request.yes && !inline;
    let yes = request.yes;
    let mut default_confirm = move |title: &str, paths: &[String]| {
        if yes {
            for path in paths {
                println!("  {path}");
            }
            Ok(true)
        } else {
            crate::wiki_tui::confirm(title, paths)
        }
    };
    let confirm = confirm.unwrap_or(&mut default_confirm);
    let writes_vault_or_local_pi = matches!(
        request.operation,
        WikiOperation::Create
            | WikiOperation::Adopt
            | WikiOperation::Repair
            | WikiOperation::Launch
    );
    anyhow::ensure!(
        !cfg!(windows) || !writes_vault_or_local_pi,
        "Vault setup, repair, and Pi launch run in WSL2 on Windows. Open Ubuntu, change to the Vault's WSL path, and rerun this command."
    );
    let home = system.home_dir().context("home directory is unavailable")?;
    let mut registered = None;
    match request.operation {
        WikiOperation::Status => return Ok(WikiOutcome::Finished(status_registered(system))),
        WikiOperation::Unregister => {
            let mut registry = WikiRegistry::load(&home)?;
            let path = registry_match_path(system, &registry, &request.vault);
            registry.unregister(&path);
            registry.save(&home)?;
            println!(
                "Unregistered {}; Vault files were not changed.",
                path.display()
            );
            return Ok(WikiOutcome::Finished(true));
        }
        WikiOperation::Open | WikiOperation::Launch | WikiOperation::Repair => {
            let registry = WikiRegistry::load(&home)?;
            let path = registry_match_path(system, &registry, &request.vault);
            registered = registry
                .vaults
                .into_iter()
                .find(|record| record.path == path);
            anyhow::ensure!(
                registered.is_some(),
                "Vault is not registered; use Create or Adopt before managing it"
            );
        }
        _ => {}
    }
    if request.operation == WikiOperation::Open {
        let vault = registered.context("registered Vault")?.path;
        anyhow::ensure!(
            vault.is_dir(),
            "registered Vault is missing: {}",
            vault.display()
        );
        return open_obsidian(system, &vault).map(WikiOutcome::Finished);
    }
    if request.operation == WikiOperation::Launch {
        let vault = registered.context("registered Vault")?.path;
        anyhow::ensure!(
            vault.is_dir(),
            "registered Vault is missing: {}",
            vault.display()
        );
        system
            .spawn_detached(&CommandSpec::new("pi", std::iter::empty::<&str>()).in_dir(&vault))?;
        return Ok(WikiOutcome::Finished(true));
    }

    let (vault_target, feynman, confluence) = if let Some(record) = registered {
        (
            record.path,
            record.feynman || request.feynman,
            record.confluence || request.confluence,
        )
    } else {
        (
            absolute_vault_target(
                system,
                &request.vault,
                request.operation == WikiOperation::Create,
            )?,
            request.feynman,
            request.confluence,
        )
    };
    let repository = crate::skills::Repository::default();
    crate::wiki_progress::run(
        system,
        interactive,
        "Preparing Wiki Vault",
        |system, local_cancelled| {
            let cancelled = if inline { cancelled } else { local_cancelled };
            manifest::sync_selected_from(
                system,
                &wiki_tool_keys(confluence),
                cancelled,
                &repository,
            )
            .map_err(anyhow::Error::msg)
        },
    )?;
    let product = product_root(system)?;
    if matches!(
        request.operation,
        WikiOperation::Create | WikiOperation::Adopt
    ) && !initialize_vault(system, &product, &request.operation, &vault_target, confirm)?
    {
        return Ok(WikiOutcome::Incomplete);
    }
    let vault = canonical_vault(&vault_target)?;
    if matches!(
        request.operation,
        WikiOperation::Create | WikiOperation::Adopt
    ) && !ensure_pi_ignored(system, &home, &product, &vault, confirm)?
    {
        return Ok(WikiOutcome::Incomplete);
    }
    let search_note = crate::wiki_progress::run(
        system,
        interactive,
        "Setting up Wiki Vault",
        |system, local_cancelled| {
            let cancelled = if inline { cancelled } else { local_cancelled };
            install_packages(system, &product, &vault, feynman)?;
            crate::skills::install_skills(
                system,
                &repository,
                &wiki_skill_names(confluence),
                &crate::skills::SkillDestination {
                    agents: vec![crate::skills::SkillAgent::AgentsStandard],
                    scope: crate::skills::SkillScope::Project,
                    home: home.clone(),
                    project_root: vault.clone(),
                },
                cancelled,
            )
            .map_err(anyhow::Error::msg)?;
            setup_qmd(system, &vault)
        },
    )?;
    if !inline {
        println!("{search_note}");
    }
    notes.push(search_note);
    let mut registry = WikiRegistry::load(&home)?;
    registry.register(vault.clone(), feynman, confluence);
    registry.save(&home)?;
    if confluence && interactive {
        crate::wiki_confluence::configure(system)?;
    }
    if feynman
        && !inline
        && matches!(
            request.operation,
            WikiOperation::Create | WikiOperation::Adopt
        )
    {
        if let Err(error) = offer_global_feynman_migration(system, &vault, request.yes) {
            println!("Global Feynman was left unchanged: {error}");
        }
    }
    if !inline {
        println!("Wiki ready. Run: cd {} && pi", vault.display());
        println!(
            "Search: qmd --index {} query \"your question\"",
            qmd_index(&vault)
        );
    }
    if confluence && request.yes && !inline {
        println!("Confluence: run `cme config edit auth.confluence` to configure authentication.");
    }
    if interactive
        && matches!(
            request.operation,
            WikiOperation::Create | WikiOperation::Adopt
        )
    {
        offer_finish_actions(system, &vault)?;
    }
    Ok(WikiOutcome::Finished(true))
}

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
    for record in registry.vaults {
        let health = inspect_vault(system, &record);
        style.row(
            if health.healthy { Mark::Ok } else { Mark::Bad },
            &tidy_path(&record.path, &home),
            "",
        );
        for (mark, label, detail) in health.rows {
            style.row(mark, label, detail);
        }
        healthy &= health.healthy;
    }
    healthy
}

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
    let packages = system
        .run_probe(&CommandSpec::new("pi", ["list", "--approve"]).in_dir(&record.path))
        .ok()
        .filter(|result| result.success)
        .map(|result| result.stdout)
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
    let ok =
        core_ready && qmd && (!record.feynman || feynman) && (!record.confluence || confluence);
    rows.push((
        if core_ready { Mark::Ok } else { Mark::Bad },
        "claude-obsidian",
        format!(
            "{} — Vault package {}; marker {}; prerequisites {}; doctor {}",
            if core_ready { "ready" } else { "repair needed" },
            if core { "installed" } else { "missing" },
            if marker { "present" } else { "missing" },
            if pins_ready { "ready" } else { "missing" },
            if doctor { "ok" } else { "failed" },
        ),
    ));
    for (label, installed, required, detail) in [
        ("Feynman", feynman, record.feynman, "Vault Pi package"),
        ("qmd", qmd, true, "tool + Vault skill"),
        (
            "Confluence",
            confluence,
            record.confluence,
            "tool + Vault skill",
        ),
    ] {
        rows.push((
            if installed {
                Mark::Ok
            } else if required {
                Mark::Bad
            } else {
                Mark::Off
            },
            label,
            if installed {
                format!("ready — {detail}")
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
    if !registry.vaults.is_empty() && !system.command_exists("qmd") {
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
            setup_qmd(system, &record.path)
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

fn percent_encode_path(path: &Path) -> String {
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

fn open_url(system: &dyn System, url: String) -> Result<()> {
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

fn open_obsidian(system: &dyn System, vault: &Path) -> Result<bool> {
    let url = format!("obsidian://open?path={}", percent_encode_path(vault));
    open_url(system, url)?;
    Ok(true)
}

fn offer_finish_actions(system: &dyn System, vault: &Path) -> Result<()> {
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

pub fn run_interactive(system: &(dyn System + Sync)) -> Result<bool> {
    run_interactive_with_default(system, false)
}

pub fn run_interactive_with_default(
    system: &(dyn System + Sync),
    feynman_default: bool,
) -> Result<bool> {
    run_interactive_outcome(system, feynman_default).map(WikiOutcome::successful_exit)
}

pub(crate) fn run_interactive_outcome(
    system: &(dyn System + Sync),
    feynman_default: bool,
) -> Result<WikiOutcome> {
    match crate::wiki_tui::run(system, feynman_default, obsidian_installed(system))? {
        crate::wiki_tui::WikiChoice::Request(request) => run_wiki_outcome(&request, system),
        crate::wiki_tui::WikiChoice::OpenObsidianDownload => {
            open_url(system, "https://obsidian.md/download".into())?;
            Ok(WikiOutcome::Incomplete)
        }
        crate::wiki_tui::WikiChoice::Cancelled => Ok(WikiOutcome::Incomplete),
        crate::wiki_tui::WikiChoice::PickPath(_) | crate::wiki_tui::WikiChoice::InspectVault => {
            unreachable!()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandResult;
    use std::sync::Mutex;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("loom-wiki-{name}-{}", std::process::id()))
    }

    #[test]
    fn vault_inspection_distinguishes_shared_tools_and_global_packages_from_vault_setup() {
        struct SharedOnly {
            root: PathBuf,
            commands: Mutex<Vec<String>>,
        }
        impl System for SharedOnly {
            fn command_exists(&self, name: &str) -> bool {
                matches!(name, "pi" | "qmd")
            }
            fn refresh_path(&self) {
                panic!("inspection must not install")
            }
            fn home_dir(&self) -> Option<PathBuf> {
                Some(self.root.clone())
            }
            fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
                self.commands.lock().unwrap().push(command.display());
                Ok(CommandResult {
                    success: command.program == "pi",
                    stdout:
                        "User packages:\n  npm:@companion-ai/feynman@latest\nProject packages:\n"
                            .into(),
                    stderr: String::new(),
                })
            }
        }
        let root = temp("shared-status");
        fs::create_dir_all(&root).unwrap();
        let system = SharedOnly {
            root: root.clone(),
            commands: Mutex::new(Vec::new()),
        };
        let health = inspect_vault(
            &system,
            &VaultRecord {
                path: root.clone(),
                feynman: true,
                confluence: false,
            },
        );
        assert!(!health.healthy);
        for (label, expected) in [
            ("qmd", crate::ui::Mark::Bad),
            ("Feynman", crate::ui::Mark::Bad),
            ("QMD (shared)", crate::ui::Mark::Ok),
        ] {
            assert!(health
                .rows
                .iter()
                .any(|(mark, name, _)| *name == label && *mark == expected));
        }
        assert!(system
            .commands
            .lock()
            .unwrap()
            .iter()
            .all(|command| command.starts_with("mise where") || command.starts_with("pi list")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_is_sorted_idempotent_and_unregister_never_deletes_vault() {
        let home = temp("registry");
        let a = home.join("a");
        let b = home.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("note.md"), "knowledge").unwrap();
        let mut registry = WikiRegistry::default();
        registry.register(b.clone(), false, false);
        registry.register(a.clone(), false, false);
        registry.register(a.clone(), true, false);
        registry.save(&home).unwrap();
        let loaded = WikiRegistry::load(&home).unwrap();
        assert_eq!(
            loaded.vaults,
            [
                VaultRecord {
                    path: a.clone(),
                    feynman: true,
                    confluence: false
                },
                VaultRecord {
                    path: b.clone(),
                    feynman: false,
                    confluence: false
                }
            ]
        );
        let system = FakeSystem {
            home: home.clone(),
            commands: Mutex::new(Vec::new()),
        };
        run_wiki(
            &WikiRequest {
                operation: WikiOperation::Unregister,
                vault: PathBuf::from("a"),
                feynman: false,
                confluence: false,
                yes: true,
            },
            &system,
        )
        .unwrap();
        assert_eq!(WikiRegistry::load(&home).unwrap().vaults[0].path, b);
        assert_eq!(fs::read_to_string(a.join("note.md")).unwrap(), "knowledge");
        fs::remove_dir_all(home).unwrap();
    }

    struct FakeSystem {
        home: PathBuf,
        commands: Mutex<Vec<CommandSpec>>,
    }
    impl System for FakeSystem {
        fn command_exists(&self, name: &str) -> bool {
            name == "pi" || name == "mise" || name == "python" || name == "qmd"
        }
        fn refresh_path(&self) {}
        fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
            self.commands.lock().unwrap().push(command.clone());
            let stdout = if command.program == "mise"
                && command.args.first().map(String::as_str) == Some("where")
            {
                format!("{}\n", self.home.join("product/claude-obsidian").display())
            } else if command.program == "pi"
                && command.args.first().map(String::as_str) == Some("list")
            {
                "Project packages:\n  /product/claude-obsidian\n  npm:@companion-ai/feynman@0.3.47\n".into()
            } else if command.program == "mise" && command.args.iter().any(|arg| arg == "doctor") {
                r#"{"schema":"claude-obsidian.doctor.v1","ok":true}"#.into()
            } else {
                String::new()
            };
            Ok(CommandResult {
                success: true,
                stdout,
                stderr: String::new(),
            })
        }
        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }
        fn current_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }
    }

    #[test]
    fn confluence_is_only_installed_when_selected_for_the_vault() {
        assert!(!wiki_tool_keys(false)
            .iter()
            .any(|key| key == CONFLUENCE_KEY));
        assert!(wiki_tool_keys(true).iter().any(|key| key == CONFLUENCE_KEY));
        assert!(wiki_skill_names(false).is_empty());
        assert_eq!(wiki_skill_names(true), [CONFLUENCE_SKILL]);
    }

    #[test]
    fn qmd_setup_is_repeatable_and_keeps_vault_indexes_separate() {
        #[derive(Default)]
        struct QmdSystem {
            collections: Mutex<std::collections::BTreeSet<String>>,
            commands: Mutex<Vec<CommandSpec>>,
            fail_embed: bool,
            empty: bool,
            embed_message: String,
            embed_stderr: String,
            fail_search: bool,
            pending: bool,
        }
        impl System for QmdSystem {
            fn command_exists(&self, _: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
                self.commands.lock().unwrap().push(command.clone());
                assert_eq!(command.program, "qmd");
                if command.args == ["skill", "install"] {
                    let skill = command.cwd.as_ref().unwrap().join(".agents/skills/qmd");
                    fs::create_dir_all(&skill).unwrap();
                    fs::write(
                        skill.join("SKILL.md"),
                        "---\nname: qmd\ndescription: test\n---\n",
                    )
                    .unwrap();
                    return Ok(CommandResult {
                        success: true,
                        stdout: String::new(),
                        stderr: String::new(),
                    });
                }
                assert_eq!(command.args[0], "--index");
                let mut collections = self.collections.lock().unwrap();
                let success = match command.args[2].as_str() {
                    "collection" if command.args[3] == "show" => {
                        collections.contains(&command.args[1])
                    }
                    "collection" => {
                        assert_eq!(
                            &command.args[3..],
                            &["add", ".", "--name", "vault", "--mask", "**/*.md"]
                        );
                        assert!(collections.insert(command.args[1].clone()));
                        true
                    }
                    "update" | "status" | "pull" | "query" => true,
                    "embed" => !self.fail_embed,
                    _ => panic!("unexpected QMD command"),
                };
                Ok(CommandResult {
                    success,
                    stdout: if command.args[2] == "status" {
                        format!(
                            "  Total:    {} files indexed\n{}",
                            if self.empty { 0 } else { 1 },
                            if self.pending {
                                "Pending: 1 need embedding"
                            } else {
                                ""
                            }
                        )
                    } else if command.args[2] == "embed" {
                        self.embed_message.clone()
                    } else {
                        String::new()
                    },
                    stderr: if command.args[2] == "embed" && !self.embed_stderr.is_empty() {
                        self.embed_stderr.clone()
                    } else {
                        "embedding failed".into()
                    },
                })
            }
            fn run_controlled(
                &self,
                command: &CommandSpec,
                timeout: std::time::Duration,
                _: &std::sync::atomic::AtomicBool,
            ) -> Result<CommandResult> {
                assert_eq!(timeout.as_secs(), 120);
                let result = self.run(command)?;
                assert!(command.args.windows(2).any(|args| args == ["-c", "vault"]));
                assert!(command.args.windows(2).any(|args| args == ["-C", "4"]));
                if self.fail_search {
                    anyhow::bail!("timeout with private result text");
                }
                Ok(result)
            }
        }
        let root = temp("qmd");
        let first = root.join("one");
        let second = root.join("two");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let system = QmdSystem::default();
        setup_qmd(&system, &first).unwrap();
        setup_qmd(&system, &first).unwrap();
        setup_qmd(&system, &second).unwrap();
        assert_eq!(system.collections.lock().unwrap().len(), 2);
        let commands = system.commands.lock().unwrap();
        assert_eq!(
            commands
                .iter()
                .filter(|c| c.args == ["skill", "install"])
                .count(),
            2
        );
        assert_eq!(
            commands
                .iter()
                .filter(|c| c.args.get(2).is_some_and(|arg| arg == "embed"))
                .count(),
            3
        );
        assert!(
            commands
                .iter()
                .filter(|c| c.cwd.as_deref() == Some(first.as_path()))
                .count()
                > commands
                    .iter()
                    .filter(|c| c.cwd.as_deref() == Some(second.as_path()))
                    .count()
        );
        let indexed = commands
            .iter()
            .filter(|c| c.args[0] == "--index")
            .map(|c| c.args[2].as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            &indexed[..8],
            [
                "collection",
                "collection",
                "update",
                "status",
                "pull",
                "embed",
                "status",
                "query"
            ]
        );
        assert_eq!(indexed.iter().filter(|c| **c == "query").count(), 3);
        drop(commands);
        for (empty, message, fail_search) in [
            (true, "", false),
            (
                false,
                "Another embed process is already running. Skipping.",
                false,
            ),
            (false, "2 chunks still failed after retries", false),
            (false, "", true),
        ] {
            let probe = QmdSystem {
                empty,
                embed_message: message.into(),
                fail_search,
                ..Default::default()
            };
            let result = setup_qmd(&probe, &first);
            if empty {
                assert!(result.unwrap().contains("empty"));
            } else if fail_search {
                let note = result.unwrap();
                assert!(note.contains("Warning"));
                assert!(!note.contains("private result text"));
            } else {
                assert!(result.is_err());
            }
            assert_eq!(
                probe
                    .commands
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|c| c.args.get(2).is_some_and(|arg| arg == "query"))
                    .count(),
                usize::from(fail_search)
            );
        }
        let stderr_busy = QmdSystem {
            embed_stderr: "Another embed process is already running. Skipping.".into(),
            ..Default::default()
        };
        assert!(setup_qmd(&stderr_busy, &first)
            .unwrap_err()
            .to_string()
            .contains("incomplete or busy"));
        let pending = QmdSystem {
            pending: true,
            ..Default::default()
        };
        assert!(setup_qmd(&pending, &first)
            .unwrap_err()
            .to_string()
            .contains("pending"));
        assert!(!pending
            .commands
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.args.get(2).is_some_and(|arg| arg == "query")));
        let failing = QmdSystem {
            fail_embed: true,
            ..Default::default()
        };
        assert!(setup_qmd(&failing, &first)
            .unwrap_err()
            .to_string()
            .contains("The operation did not complete"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repair_runs_pinned_packages_in_the_vault_working_directory() {
        let home = temp("repair");
        let vault = home.join("vault");
        fs::create_dir_all(vault.join(".pi")).unwrap();
        fs::write(
            vault.join(".pi/settings.json"),
            r#"{"packages":["../../mise/installs/github-agrici-daniel-claude-obsidian/old"]}"#,
        )
        .unwrap();
        let mut registry = WikiRegistry::default();
        registry.register(vault.clone(), true, false);
        registry.save(&home).unwrap();
        let system = FakeSystem {
            home: home.clone(),
            commands: Mutex::new(Vec::new()),
        };
        let product = PathBuf::from("/product/claude-obsidian");
        install_packages(&system, &product, &vault, true).unwrap();
        let commands = system.commands.into_inner().unwrap();
        assert_eq!(commands[0].cwd.as_deref(), Some(vault.as_path()));
        assert!(commands[1]
            .display()
            .contains("npm:@companion-ai/feynman@0.3.47"));
        assert!(commands
            .iter()
            .all(|command| command.cwd.as_deref() == Some(vault.as_path())));
        assert!(!commands
            .iter()
            .any(|command| command.args.first().map(String::as_str) == Some("remove")));
        assert!(commands
            .iter()
            .filter(|command| command.program == "pi")
            .all(|command| command.args.iter().any(|arg| arg == "--approve")));
        let settings = fs::read_to_string(vault.join(".pi/settings.json")).unwrap();
        assert!(!settings.contains("github-agrici-daniel-claude-obsidian/old"));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn update_reports_missing_vaults_without_recreating_them_and_continues() {
        let home = temp("update-many");
        let present = home.join("present");
        let missing = home.join("missing");
        fs::create_dir_all(&present).unwrap();
        let mut registry = WikiRegistry::default();
        registry.register(missing.clone(), false, false);
        registry.register(present.clone(), false, false);
        registry.save(&home).unwrap();
        let system = FakeSystem {
            home: home.clone(),
            commands: Mutex::new(Vec::new()),
        };
        assert!(!update_registered(&system, false, &Out::plain()));
        assert!(!missing.exists());
        assert!(system
            .commands
            .into_inner()
            .unwrap()
            .iter()
            .any(|command| command.program == "pi"
                && command.cwd.as_deref() == Some(present.as_path())));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn reviewed_adoption_forwards_the_exact_hash_without_force() {
        struct ReviewSystem {
            commands: Mutex<Vec<CommandSpec>>,
        }
        impl System for ReviewSystem {
            fn command_exists(&self, _name: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
                let mut commands = self.commands.lock().unwrap();
                commands.push(command.clone());
                let stdout = if commands.len() == 1 {
                    r#"{"schema":"claude-obsidian.adoption-plan.v1","status":"dry-run","changed_paths":[".claude-obsidian.json"],"approved_plan_sha256":"reviewed-hash"}"#.into()
                } else {
                    "{}".into()
                };
                Ok(CommandResult {
                    success: true,
                    stdout,
                    stderr: String::new(),
                })
            }
        }
        let root = temp("review-hash");
        let vault = root.join("vault");
        fs::create_dir_all(vault.join(".obsidian")).unwrap();
        let system = ReviewSystem {
            commands: Mutex::new(Vec::new()),
        };

        assert!(initialize_vault(
            &system,
            Path::new("/product"),
            &WikiOperation::Adopt,
            &vault,
            &mut |_, _| Ok(true),
        )
        .unwrap());
        let commands = system.commands.into_inner().unwrap();
        assert_eq!(commands.len(), 2);
        assert!(commands[1]
            .args
            .windows(2)
            .any(|pair| pair == ["--approved-plan-sha256", "reviewed-hash"]));
        assert!(commands[1].args.contains(&"--apply".into()));
        assert!(!commands
            .iter()
            .any(|command| command.args.contains(&"--force".into())));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pi_ignore_is_applied_through_a_reviewed_upstream_bundle_only() {
        struct TransactionSystem {
            commands: Mutex<Vec<CommandSpec>>,
        }
        impl System for TransactionSystem {
            fn command_exists(&self, _name: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
                self.commands.lock().unwrap().push(command.clone());
                if command.args.iter().any(|arg| arg == "inspect") {
                    return Ok(CommandResult {
                        success: true,
                        stdout: r#"{"schema":"claude-obsidian.transaction-plan.v1","valid":true,"approval_sha256":"ignore-hash","changed_paths":[".gitignore"]}"#.into(),
                        stderr: String::new(),
                    });
                }
                if command.args.iter().any(|arg| arg == "apply") {
                    let bundle = command
                        .args
                        .iter()
                        .find(|arg| arg.ends_with(".json"))
                        .unwrap();
                    let operation: serde_json::Value =
                        serde_json::from_slice(&fs::read(bundle).unwrap()).unwrap();
                    assert_eq!(operation["operation_type"], "setup");
                    assert_eq!(operation["writes"][0]["mode"], "replace");
                    assert_eq!(operation["expected_hashes"][".gitignore"], sha256(&[]));
                    let vault_index = command
                        .args
                        .iter()
                        .position(|arg| arg == "--vault")
                        .unwrap();
                    let vault = PathBuf::from(&command.args[vault_index + 1]);
                    fs::write(
                        vault.join(".gitignore"),
                        operation["writes"][0]["content"].as_str().unwrap(),
                    )
                    .unwrap();
                }
                Ok(CommandResult {
                    success: true,
                    stdout: "{}".into(),
                    stderr: String::new(),
                })
            }
            fn home_dir(&self) -> Option<PathBuf> {
                None
            }
        }
        let home = temp("ignore-review");
        let vault = home.join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(vault.join(".gitignore"), "").unwrap();
        fs::write(vault.join("note.md"), "keep me").unwrap();
        let system = TransactionSystem {
            commands: Mutex::new(Vec::new()),
        };
        assert!(!ensure_pi_ignored(
            &system,
            &home,
            Path::new("/product"),
            &vault,
            &mut |_, paths| {
                assert_eq!(paths, [".gitignore: add .pi/"]);
                Ok(false)
            }
        )
        .unwrap());
        assert_eq!(fs::read_to_string(vault.join(".gitignore")).unwrap(), "");
        assert!(!system
            .commands
            .lock()
            .unwrap()
            .iter()
            .any(|command| command.args.iter().any(|arg| arg == "apply")));
        system.commands.lock().unwrap().clear();
        assert!(ensure_pi_ignored(
            &system,
            &home,
            Path::new("/product"),
            &vault,
            &mut |_, _| Ok(true)
        )
        .unwrap());
        assert!(fs::read_to_string(vault.join(".gitignore"))
            .unwrap()
            .contains(".pi/"));
        assert_eq!(
            fs::read_to_string(vault.join("note.md")).unwrap(),
            "keep me"
        );
        let commands = system.commands.into_inner().unwrap();
        assert!(commands[1]
            .args
            .windows(2)
            .any(|pair| pair == ["--approved-plan-sha256", "ignore-hash"]));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn upstream_python_commands_use_the_selected_mise_runtime() {
        let command = python_command(
            Path::new("/product"),
            vec!["doctor".into(), "--vault".into(), "/vault".into()],
        );
        assert_eq!(command.program, "mise");
        assert_eq!(&command.args[..4], ["exec", PYTHON_KEY, "--", "python"]);
    }

    #[test]
    fn operation_stamp_is_valid_utc_and_stable_between_review_arguments() {
        let (generated, id) = operation_stamp("init");
        assert_eq!(generated.len(), 20);
        assert!(generated.ends_with('Z'));
        assert!(id.starts_with("loom-init-"));
    }

    #[test]
    fn project_package_verification_never_accepts_user_packages() {
        let product = Path::new("/product/claude-obsidian");
        let global_only =
            "User packages:\n  /product/claude-obsidian\n  npm:@companion-ai/feynman@0.3.47\n";
        assert!(!has_project_packages(global_only, product, false));
        assert!(!has_project_packages(global_only, product, true));

        let local = "User packages:\n  npm:@companion-ai/feynman@0.3.47\nProject packages:\n  /product/claude-obsidian\n  npm:@companion-ai/feynman@0.3.47\n";
        assert!(has_project_packages(local, product, true));
        assert!(!has_project_packages(
            local,
            Path::new("/stale/claude-obsidian"),
            true
        ));
    }

    #[test]
    fn registry_defaults_only_when_absent_and_rejects_invalid_bytes() {
        let home = temp("registry-errors");
        assert_eq!(WikiRegistry::load(&home).unwrap(), WikiRegistry::default());
        let path = WikiRegistry::path(&home);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(WikiRegistry::load(&home).is_err());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn missing_paths_canonicalize_their_existing_parent() {
        let root = temp("missing-canonical-parent");
        fs::create_dir_all(&root).unwrap();
        let requested = root.join("missing/vault");
        assert_eq!(
            canonicalize_with_missing_tail(&requested),
            root.canonicalize().unwrap().join("missing/vault")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn relative_create_targets_are_made_absolute_before_review() {
        let home = temp("absolute-create");
        fs::create_dir_all(&home).unwrap();
        let system = FakeSystem {
            home: home.clone(),
            commands: Mutex::new(Vec::new()),
        };
        assert_eq!(
            absolute_vault_target(&system, Path::new("New Vault"), true).unwrap(),
            home.canonicalize().unwrap().join("New Vault")
        );
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn repair_refuses_unregistered_directories_before_running_commands() {
        let home = temp("repair-unregistered");
        let vault = home.join("vault");
        fs::create_dir_all(&vault).unwrap();
        let system = FakeSystem {
            home: home.clone(),
            commands: Mutex::new(Vec::new()),
        };
        let error = run_wiki(
            &WikiRequest {
                operation: WikiOperation::Repair,
                vault,
                feynman: false,
                confluence: false,
                yes: true,
            },
            &system,
        )
        .unwrap_err();
        assert!(error.to_string().contains(if cfg!(windows) {
            "WSL2"
        } else {
            "not registered"
        }));
        assert!(system.commands.into_inner().unwrap().is_empty());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn create_resumes_only_a_partially_initialized_claude_obsidian_vault() {
        let root = temp("resume-create");
        let vault = root.join("vault");
        fs::create_dir_all(vault.join(".obsidian")).unwrap();
        fs::write(vault.join(".claude-obsidian.json"), "{}").unwrap();
        let system = FakeSystem {
            home: root.clone(),
            commands: Mutex::new(Vec::new()),
        };
        assert!(initialize_vault(
            &system,
            Path::new("/product"),
            &WikiOperation::Create,
            &vault,
            &mut |_, _| Ok(true)
        )
        .unwrap());
        let commands = system.commands.into_inner().unwrap();
        assert_eq!(commands.len(), 1);
        assert!(commands[0].args.iter().any(|arg| arg == "doctor"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn obsidian_url_percent_encodes_spaces_without_a_dependency() {
        assert_eq!(
            percent_encode_path(Path::new("/tmp/My Vault")),
            "/tmp/My%20Vault"
        );
    }
}
