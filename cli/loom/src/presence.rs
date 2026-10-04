//! Presence: is this resource on this machine, in this destination? One
//! place obtains the manager listings and owns the matching rule per
//! resource kind, so the chooser, install verification, status, update and
//! uninstall answer the question the same way.

use crate::{CommandSpec, System};
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// How much evidence a Tool needs to count as present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ToolRule {
    /// Chooser: mise manages it (it is in the Selection) or its binary is on
    /// PATH from any other installer (brew, cargo, ...): both are honestly
    /// "installed".
    Installed,
    /// Install verification and status: in the Selection and on PATH.
    Working,
    /// Uninstall: the receipt is live while the key is in the Selection,
    /// whatever PATH says.
    Selected,
}

/// `in_selection` is the caller's reading of the Selection for this tool.
pub(crate) fn tool_present(
    system: &dyn System,
    in_selection: bool,
    bin: Option<&str>,
    rule: ToolRule,
) -> bool {
    let on_path = || bin.is_some_and(|bin| system.command_exists(bin));
    match rule {
        ToolRule::Installed => in_selection || on_path(),
        ToolRule::Working => in_selection && on_path(),
        ToolRule::Selected => in_selection,
    }
}

/// A manager whose own listing is the evidence for its resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Manager {
    Pi,
    Herdr,
}

impl Manager {
    pub(crate) fn named(program: &str) -> Option<Self> {
        match program {
            "pi" => Some(Self::Pi),
            "herdr" => Some(Self::Herdr),
            _ => None,
        }
    }

    pub(crate) fn program(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::Herdr => "herdr",
        }
    }

    fn list_command(self) -> CommandSpec {
        match self {
            Self::Pi => CommandSpec::new("pi", ["list"]),
            Self::Herdr => CommandSpec::new("herdr", ["plugin", "list"]),
        }
    }
}

/// Why a manager could not be asked what it has installed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ListingError {
    /// The list command ran and reported failure.
    Failed(String),
    /// The list command could not be run.
    NotRun(String),
}

impl std::fmt::Display for ListingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(message) | Self::NotRun(message) => formatter.write_str(message),
        }
    }
}

/// What one manager reports as installed. Ask it once, then check as many
/// resources as needed.
#[derive(Clone, Debug, Default)]
pub(crate) struct Listing {
    stdout: String,
    stderr: String,
}

impl Listing {
    /// The cheapest honest answer, for choosers: the manager's on-disk
    /// registry when it is readable, because `pi list` and
    /// `herdr plugin list` boot CLIs; the list command only as a fallback
    /// when the manager is `available`.
    pub(crate) fn quick(system: &dyn System, manager: Manager, available: bool) -> Option<Self> {
        system
            .home_dir()
            .and_then(|home| match manager {
                Manager::Pi => pi_packages_listing(
                    &crate::settings::pi_agent_dir(&home).join("settings.json"),
                    "User packages:",
                ),
                Manager::Herdr => herdr_plugins_from_registry(&home),
            })
            .map(|stdout| Self {
                stdout,
                stderr: String::new(),
            })
            .or_else(|| {
                available
                    .then(|| Self::probe(system, manager, &AtomicBool::new(false)).ok())
                    .flatten()
            })
    }

    /// Ask the manager itself: the evidence after an install or before a
    /// removal, covering every scope the manager knows.
    pub(crate) fn probe(
        system: &dyn System,
        manager: Manager,
        cancelled: &AtomicBool,
    ) -> Result<Self, ListingError> {
        match system.run_controlled(
            &manager.list_command(),
            crate::system::PROBE_COMMAND_TIMEOUT,
            cancelled,
        ) {
            Ok(result) if result.success => Ok(Self {
                stdout: result.stdout,
                stderr: result.stderr,
            }),
            Ok(result) => Err(ListingError::Failed(
                crate::install::command_failure_message(&result),
            )),
            Err(error) => Err(ListingError::NotRun(error.to_string())),
        }
    }

    #[cfg(test)]
    pub(crate) fn of(stdout: &str) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    /// Pi may list both scopes; only the asked destination is evidence.
    pub(crate) fn has_pi_package(&self, target: &str, project: bool) -> bool {
        pi_package_installed(&self.stdout, target, project)
    }

    /// The plugin id as a whole word: `annotate` is neither `annotate-pro`
    /// nor the `herdr-annotate` inside another plugin's source.
    pub(crate) fn has_herdr_plugin(&self, id: &str) -> bool {
        let id = id.trim_start_matches("herdr-plugin:");
        self.stdout.split_whitespace().any(|word| word == id)
    }

