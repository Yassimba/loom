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
            change: SettingChange::HerdrKeyCommands(vec![KeyCommand {
                key: "prefix+r".into(),
                kind: "plugin_action".into(),
                command: "yassimba.reviewr.toggle".into(),
                description: None,
            }]),
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
        npm: true,
        mise: true,
        node: crate::NodeStatus::Supported,
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
    Wizard::new(model(ready()))
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
    match &wizard.stages[0] {
        Stage::Choose(stage) => stage,
        _ => unreachable!(),
    }
}

fn group_titles(wizard: &Wizard) -> Vec<String> {
    choose(wizard)
        .groups
        .iter()
        .map(|group| group.title.clone())
        .collect()
}

fn current_row(wizard: &Wizard) -> Row {
    choose(wizard).row().cloned().unwrap()
}

fn cursor(wizard: &Wizard) -> usize {
    match &wizard.stages[wizard.stage_index] {
        Stage::Choose(stage) => stage.item_cursor,
        Stage::Where(stage) => stage.cursor,
        _ => unreachable!(),
    }
}

/// Park the Choose cursor on a row, using only the keys a user has.
fn go_to(wizard: &mut Wizard, row: Row) {
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
    wizard.stages[wizard.stage_index].title()
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
    // Start in the first role profile, ready to move left-to-right.
    assert_eq!(choose(&wizard).focus, Pane::Groups);
    assert_eq!(choose(&wizard).group().title, "Engineer");
    assert_eq!(current_row(&wizard), Row::Resource(3));
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
    let wizard = Wizard::new(model);

    assert_eq!(group_titles(&wizard), ["Available here", "Everything"]);
    assert_eq!(choose(&wizard).groups[0].rows, [Row::Resource(0)]);
}

#[test]
fn choose_renders_profiles_mixed_kinds_and_required_tools() {
    let mut model = model(ready());
    model.resources[3].dependencies = vec!["gh".into()];
    model.profiles[0]
        .resources
        .extend([model.resources[0].id.clone(), model.resources[2].id.clone()]);
    let mut wizard = Wizard::new(model);
    go_to(&mut wizard, Row::Resource(3));
    press(
        &mut wizard,
        &[KeyCode::Char(' '), KeyCode::Left, KeyCode::Left],
    );

    let profile_output = screen(&mut wizard, 160, 28);
    assert!(profile_output.contains("Profiles"));
    assert!(profile_output.contains("Types"));
    assert!(profile_output.contains("Capabilities"));
    assert!(!profile_output.contains("Overview"));
    assert!(profile_output.contains("Skills"));
    assert!(profile_output.contains("Tools"));
    assert!(profile_output.contains("Pi packages"));
    assert!(profile_output.contains("Herdr plugins"));
    go_to(&mut wizard, Row::Resource(6));
    let output = screen(&mut wizard, 160, 28);

    assert!(!output.contains("Profiles"));
    assert!(output.contains("Types"));
    assert!(output.contains("Capabilities"));
    assert!(output.contains("Overview"));
    assert!(output.contains("Skills"));
    assert!(output.contains("Tools"));
    assert!(output.contains("needed by tdd"), "{output}");

    press(&mut wizard, &[KeyCode::Left]);
    let restored = screen(&mut wizard, 160, 28);
    assert!(restored.contains("Profiles"));
    assert!(!restored.contains("Overview"));
}

#[test]
fn stages_are_choose_where_review_install_and_where_needs_skills() {
    let mut wizard = wizard();
    assert_eq!(wizard.visible_stages(), [0, 2, 3]);
    go_to(&mut wizard, Row::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(wizard.visible_stages(), [0, 1, 2, 3]);
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
    go_to(&mut wizard, Row::Resource(6));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
}

#[test]
fn space_picks_and_steps_down() {
    let mut wizard = wizard();
    go_to(&mut wizard, Row::Resource(3));
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
    let mut wizard = Wizard::new(model);
    go_to_group(&mut wizard, "Engineer");

    press(&mut wizard, &[KeyCode::Char(' ')]);

    assert_eq!(
        wizard.selected,
        [false, false, false, true, true, false, true],
        "the visible dependency stays required instead of becoming a direct pick"
    );
    assert_eq!(wizard.required_note(5).as_deref(), Some("tdd"));
    assert!(choose(&wizard).group().rows.contains(&Row::Resource(5)));
    // The cursor stays on the profile so a second space clears it.
    assert_eq!(choose(&wizard).focus, Pane::Groups);
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected.iter().all(|on| !*on));
}

