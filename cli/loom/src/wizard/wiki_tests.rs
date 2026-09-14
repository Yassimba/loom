use super::*;
use crate::wiki::{VaultHealth, VaultRecord, WikiOperation, WikiRegistry};
use crate::wizard::wiki::{wiki_capabilities, Capability};
use pretty_assertions::assert_eq;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{atomic::Ordering, mpsc, Mutex};

fn root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("loom-inline-wiki-{name}-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

fn chooser(home: &Path) -> Wizard {
    let catalog = crate::Catalog::embedded().unwrap();
    let mut model = model(ready());
    model.resources = catalog.resources;
    model.profiles = catalog.profiles;
    model.installed = vec![true; model.resources.len()]; // Global installs never satisfy a Wiki.
    model.settings.clear();
    model.setting_states.clear();
    model.skill_destination.home = home.to_path_buf();
    model.skill_destination.project_root = home.to_path_buf();
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    wizard.selected.fill(false);
    let mut browser = crate::wizard::wiki::WikiBrowser::default();
    browser.load(home);
    wizard.wiki = browser;
    go_to_group(&mut wizard, "Knowledgebase");
    wizard
}

fn local_skill(root: &Path, name: &str, content: &str) {
    let path = root.join(".agents/skills").join(name);
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("SKILL.md"), content).unwrap();
}

#[test]
fn registered_wiki_can_be_unregistered_from_setup() {
    let home = root("unregister");
    let vault = home.join("Vault");
    fs::create_dir_all(&vault).unwrap();
    let mut registry = WikiRegistry::default();
    registry.vaults.push(VaultRecord {
        path: vault.clone(),
        feynman: false,
        confluence: false,
        qmd: false,
    });
    registry.save(&home).unwrap();
    let mut wizard = chooser(&home);

    let output = screen(&mut wizard, 160, 28);
    assert!(output.contains("u unregisters"), "{output}");
    assert!(press(&mut wizard, &[KeyCode::Char('u')]).is_none());
    let output = screen(&mut wizard, 160, 28);
    assert!(output.contains("Unregister this Wiki?"), "{output}");
    assert!(output.contains("files will remain untouched"), "{output}");
    press(&mut wizard, &[KeyCode::Esc]);
    assert!(wizard.wiki.confirm_unregister.is_none());
    press(&mut wizard, &[KeyCode::Char('u')]);
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::UnregisterWiki(path)) if path == vault
    ));
    assert!(vault.is_dir());
    fs::remove_dir_all(home).unwrap();
}

