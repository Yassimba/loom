#[path = "wiki_tests.rs"]
mod inline_wiki;

use super::state::*;
use crate::settings::{
    KeyCommand, SettingChange, SettingSpec, SettingState, SettingsPaths, ZedKeybinding,
};
use crate::{
    Platform, PrerequisiteStatus, Profile, Resource, ResourceKind, SkillAgent, SkillDestination,
    SkillScope,
};
use pretty_assertions::assert_eq;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use serde_json::json;

fn resource(kind: ResourceKind, group: &str, label: &str) -> Resource {
    Resource {
        id: format!("{group}:{label}"),
        kind,
        group: group.into(),
        label: label.into(),
        description: "described".into(),
        install_target: label.into(),
        next_action: "next".into(),
        dependencies: Vec::new(),
        bin: None,
        version: None,
        source: None,
        windows_wsl: false,
        companions: Vec::new(),
        bundled_skills: Vec::new(),
    }
}

/// Indices: 0 subagents, 1 themes, 2 reviewr, 3 tdd, 4 refactor, 5 mermaid,
/// 6 gh.
fn catalog() -> Vec<Resource> {
    vec![
        resource(ResourceKind::PiPackage, "Pi packages", "subagents"),
        resource(ResourceKind::PiPackage, "Pi packages", "themes"),
        resource(ResourceKind::HerdrPlugin, "Herdr plugins", "reviewr"),
        resource(ResourceKind::Skill, "Coding", "tdd"),
        resource(ResourceKind::Skill, "Coding", "refactor"),
        resource(ResourceKind::Skill, "Diagrams", "mermaid"),
        resource(ResourceKind::Tool, "Tools", "gh"),
    ]
}

fn test_settings() -> Vec<SettingSpec> {
    vec![
        SettingSpec {
            id: "herdr-key:reviewr".into(),
            group: "Herdr keybinds".into(),
            label: "Reviewr sidebar toggle".into(),
            description: "described".into(),
            related_resource: Some("Herdr plugins:reviewr".into()),
            change: SettingChange::HerdrKeyCommands {
                commands: vec![KeyCommand {
                    key: "prefix+r".into(),
                    kind: "plugin_action".into(),
                    command: "yassimba.reviewr.toggle".into(),
                    description: None,
                }],
            },
        },
        SettingSpec {
            id: "zed:zoomed-padding".into(),
            group: "Zed".into(),
            label: "Zoomed panes edge-to-edge".into(),
            description: "described".into(),
            related_resource: None,
            change: SettingChange::ZedValue {
                key: "zoomed_padding".into(),
                value: json!(false),
            },
        },
        SettingSpec {
            id: "zed:reviewr-history-keys".into(),
            group: "Zed".into(),
            label: "⌘ arrows step Reviewr history".into(),
            description: "described".into(),
            related_resource: Some("Herdr plugins:reviewr".into()),
            change: SettingChange::ZedKeymap {
                context: "Terminal".into(),
                bindings: vec![ZedKeybinding {
                    key: "cmd-left".into(),
                    action: json!(["terminal::SendText", "x"]),
                }],
            },
        },
    ]
}

fn ready() -> PrerequisiteStatus {
    PrerequisiteStatus {
        pi: true,
        herdr: true,
        mise: true,
    }
}

fn model(status: PrerequisiteStatus) -> Model {
    let settings = test_settings();
    Model {
        mode: crate::app::SelectionMode::Add,
        purpose: WizardPurpose::Install,
        uninstall_dependencies: std::collections::BTreeMap::new(),
        resources: catalog(),
        profiles: vec![
            Profile {
                id: "engineer".into(),
                label: "Engineer".into(),
                description: "Build software".into(),
                resources: vec![
                    "Coding:tdd".into(),
                    "Coding:refactor".into(),
                    "Tools:gh".into(),
                ],
            },
            Profile {
                id: "data-engineer".into(),
                label: "Data Engineer".into(),
                description: "Build data systems".into(),
                resources: vec![
                    "Coding:tdd".into(),
                    "Diagrams:mermaid".into(),
                    "Tools:gh".into(),
                ],
            },
        ],
        installed: vec![false; catalog().len()],
        setting_states: vec![SettingState::NotApplied; settings.len()],
        settings,
        zed_present: false,
        settings_paths: SettingsPaths {
            herdr_config: "/tmp/herdr-config.toml".into(),
            zed_settings: "/tmp/zed-settings.json".into(),
            zed_keymap: "/tmp/zed-keymap.json".into(),
            pi_fff_config: "/tmp/pi-fff.json".into(),
            pi_adhd_flag: "/tmp/.i-have-adhd-always".into(),
        },
        status,
        platform: Platform::Unix,
        dry_run: false,
        skill_destination: SkillDestination::new(
            SkillAgent::ALL.to_vec(),
            SkillScope::Global,
            std::path::Path::new("/tmp/loom-test-home"),
            std::path::Path::new("/tmp/loom-test-project"),
        ),
    }
}

fn wizard() -> Wizard {
    Wizard::new(model(ready()), crate::wizard::wiki::WikiBrowser::default())
}

