//! The wizard's state machine: four stages (Choose → Where → Review →
//! Install), one flat grouped selection list, key and mouse handling, and
//! install progress. Everything here is terminal-free so the whole flow is
//! unit-testable; rendering lives in `render.rs`.

use crate::settings::{SettingSpec, SettingState, SettingsPaths};
use crate::{
    build_install_plan, InstallPlan, InstallReport, Platform, PrerequisiteStatus, Resource,
    ResourceKind, SkillAgent, SkillDestination, SkillScope,
};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Everything the wizard needs to know up front; pure data so tests can
/// construct it without touching the file system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WizardPurpose {
    Install,
    Uninstall,
}

pub struct Model {
    pub mode: crate::app::SelectionMode,
    pub purpose: WizardPurpose,
    /// Uninstall uses ownership IDs and their exact dependency edges.
    pub uninstall_dependencies: BTreeMap<String, Vec<String>>,
    pub resources: Vec<Resource>,
    /// Per-resource flag: already present on this machine (plugin listed by
    /// `herdr plugin list`, package listed by `pi list`, skill in an agent
    /// tree).
    pub installed: Vec<bool>,
    pub settings: Vec<SettingSpec>,
    pub setting_states: Vec<SettingState>,
    /// Whether a Zed settings file exists on this machine; Zed tweaks are
    /// only pre-checked when there is a Zed to tweak.
    pub zed_present: bool,
    pub settings_paths: SettingsPaths,
    pub status: PrerequisiteStatus,
    pub platform: Platform,
    pub dry_run: bool,
    pub skill_destination: SkillDestination,
}

#[derive(Debug)]
pub enum WizardOutcome {
    Cancelled,
    NothingSelected,
    DryRun(InstallPlan, Vec<String>),
    Installed(InstallReport, Vec<String>, Vec<Resource>),
    /// A Wiki row routes to the Vault-scoped workflow instead of the global installer.
    WikiSelection {
        feynman: bool,
    },
    UninstallSelection(Vec<String>),
}

/// What the event loop must do after a key or mouse event.
pub enum Action {
    Exit(WizardOutcome),
    StartInstall,
}

/// One selectable thing under a group header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Item {
    Resource(usize),
    Setting(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ItemState {
    Available,
    Picked,
    Required(String),
    RequiredKeep(String),
    Installed,
    Unavailable(String),
}

/// One pickable row in a group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Row {
    Resource(usize),
    Setting(usize),
}

impl Row {
    fn item(&self) -> Item {
        match self {
            Self::Resource(index) => Item::Resource(*index),
            Self::Setting(index) => Item::Setting(*index),
        }
    }
}

/// A column-one entry: a titled set of rows. "Everything" has no rows of
/// its own; it stands for every resource at once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Group {
    pub title: String,
    pub rows: Vec<Row>,
    pub everything: bool,
}

impl Group {
    pub fn items(&self) -> Vec<Item> {
        self.rows.iter().map(Row::item).collect()
    }
}

/// Which column has the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pane {
    Groups,
    Items,
}

/// Three columns: groups, the focused group's rows, details.
pub(crate) struct ChooseStage {
    pub groups: Vec<Group>,
    pub group_cursor: usize,
    pub item_cursor: usize,
    pub focus: Pane,
}

impl ChooseStage {
    fn new(groups: Vec<Group>) -> Self {
        // Start on the first real group, in the item column.
        let group_cursor = groups
            .iter()
            .position(|group| !group.rows.is_empty())
            .unwrap_or(0);
        Self {
            groups,
            group_cursor,
            item_cursor: 0,
            focus: Pane::Items,
        }
    }

    pub fn group(&self) -> &Group {
        &self.groups[self.group_cursor]
    }

    pub fn row(&self) -> Option<&Row> {
        self.group().rows.get(self.item_cursor)
    }

    fn step(&mut self, delta: isize) {
        match self.focus {
            Pane::Groups => {
                self.group_cursor = clamp_step(self.group_cursor, delta, self.groups.len());
                self.item_cursor = 0;
            }
            Pane::Items => {
                self.item_cursor = clamp_step(self.item_cursor, delta, self.group().rows.len());
            }
        }
    }
}

fn uninstall_requires(
    resource: &str,
    target: &str,
    dependencies: &BTreeMap<String, Vec<String>>,
    seen: &mut std::collections::BTreeSet<String>,
) -> bool {
    if !seen.insert(resource.to_owned()) {
        return false;
    }
    dependencies.get(resource).is_some_and(|required| {
        required.iter().any(|dependency| {
            dependency == target || uninstall_requires(dependency, target, dependencies, seen)
        })
    })
}