#[test]
#[cfg(not(windows))]
fn folder_picks_stay_inline_and_checkmarks_are_vault_local() {
    let home = root("chooser");
    local_skill(&home, "research", "global copy");
    let mut wizard = chooser(&home);
    let a = home.join("A");
    let b = home.join("B");
    let browser = &mut wizard.wiki;
    browser.picked_path(WikiOperation::Create, a.clone());
    browser.item_cursor = 4;
    browser.toggle();
    assert_eq!(browser.count(), 2);
    assert!(!browser.installed(browser.record().unwrap(), Capability::Skill("research")));
    browser.picked_path(WikiOperation::Create, b.clone());
    assert!(!browser
        .selected(browser.record().unwrap())
        .contains(&Capability::Skill("research")));
    browser.cursor = 0;
    browser.item_cursor = 4;
    assert!(!a.exists() && !b.exists());
    assert!(WikiRegistry::load(&home).unwrap().vaults.is_empty());
    assert!(wizard.selected.iter().all(|selected| !selected));
    press(&mut wizard, &[KeyCode::Right, KeyCode::Enter]);
    assert_eq!(choose(&wizard).focus, Pane::Items);
    for (width, height) in [(160, 28), (60, 24), (40, 12)] {
        let output = screen(&mut wizard, width, height);
        println!("--- Knowledgebase {width}x{height} ---\n{output}");
        assert!(output.contains("research"), "{output}");
        assert!(output.contains("[x] research"), "{output}");
    }
    // Mouse toggles the same row; no Wiki action can leave the chooser.
    screen(&mut wizard, 160, 28);
    let (area, offset) = wizard.hits.list.unwrap();
    assert_eq!(offset, 0);
    assert!(wizard.handle_click(area.x + 4, area.y + 5).is_none());
    assert!(!wizard
        .wiki
        .selected(&wizard.wiki.vaults[0])
        .contains(&Capability::Skill("research")));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    local_skill(&a, "research", "local copy");
    let browser = &wizard.wiki;
    assert!(browser.installed(&browser.vaults[0], Capability::Skill("research")));
    assert!(!browser.installed(&browser.vaults[1], Capability::Skill("research")));
    let output = screen(&mut wizard, 160, 28);
    assert!(output.contains("✓ research"), "{output}");
    assert_eq!(
        fs::read_to_string(home.join(".agents/skills/research/SKILL.md")).unwrap(),
        "global copy"
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
#[cfg(not(windows))]
fn wiki_review_and_dry_run_include_only_curated_skills_and_their_dependencies() {
    let home = root("review");
    let vault = home.join("New Wiki");
    let mut wizard = chooser(&home);
    let browser = &mut wizard.wiki;
    browser.picked_path(WikiOperation::Create, vault.clone());
    for cursor in 4..wiki_capabilities().len() {
        browser.item_cursor = cursor;
        browser.toggle();
    }
    let jobs = wizard.wiki_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    let names = jobs[0]
        .resources
        .iter()
        .filter(|r| r.kind == ResourceKind::Skill)
        .map(|r| r.install_target.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 6);
    assert!(names.contains(&"markitdown"));
    assert!(!names.contains(&"brainstorming") && !names.contains(&"tdd"));
    assert!(jobs[0]
        .resources
        .iter()
        .all(|r| matches!(r.kind, ResourceKind::Skill | ResourceKind::Tool)));
    assert!(jobs[0].plan.prerequisites().any(|step| matches!(&step.operation, crate::Operation::Tools { tools } if tools.contains(&"npm:@mermaid-js/mermaid-cli".into()) && tools.contains(&"uv".into()))));
    for step in jobs[0].plan.resources() {
        if let crate::Operation::Skills { destination, .. } = &step.operation {
            assert_eq!(destination.scope, SkillScope::Project);
            assert_eq!(destination.project_root, vault);
            assert_eq!(destination.agents, [SkillAgent::AgentsStandard]);
        }
    }
    wizard.model.dry_run = true;
    press(&mut wizard, &[KeyCode::Char('n')]);
    assert_eq!(title(&wizard), "Review");
    for width in [60, 160] {
        let output = screen(&mut wizard, width, 30);
        println!("--- Review {width} ---\n{output}");
        assert!(
            output.contains("research") && output.contains("New Wiki"),
            "{output}"
        );
        assert!(!output.contains("Vault setup follows"), "{output}");
    }
    let Some(Action::Exit(WizardOutcome::DryRun(plan, summary))) =
        press(&mut wizard, &[KeyCode::Enter])
    else {
        panic!("expected normal dry-run outcome");
    };
    assert!(plan.resources().next().is_none());
    assert!(summary
        .iter()
        .any(|line| line.contains("New Wiki") && line.contains("research")));
    assert!(!vault.exists());
    assert!(WikiRegistry::load(&home).unwrap().vaults.is_empty());
    fs::remove_dir_all(home).unwrap();
}

#[test]
#[cfg(not(windows))]
fn searching_inside_a_wiki_never_selects_global_or_unlisted_skills() {
    let home = root("search");
    let mut wizard = chooser(&home);
    wizard
        .wiki
        .picked_path(WikiOperation::Create, home.join("Search Wiki"));
    press(
        &mut wizard,
        &[KeyCode::Right, KeyCode::Enter, KeyCode::Char('/')],
    );
    for c in "research".chars() {
        press(&mut wizard, &[KeyCode::Char(c)]);
    }
    let output = screen(&mut wizard, 160, 28);
    assert!(
        output.contains("Find here: research") && !output.contains("[ ] write-simply"),
        "{output}"
    );
    assert!(wizard.search.is_none(), "global search must remain closed");
    press(&mut wizard, &[KeyCode::Enter, KeyCode::Char(' ')]);
    assert_eq!(wizard.wiki.count(), 2);
    assert!(wizard.selected.iter().all(|selected| !selected));
    press(&mut wizard, &[KeyCode::End]);
    assert_eq!(wizard.wiki.item_cursor, 9);
    press(&mut wizard, &[KeyCode::PageUp]);
    assert_eq!(wizard.wiki.item_cursor, 0);
    press(&mut wizard, &[KeyCode::PageDown]);
    assert_eq!(wizard.wiki.item_cursor, 9);
    press(&mut wizard, &[KeyCode::Esc]);
    assert_eq!(choose(&wizard).focus, Pane::Kinds);
    press(&mut wizard, &[KeyCode::Esc]);
    assert_eq!(choose(&wizard).focus, Pane::Groups);
    press(&mut wizard, &[KeyCode::BackTab]);
    assert_eq!(choose(&wizard).focus, Pane::Items);
    press(&mut wizard, &[KeyCode::Char('/')]);
    for c in "tdd".chars() {
        press(&mut wizard, &[KeyCode::Char(c)]);
    }
    assert!(screen(&mut wizard, 160, 28).contains("No matches"));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Char('n')]);
    assert_eq!(
        title(&wizard),
        "Choose",
        "n is text while filtering, not Next"
    );
    assert_eq!(wizard.wiki.count(), 2);
    press(
        &mut wizard,
        &[KeyCode::Esc, KeyCode::Home, KeyCode::Char(' ')],
    );
    assert!(screen(&mut wizard, 160, 28).contains("[-] Wiki essentials"));
    go_to_group(&mut wizard, "Everything");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard
        .model
        .resources
        .iter()
        .zip(&wizard.selected)
        .all(|(resource, selected)| resource.group != "Wiki" || !selected));
    fs::remove_dir_all(home).unwrap();
}