#[test]
fn mcp_servers_flow_through_choose_where_review_with_gateway_exposure() {
    for name in ["sem", "context7"] {
        let root =
            std::env::temp_dir().join(format!("loom-mcp-wizard-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let mut model = model(ready());
        model.resources = crate::Catalog::embedded()
            .unwrap()
            .find(&[
                format!("mcp-server:{name}"),
                "pi-package:pi-mcp-adapter".into(),
            ])
            .unwrap();
        model.installed = vec![false; 2];
        model.settings.clear();
        model.profiles.clear();
        model.dry_run = true;
        model.skill_destination =
            SkillDestination::new(vec![SkillAgent::Pi], SkillScope::Global, &root, &root);
        let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
        assert_eq!(wizard.item_state(Item::Resource(0)), ItemState::Available);
        assert_eq!(wizard.item_state(Item::Resource(1)), ItemState::Picked);
        wizard.selected[0] = true;
        press(&mut wizard, &[KeyCode::Enter]);
        assert!(wizard.screen == Screen::Where);
        let rendered = screen(&mut wizard, 120, 40);
        assert!(rendered.contains("MCP goes to Pi only"), "{rendered}");
        assert!(rendered.contains("MCP not yet verified"), "{rendered}");
        press(
            &mut wizard,
            &[KeyCode::Home, KeyCode::Char(' '), KeyCode::Enter],
        );
        assert_eq!(wizard.skill_scope, SkillScope::Project);
        let rendered = screen(&mut wizard, 120, 40);
        assert!(rendered.contains("directTools=false"), "{rendered}");
        assert!(rendered.contains("mcp-adapter"), "{rendered}");
        assert!(
            rendered.contains(".pi/mcp.json") || rendered.contains(r".pi\mcp.json"),
            "{rendered}"
        );
        assert!(!rendered.contains("blocked"), "{rendered}");
        assert!(matches!(
            press(&mut wizard, &[KeyCode::Enter]),
            Some(Action::Exit(WizardOutcome::DryRun(_, _)))
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn press(wizard: &mut Wizard, codes: &[KeyCode]) -> Option<Action> {
    let mut action = None;
    for &code in codes {
        action = wizard.handle_key(key(code));
    }
    action
}

fn screen(wizard: &mut Wizard, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn choose(wizard: &Wizard) -> &ChooseStage {
    &wizard.choose
}

fn group_titles(wizard: &Wizard) -> Vec<String> {
    choose(wizard)
        .groups
        .iter()
        .map(|group| group.title.clone())
        .collect()
}

fn current_row(wizard: &Wizard) -> Item {
    choose(wizard).row().cloned().unwrap()
}

fn cursor(wizard: &Wizard) -> usize {
    match wizard.screen {
        Screen::Choose => wizard.choose.item_cursor,
        Screen::Where => wizard.where_cursor,
        _ => unreachable!(),
    }
}

/// Park the Choose cursor on a row, using only the keys a user has.
fn go_to(wizard: &mut Wizard, row: Item) {
    let (group, kind, index) = choose(wizard)
        .groups
        .iter()
        .enumerate()
        .find_map(|(g, candidate)| {
            candidate.kinds.iter().enumerate().find_map(|(k, section)| {
                section
                    .rows
                    .iter()
                    .position(|r| *r == row)
                    .map(|i| (g, k, i))
            })
        })
        .unwrap();
    press(wizard, &[KeyCode::Left, KeyCode::Left, KeyCode::Home]);
    press(wizard, &vec![KeyCode::Down; group]);
    press(wizard, &[KeyCode::Right, KeyCode::Home]);
    if !wizard.model.profiles.is_empty() && wizard.model.purpose == WizardPurpose::Install {
        press(wizard, &vec![KeyCode::Down; kind]);
        press(wizard, &[KeyCode::Right, KeyCode::Home]);
    }
    press(wizard, &vec![KeyCode::Down; index]);
    assert_eq!(current_row(wizard), row);
}

/// Park the groups-column cursor on a titled group.
fn go_to_group(wizard: &mut Wizard, title: &str) {
    let group = group_titles(wizard)
        .iter()
        .position(|candidate| candidate == title)
        .unwrap();
    press(wizard, &[KeyCode::Left, KeyCode::Left, KeyCode::Home]);
    press(wizard, &vec![KeyCode::Down; group]);
    assert_eq!(choose(wizard).group().title, title);
}

fn title(wizard: &Wizard) -> &'static str {
    wizard.screen.title()
}

#[test]
fn choose_lists_profiles_then_settings_in_catalog_order() {
    let wizard = wizard();
    assert_eq!(
        group_titles(&wizard),
        ["Engineer", "Data Engineer", "Everything"]
    );
    assert!(choose(&wizard).groups[2].everything);
    assert_eq!(choose(&wizard).groups[2].bulk_rows.len(), catalog().len());
    assert_eq!(
        choose(&wizard).groups[0]
            .kinds
            .iter()
            .map(|kind| kind.title.as_str())
            .collect::<Vec<_>>(),
        ["Skills", "Tools", "Settings"]
    );
    // Start on the first goal without selecting it, ready to move left-to-right.
    assert_eq!(choose(&wizard).focus, Pane::Groups);
    assert_eq!(choose(&wizard).group().title, "Engineer");
    assert_eq!(current_row(&wizard), Item::Resource(3));
}

#[test]
fn filtered_and_empty_profiles_use_only_resources_in_the_model() {
    let mut model = model(ready());
    model.resources.truncate(1);
    model.profiles = vec![
        Profile {
            id: "empty".into(),
            label: "Empty after filtering".into(),
            description: "No resources on this platform".into(),
            resources: vec!["missing".into()],
        },
        Profile {
            id: "available".into(),
            label: "Available here".into(),
            description: "One resource on this platform".into(),
            resources: vec![model.resources[0].id.clone(), "missing".into()],
        },
    ];
    model.settings.clear();
    let wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());

    assert_eq!(group_titles(&wizard), ["Available here", "Everything"]);
    assert_eq!(
        choose(&wizard).groups[0].items().collect::<Vec<_>>(),
        [Item::Resource(0)]
    );
}

#[test]
fn fully_installed_sections_show_a_checkmark_instead_of_a_goal_checkbox() {
    let section_mark = |output: &str, title: &str| {
        output
            .lines()
            .filter_map(|line| line.split('│').nth(1))
            .find(|cell| cell.contains(title))
            .unwrap()
            .split_once(title)
            .unwrap()
            .0
            .trim()
            .to_owned()
    };
    for width in [60, 160] {
        for pick_goal in [false, true] {
            let mut model = model(ready());
            model.settings.clear();
            let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
            if pick_goal {
                press(&mut wizard, &[KeyCode::Char(' ')]);
            }
            let checkbox = if pick_goal { "[x]" } else { "[ ]" };
            assert_eq!(
                section_mark(&screen(&mut wizard, width, 24), "Engineer"),
                checkbox
            );

            let mut installed = vec![false; catalog().len()];
            installed[3] = true;
            wizard.set_installed(installed.clone());
            assert_eq!(
                section_mark(&screen(&mut wizard, width, 24), "Engineer"),
                checkbox,
                "partly installed or merely picked is not complete"
            );

            installed[4] = true;
            installed[6] = true;
            wizard.set_installed(installed);
            let output = screen(&mut wizard, width, 24);
            assert_eq!(section_mark(&output, "Engineer"), "✓", "{output}");
            assert_ne!(section_mark(&output, "Everything"), "✓");
            press(&mut wizard, &[KeyCode::Right]);
            let types = screen(&mut wizard, 60, 24);
            assert_eq!(section_mark(&types, "Skills"), "✓");
            assert_eq!(section_mark(&types, "Tools"), "✓");

            wizard.set_installed(vec![true; catalog().len()]);
            press(&mut wizard, &[KeyCode::Left]);
            assert_eq!(
                section_mark(&screen(&mut wizard, width, 24), "Everything"),
                "✓"
            );
        }
    }
}

#[test]
fn concise_labels_keep_selection_counts_and_installed_scope_without_repetition() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to(&mut wizard, Item::Resource(3));
    let output = screen(&mut wizard, 200, 32);
    assert!(output.contains("Skills · 1/2 picked"), "{output}");
    assert!(!output.contains("Capabilities ·"), "{output}");
    assert!(!output.contains("Picked individually"), "{output}");
    assert!(output.contains("described"), "{output}");
    assert!(wizard.hits.kinds.is_some() && wizard.hits.list.is_some());
    assert!(output.contains("Overview"), "{output}");

    let mut installed = vec![false; wizard.model.resources.len()];
    installed[3] = true;
    installed[4] = true;
    wizard.set_installed_scoped(installed.clone(), vec![false; installed.len()]);
    let output = screen(&mut wizard, 200, 32);
    assert!(output.contains("Skills · 2 installed"), "{output}");
    assert!(!output.contains("0 required"), "{output}");
    assert!(!output.contains("0 unavailable"), "{output}");
    assert_eq!(output.matches("Already installed").count(), 1, "{output}");
    assert!(output.contains('✓'), "{output}");
}

#[test]
fn choose_renders_profiles_mixed_kinds_and_required_tools() {
    let mut model = model(ready());
    model.resources[3].dependencies = vec!["gh".into()];
    model.profiles[0]
        .resources
        .extend([model.resources[0].id.clone(), model.resources[2].id.clone()]);
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to(&mut wizard, Item::Resource(3));
    press(
        &mut wizard,
        &[KeyCode::Char(' '), KeyCode::Left, KeyCode::Left],
    );

    // On a goal, the third column previews the goal instead of one type.
    let profile_output = screen(&mut wizard, 160, 28);
    assert!(profile_output.contains("Goals"));
    assert!(profile_output.contains("Types"));
    assert!(!profile_output.contains("Capabilities"));
    assert!(profile_output.contains("Overview"));
    assert!(
        profile_output.contains("space picks all"),
        "{profile_output}"
    );
    assert!(profile_output.contains("Skills"));
    assert!(profile_output.contains("Tools"));
    assert!(profile_output.contains("Pi packages"));
    assert!(profile_output.contains("Herdr plugins"));
    go_to(&mut wizard, Item::Resource(6));
    let output = screen(&mut wizard, 160, 28);

    assert!(!output.contains("Goals"));
    assert!(output.contains("Types"));
    assert!(output.contains("Tools · 1 required"));
    assert!(!output.contains("Capabilities ·"));
    assert!(output.contains("Overview"));
    assert!(output.contains("Skills"));
    assert!(output.contains("Tools"));
    assert!(output.contains("needed by tdd"), "{output}");

    press(&mut wizard, &[KeyCode::Left]);
    let restored = screen(&mut wizard, 160, 28);
    assert!(restored.contains("Goals"));
    assert!(restored.contains("Tools · 1 required"));
    assert!(!restored.contains("Overview"));
}

#[test]
fn stages_are_choose_where_review_install_and_where_needs_skills() {
    let mut wizard = wizard();
    assert_eq!(
        wizard.visible_stages(),
        [Screen::Choose, Screen::Review, Screen::Install]
    );
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(
        wizard.visible_stages(),
        [
            Screen::Choose,
            Screen::Where,
            Screen::Review,
            Screen::Install
        ]
    );
    press(&mut wizard, &[KeyCode::Enter]);
    assert_eq!(title(&wizard), "Where");
    press(&mut wizard, &[KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
    press(&mut wizard, &[KeyCode::Esc, KeyCode::Esc]);
    assert_eq!(title(&wizard), "Choose");
}

#[test]
fn a_tool_only_selection_skips_where() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(6));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
}

#[test]
fn space_picks_and_steps_down() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(3));
    let before = cursor(&wizard);
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected[3]);
    assert_eq!(cursor(&wizard), before + 1);
    press(&mut wizard, &[KeyCode::Up, KeyCode::Char(' ')]);
    assert!(!wizard.selected[3]);
}

#[test]
fn space_on_a_profile_toggles_only_its_direct_members() {
    let mut model = model(ready());
    model.resources[3].dependencies = vec!["mermaid".into()];
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to_group(&mut wizard, "Engineer");

    press(&mut wizard, &[KeyCode::Char(' ')]);

    assert_eq!(
        wizard.selected,
        [false, false, false, true, true, false, true],
        "the visible dependency stays required instead of becoming a direct pick"
    );
    assert_eq!(wizard.required_note(5).as_deref(), Some("tdd"));
    assert!(choose(&wizard)
        .group()
        .items()
        .any(|item| item == Item::Resource(5)));
    // The cursor stays on the profile so a second space clears it.
    assert_eq!(choose(&wizard).focus, Pane::Groups);
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected.iter().all(|on| !*on));
}

#[test]
fn combined_goals_keep_shared_items_and_respect_individual_choices() {
    let mut wizard = wizard();
    go_to_group(&mut wizard, "Engineer");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to_group(&mut wizard, "Data Engineer");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to_group(&mut wizard, "Engineer");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected[3], "Data Engineer still needs tdd");
    assert!(wizard.selected[6], "Data Engineer still includes gh");
    assert!(!wizard.selected[4], "refactor belonged only to Engineer");
    go_to(&mut wizard, Item::Resource(3));
    let output = screen(&mut wizard, 160, 32);
    assert!(output.contains("Included by Data Engineer"), "{output}");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to_group(&mut wizard, "Engineer");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(
        !wizard.selected[3],
        "an explicit exclusion survives goal changes"
    );
    assert_eq!(
        wizard
            .expanded_selection()
            .iter()
            .filter(|r| r.label == "gh")
            .count(),
        1,
    );
}

#[test]
fn review_separates_picks_dependencies_and_safe_changes_at_both_widths() {
    let mut model = model(ready());
    model.resources[3].dependencies = vec!["mermaid".into()];
    model.settings.clear();
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to_group(&mut wizard, "Engineer");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to(&mut wizard, Item::Resource(1));
    press(
        &mut wizard,
        &[KeyCode::Char(' '), KeyCode::Enter, KeyCode::Enter],
    );
    assert_eq!(title(&wizard), "Review");
    let wide = screen(&mut wizard, 160, 32);
    for text in [
        "Selected capabilities",
        "Required to work",
        "Writes & notes",
        "Included by Engineer",
        "themes",
        "Needed by tdd",
        "Skills go to",
    ] {
        assert!(wide.contains(text), "missing {text}:\n{wide}");
    }
    let mut narrow = screen(&mut wizard, 40, 14);
    for _ in 0..80 {
        press(&mut wizard, &[KeyCode::Down]);
        narrow.push_str(&screen(&mut wizard, 40, 14));
    }
    for text in [
        "Selected capabilities",
        "Required to work",
        "Writes & notes",
        "themes",
        "mermaid",
        "Engineer",
    ] {
        assert!(narrow.contains(text), "missing {text}:\n{narrow}");
    }
    for output in [&wide, &narrow] {
        assert!(!output.contains("Picked individually"), "{output}");
        assert!(!output.contains("scroll to inspect"), "{output}");
        assert!(output.contains("↑↓ scroll"), "{output}");
    }
    assert!(
        !wizard.install_running(),
        "review and scrolling never install"
    );
}