#[test]
fn space_on_a_type_toggles_only_direct_capabilities_of_that_type() {
    let mut model = model(ready());
    model.resources[3].dependencies = vec!["mermaid".into()];
    let mut wizard = Wizard::new(model);

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
    assert_eq!(current_row(&wizard), Row::Resource(3));
    press(&mut wizard, &[KeyCode::Tab]);
    assert_eq!(choose(&wizard).focus, Pane::Groups);
}

#[test]
fn everything_picks_the_whole_catalog_and_clears_it_again() {
    let mut model = model(ready());
    model.installed[0] = true;
    let mut wizard = Wizard::new(model);
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
    let mut wizard = Wizard::new(model);
    go_to(&mut wizard, Row::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.selected[0]);
    assert_eq!(wizard.item_state(Item::Resource(0)), ItemState::Installed);
}

#[test]
fn the_probe_drops_picks_it_proves_redundant() {
    let mut wizard = wizard();
    go_to(&mut wizard, Row::Resource(0));
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
    let mut wizard = Wizard::new(model);
    go_to(&mut wizard, Row::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(wizard.required_note(5).as_deref(), Some("tdd"));
    assert!(wizard.item_on(Item::Resource(5)));
    go_to(&mut wizard, Row::Resource(5));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.selected[5], "locked rows do not flip");
    assert!(wizard.required_note(5).is_some());
}

#[test]
fn search_returns_an_overlapping_capability_once() {
    let mut wizard = wizard();
    wizard.search = Some("gh".into());

    assert_eq!(wizard.search_matches().len(), 1);
    assert_eq!(
        wizard.search_row(wizard.search_matches()[0]),
        Row::Resource(6)
    );
}

#[test]
fn uninstall_keeps_ownership_groups_instead_of_role_profiles() {
    let mut model = model(ready());
    model.purpose = WizardPurpose::Uninstall;
    let wizard = Wizard::new(model);

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
}