fn clamp_step(cursor: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (cursor as isize + delta).clamp(0, len as isize - 1) as usize
}

/// Row zero is scope; remaining rows follow `SkillAgent::ALL`.
pub(crate) struct WhereStage {
    pub cursor: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecStatus {
    Pending,
    Running,
    Ok(String),
    Failed(String),
    Skipped(String),
}

#[derive(Clone, Debug)]
pub struct ExecItem {
    pub label: String,
    pub detail: String,
    pub status: ExecStatus,
}

pub(crate) struct InstallStage {
    pub items: Vec<ExecItem>,
    pub running: bool,
    pub report: Option<InstallReport>,
    pub tick: usize,
    pub scroll: u16,
}

pub(crate) enum Stage {
    Choose(ChooseStage),
    Where(WhereStage),
    Review { scroll: u16 },
    Install(InstallStage),
}

impl Stage {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Choose(_) => "Choose",
            Self::Where(_) => "Where",
            Self::Review { .. } => "Review",
            Self::Install(_) => "Install",
        }
    }
}

/// Events sent by the install worker thread.
#[derive(Debug)]
pub enum InstallEvent {
    Status(usize, ExecStatus),
    Done(InstallReport),
}

/// The work handed to the install worker thread.
pub struct InstallJob {
    pub plan: InstallPlan,
    pub settings: Vec<SettingSpec>,
    pub paths: SettingsPaths,
    pub cancelled: Arc<AtomicBool>,
}

/// Screen regions remembered from the last draw, for mouse hit-testing.
#[derive(Default)]
pub(crate) struct HitMap {
    pub back_button: Rect,
    pub next_button: Rect,
    /// (area, first-visible-row) of the stage's main list.
    pub list: Option<(Rect, usize)>,
    /// (area, first-visible-row) of the Choose groups column.
    pub groups: Option<(Rect, usize)>,
}

pub struct Wizard {
    pub(crate) model: Model,
    pub(crate) selected: Vec<bool>,
    pub(crate) setting_on: Vec<bool>,
    pub(crate) agent_on: Vec<bool>,
    pub(crate) skill_scope: SkillScope,
    /// Settings the user explicitly toggled; contextual pre-checks leave
    /// those alone.
    pub(crate) setting_touched: Vec<bool>,
    pub(crate) stages: Vec<Stage>,
    pub(crate) stage_index: usize,
    pub(crate) hits: HitMap,
    /// `Some(query)` while `/` search filters the Choose list.
    pub(crate) search: Option<String>,
    pub(crate) search_cursor: usize,
    pub(crate) show_help: bool,
    /// True while the installed-state probe still runs in the background.
    pub(crate) probing: bool,
    /// Quit confirmation pending (a non-empty selection would be discarded).
    pub(crate) confirm_quit: bool,
    /// First Ctrl-C during install arms cancellation; a second confirms it.
    pub(crate) confirm_cancel: bool,
    pub(crate) cancelled: Arc<AtomicBool>,
}

const CHOOSE: usize = 0;
const WHERE: usize = 1;
const REVIEW: usize = 2;
const INSTALL: usize = 3;