#[test]
fn ready_uses_successful_goal_actions_and_freezes_completed_step_time() {
    let mut model = model(ready());
    model.resources[3].next_action = "Run /tdd in Pi".into();
    model.resources[5].next_action = "Run /mermaid-skill in Pi".into();
    model.profiles[1].resources.swap(0, 1);
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to_group(&mut wizard, "Engineer");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to_group(&mut wizard, "Data Engineer");
    press(
        &mut wizard,
        &[
            KeyCode::Char(' '),
            KeyCode::Enter,
            KeyCode::Enter,
            KeyCode::Enter,
        ],
    );
    let job = wizard.begin_install().unwrap();
    wizard.handle_install_event(InstallEvent::Status(0, ExecStatus::Running));
    {
        let stage = &mut wizard.install;
        stage.items[0].started =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
    }
    wizard.handle_install_event(InstallEvent::Status(0, ExecStatus::Ok("installed".into())));
    {
        let stage = &wizard.install;
        assert!(stage.items[0].started.is_none());
        assert!(stage.items[0].elapsed.as_secs() >= 3);
    }
    let report = crate::InstallReport {
        installed: vec!["skills".into()],
        failures: vec![],
    };
    assert_eq!(
        wizard.goal_next_actions(&report),
        vec![
            ("Engineer".into(), "Run /tdd in Pi".into()),
            ("Data Engineer".into(), "Run /mermaid-skill in Pi".into()),
        ]
    );
    assert!(wizard
        .goal_next_actions(&crate::InstallReport::default())
        .is_empty());
    finish_test_job(&mut wizard, job, report);
    let output = screen(&mut wizard, 100, 32);
    for text in [
        "Ready to try",
        "Run /tdd in Pi",
        "Run /mermaid-skill in Pi",
        "3s",
    ] {
        assert!(output.contains(text), "missing {text}:\n{output}");
    }
}

#[test]
fn space_on_a_type_toggles_only_direct_capabilities_of_that_type() {
    let mut model = model(ready());
    model.resources[3].dependencies = vec!["mermaid".into()];
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());

    press(&mut wizard, &[KeyCode::Right, KeyCode::Char(' ')]);

    assert_eq!(choose(&wizard).focus, Pane::Kinds);
    assert_eq!(choose(&wizard).kind().title, "Skills");
    assert!(wizard.selected[3]);
    assert!(wizard.selected[4]);
    assert!(!wizard.selected[5]);
    assert_eq!(wizard.required_note(5).as_deref(), Some("tdd"));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected.iter().all(|on| !*on));
}

#[test]
fn arrows_move_between_columns_and_down_changes_the_group() {
    let mut wizard = wizard();
    press(&mut wizard, &[KeyCode::Left]);
    assert_eq!(choose(&wizard).focus, Pane::Groups);
    press(&mut wizard, &[KeyCode::Down]);
    assert_eq!(choose(&wizard).group().title, "Data Engineer");
    assert_eq!(cursor(&wizard), 0, "a new profile starts at its first row");
    press(&mut wizard, &[KeyCode::Right]);
    assert_eq!(choose(&wizard).focus, Pane::Kinds);
    assert_eq!(choose(&wizard).kind().title, "Skills");
    press(&mut wizard, &[KeyCode::Right]);
    assert_eq!(choose(&wizard).focus, Pane::Items);
    assert_eq!(current_row(&wizard), Item::Resource(3));
    press(&mut wizard, &[KeyCode::Tab]);
    assert_eq!(choose(&wizard).focus, Pane::Groups);
    press(&mut wizard, &[KeyCode::BackTab, KeyCode::End]);
    assert_eq!(choose(&wizard).focus, Pane::Items);
    assert_eq!(current_row(&wizard), Item::Resource(5));
    press(&mut wizard, &[KeyCode::PageUp]);
    assert_eq!(current_row(&wizard), Item::Resource(3));
    press(&mut wizard, &[KeyCode::PageDown]);
    assert_eq!(current_row(&wizard), Item::Resource(5));
    press(&mut wizard, &[KeyCode::Home, KeyCode::BackTab]);
    assert_eq!(current_row(&wizard), Item::Resource(3));
    assert_eq!(choose(&wizard).focus, Pane::Kinds);
}

#[test]
fn everything_picks_the_whole_catalog_and_clears_it_again() {
    let mut model = model(ready());
    model.installed[0] = true;
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to_group(&mut wizard, "Everything");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(
        wizard.selected,
        [false, true, true, true, true, true, true],
        "everything but the installed one"
    );
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected.iter().all(|on| !*on));
}

#[test]
fn installed_resources_show_but_cannot_be_picked() {
    let mut model = model(ready());
    model.installed[0] = true;
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to(&mut wizard, Item::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.selected[0]);
    assert_eq!(wizard.item_state(Item::Resource(0)), ItemState::Installed);
}

#[test]
fn the_probe_drops_picks_it_proves_redundant() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    let mut installed = vec![false; 7];
    installed[0] = true;
    wizard.set_installed(installed);
    assert!(!wizard.selected[0]);
    assert!(!wizard.probing);
}

#[test]
fn dependencies_lock_as_needed_and_cannot_be_deselected() {
    let mut model = model(ready());
    model.resources[3].dependencies = vec!["mermaid".into()];
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(wizard.required_note(5).as_deref(), Some("tdd"));
    assert!(wizard.item_on(Item::Resource(5)));
    go_to(&mut wizard, Item::Resource(5));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.selected[5], "locked rows do not flip");
    assert!(wizard.required_note(5).is_some());
}

#[test]
fn search_returns_an_overlapping_capability_once() {
    let mut wizard = wizard();
    for (query, expected) in [
        ("gh", vec![(0, Item::Resource(6))]),
        ("zoomed", vec![(0, Item::Setting(1))]),
        ("no-such-capability", vec![]),
        (
            "",
            vec![
                (2, Item::Resource(0)),
                (2, Item::Resource(1)),
                (2, Item::Resource(2)),
                (0, Item::Resource(3)),
                (0, Item::Resource(4)),
                (1, Item::Resource(5)),
                (0, Item::Resource(6)),
                (0, Item::Setting(0)),
                (0, Item::Setting(1)),
                (0, Item::Setting(2)),
            ],
        ),
    ] {
        wizard.search = Some(query.into());
        assert_eq!(wizard.search_matches(), expected, "query: {query}");
    }
}

#[test]
fn uninstall_keeps_ownership_groups_instead_of_role_profiles() {
    let mut model = model(ready());
    model.purpose = WizardPurpose::Uninstall;
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());

    assert_eq!(
        group_titles(&wizard),
        [
            "Everything",
            "Skills · Coding",
            "Skills · Diagrams",
            "Tools",
            "Pi packages",
            "Herdr plugins",
            "Settings · Herdr keybinds",
            "Settings · Zed",
        ]
    );
    press(&mut wizard, &[KeyCode::BackTab]);
    assert_eq!(choose(&wizard).focus, Pane::Items);
    press(&mut wizard, &[KeyCode::BackTab]);
    assert_eq!(choose(&wizard).focus, Pane::Groups);
}

#[test]
fn settings_precheck_follows_the_related_plugin_and_respects_touches() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Setting(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(
        !wizard.setting_on[0],
        "a related setting cannot be selected before its package"
    );
    go_to(&mut wizard, Item::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(
        wizard.setting_on,
        [true, false, false],
        "no Zed: keymap stays off"
    );
    go_to(&mut wizard, Item::Setting(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.setting_on[0]);
    // Toggling the plugin off and on again does not override the user's no.
    go_to(&mut wizard, Item::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to(&mut wizard, Item::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected[2]);
    assert!(!wizard.setting_on[0]);
}

#[test]
fn zed_settings_precheck_only_with_zed_present() {
    let mut model = model(ready());
    model.zed_present = true;
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    assert_eq!(wizard.setting_on, [false, true, false]);
    go_to(&mut wizard, Item::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(wizard.setting_on, [true, true, true]);
}

#[test]
fn applied_settings_cannot_be_selected() {
    let mut model = model(ready());
    model.setting_states[1] = SettingState::Applied;
    model.zed_present = true;
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    assert!(!wizard.setting_on[1]);
    go_to(&mut wizard, Item::Setting(1));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.setting_on[1]);
}

#[test]
fn where_toggles_scope_and_agents_and_reports_exact_trees() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    assert_eq!(title(&wizard), "Where");
    assert_eq!(cursor(&wizard), 1, "the first agent, not the scope row");
    press(&mut wizard, &[KeyCode::Home, KeyCode::Char(' ')]);
    assert_eq!(wizard.skill_scope, SkillScope::Project);
    press(&mut wizard, &[KeyCode::Down, KeyCode::Char(' ')]);
    assert!(!wizard.agent_on[0]);
    assert_eq!(cursor(&wizard), 2, "space steps to the next agent");
    let destination = wizard.skill_destination();
    assert_eq!(destination.scope, SkillScope::Project);
    assert_eq!(destination.agents.len(), SkillAgent::ALL.len() - 1);
    assert!(destination
        .trees()
        .iter()
        .all(|tree| tree.starts_with("/tmp/loom-test-project")));
}

#[test]
fn review_then_enter_starts_the_install() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::StartInstall)
    ));
    assert_eq!(title(&wizard), "Install");
    let job = wizard.begin_install().unwrap();
    assert_eq!(job.session.plan.resources().count(), 1);
}

#[test]
fn empty_selection_confirms_as_nothing_selected() {
    let mut wizard = wizard();
    press(&mut wizard, &[KeyCode::Enter]);
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::Exit(WizardOutcome::NothingSelected))
    ));
}