#[test]
fn settings_precheck_follows_the_related_plugin_and_respects_touches() {
    let mut wizard = wizard();
    go_to(&mut wizard, Row::Setting(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(
        !wizard.setting_on[0],
        "a related setting cannot be selected before its package"
    );
    go_to(&mut wizard, Row::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(
        wizard.setting_on,
        [true, false, false],
        "no Zed: keymap stays off"
    );
    go_to(&mut wizard, Row::Setting(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.setting_on[0]);
    // Toggling the plugin off and on again does not override the user's no.
    go_to(&mut wizard, Row::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to(&mut wizard, Row::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(wizard.selected[2]);
    assert!(!wizard.setting_on[0]);
}

#[test]
fn zed_settings_precheck_only_with_zed_present() {
    let mut model = model(ready());
    model.zed_present = true;
    let mut wizard = Wizard::new(model);
    assert_eq!(wizard.setting_on, [false, true, false]);
    go_to(&mut wizard, Row::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert_eq!(wizard.setting_on, [true, true, true]);
}

#[test]
fn applied_settings_cannot_be_selected() {
    let mut model = model(ready());
    model.setting_states[1] = SettingState::Applied;
    model.zed_present = true;
    let mut wizard = Wizard::new(model);
    assert!(!wizard.setting_on[1]);
    go_to(&mut wizard, Row::Setting(1));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(!wizard.setting_on[1]);
}

#[test]
fn where_toggles_scope_and_agents_and_reports_exact_trees() {
    let mut wizard = wizard();
    go_to(&mut wizard, Row::Resource(3));
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
    go_to(&mut wizard, Row::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::StartInstall)
    ));
    assert_eq!(title(&wizard), "Install");
    let job = wizard.begin_install().unwrap();
    assert_eq!(job.plan.resources.len(), 1);
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
    go_to(&mut wizard, Row::Resource(3));
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
    let mut wizard = Wizard::new(model);
    go_to(&mut wizard, Row::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    match press(&mut wizard, &[KeyCode::Enter]) {
        Some(Action::Exit(WizardOutcome::DryRun(plan, _))) => {
            assert_eq!(plan.resources.len(), 1);
        }
        _ => panic!("expected a dry-run exit"),
    }
}

#[test]
fn install_events_drive_the_install_screen_to_completion() {
    let mut wizard = wizard();
    go_to(&mut wizard, Row::Resource(0));
    press(
        &mut wizard,
        &[KeyCode::Char(' '), KeyCode::Enter, KeyCode::Enter],
    );
    wizard.begin_install().unwrap();
    assert!(wizard.install_running());
    assert!(press(&mut wizard, &[KeyCode::Enter]).is_none(), "keys wait");
    wizard.handle_install_event(InstallEvent::Status(0, ExecStatus::Ok("installed".into())));
    let report = crate::InstallReport {
        installed: vec!["Pi packages:subagents".into()],
        failures: vec![],
    };
    wizard.handle_install_event(InstallEvent::Done(report));
    assert!(!wizard.install_running());
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::Exit(WizardOutcome::Installed(report, _, _))) if report.installed.len() == 1
    ));
}

#[test]
fn quitting_with_picks_asks_first_and_esc_on_choose_quits() {
    let mut empty = Wizard::new(model(ready()));
    assert!(matches!(
        press(&mut empty, &[KeyCode::Esc]),
        Some(Action::Exit(WizardOutcome::Cancelled))
    ));
    let mut wizard = wizard();
    go_to(&mut wizard, Row::Resource(0));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    assert!(press(&mut wizard, &[KeyCode::Char('q')]).is_none());
    assert!(wizard.confirm_quit);
    assert!(press(&mut wizard, &[KeyCode::Char('n')]).is_none());
    assert!(!wizard.confirm_quit);
    press(&mut wizard, &[KeyCode::Char('q')]);
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::Exit(WizardOutcome::Cancelled))
    ));
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
        .map(|&hit| match wizard.search_row(hit) {
            Row::Resource(index) => wizard.model.resources[index].label.clone(),
            Row::Setting(index) => wizard.model.settings[index].label.clone(),
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
    assert_eq!(current_row(&wizard), Row::Resource(5));
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
    assert_eq!(
        wizard.search_row(first),
        Row::Resource(4),
        "refactor outranks fuzzy hits"
    );
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
    go_to(&mut wizard, Row::Resource(3));
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
    wizard.begin_install().unwrap();
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    wizard.handle_install_event(InstallEvent::Done(crate::InstallReport {
        installed: vec!["skills".into()],
        failures: vec![],
    }));
    terminal.draw(|frame| wizard.draw(frame)).unwrap();
    // A tiny terminal must not panic either.
    let mut tiny = Terminal::new(TestBackend::new(20, 6)).unwrap();
    tiny.draw(|frame| wizard.draw(frame)).unwrap();
}

/// Prints every screen; run with `--nocapture` to eyeball the layout.
#[test]
fn render_gallery() {
    let mut wizard = wizard();
    let mut terminal = Terminal::new(TestBackend::new(104, 26)).unwrap();
    let show = |wizard: &mut Wizard, terminal: &mut Terminal<TestBackend>| {
        terminal.draw(|frame| wizard.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        println!("{out}");
    };
    go_to(&mut wizard, Row::Resource(2));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    go_to(&mut wizard, Row::Resource(3));
    press(&mut wizard, &[KeyCode::Char(' ')]);
    show(&mut wizard, &mut terminal);
    go_to_group(&mut wizard, "Everything");
    show(&mut wizard, &mut terminal);
    press(&mut wizard, &[KeyCode::Enter]);
    show(&mut wizard, &mut terminal);
    press(&mut wizard, &[KeyCode::Enter]);
    show(&mut wizard, &mut terminal);
    press(&mut wizard, &[KeyCode::Enter]);
    wizard.begin_install().unwrap();
    wizard.handle_install_event(InstallEvent::Status(0, ExecStatus::Ok("installed".into())));
    wizard.handle_install_event(InstallEvent::Status(1, ExecStatus::Running));
    show(&mut wizard, &mut terminal);
}

#[test]
fn setup_starts_in_the_first_role_profile() {
    let mut model = model(ready());
    model.mode = crate::app::SelectionMode::Setup;
    let wizard = Wizard::new(model);

    assert_eq!(choose(&wizard).group().title, "Engineer");
    assert_eq!(choose(&wizard).focus, Pane::Groups);
}

#[test]
fn wiki_group_routes_to_the_vault_workflow_with_optional_feynman() {
    let mut model = model(ready());
    let mut obsidian = resource(ResourceKind::Tool, "Wiki", "Obsidian");
    obsidian.dependencies = vec!["core-target".into()];
    let mut core = resource(ResourceKind::Tool, "Wiki", "claude-obsidian");
    core.install_target = "core-target".into();
    let mut feynman = resource(ResourceKind::PiPackage, "Wiki", "feynman");
    feynman.install_target = "@companion-ai/feynman".into();
    feynman.dependencies = vec!["core-target".into()];
    model.resources = vec![obsidian, core, feynman];
    model.profiles = vec![Profile {
        id: "knowledge-wiki".into(),
        label: "Knowledge & Wiki".into(),
        description: "Set up a Vault".into(),
        resources: vec!["Wiki:Obsidian".into(), "Wiki:feynman".into()],
    }];
    model.installed = vec![false; 3];
    let mut wizard = Wizard::new(model);

    assert_eq!(group_titles(&wizard)[0], "Knowledge & Wiki");
    go_to_group(&mut wizard, "Knowledge & Wiki");
    press(&mut wizard, &[KeyCode::Char(' '), KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::Exit(WizardOutcome::WikiSelection { feynman: true }))
    ));
}

#[test]
fn mixed_wiki_selection_installs_generic_resources_before_handoff() {
    let mut model = model(ready());
    model.resources = vec![
        resource(ResourceKind::Tool, "Essentials", "generic"),
        resource(ResourceKind::Tool, "Wiki", "claude-obsidian"),
    ];
    model.installed = vec![false; 2];
    let mut wizard = Wizard::new(model);
    wizard.selected = vec![true, true];

    press(&mut wizard, &[KeyCode::Enter]);
    assert_eq!(title(&wizard), "Review");
    assert!(matches!(
        press(&mut wizard, &[KeyCode::Enter]),
        Some(Action::StartInstall)
    ));
    let job = wizard.begin_install().unwrap();
    assert_eq!(job.plan.prerequisites.len() + job.plan.resources.len(), 1);
    assert!(job
        .plan
        .prerequisites
        .iter()
        .all(|step| !step.target.contains("claude-obsidian")));
}

#[test]
fn wiki_dry_run_never_enters_the_mutating_handoff() {
    let mut model = model(ready());
    model.dry_run = true;
    model.resources = vec![resource(ResourceKind::Tool, "Wiki", "claude-obsidian")];
    model.installed = vec![false];
    let mut wizard = Wizard::new(model);
    wizard.selected[0] = true;

    press(&mut wizard, &[KeyCode::Enter]);
    match press(&mut wizard, &[KeyCode::Enter]) {
        Some(Action::Exit(WizardOutcome::DryRun(plan, summary))) => {
            assert!(plan.resources.is_empty());
            assert!(summary.iter().any(|line| line.contains("no Vault changes")));
        }
        _ => panic!("expected a non-mutating dry-run outcome"),
    }
}

#[test]
fn modal_overlays_consume_mouse_and_scroll_input() {
    let mut wizard = wizard();
    let mut terminal = Terminal::new(TestBackend::new(110, 30)).unwrap();
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
    let mut wizard = Wizard::new(model);
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
    let mut wizard = Wizard::new(model);
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
    go_to(&mut wizard, Row::Resource(0));
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
    go_to(&mut wizard, Row::Resource(3));
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

    assert!(text.contains("(•) Global    ( ) Project"));
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
    let mut wizard = wizard();
    let output = screen(&mut wizard, 104, 24);
    assert!(output.contains("Profiles"));
    assert!(output.contains("Types"));
    assert!(output.contains("Capabilities"));
    assert!(!output.contains("Overview"));
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
    let mut wizard = Wizard::new(model);

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
    let mut wizard = Wizard::new(model);
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

    assert!(text.contains("Remove"));
    assert!(text.contains("review"));
}
