use crate::{manifest, CommandSpec, System};
use anyhow::{Context, Result};
use health::{offer_finish_actions, open_obsidian, open_url};
use product::{
    canonical_vault, ensure_pi_ignored, initialize_vault, install_packages,
    offer_global_feynman_migration, product_root, qmd_index, registry_match_path, setup_qmd,
    wiki_skill_names, wiki_tool_keys, Confirm,
};
use std::path::PathBuf;

mod health;
mod product;
mod registry;

pub(crate) use health::{inspect_vault, obsidian_installed, VaultHealth};
pub use health::{status_registered, update_registered};
pub(crate) use product::absolute_vault_target;
pub use registry::{VaultRecord, WikiRegistry};

pub const PRODUCT_KEY: &str = "github:AgriciDaniel/claude-obsidian";
pub const PYTHON_KEY: &str = "python";
pub(super) const QMD_KEY: &str = "npm:@tobilu/qmd";
pub(super) const CONFLUENCE_KEY: &str = "pipx:confluence-markdown-exporter";
pub(super) const CONFLUENCE_SKILL: &str = "confluence-export";

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
    pub qmd: bool,
    pub yes: bool,
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
    let (vault_target, feynman, confluence, qmd) = match request.operation {
        WikiOperation::Status => return Ok(WikiOutcome::Finished(status_registered(system))),
        WikiOperation::Unregister => {
            let mut registry = WikiRegistry::load(&home)?;
            let path = registry_match_path(system, &registry, &request.vault);
            anyhow::ensure!(
                registry.unregister(&path),
                "Vault is not registered; use Create or Adopt before managing it"
            );
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
            let record = registry
                .vaults
                .into_iter()
                .find(|record| record.path == path)
                .context("Vault is not registered; use Create or Adopt before managing it")?;
            if request.operation != WikiOperation::Repair {
                anyhow::ensure!(
                    record.path.is_dir(),
                    "registered Vault is missing: {}",
                    record.path.display()
                );
                if request.operation == WikiOperation::Open {
                    return open_obsidian(system, &record.path).map(WikiOutcome::Finished);
                }
                system.spawn_detached(
                    &CommandSpec::new("pi", std::iter::empty::<&str>()).in_dir(&record.path),
                )?;
                return Ok(WikiOutcome::Finished(true));
            }
            (
                record.path,
                record.feynman || request.feynman,
                record.confluence || request.confluence,
                record.qmd || request.qmd,
            )
        }
        WikiOperation::Create | WikiOperation::Adopt => (
            absolute_vault_target(
                system,
                &request.vault,
                request.operation == WikiOperation::Create,
            )?,
            request.feynman,
            request.confluence,
            request.qmd,
        ),
    };
    let initializing = matches!(
        request.operation,
        WikiOperation::Create | WikiOperation::Adopt
    );
    let repository = crate::skills::Repository::default();
    crate::wiki_progress::run(
        system,
        interactive,
        "Preparing Wiki Vault",
        |system, local_cancelled| {
            let cancelled = if inline { cancelled } else { local_cancelled };
            manifest::sync_selected_from(
                system,
                &wiki_tool_keys(qmd, confluence),
                cancelled,
                &repository,
            )
            .map_err(anyhow::Error::msg)
        },
    )?;
    let product = product_root(system)?;
    if initializing
        && !initialize_vault(system, &product, &request.operation, &vault_target, confirm)?
    {
        return Ok(WikiOutcome::Incomplete);
    }
    let vault = canonical_vault(&vault_target)?;
    if initializing && !ensure_pi_ignored(system, &home, &product, &vault, confirm)? {
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
            if qmd {
                setup_qmd(system, &vault)
            } else {
                Ok(String::new())
            }
        },
    )?;
    if !inline && !search_note.is_empty() {
        println!("{search_note}");
    }
    notes.push(search_note);
    let mut registry = WikiRegistry::load(&home)?;
    registry.register(vault.clone(), feynman, confluence, qmd);
    registry.save(&home)?;
    if confluence && interactive {
        crate::wiki_confluence::configure(system)?;
    }
    if feynman && !inline && initializing {
        if let Err(error) = offer_global_feynman_migration(system, &vault, request.yes) {
            println!("Global Feynman was left unchanged: {error}");
        }
    }
    if !inline {
        println!("Wiki ready. Run: cd {} && pi", vault.display());
        if qmd {
            println!(
                "Search: qmd --index {} query \"your question\"",
                qmd_index(&vault)
            );
        }
    }
    if confluence && request.yes && !inline {
        println!("Confluence: run `cme config edit auth.confluence` to configure authentication.");
    }
    if interactive && initializing {
        offer_finish_actions(system, &vault)?;
    }
    Ok(WikiOutcome::Finished(true))
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
    use super::health::percent_encode_path;
    use super::product::*;
    use super::*;
    use crate::ui::Out;
    use crate::CommandResult;
    use std::fs;
    use std::path::Path;
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
                qmd: false,
            },
        );
        assert!(!health.healthy);
        for (label, expected) in [
            ("qmd", crate::ui::Mark::Off),
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
    fn inspect_requires_qmd_only_when_the_vault_selected_it() {
        struct SharedOnly {
            root: PathBuf,
        }
        impl System for SharedOnly {
            fn command_exists(&self, name: &str) -> bool {
                matches!(name, "pi" | "qmd")
            }
            fn refresh_path(&self) {}
            fn home_dir(&self) -> Option<PathBuf> {
                Some(self.root.clone())
            }
            fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
                Ok(CommandResult {
                    success: command.program == "pi",
                    stdout: "User packages:\nProject packages:\n".into(),
                    stderr: String::new(),
                })
            }
        }
        let root = temp("optional-qmd");
        let skill = root.join(".agents/skills/qmd");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "leftover").unwrap();
        let selection = crate::manifest::conf_d_target(&root);
        fs::create_dir_all(selection.parent().unwrap()).unwrap();
        fs::write(&selection, "[tools]\n\"npm:@tobilu/qmd\" = \"1\"\n").unwrap();
        let system = SharedOnly { root: root.clone() };
        let leftover = inspect_vault(
            &system,
            &VaultRecord {
                path: root.clone(),
                feynman: false,
                confluence: false,
                qmd: false,
            },
        );
        fs::remove_file(skill.join("SKILL.md")).unwrap();
        let selected = inspect_vault(
            &system,
            &VaultRecord {
                path: root.clone(),
                feynman: false,
                confluence: false,
                qmd: true,
            },
        );
        assert!(leftover
            .rows
            .iter()
            .any(|(mark, name, _)| *name == "qmd" && matches!(mark, crate::ui::Mark::Off)));
        assert!(selected
            .rows
            .iter()
            .any(|(mark, name, _)| *name == "qmd" && matches!(mark, crate::ui::Mark::Bad)));
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
        registry.register(b.clone(), false, false, false);
        registry.register(a.clone(), false, false, false);
        registry.register(a.clone(), true, false, false);
        registry.vaults.push(VaultRecord {
            path: a.clone(),
            feynman: false,
            confluence: true,
            qmd: true,
        });
        registry.save(&home).unwrap();
        let loaded = WikiRegistry::load(&home).unwrap();
        assert_eq!(
            loaded.vaults,
            [
                VaultRecord {
                    path: a.clone(),
                    feynman: true,
                    confluence: false,
                    qmd: false
                },
                VaultRecord {
                    path: b.clone(),
                    feynman: false,
                    confluence: false,
                    qmd: false
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
                qmd: false,
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
        assert!(!wiki_tool_keys(false, false)
            .iter()
            .any(|key| key == CONFLUENCE_KEY || key == QMD_KEY));
        assert!(wiki_tool_keys(false, true)
            .iter()
            .any(|key| key == CONFLUENCE_KEY));
        assert!(wiki_tool_keys(true, false).iter().any(|key| key == QMD_KEY));
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
        registry.register(vault.clone(), true, false, false);
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
        registry.register(missing.clone(), false, false, false);
        registry.register(present.clone(), false, false, false);
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
                qmd: false,
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