#[test]
fn unbuildable_plan_blocks_the_install() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    // Turn every agent off.
    for _ in SkillAgent::ALL {
        press(&mut wizard, &[KeyCode::Char(' ')]);
    }
    assert!(wizard.selected_agents().is_empty());
    press(&mut wizard, &[KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
    assert!(press(&mut wizard, &[KeyCode::Enter]).is_none());
    assert_eq!(title(&wizard), "Review");
}

#[test]
fn dry_run_exits_with_the_plan_instead_of_installing() {
    let mut model = model(ready());
    model.dry_run = true;
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to(&mut wizard, Item::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    match press(&mut wizard, &[KeyCode::Enter]) {
        Some(Action::Exit(WizardOutcome::DryRun(plan, _))) => {
            assert_eq!(plan.resources().count(), 1);
        }
        _ => panic!("expected a dry-run exit"),
    }
}

#[test]
fn wiki_resources_are_exclusive_to_wiki_even_when_globally_present() {
    let mut model = model(ready());
    let wiki = resource(ResourceKind::PiPackage, "Wiki", "feynman");
    model.resources.push(wiki.clone());
    model.installed.push(true);
    model.profiles[0].resources.push(wiki.id.clone());
    model.profiles.push(Profile {
        id: "knowledge-wiki".into(),
        label: "Wiki".into(),
        description: "Choose a Vault".into(),
        resources: vec![wiki.id],
    });
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    let index = wizard.model.resources.len() - 1;
    for group in &choose(&wizard).groups {
        assert_eq!(
            group.items().any(|item| item == Item::Resource(index)),
            group.title == "Wiki"
        );
    }
    assert!(!wizard.resource_installed(index));
    go_to(&mut wizard, Item::Resource(index));
    let rendered = screen(&mut wizard, 120, 30);
    assert!(rendered.contains("No registered Wikis yet"), "{rendered}");
    assert!(!rendered.contains("pi install npm:feynman"), "{rendered}");
}

#[test]
fn retry_keeps_the_reviewed_plan_destination_and_completed_items() {
    let mut wizard = wizard();
    wizard.selected[0] = true;
    wizard.selected[1] = true;
    press(&mut wizard, &[KeyCode::Enter, KeyCode::Enter]);
    let initial = wizard.begin_install().unwrap();
    let initial_plan = initial.session.plan.clone();
    wizard.handle_install_event(InstallEvent::Status(0, ExecStatus::Ok("installed".into())));
    wizard.handle_install_event(InstallEvent::Status(
        1,
        ExecStatus::Failed("timed out after 30s".into()),
    ));
    finish_test_job(
        &mut wizard,
        initial,
        crate::InstallReport {
            installed: vec!["Pi packages:subagents".into()],
            failures: vec![crate::InstallFailure {
                target: "Pi packages:themes".into(),
                message: "timed out after 30s".into(),
            }],
        },
    );
    let rendered = screen(&mut wizard, 100, 28);
    assert!(rendered.contains("Retry"), "{rendered}");
    assert!(rendered.contains("Completed work stays"), "{rendered}");
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::StartInstall)
    ));
    let retried = wizard.begin_install().unwrap();
    assert_eq!(retried.session.plan, initial_plan);
    assert_eq!(retried.session.completed, vec![0]);
    assert!(!retried.cancelled.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn retry_rechecks_completed_packages_and_reinstalls_only_missing_or_failed_work() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };
    struct RetrySystem {
        installed: Mutex<Vec<String>>,
        commands: Mutex<Vec<crate::CommandSpec>>,
        fail_once: AtomicBool,
    }
    impl crate::System for RetrySystem {
        fn command_exists(&self, _: &str) -> bool {
            true
        }
        fn refresh_path(&self) {}
        fn run(&self, command: &crate::CommandSpec) -> anyhow::Result<crate::CommandResult> {
            self.commands.lock().unwrap().push(command.clone());
            if command.args[0] == "list" {
                return Ok(crate::CommandResult {
                    success: true,
                    stdout: format!(
                        "User packages:\n{}\nProject packages:\n  npm:subagents@latest",
                        self.installed.lock().unwrap().join("\n")
                    ),
                    stderr: String::new(),
                });
            }
            let name = command.args.last().unwrap().clone();
            let success =
                !name.contains("themes") || !self.fail_once.swap(false, Ordering::Relaxed);
            if success {
                self.installed.lock().unwrap().push(name);
            }
            Ok(crate::CommandResult {
                success,
                stdout: String::new(),
                stderr: if success {
                    String::new()
                } else {
                    "ETIMEDOUT Authorization: Bearer PRIVATE-TOKEN".into()
                },
            })
        }
    }
    for (removed_after_success, cancel_retry) in [(false, false), (true, false), (false, true)] {
        let system = RetrySystem {
            installed: Mutex::new(Vec::new()),
            commands: Mutex::new(Vec::new()),
            fail_once: AtomicBool::new(true),
        };
        let mut wizard = wizard();
        wizard.selected[0] = true;
        wizard.selected[1] = true;
        press(&mut wizard, &[KeyCode::Enter, KeyCode::Enter]);
        let reviewed_destination = wizard.skill_destination();
        let initial = wizard.begin_install().unwrap();
        let initial_plan = initial.session.plan.clone();
        let (sender, events) = std::sync::mpsc::channel();
        super::run_install_job(initial, &system, &sender);
        for event in events.try_iter() {
            wizard.handle_install_event(event);
        }
        let rendered = screen(&mut wizard, 100, 28);
        assert!(rendered.contains("timed out"), "{rendered}");
        assert!(!rendered.contains("PRIVATE-TOKEN"));
        assert!(wizard.can_retry());
        if removed_after_success {
            system.installed.lock().unwrap().clear();
        }
        // Late initial probes must not discard the frozen selection.
        wizard.set_installed(vec![true; wizard.model.resources.len()]);
        assert!(wizard.selected[0] && wizard.selected[1]);
        assert!(matches!(
            press(&mut wizard, &[KeyCode::Char('r')]),
            Some(Action::StartInstall)
        ));
        let retry = wizard.begin_install().unwrap();
        assert_eq!(retry.session.plan, initial_plan);
        retry.cancelled.store(cancel_retry, Ordering::Relaxed);
        super::run_install_job(retry, &system, &sender);
        for event in events.try_iter() {
            wizard.handle_install_event(event);
        }
        assert_eq!(wizard.can_retry(), cancel_retry);
        let Some(Action::Exit(WizardOutcome::Installed {
            report,
            resources,
            destination,
            written,
        })) = press(&mut wizard, &[KeyCode::Esc])
        else {
            panic!("expected install result");
        };
        assert_eq!(report.installed.len(), if cancel_retry { 0 } else { 2 });
        assert_eq!(report.failures.is_empty(), !cancel_retry);
        assert_eq!(resources.len(), 2);
        assert_eq!(destination, reviewed_destination);
        assert_eq!(written.len(), if cancel_retry { 1 } else { 2 });
        assert!(written.contains(&"Pi packages:subagents".into()));
        let commands = system.commands.lock().unwrap();
        let installs = |name: &str| {
            commands
                .iter()
                .filter(|command| {
                    command.args[0] == "install" && command.args.last().unwrap().contains(name)
                })
                .count()
        };
        assert_eq!(
            installs("subagents"),
            if removed_after_success { 2 } else { 1 }
        );
        assert_eq!(installs("themes"), if cancel_retry { 1 } else { 2 });
    }
}

#[test]
fn install_events_drive_the_install_screen_to_completion() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(0));
    press(
        &mut wizard,
        &[KeyCode::Char(' '), KeyCode::Enter, KeyCode::Enter],
    );
    let job = wizard.begin_install().unwrap();
    assert!(wizard.install_running());
    assert!(press(&mut wizard, &[KeyCode::Enter]).is_none(), "keys wait");
    wizard.handle_install_event(InstallEvent::Status(0, ExecStatus::Ok("installed".into())));
    let report = crate::InstallReport {
        installed: vec!["Pi packages:subagents".into()],
        failures: vec![],
    };
    finish_test_job(&mut wizard, job, report);
    assert!(!wizard.install_running());
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::Exit(WizardOutcome::Installed { report, .. })) if report.installed.len() == 1
    ));
}

#[test]
fn quitting_with_picks_asks_first_and_esc_on_choose_quits() {
    let mut empty = Wizard::new(model(ready()), crate::wizard::wiki::WikiBrowser::default());
    assert!(matches!(
        press(&mut empty, &[KeyCode::Esc]),
        Some(Action::Exit(WizardOutcome::Cancelled))
    ));
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(press(&mut wizard, &[KeyCode::Char('q')]).is_none());
    assert!(wizard.confirm_quit);
    assert!(press(&mut wizard, &[KeyCode::Char('n')]).is_none());
    assert!(!wizard.confirm_quit);
    // A reflex enter stays; only y/q throw the picks away.
    press(&mut wizard, &[KeyCode::Char('q')]);
    assert!(press(&mut wizard, &[KeyCode::Enter]).is_none());
    assert!(!wizard.confirm_quit);
    press(&mut wizard, &[KeyCode::Char('q')]);
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Char('y')]),
        Some(Action::Exit(WizardOutcome::Cancelled))
    ));
}

#[test]
fn c_clears_every_pick_but_keeps_setup_requirements() {
    let mut wizard = wizard();
    go_to_group(&mut wizard, "Engineer");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.user_picked() > 0);
    press(&mut wizard, &[KeyCode::Char('c')]);
    assert_eq!(wizard.user_picked(), 0);
    assert!(wizard.picked_goals.is_empty());
}