impl Wizard {
    pub fn new(model: Model) -> Self {
        let stages = vec![
            Stage::Choose(ChooseStage::new(choose_groups(&model))),
            Stage::Where(WhereStage { cursor: 1 }),
            Stage::Review { scroll: 0 },
            Stage::Install(InstallStage {
                items: Vec::new(),
                running: false,
                report: None,
                tick: 0,
                scroll: 0,
            }),
        ];
        let agent_on = SkillAgent::ALL
            .iter()
            .map(|agent| model.skill_destination.agents.contains(agent))
            .collect();
        let skill_scope = model.skill_destination.scope;
        let uninstalling = model.purpose == WizardPurpose::Uninstall;
        let mut wizard = Self {
            selected: vec![uninstalling; model.resources.len()],
            setting_on: vec![false; model.settings.len()],
            agent_on,
            skill_scope,
            setting_touched: vec![false; model.settings.len()],
            stages,
            stage_index: CHOOSE,
            model,
            hits: HitMap::default(),
            search: None,
            search_cursor: 0,
            show_help: false,
            probing: false,
            confirm_quit: false,
            confirm_cancel: false,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        wizard.precheck_settings();
        wizard
    }

    // ---- selection helpers -------------------------------------------------

    pub(crate) fn selection(&self) -> Vec<Resource> {
        self.model
            .resources
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                self.selected[*index]
                    && (self.model.purpose == WizardPurpose::Install
                        || self.required_note(*index).is_none())
            })
            .map(|(_, resource)| resource.clone())
            .collect()
    }

    /// The selection with skill dependencies pulled in.
    pub(crate) fn expanded_selection(&self) -> Vec<Resource> {
        if self.model.purpose == WizardPurpose::Uninstall {
            self.selection()
        } else {
            crate::expand_skill_dependencies(&self.model.resources, self.selection())
        }
    }

    pub(crate) fn selected_settings(&self) -> Vec<SettingSpec> {
        self.model
            .settings
            .iter()
            .zip(&self.setting_on)
            .filter(|(_, on)| **on)
            .map(|(spec, _)| spec.clone())
            .collect()
    }

    pub(crate) fn selected_agents(&self) -> Vec<SkillAgent> {
        SkillAgent::ALL
            .into_iter()
            .zip(&self.agent_on)
            .filter(|(_, on)| **on)
            .map(|(agent, _)| agent)
            .collect()
    }

    pub(crate) fn skill_destination(&self) -> SkillDestination {
        let mut destination = self.model.skill_destination.clone();
        destination.agents = self.selected_agents();
        destination.scope = self.skill_scope;
        destination
    }

    pub(crate) fn has_skills(&self) -> bool {
        self.expanded_selection()
            .iter()
            .any(|resource| resource.kind == ResourceKind::Skill)
    }

    pub(crate) fn resource_installed(&self, index: usize) -> bool {
        if self.model.purpose == WizardPurpose::Uninstall {
            return false;
        }
        let resource = &self.model.resources[index];
        if resource.kind == ResourceKind::Skill {
            let destination = self.skill_destination();
            let unchanged_destination = destination.scope == self.model.skill_destination.scope
                && destination.agents == self.model.skill_destination.agents;
            return (unchanged_destination && self.model.installed[index])
                || destination
                    .trees()
                    .iter()
                    .any(|tree| crate::skills::skill_present_in(tree, &resource.install_target));
        }
        self.model.installed[index]
    }

    pub(crate) fn setting_applied(&self, index: usize) -> bool {
        self.model.setting_states[index] == SettingState::Applied
    }

    fn setting_available(&self, index: usize) -> bool {
        let Some(related) = &self.model.settings[index].related_resource else {
            return true;
        };
        self.model
            .resources
            .iter()
            .enumerate()
            .any(|(resource_index, resource)| {
                resource.id == *related
                    && (self.selected[resource_index] || self.resource_installed(resource_index))
            })
    }

    pub(crate) fn nothing_chosen(&self) -> bool {
        self.selection().is_empty() && self.selected_settings().is_empty()
    }

    pub(crate) fn plan(&self) -> Result<InstallPlan> {
        let resources = self
            .expanded_selection()
            .into_iter()
            .filter(|resource| resource.group != "Wiki")
            .collect::<Vec<_>>();
        build_install_plan(
            &resources,
            &[],
            self.model.status,
            self.model.platform,
            &self.skill_destination(),
        )
    }

    pub(crate) fn total_selected(&self) -> usize {
        self.selection().len() + self.setting_on.iter().filter(|on| **on).count()
    }

    /// The items a group stands for: its rows, or every resource for the
    /// "Everything" group.
    pub(crate) fn group_items(&self, group: &Group) -> Vec<Item> {
        if group.everything {
            (0..self.model.resources.len())
                .map(Item::Resource)
                .collect()
        } else {
            group.items()
        }
    }

    pub(crate) fn item_state(&self, item: Item) -> ItemState {
        match item {
            Item::Resource(index) if self.resource_installed(index) => ItemState::Installed,
            Item::Resource(index) => self.required_note(index).map_or_else(
                || {
                    if self.selected[index] {
                        ItemState::Picked
                    } else {
                        ItemState::Available
                    }
                },
                |reason| {
                    if self.model.purpose == WizardPurpose::Uninstall {
                        ItemState::RequiredKeep(reason)
                    } else {
                        ItemState::Required(reason)
                    }
                },
            ),
            Item::Setting(index) if self.setting_applied(index) => ItemState::Installed,
            Item::Setting(index) if !self.setting_available(index) => {
                ItemState::Unavailable("pick or install its related capability first".into())
            }
            Item::Setting(index) if self.setting_on[index] => ItemState::Picked,
            Item::Setting(_) => ItemState::Available,
        }
    }

    /// Whether an item is on: selected, or required by a selection.
    pub(crate) fn item_on(&self, item: Item) -> bool {
        matches!(
            self.item_state(item),
            ItemState::Picked | ItemState::Required(_)
        )
    }

    /// Items the user can still act on: available or directly picked.
    pub(crate) fn actionable(&self, items: &[Item]) -> Vec<Item> {
        items
            .iter()
            .copied()
            .filter(|item| {
                matches!(
                    self.item_state(*item),
                    ItemState::Available | ItemState::Picked
                )
            })
            .collect()
    }

    /// Pre-check settings that pair with what the user picked, unless the
    /// user already touched them.
    fn precheck_settings(&mut self) {
        let selection = self.selection();
        for (index, spec) in self.model.settings.iter().enumerate() {
            if self.setting_touched[index] || self.setting_applied(index) {
                continue;
            }
            self.setting_on[index] = match &spec.related_resource {
                Some(resource_id) => {
                    (selection.iter().any(|resource| resource.id == *resource_id)
                        || self.model.resources.iter().enumerate().any(|(index, resource)| {
                            self.resource_installed(index) && resource.id == *resource_id
                        }))
                        // A Zed-targeting setting stays off without a Zed
                        // install, even when its plugin is selected.
                        && (!spec.requires_zed() || self.model.zed_present)
                }
                None => self.model.zed_present,
            };
        }
    }

    fn set_item(&mut self, item: Item, on: bool) {
        match item {
            Item::Resource(index) => self.selected[index] = on,
            Item::Setting(index) => {
                self.setting_on[index] = on;
                self.setting_touched[index] = true;
            }
        }
    }

    fn toggle_item(&mut self, item: Item) {
        if self.actionable(&[item]).is_empty() {
            return;
        }
        let on = self.item_on(item);
        self.set_item(item, !on);
        self.precheck_settings();
    }

    fn toggle_group(&mut self, items: &[Item]) {
        let actionable = self.actionable(items);
        let all_on = actionable.iter().all(|item| self.item_on(*item));
        for item in actionable {
            self.set_item(item, !all_on);
        }
        self.precheck_settings();
    }

    /// The background probe finished: adopt the real installed marks and
    /// drop any picks the probe proved redundant.
    pub fn set_installed(&mut self, installed: Vec<bool>) {
        if installed.len() == self.model.installed.len() {
            self.model.installed = installed;
            for (index, present) in self.model.installed.iter().enumerate() {
                if *present && self.model.resources[index].kind != ResourceKind::Skill {
                    self.selected[index] = false;
                }
            }
            self.precheck_settings();
        }
        self.probing = false;
    }

    /// The selected resource (label) that pulls `index` in as a dependency,
    /// when `index` is neither installed nor directly selected. Locked rows:
    /// shown as selected, not deselectable while the parent stays picked.
    pub(crate) fn required_note(&self, index: usize) -> Option<String> {
        if self.model.purpose == WizardPurpose::Uninstall {
            if !self.selected[index] {
                return None;
            }
            let target = &self.model.resources[index].id;
            for (parent_index, kept) in self.selected.iter().enumerate() {
                if *kept {
                    continue;
                }
                let parent = &self.model.resources[parent_index];
                if uninstall_requires(
                    &parent.id,
                    target,
                    &self.model.uninstall_dependencies,
                    &mut std::collections::BTreeSet::new(),
                ) {
                    return Some(parent.label.clone());
                }
            }
            return None;
        }
        if self.resource_installed(index) || self.selected[index] {
            return None;
        }
        let target = &self.model.resources[index].id;
        for (parent_index, on) in self.selected.iter().enumerate() {
            if !*on {
                continue;
            }
            let expanded = crate::expand_skill_dependencies(
                &self.model.resources,
                vec![self.model.resources[parent_index].clone()],
            );
            if expanded.iter().any(|resource| &resource.id == target) {
                return Some(self.model.resources[parent_index].label.clone());
            }
        }
        None
    }

    // ---- navigation --------------------------------------------------------

    pub(crate) fn install_running(&self) -> bool {
        matches!(
            &self.stages[self.stage_index],
            Stage::Install(stage) if stage.running
        )
    }

    /// Where only exists when skills are going somewhere.
    pub(crate) fn stage_visible(&self, index: usize) -> bool {
        index != WHERE || (self.model.purpose == WizardPurpose::Install && self.has_skills())
    }

    pub(crate) fn visible_stages(&self) -> Vec<usize> {
        (0..self.stages.len())
            .filter(|&index| self.stage_visible(index))
            .collect()
    }

    fn go_forward(&mut self) {
        // Review is the last stage reachable by plain navigation; Install
        // starts only from Review's confirm.
        if let Some(index) =
            (self.stage_index + 1..=REVIEW).find(|&index| self.stage_visible(index))
        {
            self.stage_index = index;
            self.entered_stage();
        }
    }

    fn go_back(&mut self) {
        self.search = None;
        if self.stage_index == INSTALL {
            return;
        }
        if let Some(index) = (0..self.stage_index)
            .rev()
            .find(|&index| self.stage_visible(index))
        {
            self.stage_index = index;
        }
    }

    fn entered_stage(&mut self) {
        self.search = None;
        if let Stage::Review { scroll } = &mut self.stages[self.stage_index] {
            *scroll = 0;
        }
    }

    fn confirm_review(&mut self) -> Option<Action> {
        if self.nothing_chosen() {
            return Some(Action::Exit(WizardOutcome::NothingSelected));
        }
        if self.model.purpose == WizardPurpose::Uninstall {
            return Some(Action::Exit(WizardOutcome::UninstallSelection(
                self.selection()
                    .into_iter()
                    .map(|resource| resource.id)
                    .collect(),
            )));
        }
        let selected = self.expanded_selection();
        let has_wiki = selected.iter().any(|resource| resource.group == "Wiki");
        let only_wiki = has_wiki && selected.iter().all(|resource| resource.group == "Wiki");
        if only_wiki && !self.model.dry_run {
            return Some(Action::Exit(WizardOutcome::WikiSelection {
                feynman: selected
                    .iter()
                    .any(|resource| resource.install_target == "@companion-ai/feynman"),
            }));
        }
        let Ok(plan) = self.plan() else {
            // The review screen explains why the plan cannot run; stay.
            return None;
        };
        if self.model.dry_run {
            let mut summary = self
                .selected_settings()
                .iter()
                .flat_map(|spec| {
                    let path = spec.target_path(&self.model.settings_paths).display();
                    spec.change_summary()
                        .into_iter()
                        .map(move |line| format!("{path}: {line}"))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            if has_wiki {
                summary
                    .push("Wiki: would enter the Vault-scoped setup; no Vault changes made".into());
            }
            return Some(Action::Exit(WizardOutcome::DryRun(plan, summary)));
        }
        self.stage_index = INSTALL;
        Some(Action::StartInstall)
    }

    // ---- install execution -------------------------------------------------

    /// Build the worker job and seed the install screen's step list.
    pub fn begin_install(&mut self) -> Result<InstallJob> {
        let plan = self.plan()?;
        let settings = self.selected_settings();
        let mut items = Vec::new();
        for step in &plan.prerequisites {
            items.push(ExecItem {
                label: format!("Install {}", step.target),
                detail: step.action.display(),
                status: ExecStatus::Pending,
            });
        }
        for step in &plan.resources {
            // Show the human name from the catalog, not the resource id.
            let name = self
                .model
                .resources
                .iter()
                .find(|resource| resource.id == step.target)
                .map(|resource| resource.label.as_str())
                .unwrap_or(&step.target);
            items.push(ExecItem {
                label: format!("Install {name}"),
                detail: step.action.display(),
                status: ExecStatus::Pending,
            });
        }
        for spec in &settings {
            items.push(ExecItem {
                label: format!("Configure {}", spec.label),
                detail: spec
                    .target_path(&self.model.settings_paths)
                    .display()
                    .to_string(),
                status: ExecStatus::Pending,
            });
        }
        let Stage::Install(stage) = &mut self.stages[self.stage_index] else {
            anyhow::bail!("install started outside the install stage");
        };
        stage.items = items;
        stage.running = true;
        self.confirm_cancel = false;
        self.cancelled.store(false, Ordering::Relaxed);
        Ok(InstallJob {
            plan,
            settings,
            paths: self.model.settings_paths.clone(),
            cancelled: Arc::clone(&self.cancelled),
        })
    }

    pub fn handle_install_event(&mut self, event: InstallEvent) {
        let Stage::Install(stage) = &mut self.stages[self.stage_index] else {
            return;
        };
        match event {
            InstallEvent::Status(index, status) => {
                if let Some(item) = stage.items.get_mut(index) {
                    item.status = status;
                }
            }
            InstallEvent::Done(report) => {
                stage.running = false;
                stage.report = Some(report);
                self.confirm_cancel = false;
            }
        }
    }

    pub fn tick(&mut self) {
        if let Stage::Install(stage) = &mut self.stages[self.stage_index] {
            stage.tick = stage.tick.wrapping_add(1);
        }
    }

    // ---- input -------------------------------------------------------------

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        let is_ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        if self.install_running() {
            if is_ctrl_c {
                if self.confirm_cancel {
                    self.cancelled.store(true, Ordering::Relaxed);
                } else {
                    self.confirm_cancel = true;
                }
            }
            return None;
        }
        if self.show_help && !is_ctrl_c {
            // Any key dismisses the help overlay.
            self.show_help = false;
            return None;
        }
        if self.confirm_quit && !is_ctrl_c {
            // Enter/y/q confirm the discard; anything else stays.
            self.confirm_quit = false;
            if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('q')
            ) {
                return Some(Action::Exit(WizardOutcome::Cancelled));
            }
            return None;
        }
        if self.search.is_some() && !is_ctrl_c {
            return self.handle_search_key(key.code);
        }
        if is_ctrl_c {
            return Some(Action::Exit(self.exit_outcome()));
        }
        if key.code == KeyCode::Char('q') {
            return self.quit();
        }
        if key.code == KeyCode::Esc {
            if self.stage_index == INSTALL {
                return Some(Action::Exit(self.exit_outcome()));
            }
            if self.stage_index > CHOOSE {
                self.go_back();
                return None;
            }
            return self.quit();
        }
        if key.code == KeyCode::Enter {
            return self.handle_enter();
        }
        if key.code == KeyCode::Char('?') {
            self.show_help = true;
            return None;
        }
        if key.code == KeyCode::Char('/') && self.stage_index == CHOOSE {
            self.search = Some(String::new());
            self.search_cursor = 0;
            return None;
        }

        match &mut self.stages[self.stage_index] {
            Stage::Choose(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => stage.step(-1),
                KeyCode::Down | KeyCode::Char('j') => stage.step(1),
                KeyCode::PageUp => stage.step(-10),
                KeyCode::PageDown => stage.step(10),
                KeyCode::Home => stage.step(isize::MIN / 2),
                KeyCode::End => stage.step(isize::MAX / 2),
                KeyCode::Left | KeyCode::Char('h') => stage.focus = Pane::Groups,
                KeyCode::Right | KeyCode::Char('l') => stage.focus = Pane::Items,
                KeyCode::Tab | KeyCode::BackTab => {
                    stage.focus = match stage.focus {
                        Pane::Groups => Pane::Items,
                        Pane::Items => Pane::Groups,
                    }
                }
                KeyCode::Char(' ') => match stage.focus {
                    Pane::Groups => {
                        let group = stage.group().clone();
                        let items = self.group_items(&group);
                        self.toggle_group(&items);
                    }
                    Pane::Items => {
                        if let Some(row) = stage.row().cloned() {
                            self.activate_row(&row);
                            // Space picks and steps down, so a run of picks
                            // is a run of spaces.
                            if let Stage::Choose(stage) = &mut self.stages[CHOOSE] {
                                stage.step(1);
                            }
                        }
                    }
                },
                _ => {}
            },
            Stage::Where(stage) => {
                let last = SkillAgent::ALL.len();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        stage.cursor = stage.cursor.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        stage.cursor = (stage.cursor + 1).min(last)
                    }
                    KeyCode::Home => stage.cursor = 0,
                    KeyCode::End => stage.cursor = last,
                    KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                        let cursor = stage.cursor;
                        self.toggle_where_row(cursor);
                        if let Stage::Where(stage) = &mut self.stages[WHERE] {
                            if key.code == KeyCode::Char(' ') && cursor > 0 {
                                stage.cursor = (cursor + 1).min(last);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Stage::Review { scroll } => match key.code {
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                _ => {}
            },
            Stage::Install(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => stage.scroll = stage.scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    stage.scroll =
                        (stage.scroll + 1).min(stage.items.len().saturating_sub(1) as u16)
                }
                _ => {}
            },
        }
        None
    }

    /// Leaving from the install screen keeps the report; elsewhere a
    /// non-empty selection asks first.
    fn exit_outcome(&self) -> WizardOutcome {
        if let Stage::Install(stage) = &self.stages[self.stage_index] {
            if let Some(report) = &stage.report {
                return WizardOutcome::Installed(
                    report.clone(),
                    self.next_actions(report),
                    self.expanded_selection(),
                );
            }
        }
        WizardOutcome::Cancelled
    }

    fn quit(&mut self) -> Option<Action> {
        if self.stage_index == INSTALL {
            return Some(Action::Exit(self.exit_outcome()));
        }
        if self.total_selected() > 0 {
            self.confirm_quit = true;
            return None;
        }
        Some(Action::Exit(WizardOutcome::Cancelled))
    }

    fn handle_enter(&mut self) -> Option<Action> {
        match &self.stages[self.stage_index] {
            Stage::Review { .. } => self.confirm_review(),
            Stage::Install(stage) => stage.report.as_ref().map(|report| {
                Action::Exit(WizardOutcome::Installed(
                    report.clone(),
                    self.next_actions(report),
                    self.expanded_selection(),
                ))
            }),
            Stage::Choose(_) => {
                self.go_forward();
                None
            }
            Stage::Where(_) => {
                self.go_forward();
                None
            }
        }
    }

    fn activate_row(&mut self, row: &Row) {
        match row {
            Row::Resource(index) => self.toggle_item(Item::Resource(*index)),
            Row::Setting(index) => self.toggle_item(Item::Setting(*index)),
        }
    }

    fn toggle_where_row(&mut self, cursor: usize) {
        if cursor == 0 {
            self.skill_scope = match self.skill_scope {
                SkillScope::Global => SkillScope::Project,
                SkillScope::Project => SkillScope::Global,
            };
        } else if let Some(on) = self.agent_on.get_mut(cursor - 1) {
            *on = !*on;
        }
    }

    pub(crate) fn next_actions(&self, report: &crate::InstallReport) -> Vec<String> {
        crate::app::next_actions(&self.expanded_selection(), report)
    }

    // ---- search ------------------------------------------------------------

    /// (group, row) pairs matching the live query, best match first. A
    /// hit on the label outranks the same hit in the description; an empty
    /// query lists everything in catalog order.
    pub(crate) fn search_matches(&self) -> Vec<(usize, usize)> {
        let (Some(query), Stage::Choose(stage)) = (&self.search, &self.stages[CHOOSE]) else {
            return Vec::new();
        };
        let mut matcher = nucleo_matcher::Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = nucleo_matcher::pattern::Pattern::parse(
            query,
            nucleo_matcher::pattern::CaseMatching::Ignore,
            nucleo_matcher::pattern::Normalization::Smart,
        );
        let mut buffer = Vec::new();
        let mut score_of = |text: &str| {
            let haystack = nucleo_matcher::Utf32Str::new(text, &mut buffer);
            pattern.score(haystack, &mut matcher)
        };
        let mut scored = Vec::new();
        for (group_index, group) in stage.groups.iter().enumerate() {
            for (row_index, row) in group.rows.iter().enumerate() {
                let (label, description) = match row {
                    Row::Resource(index) => {
                        let resource = &self.model.resources[*index];
                        (&resource.label, &resource.description)
                    }
                    Row::Setting(index) => {
                        let spec = &self.model.settings[*index];
                        (&spec.label, &spec.description)
                    }
                };
                let best = score_of(label)
                    .map(|score| score * 2)
                    .into_iter()
                    .chain(score_of(description))
                    .max();
                if let Some(score) = best {
                    scored.push((score, group_index, row_index));
                }
            }
        }
        // Stable sort: equal scores keep catalog order.
        scored.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        scored
            .into_iter()
            .map(|(_, group, row)| (group, row))
            .collect()
    }

    pub(crate) fn search_row(&self, hit: (usize, usize)) -> Row {
        let Stage::Choose(stage) = &self.stages[CHOOSE] else {
            unreachable!("search only runs on Choose")
        };
        stage.groups[hit.0].rows[hit.1].clone()
    }

    fn handle_search_key(&mut self, code: KeyCode) -> Option<Action> {
        match code {
            KeyCode::Esc => self.search = None,
            KeyCode::Enter => self.accept_search(),
            KeyCode::Backspace => {
                let empty = match &mut self.search {
                    Some(query) => {
                        query.pop();
                        query.is_empty()
                    }
                    None => true,
                };
                // Backspace on an empty query leaves search, like fzf.
                if empty {
                    self.search = None;
                }
                self.search_cursor = 0;
            }
            KeyCode::Up => self.search_cursor = self.search_cursor.saturating_sub(1),
            KeyCode::Down => {
                let len = self.search_matches().len();
                if len > 0 {
                    self.search_cursor = (self.search_cursor + 1).min(len - 1);
                }
            }
            KeyCode::Char(' ') => {
                let matches = self.search_matches();
                if let Some(&hit) = matches.get(self.search_cursor) {
                    let row = self.search_row(hit);
                    self.activate_row(&row);
                    self.search_cursor = (self.search_cursor + 1).min(matches.len() - 1);
                }
            }
            KeyCode::Char(c) => {
                if let Some(query) = &mut self.search {
                    query.push(c);
                }
                self.search_cursor = 0;
            }
            _ => {}
        }
        None
    }

    /// Leave search mode with the real cursor parked on the highlighted hit.
    fn accept_search(&mut self) {
        let hit = self.search_matches().get(self.search_cursor).copied();
        self.search = None;
        if let (Some((group, row)), Stage::Choose(stage)) = (hit, &mut self.stages[CHOOSE]) {
            stage.group_cursor = group;
            stage.item_cursor = row;
            stage.focus = Pane::Items;
        }
    }

    // ---- mouse -------------------------------------------------------------

    pub fn handle_click(&mut self, column: u16, row: u16) -> Option<Action> {
        if self.show_help || self.confirm_quit || self.install_running() {
            return None;
        }
        if contains(self.hits.back_button, column, row) {
            self.go_back();
            return None;
        }
        if contains(self.hits.next_button, column, row) {
            return self.handle_enter();
        }
        if let Some((area, offset)) = self.hits.groups {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                if let Stage::Choose(stage) = &mut self.stages[self.stage_index] {
                    if index < stage.groups.len() {
                        stage.focus = Pane::Groups;
                        stage.group_cursor = index;
                        stage.item_cursor = 0;
                    }
                }
                return None;
            }
        }
        if let Some((area, offset)) = self.hits.list {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                self.click_row(index);
            }
        }
        None
    }

    fn click_row(&mut self, index: usize) {
        if self.search.is_some() {
            if let Some(&hit) = self.search_matches().get(index) {
                self.search_cursor = index;
                let row = self.search_row(hit);
                self.activate_row(&row);
            }
            return;
        }
        match &mut self.stages[self.stage_index] {
            Stage::Choose(stage) => {
                if let Some(row) = stage.group().rows.get(index).cloned() {
                    stage.focus = Pane::Items;
                    stage.item_cursor = index;
                    self.activate_row(&row);
                }
            }
            Stage::Where(stage) if index <= SkillAgent::ALL.len() => {
                stage.cursor = index;
                self.toggle_where_row(index);
            }
            _ => {}
        }
    }

    pub fn handle_scroll(&mut self, down: bool) {
        if self.show_help || self.confirm_quit {
            return;
        }
        let code = if down { KeyCode::Down } else { KeyCode::Up };
        let _ = self.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
}