    /// Uninstall's receipt rule, kept as it was pending a decision: a raw
    /// substring of stdout or stderr, or the target's first `@`/`#` fragment
    /// in stdout, whatever the scope. Looser than `has_pi_package` and
    /// `has_herdr_plugin`; tightening it changes which receipts uninstall
    /// treats as already gone.
    pub(crate) fn mentions(&self, target: &str) -> bool {
        self.stdout.contains(target)
            || self.stderr.contains(target)
            || target
                .split(['@', '#'])
                .find(|part| !part.is_empty())
                .is_some_and(|part| self.stdout.contains(part))
    }
}

/// Prefer Herdr's registry file so the wizard probe does not boot `herdr`.
fn herdr_plugins_from_registry(home: &Path) -> Option<String> {
    let content =
        std::fs::read_to_string(crate::settings::herdr_dir(home).join("plugins.json")).ok()?;
    let plugins = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let plugins = plugins.as_array()?;
    let mut listed = String::new();
    for plugin in plugins {
        if let Some(id) = plugin.get("plugin_id").and_then(serde_json::Value::as_str) {
            listed.push_str(id);
            listed.push('\n');
        }
    }
    Some(listed)
}

/// A `pi list`-shaped listing read straight from a Pi settings file, so the
/// installed-state probe never has to boot the Node CLI. `None` when the file
/// cannot be read safely; a missing file lists nothing.
pub(crate) fn pi_packages_listing(settings_path: &Path, heading: &str) -> Option<String> {
    let content = match std::fs::read_to_string(settings_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(format!("{heading}\n"));
        }
        Err(_) => return None,
    };
    let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
    let mut listed = format!("{heading}\n");
    let Some(packages) = settings.get("packages") else {
        return Some(listed);
    };
    let packages = packages.as_array()?;
    for package in packages {
        if let Some(source) = package
            .as_str()
            .or_else(|| package.get("source").and_then(serde_json::Value::as_str))
        {
            let path = Path::new(source);
            let resolved = path
                .is_relative()
                .then(|| {
                    settings_path
                        .parent()
                        .unwrap_or_else(|| Path::new(""))
                        .join(path)
                })
                .filter(|path| path.is_dir())
                .map(|path| path.canonicalize().unwrap_or(path));
            listed.push_str("  ");
            listed.push_str(resolved.as_deref().and_then(Path::to_str).unwrap_or(source));
            listed.push('\n');
        }
    }
    Some(listed)
}

fn path_package_matches(path: &Path, target: &str, unscoped: &str) -> bool {
    if let Some(name) = std::fs::read(path.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|package| package["name"].as_str().map(str::to_owned))
    {
        return name == target;
    }
    path.to_string_lossy()
        .to_ascii_lowercase()
        .contains(&unscoped.to_ascii_lowercase())
}