#[test]
fn search_filters_picks_and_lands_the_cursor() {
    let mut wizard = wizard();
    press(
        &mut wizard,
        &[KeyCode::Char('/'), KeyCode::Char('m'), KeyCode::Char('e')],
    );
    let matches = wizard.search_matches();
    let labels = matches
        .iter()
        .map(|&hit| match hit.1 {
            Item::Resource(index) => wizard.model.resources[index].label.clone(),
            Item::Setting(index) => wizard.model.settings[index].label.clone(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        ["mermaid", "themes", "Zoomed panes edge-to-edge"],
        "best label match first, ties in catalog order"
    );
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected[5]);
    press(&mut wizard, &[KeyCode::Up, KeyCode::Enter]);
    assert!(wizard.search.is_none());
    assert_eq!(current_row(&wizard), Item::Resource(5));
    assert_eq!(choose(&wizard).group().title, "Data Engineer");
}

#[test]
fn search_ranks_the_closest_label_first() {
    let mut wizard = wizard();
    press(&mut wizard, &[KeyCode::Char('/')]);
    for c in "refac".chars() {
        press(&mut wizard, &[KeyCode::Char(c)]);
    }
    let first = wizard.search_matches()[0];
    assert_eq!(first.1, Item::Resource(4), "refactor outranks fuzzy hits");
}

#[test]
fn clicking_a_type_then_a_row_toggles_that_capability() {
    let mut wizard = wizard();
    let mut terminal = Terminal::new(TestBackend::new(110, 30)).unwrap();
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    let (groups, _) = wizard.hits.groups.unwrap();
    let engineer = group_titles(&wizard)
        .iter()
        .position(|title| title == "Engineer")
        .unwrap() as u16;
    wizard.handle_click(groups.x + 3, groups.y + 1 + engineer);
    terminal.draw(|frame| wizard.draw(frame)).unwrap();

    let (kinds, _) = wizard.hits.kinds.unwrap();
    wizard.handle_click(kinds.x + 3, kinds.y + 2); // Tools
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    assert_eq!(choose(&wizard).kind().title, "Tools");

    let (area, _) = wizard.hits.list.unwrap();
    wizard.handle_click(area.x + 3, area.y + 1);
    assert!(wizard.selected[6]);
}

#[test]
fn every_stage_renders_without_panicking() {
    let mut wizard = wizard();
    let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    press(&mut wizard, &[KeyCode::Char('?')]);
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    press(
        &mut wizard,
        &[KeyCode::Esc, KeyCode::Char('/'), KeyCode::Char('z')],
    );
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    press(&mut wizard, &[KeyCode::Esc, KeyCode::Enter]);
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    press(&mut wizard, &[KeyCode::Enter]);
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    press(&mut wizard, &[KeyCode::Enter]);
    let job = wizard.begin_install().unwrap();
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    finish_test_job(
        &mut wizard,
        job,
        crate::InstallReport {
            installed: vec!["skills".into()],
            failures: vec![],
        },
    );
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    // A tiny terminal must not panic either.
    let mut tiny = Terminal::new(TestBackend::new(20, 6)).unwrap();
    tiny.draw(|frame| wizard.draw(frame)).unwrap();
}

/// Prints every screen; run with `--nocapture` to eyeball the layout.
#[test]
fn render_gallery() {
    if !crate::snapshot_tests::isolated("wizard::tests::render_gallery") {
        return;
    }
    let mut frames = Vec::new();
    let mut wizard = wizard();
    let mut terminal = Terminal::new(TestBackend::new(104, 26)).unwrap();
    let mut show = |wizard: &mut Wizard, terminal: &mut Terminal<TestBackend>| {
        freeze_gallery_time(wizard);
        terminal.draw(|frame| wizard.draw(frame)).unwrap();
        frames.push(format!("{:?}", terminal.backend().buffer()));
    };
    // Also show the shipped goal names and dependency explanations, not just fixtures.
    let mut catalog_model = model(ready());
    let catalog = crate::Catalog::embedded().unwrap();
    catalog_model.resources = catalog.resources;
    catalog_model.profiles = catalog.profiles;
    catalog_model.installed = vec![false; catalog_model.resources.len()];
    catalog_model.settings.clear();
    let mut goals = Wizard::new(catalog_model, crate::wizard::wiki::WikiBrowser::default());
    let mut wide = Terminal::new(TestBackend::new(160, 34)).unwrap();
    show(&mut goals, &mut wide);
    go_to_group(&mut goals, "Research deeply");
    press(
        &mut goals,
        &[KeyCode::Char(' '), KeyCode::Enter, KeyCode::Enter],
    );
    show(&mut goals, &mut wide);
    show(&mut goals, &mut terminal);

    go_to(&mut wizard, Item::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    show(&mut wizard, &mut terminal);
    go_to_group(&mut wizard, "Everything");
    show(&mut wizard, &mut terminal);
    press(&mut wizard, &[KeyCode::Enter]);
    show(&mut wizard, &mut terminal);
    press(&mut wizard, &[KeyCode::Enter]);
    show(&mut wizard, &mut terminal);
    press(&mut wizard, &[KeyCode::Enter]);
    let job = wizard.begin_install().unwrap();
    wizard.handle_install_event(InstallEvent::Status(0, ExecStatus::Ok("installed".into())));
    wizard.handle_install_event(InstallEvent::Status(1, ExecStatus::Running));
    show(&mut wizard, &mut terminal);
    wizard.handle_install_event(InstallEvent::Status(
        1,
        ExecStatus::Failed(
            "herdr plugin install reviewr --yes\nexit status 1: no such plugin".into(),
        ),
    ));
    finish_test_job(
        &mut wizard,
        job,
        crate::InstallReport {
            installed: vec!["skills".into()],
            failures: vec![crate::InstallFailure {
                target: "Herdr plugins:reviewr".into(),
                message: "exit status 1: no such plugin".into(),
            }],
        },
    );
    show(&mut wizard, &mut terminal);
    press(&mut wizard, &[KeyCode::Char('d')]);
    show(&mut wizard, &mut terminal);

    // Overlays and the narrow single-column layout.
    let mut fresh = self::wizard();
    press(&mut fresh, &[KeyCode::Char('?')]);
    show(&mut fresh, &mut terminal);
    press(
        &mut fresh,
        &[KeyCode::Esc, KeyCode::Char('/'), KeyCode::Char('t')],
    );
    show(&mut fresh, &mut terminal);
    press(&mut fresh, &[KeyCode::Esc]);
    go_to(&mut fresh, Item::Resource(3));
    press(&mut fresh, &[KeyCode::Char(' '), KeyCode::Esc]);
    show(&mut fresh, &mut terminal);
    press(&mut fresh, &[KeyCode::Esc]);
    let mut narrow = Terminal::new(TestBackend::new(60, 20)).unwrap();
    show(&mut fresh, &mut narrow);
    press(&mut fresh, &[KeyCode::Right]);
    show(&mut fresh, &mut narrow);
    press(&mut fresh, &[KeyCode::Right]);
    show(&mut fresh, &mut narrow);
    press(&mut fresh, &[KeyCode::Enter]);
    show(&mut fresh, &mut narrow);
    crate::snapshot_tests::assert_snapshot("wizard-gallery", &frames.join("\n"));
}

#[test]
fn automatic_pi_loom_package_is_hidden_and_selected_for_existing_pi() {
    let mut model = model(ready());
    model.mode = crate::app::SelectionMode::Setup;
    let mut pi_loom = resource(ResourceKind::PiPackage, "Pi packages", "Loom");
    pi_loom.id = "pi-package:@yassimba/pi-loom".into();
    model.resources.push(pi_loom);
    model.installed.push(false);
    let wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());

    assert!(wizard
        .selection()
        .iter()
        .any(Resource::is_automatic_pi_package));
    assert!(choose(&wizard)
        .groups
        .iter()
        .flat_map(|group| group.items())
        .all(|row| !matches!(row, Item::Resource(index) if wizard.model.resources[index].is_automatic_pi_package())));
}

#[test]
fn setup_starts_on_the_first_goal_without_picking_it() {
    let mut model = model(ready());
    model.mode = crate::app::SelectionMode::Setup;
    let wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());

    assert_eq!(choose(&wizard).group().title, "Engineer");
    assert_eq!(choose(&wizard).focus, Pane::Groups);
}

fn vault_browser_wizard() -> Wizard {
    let mut model = model(ready());
    model
        .resources
        .push(resource(ResourceKind::Tool, "Wiki", "claude-obsidian"));
    model.installed.push(false);
    model.profiles.push(Profile {
        id: "knowledge-wiki".into(),
        label: "Wiki".into(),
        description: "Your Wikis".into(),
        resources: vec!["Wiki:claude-obsidian".into()],
    });
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());

    go_to_group(&mut wizard, "Wiki");
    wizard
}

#[test]
fn wiki_browser_keeps_three_lanes_and_inspects_only_the_selected_vault() {
    use crate::wiki::{VaultHealth, VaultRecord};
    let mut wizard = vault_browser_wizard();
    wizard.selected[0] = true; // An unrelated setup choice survives Wiki navigation.
    let picks = wizard.selected.clone();
    let records = ["/tmp/Vault A", "/tmp/Vault B"].map(|path| VaultRecord {
        path: path.into(),
        feynman: false,
        confluence: false,
        qmd: false,
    });
    let browser = &mut wizard.wiki;
    browser.vaults = records.to_vec();
    assert_eq!(browser.next_probe().unwrap(), records[0]);
    assert!(
        browser.next_probe().is_none(),
        "only one health probe at a time"
    );
    browser.checking = false;
    browser.health.insert(
        records[0].path.clone(),
        VaultHealth {
            healthy: false,
            rows: vec![
                (
                    crate::ui::Mark::Ok,
                    "Feynman",
                    "A-only Vault package".into(),
                ),
                (crate::ui::Mark::Off, "qmd", "missing from this Wiki".into()),
                (
                    crate::ui::Mark::Ok,
                    "QMD (shared)",
                    "installed on this machine".into(),
                ),
            ],
        },
    );
    browser.item_cursor = 3;
    let wide = screen(&mut wizard, 160, 28);
    for text in [
        "Goals",
        "Your Wikis",
        "This Wiki",
        "Vault A",
        "Vault B",
        "✓ Feynman",
        "QMD is installed on this machine. Select it to enable this Wiki.",
    ] {
        assert!(wide.contains(text), "missing {text}:\n{wide}");
    }
    if !cfg!(windows) {
        assert!(wide.contains("Connect existing Wiki") && wide.contains("Create new Wiki"));
    }
    println!("{wide}");
    press(&mut wizard, &[KeyCode::Right]);
    let narrow = screen(&mut wizard, 50, 18);
    assert!(
        narrow.contains("Your Wikis") && narrow.contains("Vault A"),
        "{narrow}"
    );
    assert!(
        press(&mut wizard, &[KeyCode::Enter]).is_none(),
        "opening a Wiki only inspects"
    );
    let details = screen(&mut wizard, 50, 24);
    assert!(details.contains("✓ Feynman"), "{details}");
    assert!(
        press(&mut wizard, &[KeyCode::Enter]).is_none(),
        "capabilities stay in the chooser, never open a separate manager"
    );
    press(&mut wizard, &[KeyCode::Esc, KeyCode::Down]);
    let second = screen(&mut wizard, 160, 28);
    assert!(
        second.contains("Checking this Wiki") && !second.contains("✓ Feynman"),
        "{second}"
    );
    assert_eq!(wizard.wiki.next_probe().unwrap(), records[1]);
    assert_eq!(wizard.selected, picks);
    assert!(wizard
        .selection()
        .iter()
        .all(|resource| resource.group != "Wiki"));
    wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(
        title(&wizard),
        "Review",
        "Ctrl-Enter continues the other setup choices"
    );
}