/// The Choose columns: Recommended for first setup, Everything, then skills
/// by category, tools, Pi packages, Herdr plugins, and settings by group.
fn choose_groups(model: &Model) -> Vec<Group> {
    let mut groups = Vec::new();
    if model.mode == crate::app::SelectionMode::Setup {
        let recommended = [
            "brainstorming",
            "tdd",
            "diagnosing-bugs",
            "code-review",
            "commit",
            "writing-clearly-and-concisely",
        ];
        let rows = model
            .resources
            .iter()
            .enumerate()
            .filter(|(_, resource)| {
                resource.kind == ResourceKind::Skill
                    && recommended.contains(&resource.install_target.as_str())
            })
            .map(|(index, _)| Row::Resource(index))
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            groups.push(Group {
                title: "Recommended".into(),
                rows,
                everything: false,
            });
        }
    }
    if !model.resources.is_empty() {
        groups.push(Group {
            title: "Everything".into(),
            rows: Vec::new(),
            everything: true,
        });
    }
    let mut push_group = |title: String, items: Vec<usize>| {
        if !items.is_empty() {
            groups.push(Group {
                title,
                rows: items.into_iter().map(Row::Resource).collect(),
                everything: false,
            });
        }
    };
    push_group(
        "Wiki".into(),
        indices(&model.resources, |resource| resource.group == "Wiki"),
    );
    for category in groups_of(&model.resources, ResourceKind::Skill) {
        let items = indices(&model.resources, |resource| {
            resource.kind == ResourceKind::Skill && resource.group == category
        });
        push_group(format!("Skills · {category}"), items);
    }
    for (kind, title) in [
        (ResourceKind::Tool, "Tools"),
        (ResourceKind::PiPackage, "Pi packages"),
        (ResourceKind::HerdrPlugin, "Herdr plugins"),
    ] {
        push_group(
            title.into(),
            indices(&model.resources, |resource| {
                resource.kind == kind && resource.group != "Wiki"
            }),
        );
    }
    let mut seen: Vec<&str> = Vec::new();
    for spec in &model.settings {
        if seen.contains(&spec.group.as_str()) {
            continue;
        }
        seen.push(&spec.group);
        groups.push(Group {
            title: format!("Settings · {}", spec.group),
            everything: false,
            rows: model
                .settings
                .iter()
                .enumerate()
                .filter(|(_, other)| other.group == spec.group)
                .map(|(index, _)| Row::Setting(index))
                .collect(),
        });
    }
    groups
}

fn groups_of(resources: &[Resource], kind: ResourceKind) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for resource in resources.iter().filter(|resource| resource.kind == kind) {
        if !groups.contains(&resource.group) {
            groups.push(resource.group.clone());
        }
    }
    groups
}

fn indices(resources: &[Resource], keep: impl Fn(&Resource) -> bool) -> Vec<usize> {
    resources
        .iter()
        .enumerate()
        .filter(|(_, resource)| keep(resource))
        .map(|(index, _)| index)
        .collect()
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}