fn pi_package_installed(listed: &str, target: &str, project: bool) -> bool {
    let target = target.strip_prefix("npm:").unwrap_or(target);
    let target = target
        .rsplit_once('@')
        .filter(|(name, _)| !name.is_empty())
        .map_or(target, |(name, _)| name);
    let target = target.strip_prefix("git:").map_or(target, |source| {
        source
            .rsplit('/')
            .next()
            .unwrap_or(source)
            .trim_end_matches(".git")
    });
    let mut in_scope = false;
    let unscoped = target.rsplit('/').next().unwrap_or(target);
    listed.lines().map(str::trim).any(|line| {
        match line {
            "User packages:" => {
                in_scope = !project;
                return false;
            }
            "Project packages:" => {
                in_scope = project;
                return false;
            }
            _ => {}
        }
        if !in_scope {
            return false;
        }
        if let Some(spec) = line.strip_prefix("npm:") {
            return spec
                .strip_prefix(target)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('@'));
        }
        if let Some(source) = line.strip_prefix("git:") {
            return source.rsplit('/').next().is_some_and(|name| {
                name.split('@')
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(".git")
                    == unscoped
            });
        }
        let path = Path::new(line);
        path.is_absolute() && path.is_dir() && path_package_matches(path, target, unscoped)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{ok, ScriptedSystem};
    use crate::{InstallStep, Operation, Receipt, ReceiptStatus};

    #[test]
    fn package_verification_requires_the_exact_identity_and_destination() {
        let listed = "User packages:\n npm:@tintinweb/pi-subagents-extra@1.0.0\n npm:@other/foo@1\nProject packages:\n npm:@tintinweb/pi-subagents@0.19.0\n npm:@example/foo@1";
        assert!(!pi_package_installed(
            listed,
            "@tintinweb/pi-subagents",
            false
        ));
        assert!(pi_package_installed(
            listed,
            "npm:@tintinweb/pi-subagents@latest",
            true
        ));
        assert!(!pi_package_installed(listed, "@example/foo", false));
        assert!(pi_package_installed(listed, "@example/foo", true));
        assert!(!pi_package_installed(
            "npm:@tintinweb/pi-subagents",
            "@tintinweb/pi-subagents",
            false
        ));
        let root = std::env::temp_dir().join(format!(
            "loom-path-pkg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package = root
            .join("github-agrici-daniel-claude-obsidian")
            .join("2.1.1");
        std::fs::create_dir_all(&package).unwrap();
        let listed = format!("Project packages:\n  {}\n", package.display());
        assert!(pi_package_installed(
            &listed,
            "github:AgriciDaniel/claude-obsidian",
            true
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    /// The four answers to "is it here?", in column order: the chooser
    /// (`detect_installed`), install verification (`step_is_present`), status
    /// (healthy once Loom expects the resource) and uninstall (receipt is
    /// `Clean`).
    fn answers(
        root: &Path,
        resource_id: &str,
        selection: &str,
        on_path: &[&str],
        pi_list: &str,
        herdr_list: &str,
    ) -> [bool; 4] {
        let catalog = crate::Catalog::embedded().unwrap();
        let resource = catalog
            .resources
            .iter()
            .find(|resource| resource.id == resource_id)
            .unwrap()
            .clone();
        let selection_file = crate::manifest::conf_d_target(root);
        std::fs::create_dir_all(selection_file.parent().unwrap()).unwrap();
        std::fs::write(&selection_file, selection).unwrap();
        // Unreadable settings and no Herdr registry: the chooser falls back
        // to the same list commands the other callers run.
        let settings = crate::settings::pi_agent_dir(root).join("settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&settings, "not json").unwrap();

        let (step, receipt) = match resource.kind {
            crate::ResourceKind::Tool => (
                Operation::Tools {
                    tools: vec![resource.install_target.clone()],
                },
                Receipt::MiseTool {
                    key: resource.install_target.clone(),
                },
            ),
            crate::ResourceKind::PiPackage => (
                Operation::PiPackage {
                    spec: resource.pi_install_spec(),
                    name: resource.install_target.clone(),
                    project: false,
                },
                Receipt::Manager {
                    manager: "pi".into(),
                    target: resource.pi_install_spec(),
                },
            ),
            _ => (
                Operation::HerdrPlugin {
                    source: resource.install_target.clone(),
                    name: resource.id.trim_start_matches("herdr-plugin:").into(),
                },
                Receipt::Manager {
                    manager: "herdr".into(),
                    target: resource.id.trim_start_matches("herdr-plugin:").into(),
                },
            ),
        };
        // Status reports against what Loom expects: the ownership ledger.
        let mut state = crate::InstallState {
            schema_version: 1,
            resources: std::collections::BTreeMap::new(),
        };
        state.record(crate::OwnedResource {
            id: resource.id.clone(),
            scope: crate::OwnershipScope::Global,
            depends_on: Vec::new(),
            receipts: vec![receipt.clone()],
        });
        state.save(root).unwrap();

        let binaries = [&["pi", "herdr"], on_path].concat();
        let system = ScriptedSystem::new()
            .home(root)
            .cwd(root)
            .only(&binaries)
            .on("pi list", ok(pi_list))
            .on("herdr plugin list", ok(herdr_list));
        let destination =
            crate::SkillDestination::new(Vec::new(), crate::SkillScope::Global, root, root);
        let status = crate::PrerequisiteStatus {
            pi: true,
            herdr: true,
            mise: true,
        };

        // One chooser pass over many resources asks each manager once.
        let chooser = crate::app::detect_installed(
            &[resource.clone(), resource.clone()],
            status,
            &system,
            &destination,
        )[0];
        assert_eq!(system.shown(), ["pi list", "herdr plugin list"]);
        let verify = crate::install::step_is_present(
            &InstallStep {
                target: resource.id.clone(),
                operation: step,
            },
            &system,
            &AtomicBool::new(false),
        );
        let status = crate::status::print_managed_resources(&system, &crate::ui::Out::plain());
        let uninstall = crate::uninstall::receipt_status_on_system(&receipt, &system, root)
            == ReceiptStatus::Clean;
        // No caller spells a probe of its own.
        assert!(system
            .shown()
            .iter()
            .all(|command| command == "pi list" || command == "herdr plugin list"));
        [chooser, verify, status, uninstall]
    }

    type Case = (
        &'static str,
        &'static str,
        &'static str,
        &'static [&'static str],
        &'static str,
        &'static str,
        [bool; 4],
    );

    #[test]
    fn every_caller_reads_the_same_listings_the_same_way() {
        const MARKER: &str = "LOOM_TEST_PRESENCE_HOME";
        let Some(root) = std::env::var_os(MARKER) else {
            let root = std::env::temp_dir().join(format!(
                "loom-presence-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&root).unwrap();
            // Isolate the registries in a child; never mutate the parallel test harness's environment.
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "presence::tests::every_caller_reads_the_same_listings_the_same_way",
                ])
                .env(MARKER, &root)
                .env("HOME", &root)
                .env("USERPROFILE", &root)
                .env("XDG_CONFIG_HOME", root.join(".config"))
                .env_remove("PI_CODING_AGENT_DIR")
                .status()
                .unwrap();
            std::fs::remove_dir_all(root).unwrap();
            assert!(status.success());
            return;
        };
        let root = std::path::PathBuf::from(root);
        const TOOL: &str = "tool:gh";
        const PACKAGE: &str = "pi-package:@tintinweb/pi-subagents";
        const PLUGIN: &str = "herdr-plugin:annotate";
        const SELECTED: &str = "[tools]\ngh = \"2.97.0\"\n";
        const NO_PACKAGES: &str = "User packages:\n";
        const NO_PLUGINS: &str = "0 plugins installed\n";
        let all = [true; 4];
        let none = [false; 4];

        // (case, resource, selection, extra binaries on PATH, `pi list`,
        //  `herdr plugin list`, [chooser, install-verify, status, uninstall])
        let table: &[Case] = &[
            // Tools. The chooser offers anything honestly installed; install
            // verification and status want mise to manage a working binary;
            // uninstall only asks whether the Selection still holds the key.
            (
                "tool selected and on PATH",
                TOOL,
                SELECTED,
                &["gh"],
                NO_PACKAGES,
                NO_PLUGINS,
                all,
            ),
            (
                "tool selected, binary missing",
                TOOL,
                SELECTED,
                &[],
                NO_PACKAGES,
                NO_PLUGINS,
                [true, false, false, true],
            ),
            // Status is healthy here only because it never lists a tool
            // outside the Selection.
            (
                "tool on PATH from another installer",
                TOOL,
                "",
                &["gh"],
                NO_PACKAGES,
                NO_PLUGINS,
                [true, false, true, false],
            ),
            (
                "tool absent",
                TOOL,
                "",
                &[],
                NO_PACKAGES,
                NO_PLUGINS,
                [false, false, true, false],
            ),
            // Pi packages.
            (
                "package in the user scope",
                PACKAGE,
                "",
                &[],
                "User packages:\n  npm:@tintinweb/pi-subagents@0.19.0\n",
                NO_PLUGINS,
                all,
            ),
            ("package absent", PACKAGE, "", &[], NO_PACKAGES, NO_PLUGINS, none),
            // Open decision: uninstall's substring rule ignores the scope...
            (
                "package only in the project scope",
                PACKAGE,
                "",
                &[],
                "User packages:\nProject packages:\n  npm:@tintinweb/pi-subagents@0.19.0\n",
                NO_PLUGINS,
                [false, false, false, true],
            ),
            // ...and its first-fragment fallback is `npm:` for a scoped
            // package, which any listed npm package satisfies.
            (
                "only another npm package",
                PACKAGE,
                "",
                &[],
                "User packages:\n  npm:pi-web-access@1.0.0\n",
                NO_PLUGINS,
                [false, false, false, true],
            ),
            // Herdr plugins.
            (
                "plugin listed",
                PLUGIN,
                "",
                &[],
                NO_PACKAGES,
                "1 plugins installed:\n- annotate (Annotate) enabled [github:Yassimba/herdr-annotate@b11b8e9]\n  config: /home/.config/herdr/plugins/config/annotate\n",
                all,
            ),
            ("plugin absent", PLUGIN, "", &[], NO_PACKAGES, NO_PLUGINS, none),
            // Open decision: uninstall's substring rule accepts a lookalike.
            (
                "only a plugin whose id contains the name",
                PLUGIN,
                "",
                &[],
                NO_PACKAGES,
                "1 plugins installed:\n- annotate-pro (Pro) enabled [github:someone/annotate-pro@1]\n",
                [false, false, false, true],
            ),
        ];
        for (case, resource, selection, on_path, pi_list, herdr_list, expected) in table {
            assert_eq!(
                answers(&root, resource, selection, on_path, pi_list, herdr_list),
                *expected,
                "{case}: [chooser, install-verify, status, uninstall]"
            );
        }
    }
}