#[test]
fn wiki_browser_selects_new_registrations_and_does_not_replace_a_broken_registry() {
    let root = std::env::temp_dir().join(format!("loom-vault-browser-{}", std::process::id()));
    let record = |name: &str| crate::wiki::VaultRecord {
        path: root.join(name),
        feynman: false,
        confluence: false,
        qmd: false,
    };
    let mut registry = crate::wiki::WikiRegistry::default();
    registry.vaults.push(record("Old Wiki"));
    registry.save(&root).unwrap();
    let mut browser = crate::wizard::wiki::WikiBrowser::default();
    browser.load(&root);
    browser.cursor = browser.vaults.len(); // The Connect action.
    registry.vaults.push(record("New Wiki"));
    registry.save(&root).unwrap();
    browser.load(&root);
    assert_eq!(browser.record(), Some(&record("New Wiki")));
    let path = root.join(".config/loom/wiki-vaults.json");
    std::fs::write(&path, "broken registry").unwrap();
    browser.load(&root);
    assert!(browser.entry().is_none());
    assert!(browser.next_probe().is_none());
    assert!(browser.vaults.is_empty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "broken registry");
    assert!(
        !root.join("New Wiki").exists(),
        "browsing must not recreate missing Vaults"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(not(windows))]
fn empty_wiki_browser_opens_explicit_folder_actions_by_keyboard_and_mouse() {
    let mut wizard = vault_browser_wizard();
    let picks = wizard.selected.clone();
    let output = screen(&mut wizard, 160, 28);
    assert!(output.contains("No registered Wikis yet"), "{output}");
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::PickWiki(crate::wiki::WikiOperation::Adopt))
    ));
    press(&mut wizard, &[KeyCode::Down]);
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::PickWiki(crate::wiki::WikiOperation::Create))
    ));
    screen(&mut wizard, 160, 28);
    let (area, _) = wizard.hits.kinds.unwrap();
    assert!(matches!(
        wizard.handle_click(area.x + 3, area.y + 1),
        Some(Action::PickWiki(crate::wiki::WikiOperation::Adopt))
    ));
    assert!(matches!(
        wizard.handle_click(area.x + 3, area.y + 2),
        Some(Action::PickWiki(crate::wiki::WikiOperation::Create))
    ));
    assert_eq!(wizard.selected, picks);
    assert!(wizard.picked_goals.is_empty());
    for accept in [KeyCode::Enter, KeyCode::Char(' ')] {
        press(&mut wizard, &[KeyCode::Char('/')]);
        for c in "claude-obsidian".chars() {
            press(&mut wizard, &[KeyCode::Char(c)]);
        }
        press(&mut wizard, &[accept]);
        assert!(wizard.browsing_wiki());
        assert_eq!(choose(&wizard).focus, Pane::Kinds);
        assert_eq!(
            wizard.selected, picks,
            "search opens Wiki navigation instead of selecting global Wiki tools"
        );
    }
    press(&mut wizard, &[KeyCode::Char('n')]);
    assert_eq!(
        title(&wizard),
        "Review",
        "Next also works without extended terminal key support"
    );
}

#[test]
fn modal_overlays_consume_mouse_and_scroll_input() {
    let mut wizard = wizard();
    let mut terminal = Terminal::new(TestBackend::new(110, 30)).unwrap();
    go_to(&mut wizard, Item::Resource(3));
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    let before = wizard.selected.clone();
    wizard.show_help = true;
    let (area, _) = wizard.hits.list.unwrap();
    wizard.handle_click(area.x + 3, area.y + 1);
    wizard.handle_scroll(true);
    assert_eq!(wizard.selected, before);
    assert_eq!(cursor(&wizard), 0);

    wizard.show_help = false;
    wizard.confirm_quit = true;
    wizard.handle_click(area.x + 3, area.y + 1);
    assert_eq!(wizard.selected, before);
}

#[test]
fn late_probe_refreshes_only_untouched_contextual_settings() {
    let mut wizard = wizard();
    let mut installed = vec![false; wizard.model.resources.len()];
    installed[2] = true;
    wizard.set_installed(installed);
    assert!(wizard.setting_on[0]);

    wizard.setting_touched[0] = true;
    wizard.setting_on[0] = false;
    wizard.set_installed(vec![false; wizard.model.resources.len()]);
    assert!(!wizard.setting_on[0]);
}

#[test]
fn unavailable_settings_have_an_explicit_state() {
    let wizard = wizard();
    assert!(matches!(
        wizard.item_state(Item::Setting(0)),
        ItemState::Unavailable(_)
    ));
}

#[test]
fn result_keeps_every_distinct_next_action() {
    let mut model = model(ready());
    model.resources[0].next_action = "try subagents".into();
    model.resources[1].next_action = "choose a theme".into();
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    wizard.selected[0] = true;
    wizard.selected[1] = true;
    let report = crate::InstallReport {
        installed: vec!["Pi packages:subagents".into(), "Pi packages:themes".into()],
        failures: vec![],
    };

    assert_eq!(
        wizard.next_actions(&report),
        vec!["try subagents".to_string(), "choose a theme".to_string()]
    );
}

#[test]
fn result_includes_next_actions_from_dependencies() {
    let mut model = model(ready());
    model.resources[0].dependencies = vec![model.resources[1].install_target.clone()];
    model.resources[0].next_action = "use the package".into();
    model.resources[1].next_action = "configure its dependency".into();
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    wizard.selected[0] = true;
    let report = crate::InstallReport {
        installed: vec!["Pi packages:subagents".into(), "Pi packages:themes".into()],
        failures: vec![],
    };

    assert_eq!(
        wizard.next_actions(&report),
        vec![
            "use the package".to_string(),
            "configure its dependency".to_string(),
        ]
    );
}

#[test]
fn running_install_requires_two_ctrl_c_presses_to_cancel() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(0));
    press(
        &mut wizard,
        &[KeyCode::Char(' '), KeyCode::Enter, KeyCode::Enter],
    );
    let _job = wizard.begin_install().unwrap();
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    wizard.handle_key(ctrl_c);
    assert!(wizard.confirm_cancel);
    assert!(!wizard.cancelled.load(std::sync::atomic::Ordering::Relaxed));
    wizard.handle_key(ctrl_c);
    assert!(wizard.cancelled.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn tiny_terminal_asks_for_a_resize() {
    let mut wizard = wizard();
    let mut terminal = Terminal::new(TestBackend::new(30, 8)).unwrap();
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("more room"));
    assert!(wizard.hits.list.is_none());
}

#[test]
fn where_scope_options_fit_a_standard_terminal() {
    let mut wizard = wizard();
    go_to(&mut wizard, Item::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    let mut terminal = Terminal::new(TestBackend::new(104, 26)).unwrap();
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(
        text.contains("(•) All projects    ( ) This project")
            || text.contains("(*) All projects    ( ) This project"),
        "scope options must fit on one row: {text}"
    );
}

#[test]
fn profile_choose_renders_in_plain_terminal_modes() {
    let executable = std::env::current_exe().unwrap();
    for (name, value) in [("NO_COLOR", "1"), ("TERM", "dumb")] {
        let mut command = std::process::Command::new(&executable);
        command.args([
            "--ignored",
            "--exact",
            "wizard::tests::profile_choose_plain_terminal_child",
        ]);
        command.env_remove("NO_COLOR").env("TERM", "xterm");
        command.env(name, value);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{name} child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "run in isolated subprocess by profile_choose_renders_in_plain_terminal_modes"]
fn profile_choose_plain_terminal_child() {
    where_scope_options_fit_a_standard_terminal();
    let mut wizard = wizard();
    let output = screen(&mut wizard, 104, 24);
    assert!(output.contains("Goals"));
    assert!(output.contains("Types"));
    assert!(output.contains("Overview"));
    assert!(!output.contains("Capabilities"));
}

#[test]
fn narrow_terminals_render_one_column_without_panicking() {
    let mut wizard = wizard();
    for (w, h) in [(72u16, 20u16), (60, 20), (40, 12), (24, 8)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| wizard.draw(frame)).unwrap();
        press(&mut wizard, &[KeyCode::Left]);
        terminal.draw(|frame| wizard.draw(frame)).unwrap();
        press(&mut wizard, &[KeyCode::Right]);
    }
    // Under 70 columns only the focused lane is on screen and clickable.
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    press(&mut wizard, &[KeyCode::Left]);
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    assert!(wizard.hits.groups.is_some());
    assert!(wizard.hits.kinds.is_none());
    assert!(wizard.hits.list.is_none());
    press(&mut wizard, &[KeyCode::Right]);
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    assert!(wizard.hits.kinds.is_some());
}

#[test]
fn medium_terminals_keep_two_readable_lanes_and_correct_click_targets() {
    for width in [70, 72, 80, 99] {
        let mut wizard = wizard();
        let output = screen(&mut wizard, width, 20);
        assert!(output.contains("Data Engineer"), "{width}:\n{output}");
        assert!(output.contains("Build software"), "{width}:\n{output}");
        assert!(wizard.hits.groups.is_some());
        assert!(wizard.hits.kinds.is_none());

        press(&mut wizard, &[KeyCode::Right]);
        screen(&mut wizard, width, 20);
        assert!(wizard.hits.groups.is_none());
        assert!(wizard.hits.kinds.is_some());
        assert!(wizard.hits.list.is_some());

        press(&mut wizard, &[KeyCode::Right]);
        let output = screen(&mut wizard, width, 20);
        assert!(output.contains("Overview"), "{width}:\n{output}");
        assert!(output.contains("described"), "{width}:\n{output}");
        let (items, _) = wizard.hits.list.unwrap();
        assert!(items.width >= 34);
        assert!(wizard.hits.kinds.is_none());
        wizard.handle_click(items.x + 2, items.y + 1);
        assert!(
            wizard.selected[3],
            "click should pick tdd at {width} columns"
        );
    }
}

#[test]
fn narrow_item_columns_hide_unreadable_description_fragments() {
    let mut model = model(ready());
    model.resources[4].label = "really-long-capability-label".into();
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to(&mut wizard, Item::Resource(3));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    let (area, _) = wizard.hits.list.unwrap();
    let row = (area.x + 2..area.right() - 2)
        .map(|x| terminal.backend().buffer()[(x, area.y + 1)].symbol())
        .collect::<String>();
    assert!(row.trim_end().ends_with("tdd"), "{row:?}");
}

#[test]
fn narrow_search_help_and_footer_keep_recovery_keys_visible() {
    let mut wizard = wizard();
    let output = screen(&mut wizard, 40, 10);
    assert!(
        output.contains("space pick · enter next · ? keys"),
        "{output}"
    );
    assert!(!output.contains("Back"), "{output}");
    assert_eq!(wizard.hits.next_button.y, 9);
    assert_eq!(wizard.hits.next_button.height, 1);
    assert_eq!(wizard.hits.back_button, ratatui::layout::Rect::default());

    press(&mut wizard, &[KeyCode::Char('?')]);
    let output = screen(&mut wizard, 40, 10);
    for text in ["/ search", "q quit", "first / last", "any key closes this"] {
        assert!(output.contains(text), "missing {text}:\n{output}");
    }
    press(&mut wizard, &[KeyCode::Esc, KeyCode::Char('/')]);
    for c in "zzzzzzzz".chars() {
        press(&mut wizard, &[KeyCode::Char(c)]);
    }
    let output = screen(&mut wizard, 40, 10);
    for text in ["No matches", "Backspace widens", "esc returns to browsing."] {
        assert!(output.contains(text), "missing {text}:\n{output}");
    }
    press(&mut wizard, &[KeyCode::Esc]);
    assert!(wizard.search.is_none());
    screen(&mut wizard, 40, 10);
    let next = wizard.hits.next_button;
    wizard.handle_click(next.x, next.y);
    assert_eq!(wizard.screen, Screen::Review);
    let output = screen(&mut wizard, 40, 10);
    assert!(output.contains("[ ◂ Back ]"), "{output}");
    assert!(wizard.hits.back_button.width > 0);
}

#[test]
fn install_footer_names_cancel_and_retry_instead_of_finish_while_running() {
    let mut wizard = wizard();
    wizard.screen = Screen::Install;
    wizard.install.running = true;
    let output = screen(&mut wizard, 40, 10);
    assert!(output.contains("ctrl-c cancel"), "{output}");
    assert!(!output.contains("enter finish"), "{output}");
    assert_eq!(wizard.hits.next_button, ratatui::layout::Rect::default());
    wizard.install.running = false;
    wizard.install.report = Some(crate::InstallReport {
        installed: vec![],
        failures: vec![crate::InstallFailure {
            target: "test".into(),
            message: "network".into(),
        }],
    });
    let output = screen(&mut wizard, 40, 10);
    assert!(
        output.contains("enter retry · d details · esc finish"),
        "{output}"
    );
    assert!(wizard.hits.next_button.width > 0);
}

#[test]
fn uninstall_starts_selected_and_locks_dependencies_of_kept_resources() {
    let mut model = model(ready());
    model.purpose = WizardPurpose::Uninstall;
    model.resources.truncate(2);
    model.installed.truncate(2);
    let dependency = model.resources[0].id.clone();
    let dependent = model.resources[1].id.clone();
    model
        .uninstall_dependencies
        .insert(dependent, vec![dependency]);
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());

    assert_eq!(wizard.selection().len(), 2);
    wizard.selected[1] = false;

    assert!(matches!(
        wizard.item_state(Item::Resource(0)),
        ItemState::RequiredKeep(_)
    ));
    assert!(wizard.selection().is_empty());
}