struct SetupSystem {
    home: PathBuf,
    commands: Mutex<Vec<crate::CommandSpec>>,
}
impl crate::System for SetupSystem {
    fn command_exists(&self, _: &str) -> bool {
        true
    }
    fn refresh_path(&self) {}
    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }
    fn run(&self, command: &crate::CommandSpec) -> anyhow::Result<crate::CommandResult> {
        self.commands.lock().unwrap().push(command.clone());
        let mut stdout = String::new();
        match command.program.as_str() {
            "curl" => {}
            "tar" => {
                let repo = PathBuf::from(command.args.last().unwrap()).join("fixture");
                fs::create_dir_all(repo.join("manifest"))?;
                fs::write(
                    repo.join("manifest/loom.toml"),
                    include_str!("../../../../manifest/loom.toml"),
                )?;
                for capability in wiki_capabilities() {
                    if let Capability::Skill(name) = capability {
                        let skill = repo.join("skills").join(name);
                        fs::create_dir_all(&skill)?;
                        fs::write(skill.join("SKILL.md"), format!("# {name}"))?;
                    }
                }
            }
            "mise" if command.args.first().is_some_and(|arg| arg == "where") => {
                stdout = self.home.join("product").display().to_string()
            }
            "mise" if command.args.iter().any(|arg| arg == "init") => {
                let index = command.args.iter().position(|arg| arg == "init").unwrap();
                let vault = PathBuf::from(&command.args[index + 1]);
                if command.args.iter().any(|arg| arg == "--apply") {
                    assert!(command.args.iter().any(|arg| arg == "reviewed-core"));
                    fs::create_dir_all(vault.join(".obsidian"))?;
                    fs::write(vault.join(".claude-obsidian.json"), "{}")?;
                    fs::write(vault.join(".gitignore"), ".pi/\n")?;
                } else {
                    stdout = json!({"schema":"claude-obsidian.initialization-plan.v1", "status":"dry-run", "changed_paths":[".obsidian", ".claude-obsidian.json"], "approved_plan_sha256":"reviewed-core"}).to_string();
                }
            }
            "mise" if command.args.iter().any(|arg| arg == "doctor") => {
                stdout = json!({"schema":"claude-obsidian.doctor.v1", "ok":true}).to_string()
            }
            "mise" => {}
            "pi" => {
                let vault = command
                    .cwd
                    .as_ref()
                    .expect("Pi packages must be Vault-local");
                let settings = vault.join(".pi/settings.json");
                if command.args[0] == "install" {
                    assert!(command.args.iter().any(|arg| arg == "-l"));
                    let mut content: serde_json::Value = fs::read_to_string(&settings)
                        .ok()
                        .map(|s| serde_json::from_str(&s).unwrap())
                        .unwrap_or_else(|| json!({"packages":[]}));
                    content["packages"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(command.args.last().unwrap()));
                    fs::create_dir_all(settings.parent().unwrap())?;
                    fs::write(&settings, content.to_string())?;
                } else {
                    assert_eq!(command.args[0], "list");
                    let content: serde_json::Value =
                        serde_json::from_str(&fs::read_to_string(settings)?)?;
                    stdout =
                        "User packages:\n  npm:@companion-ai/feynman@global\nProject packages:\n"
                            .into();
                    for package in content["packages"].as_array().unwrap() {
                        stdout.push_str(&format!("  {}\n", package.as_str().unwrap()));
                    }
                }
            }
            "qmd" if command.args == ["skill", "install"] => {
                local_skill(command.cwd.as_ref().unwrap(), "qmd", "local search")
            }
            "qmd" => stdout = "Total: 0\n".into(),
            _ => anyhow::bail!("unexpected command: {}", command.display()),
        }
        Ok(crate::CommandResult {
            success: true,
            stdout,
            stderr: String::new(),
        })
    }
    fn spawn_detached(&self, _: &crate::CommandSpec) -> anyhow::Result<()> {
        panic!("setup must not launch another UI or Pi");
    }
}