#[test]
fn uninstall_review_renders_on_a_narrow_terminal() {
    let mut model = model(ready());
    model.purpose = WizardPurpose::Uninstall;
    model.resources.truncate(2);
    model.installed.truncate(2);
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    press(&mut wizard, &[KeyCode::Enter]);
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();

    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(text.contains("Remove (2)"));
    assert!(text.contains("- subagents"));
}

#[test]
fn bundled_skill_rows_are_included_for_selected_and_verified_installed_packages() {
    let home = std::env::temp_dir().join(format!(
        "loom-wizard-bundle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let catalog = crate::Catalog::embedded().unwrap();
    let mut model = model(ready());
    model.resources = catalog
        .find(&[
            "pi-package:@dietrichgebert/ponytail".into(),
            "skill:ponytail".into(),
        ])
        .unwrap();
    model.profiles.clear();
    model.installed = vec![false; 2];
    model.skill_destination = SkillDestination::new(
        vec![SkillAgent::Pi, SkillAgent::Claude],
        SkillScope::Global,
        &home,
        &home,
    );
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    wizard.selected[0] = true;
    assert_eq!(
        wizard.included_note(1).as_deref(),
        Some("Included with ponytail for Pi")
    );
    assert!(wizard.item_on(Item::Resource(1)));
    wizard.agent_on.fill(false);
    assert!(
        wizard.included_note(1).is_some(),
        "the package supplies its own Pi destination"
    );
    assert!(
        !wizard.has_skills(),
        "no standalone destinations means no copy step"
    );
    assert!(wizard.plan().is_ok());
    for (index, agent) in SkillAgent::ALL.iter().enumerate() {
        wizard.agent_on[index] = matches!(agent, SkillAgent::Pi | SkillAgent::Claude);
    }
    assert_eq!(
        wizard
            .expanded_selection()
            .iter()
            .filter(|r| r.id == "skill:ponytail")
            .count(),
        1
    );
    wizard.selected[1] = true;
    assert_eq!(
        wizard
            .expanded_selection()
            .iter()
            .filter(|r| r.id == "skill:ponytail")
            .count(),
        1
    );
    wizard.selected[0] = false;
    assert!(
        wizard.included_note(1).is_none(),
        "skill alone stays standalone"
    );
    wizard.set_installed(vec![true, false]);
    assert!(
        wizard.included_note(1).is_none(),
        "list output alone is not skill proof"
    );
    let root = home.join(".pi/agent/npm/node_modules/@dietrichgebert/ponytail");
    std::fs::create_dir_all(root.join("skills/ponytail")).unwrap();
    std::fs::write(root.join("skills/ponytail/SKILL.md"), "# bundled").unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"pi":{"skills":["./skills"]}}"#,
    )
    .unwrap();
    let settings = home.join(".pi/agent/settings.json");
    std::fs::write(
        &settings,
        r#"{"packages":["npm:@dietrichgebert/ponytail@4.9.0"]}"#,
    )
    .unwrap();
    assert!(wizard.included_note(1).is_some());
    assert!(
        !wizard.resource_installed(1),
        "Claude still needs its standalone skill"
    );
    wizard.selected[1] = false;
    assert!(!wizard.actionable(&[Item::Resource(1)]).is_empty());
    go_to(&mut wizard, Item::Resource(1));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.nothing_chosen());
    wizard.screen = Screen::Review;
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::StartInstall)
    ));
    std::fs::write(
        &settings,
        r#"{"packages":[{"source":"npm:@dietrichgebert/ponytail@4.9.0","skills":[]}]}"#,
    )
    .unwrap();
    assert!(wizard.included_note(1).is_none());
    std::fs::remove_dir_all(home).unwrap();
}

fn adhd_wizard() -> Wizard {
    let mut model = model(ready());
    model.mode = crate::app::SelectionMode::Setup;
    let mut package = resource(ResourceKind::PiPackage, "Pi packages", "i-have-adhd");
    package.id = "pi-package:i-have-adhd".into();
    model.resources.push(package);
    model.installed.push(false);
    model.settings.clear();
    model.setting_states.clear();
    model.settings_paths.pi_adhd_flag = std::env::temp_dir()
        .join(format!(
            "loom-adhd-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("custom-agent/.i-have-adhd-always");
    Wizard::new(model, crate::wizard::wiki::WikiBrowser::default())
}

#[test]
fn adhd_question_requires_explicit_consent_and_reviews_the_flag() {
    let mut wizard = adhd_wizard();
    press(&mut wizard, &[KeyCode::Enter]);
    assert_eq!(title(&wizard), "Pi responses");
    let rendered = screen(&mut wizard, 90, 18);
    assert!(rendered.contains("Do you have ADHD and want ADHD-friendly responses in Pi?"));
    assert!(rendered.contains("Yes — always enable"));
    assert!(rendered.contains("No — leave settings unchanged"));
    assert!(wizard.selected_settings().is_empty());
    press(&mut wizard, &[KeyCode::Up, KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
    assert!(wizard
        .selection()
        .iter()
        .any(|r| r.id == "pi-package:i-have-adhd"));
    assert_eq!(
        wizard.selected_settings(),
        vec![crate::settings::pi_adhd_setting()]
    );
    assert!(screen(&mut wizard, 180, 30).contains(".i-have-adhd-always"));
    assert!(!wizard.model.settings_paths.pi_adhd_flag.exists());
    // Going back and choosing No removes only the implicit package selection.
    press(&mut wizard, &[KeyCode::Esc, KeyCode::Down, KeyCode::Enter]);
    assert!(wizard.selection().is_empty());
    assert!(wizard.selected_settings().is_empty());
    assert!(screen(&mut wizard, 90, 18).contains("Pi responses: leave settings unchanged"));
}

#[test]
fn adhd_no_bulk_selection_cancel_and_dry_run_never_write() {
    let mut wizard = adhd_wizard();
    // Everything can install the package but must not choose always-on.
    press(&mut wizard, &[KeyCode::Home, KeyCode::Char(' ')]);
    assert!(!wizard.adhd_enabled);
    assert!(wizard.selected_settings().is_empty());
    assert!(!wizard.model.settings_paths.pi_adhd_flag.exists());
    let mut profile_model = wizard.model;
    profile_model.profiles = vec![crate::catalog::Profile {
        id: "responses".into(),
        label: "Responses".into(),
        description: "Pi response tools".into(),
        resources: vec!["pi-package:i-have-adhd".into()],
    }];
    let mut profile_wizard =
        Wizard::new(profile_model, crate::wizard::wiki::WikiBrowser::default());
    go_to_group(&mut profile_wizard, "Responses");
    press(&mut profile_wizard, &[KeyCode::Home, KeyCode::Char(' ')]);
    assert!(profile_wizard
        .selection()
        .iter()
        .any(|r| r.id == "pi-package:i-have-adhd"));
    assert!(profile_wizard.selected_settings().is_empty());
    let mut wizard = adhd_wizard();
    wizard.model.dry_run = true;
    press(&mut wizard, &[KeyCode::Enter, KeyCode::Up, KeyCode::Enter]);
    let Some(Action::Exit(WizardOutcome::DryRun(plan, changes))) =
        press(&mut wizard, &[KeyCode::Enter])
    else {
        panic!("expected dry run");
    };
    assert!(plan
        .resources()
        .any(|r| r.target == "pi-package:i-have-adhd"));
    assert!(changes
        .iter()
        .any(|line| line.contains(".i-have-adhd-always")));
    assert!(!wizard.model.settings_paths.pi_adhd_flag.exists());
    assert!(matches!(
        wizard.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Some(Action::Exit(WizardOutcome::Cancelled))
    ));
    assert!(!wizard.model.settings_paths.pi_adhd_flag.exists());
}

#[test]
fn adhd_question_supports_mouse_resize_and_existing_flags() {
    let mut wizard = adhd_wizard();
    let flag = wizard.model.settings_paths.pi_adhd_flag.clone();
    std::fs::create_dir_all(flag.parent().unwrap()).unwrap();
    std::fs::write(&flag, "keep this content").unwrap();
    press(&mut wizard, &[KeyCode::Enter]);
    assert!(screen(&mut wizard, 70, 15).contains("Already enabled"));
    screen(&mut wizard, 40, 10);
    assert!(wizard.hits.list.is_some());
    let (list, _) = wizard.hits.list.unwrap();
    wizard.handle_click(list.x + 1, list.y + 1);
    press(&mut wizard, &[KeyCode::Enter]);
    assert!(wizard.adhd_enabled);
    press(&mut wizard, &[KeyCode::Esc]);
    screen(&mut wizard, 70, 15);
    let (list, _) = wizard.hits.list.unwrap();
    wizard.handle_click(list.x + 1, list.y + 2);
    press(&mut wizard, &[KeyCode::Enter]);
    assert!(!wizard.adhd_enabled);
    assert!(wizard.selected_settings().is_empty());
    assert_eq!(std::fs::read_to_string(&flag).unwrap(), "keep this content");
    std::fs::remove_dir_all(flag.parent().unwrap().parent().unwrap()).unwrap();
}

#[test]
fn adhd_question_does_not_change_add_or_uninstall() {
    for purpose in [WizardPurpose::Install, WizardPurpose::Uninstall] {
        let mut wizard = adhd_wizard();
        wizard.model.purpose = purpose;
        if purpose == WizardPurpose::Install {
            wizard.model.mode = crate::app::SelectionMode::Add;
        }
        assert!(!wizard.visible_stages().contains(&Screen::Responses));
        press(&mut wizard, &[KeyCode::Enter]);
        assert_eq!(title(&wizard), "Review");
        assert!(wizard.selected_settings().is_empty());
    }
}

struct AdhdInstallSystem(bool);
impl crate::System for AdhdInstallSystem {
    fn command_exists(&self, _: &str) -> bool {
        true
    }
    fn refresh_path(&self) {}
    fn run(&self, _: &crate::CommandSpec) -> anyhow::Result<crate::CommandResult> {
        Ok(crate::CommandResult {
            success: self.0,
            stdout: "User packages:\n  npm:i-have-adhd".into(),
            stderr: "package install failed".into(),
        })
    }
}

#[test]
fn adhd_install_job_writes_only_after_success_and_preserves_existing_flag() {
    for (succeeds, cancelled, installed) in [
        (true, false, false),
        (false, false, false),
        (true, true, false),
        (false, false, true),
    ] {
        let mut wizard = adhd_wizard();
        *wizard.model.installed.last_mut().unwrap() = installed;
        let flag = wizard.model.settings_paths.pi_adhd_flag.clone();
        std::fs::create_dir_all(flag.parent().unwrap()).unwrap();
        let config = flag.parent().unwrap().join("settings.json");
        std::fs::write(&config, "{\"theme\":\"light\"}\n").unwrap();
        press(&mut wizard, &[KeyCode::Enter, KeyCode::Up, KeyCode::Enter]);
        assert!(matches!(
            press(&mut wizard, &[KeyCode::Enter]),
            Some(Action::StartInstall)
        ));
        let job = wizard.begin_install().unwrap();
        job.cancelled
            .store(cancelled, std::sync::atomic::Ordering::Relaxed);
        let (sender, receiver) = std::sync::mpsc::channel();
        super::run_install_job(job, &AdhdInstallSystem(succeeds), &sender);
        assert_eq!(flag.exists(), (succeeds || installed) && !cancelled);
        assert_eq!(
            std::fs::read_to_string(config).unwrap(),
            "{\"theme\":\"light\"}\n"
        );
        assert!(receiver
            .try_iter()
            .any(|event| matches!(event, InstallEvent::Finished(_, _))));
        if flag.exists() {
            std::fs::write(&flag, "already configured").unwrap();
            assert!(!crate::settings::apply_setting(
                &crate::settings::pi_adhd_setting(),
                &wizard.model.settings_paths
            )
            .unwrap());
            assert_eq!(
                std::fs::read_to_string(&flag).unwrap(),
                "already configured"
            );
            assert_eq!(
                crate::settings::setting_state(
                    &crate::settings::pi_adhd_setting(),
                    &wizard.model.settings_paths
                ),
                SettingState::Applied
            );
        }
        std::fs::remove_dir_all(flag.parent().unwrap().parent().unwrap()).unwrap();
    }
}

#[test]
fn success_offers_the_next_command_to_copy() {
    let mut wizard = wizard();
    wizard.model.resources[6].next_action = "Run `gh auth login` once.".into();
    go_to(&mut wizard, Item::Resource(6));
    press(
        &mut wizard,
        &[KeyCode::Char(' '), KeyCode::Enter, KeyCode::Enter],
    );
    let job = wizard.begin_install().unwrap();
    finish_test_job(
        &mut wizard,
        job,
        crate::InstallReport {
            installed: vec!["Tools:gh".into()],
            failures: vec![],
        },
    );
    assert_eq!(wizard.next_command().as_deref(), Some("gh auth login"));
    let output = screen(&mut wizard, 104, 26);
    assert!(output.contains("c copies `gh auth login`"), "{output}");
}

#[test]
fn render_gallery_extra() {
    if !crate::snapshot_tests::isolated("wizard::tests::render_gallery_extra") {
        return;
    }
    let mut frames = Vec::new();
    let mut show = |wizard: &mut Wizard, w: u16, h: u16| {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| wizard.draw(frame)).unwrap();
        frames.push(format!("{:?}", terminal.backend().buffer()));
    };
    // Uninstall
    let mut model = model(ready());
    model.purpose = WizardPurpose::Uninstall;
    model.installed = vec![true; model.resources.len()];
    let mut un = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    show(&mut un, 104, 24);
    press(
        &mut un,
        &[KeyCode::Right, KeyCode::Char(' '), KeyCode::Enter],
    );
    show(&mut un, 104, 24);
    // Responses stage
    let mut m = self::model(ready());
    m.mode = crate::app::SelectionMode::Setup;
    let mut setup = Wizard::new(m, crate::wizard::wiki::WikiBrowser::default());
    go_to(&mut setup, Item::Resource(0));
    press(&mut setup, &[KeyCode::Char(' '), KeyCode::Enter]);
    show(&mut setup, 104, 24);
    // Nothing chosen review
    let mut empty = self::wizard();
    press(&mut empty, &[KeyCode::Enter]);
    show(&mut empty, 104, 24);
    crate::snapshot_tests::assert_snapshot("wizard-extra", &frames.join("\n"));
}

#[test]
fn project_scope_honours_global_installs_but_global_scope_ignores_project_ones() {
    let mut model = model(ready());
    model
        .resources
        .push(resource(ResourceKind::McpServer, "MCP servers", "sem"));
    model.installed.push(false);
    let sem = model.resources.len() - 1;
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    // sem configured only for this project.
    let mut project = vec![false; sem + 1];
    project[sem] = true;
    wizard.set_installed_scoped(vec![false; sem + 1], project);
    assert!(!wizard.resource_installed(sem));
    wizard.skill_scope = SkillScope::Project;
    assert!(wizard.resource_installed(sem));
    assert!(!wizard.installed_globally_only(sem));
    // sem configured globally: counts everywhere, and says so in project scope.
    let mut global = vec![false; sem + 1];
    global[sem] = true;
    wizard.set_installed_scoped(global, vec![false; sem + 1]);
    assert!(wizard.resource_installed(sem));
    assert!(wizard.installed_globally_only(sem));
    assert_eq!(wizard.selection_reason(sem), "Already installed globally");
    go_to(&mut wizard, Item::Resource(sem));
    let output = screen(&mut wizard, 200, 32);
    assert!(output.contains("Already installed globally"), "{output}");
    assert_eq!(output.matches("Already installed").count(), 1, "{output}");
    wizard.skill_scope = SkillScope::Global;
    assert!(wizard.resource_installed(sem));
    assert!(!wizard.installed_globally_only(sem));
}

/// Keep rendering deterministic without adding a clock abstraction to production.
fn freeze_gallery_time(wizard: &mut Wizard) {
    let stage = &mut wizard.install;
    stage.tick = 0;
    stage.started = None;
    stage.elapsed = std::time::Duration::ZERO;
    for item in &mut stage.items {
        item.started = None;
        item.elapsed = std::time::Duration::ZERO;
    }
}

#[test]
fn render_width_boundaries() {
    if !crate::snapshot_tests::isolated("wizard::tests::render_width_boundaries") {
        return;
    }
    let mut frames = Vec::new();
    for (width, height) in [
        (40, 10),
        (69, 20),
        (70, 20),
        (72, 20),
        (80, 24),
        (99, 24),
        (100, 24),
        (120, 30),
    ] {
        let mut wizard = wizard();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        for code in [KeyCode::Left, KeyCode::Right, KeyCode::Right] {
            press(&mut wizard, &[code]);
            terminal.draw(|frame| wizard.draw(frame)).unwrap();
            frames.push(format!("{:?}", terminal.backend().buffer()));
        }
    }
    crate::snapshot_tests::assert_snapshot("wizard-widths", &frames.join("\n"));
}

/// Simulate the worker returning the reviewed job with its completed rows.
fn finish_test_job(wizard: &mut Wizard, mut job: InstallJob, report: crate::InstallReport) {
    job.session.completed = wizard
        .install
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| matches!(item.status, ExecStatus::Ok(_)).then_some(index))
        .collect();
    job.session.absorb(&report);
    wizard.handle_install_event(InstallEvent::Finished(Box::new(job), report));
}

#[test]
fn scrolled_native_table_clicks_the_rendered_item() {
    let mut model = model(ready());
    model.resources = (0..15)
        .map(|index| {
            resource(
                ResourceKind::PiPackage,
                "Pi packages",
                &format!("package-{index}"),
            )
        })
        .collect();
    model.installed = vec![false; model.resources.len()];
    model.profiles.clear();
    model.settings.clear();
    model.setting_states.clear();
    let mut wizard = Wizard::new(model, crate::wizard::wiki::WikiBrowser::default());
    go_to(&mut wizard, Item::Resource(10));
    screen(&mut wizard, 50, 10);
    let (area, offset) = wizard.hits.list.unwrap();
    assert!(offset > 0, "the focused item must scroll into view");
    wizard.handle_click(area.x + 2, area.y + 1);
    assert!(
        wizard.selected[offset],
        "the first visible row uses the native table offset"
    );
    assert_eq!(
        wizard.selected.iter().filter(|&&selected| selected).count(),
        1
    );
}