fn install(wizard: &mut Wizard, system: &SetupSystem, approve: bool) {
    let job = wizard.begin_install().unwrap();
    let (sender, events) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| crate::wizard::run_install_job(job, system, &sender));
        loop {
            let event = events
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            if let InstallEvent::Confirm(_, lines, reply) = event {
                assert!(lines[0].contains("Knowledgebase:"));
                assert!(lines.iter().any(|line| line == ".claude-obsidian.json"));
                reply.send(approve).unwrap();
            } else {
                let done = matches!(event, InstallEvent::Finished(_, _));
                wizard.handle_install_event(event);
                if done {
                    break;
                }
            }
        }
    });
}

#[test]
#[cfg(not(windows))]
fn inline_install_requires_file_approval_retries_and_writes_only_the_selected_vault() {
    let home = root("install");
    let vault = home.join("New Wiki");
    local_skill(&home, "research", "global untouched");
    local_skill(&home.join("Other Wiki"), "research", "other untouched");
    let mut wizard = chooser(&home);
    let browser = &mut wizard.wiki;
    browser.picked_path(WikiOperation::Create, vault.clone());
    for cursor in [1, 4, 8] {
        browser.item_cursor = cursor;
        browser.toggle();
    }
    press(&mut wizard, &[KeyCode::Char('n')]);
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::StartInstall)
    ));
    let system = SetupSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };
    install(&mut wizard, &system, false);
    assert!(wizard.can_retry());
    assert!(
        !vault.exists(),
        "declining the exact plan must not create the Vault"
    );
    assert!(WikiRegistry::load(&home).unwrap().vaults.is_empty());
    install(&mut wizard, &system, true);
    let stage = &wizard.install;
    let report = stage.report.as_ref().unwrap();
    assert!(report.failures.is_empty(), "{report:?}");
    assert!(
        matches!(&stage.items[0].status, ExecStatus::Ok(_)),
        "inline Wiki install must finish without QMD unless it was selected"
    );
    assert!(report
        .installed
        .iter()
        .any(|id| id == &format!("wiki:{}", vault.display())));
    let result = screen(&mut wizard, 160, 30);
    println!("--- Result ---\n{result}");
    assert!(
        result.contains("Ready to try") && !result.contains("Wiki setup is next"),
        "{result}"
    );
    assert!(
        result.contains("New Wiki") && result.contains("run pi"),
        "{result}"
    );
    assert!(vault.join(".agents/skills/research/SKILL.md").is_file());
    assert!(vault
        .join(".agents/skills/mermaid-skill/SKILL.md")
        .is_file());
    assert_eq!(
        fs::read_to_string(home.join(".agents/skills/research/SKILL.md")).unwrap(),
        "global untouched"
    );
    assert_eq!(
        fs::read_to_string(home.join("Other Wiki/.agents/skills/research/SKILL.md")).unwrap(),
        "other untouched"
    );
    let registry = WikiRegistry::load(&home).unwrap();
    assert_eq!(
        registry.vaults,
        [VaultRecord {
            path: vault.clone(),
            feynman: true,
            confluence: false,
            qmd: false
        }]
    );
    let ownership = crate::ownership::InstallState::load(&home).unwrap();
    let mermaid_id = format!("project:{}:skill:mermaid-skill", vault.display());
    let mermaid = ownership.resources.get(&mermaid_id).unwrap();
    assert!(mermaid.depends_on.iter().any(|id| id == "tool:mermaid-cli"));
    assert!(ownership.resources.contains_key("tool:mermaid-cli"));
    let commands_before = system.commands.lock().unwrap().len();
    // Retrying another failed task keeps this exact, completed Wiki selection.
    let job = wizard.begin_install().unwrap();
    assert_eq!(job.wikis[0].destination.project_root, vault);
    assert!(job.wikis[0]
        .resources
        .iter()
        .any(|resource| resource.install_target == "research"));
    assert_eq!(job.session.completed.len(), 1);
    wizard.cancelled.store(true, Ordering::Relaxed);
    let (sender, _) = mpsc::channel();
    assert!(job.wikis[0]
        .run(&system, &wizard.cancelled, 0, &sender, &job.session.paths)
        .is_err());
    assert_eq!(system.commands.lock().unwrap().len(), commands_before);
    fs::write(
        home.join(".config/loom/wiki-vaults.json"),
        "broken registry",
    )
    .unwrap();
    wizard.cancelled.store(false, Ordering::Relaxed);
    assert!(job.wikis[0]
        .run(&system, &wizard.cancelled, 0, &sender, &job.session.paths)
        .unwrap_err()
        .to_string()
        .contains("registry"));
    assert_eq!(system.commands.lock().unwrap().len(), commands_before);
    fs::remove_dir_all(home).unwrap();
}

#[test]
#[cfg(not(windows))]
fn selecting_general_skills_on_a_ready_vault_does_not_schedule_wiki_repair() {
    let home = root("ready");
    let vault = home.join("Existing");
    fs::create_dir_all(&vault).unwrap();
    let mut registry = WikiRegistry::default();
    registry.vaults.push(VaultRecord {
        path: vault.clone(),
        feynman: false,
        confluence: false,
        qmd: false,
    });
    registry.save(&home).unwrap();
    let mut wizard = chooser(&home);
    let browser = &mut wizard.wiki;
    browser.health.insert(
        vault.clone(),
        VaultHealth {
            healthy: true,
            rows: vec![
                (crate::ui::Mark::Ok, "claude-obsidian", "ready".into()),
                (crate::ui::Mark::Ok, "qmd", "ready".into()),
            ],
        },
    );
    browser.item_cursor = 4;
    browser.toggle();
    let jobs = wizard.wiki_jobs().unwrap();
    assert!(jobs[0].operation.is_none());
    assert_eq!(jobs[0].resources.len(), 1);
    press(&mut wizard, &[KeyCode::Char('n'), KeyCode::Enter]);
    let system = SetupSystem {
        home: home.clone(),
        commands: Mutex::new(Vec::new()),
    };
    install(&mut wizard, &system, false); // No file-plan prompt is needed for a new skill copy.
    assert!(!wizard.can_retry());
    assert!(vault.join(".agents/skills/research/SKILL.md").is_file());
    assert!(system
        .commands
        .lock()
        .unwrap()
        .iter()
        .all(|command| matches!(command.program.as_str(), "curl" | "tar")));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn wiki_skills_are_reviewed_catalog_skills() {
    let catalog = crate::Catalog::embedded().unwrap();
    let list = wiki_capabilities();
    assert_eq!(
        &list[..4],
        [
            Capability::Essentials,
            Capability::Feynman,
            Capability::Confluence,
            Capability::Qmd,
        ]
    );
    assert!(list.len() > 4);
    for capability in &list[4..] {
        let Capability::Skill(name) = capability else {
            panic!("wiki skill list mixed in a special capability");
        };
        assert!(
            catalog.resources.iter().any(|resource| {
                resource.kind == crate::ResourceKind::Skill && resource.install_target == *name
            }),
            "{name}"
        );
    }
}
