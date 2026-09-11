//! The wizard's state machine: four stages (Choose → Where → Review →
//! Install), overlapping role profiles over one selection, key and mouse
//! handling, and install progress. Everything here is terminal-free so the whole flow is
//! unit-testable; rendering lives in `render.rs`.

use crate::settings::{SettingSpec, SettingState, SettingsPaths};
use crate::{
    build_install_plan, InstallPlan, InstallReport, Platform, PrerequisiteStatus, Resource,
    ResourceKind, SkillAgent, SkillDestination, SkillScope,
};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use std::collections::{BTreeMap, HashSet};
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
    pub profiles: Vec<crate::Profile>,
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
    Installed {
        report: InstallReport,
        resources: Vec<Resource>,
        destination: SkillDestination,
        written: Vec<String>,
    },
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
    PickWiki(crate::wiki::WikiOperation),
    UnregisterWiki(std::path::PathBuf),
}

/// One selectable thing under a group header.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

/// One capability type inside a profile. Visible rows include dependency
/// closure; bulk rows contain only direct members of this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KindGroup {
    pub title: String,
    pub rows: Vec<Row>,
    pub bulk_rows: Vec<Row>,
}

impl KindGroup {
    pub fn items(&self) -> Vec<Item> {
        self.rows.iter().map(Row::item).collect()
    }

    pub fn bulk_items(&self) -> Vec<Item> {
        self.bulk_rows.iter().map(Row::item).collect()
    }
}

/// A profile or legacy uninstall group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Group {
    pub title: String,
    pub description: String,
    pub rows: Vec<Row>,
    pub bulk_rows: Vec<Row>,
    pub kinds: Vec<KindGroup>,
    pub everything: bool,
}

impl Group {
    pub fn bulk_items(&self) -> Vec<Item> {
        self.bulk_rows.iter().map(Row::item).collect()
    }
}

/// Which column has the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pane {
    Groups,
    Kinds,
    Items,
}

/// Profiles, capability types, capabilities, and an overview pane.
pub(crate) struct ChooseStage {
    pub groups: Vec<Group>,
    pub group_cursor: usize,
    pub kind_cursor: usize,
    pub item_cursor: usize,
    pub focus: Pane,
}

impl ChooseStage {
    fn new(groups: Vec<Group>) -> Self {
        // Start on the first real group, in the item column.
        let group_cursor = groups
            .iter()
            .position(|group| !group.everything && !group.rows.is_empty())
            .or_else(|| groups.iter().position(|group| !group.rows.is_empty()))
            .unwrap_or(0);
        Self {
            groups,
            group_cursor,
            kind_cursor: 0,
            item_cursor: 0,
            focus: Pane::Groups,
        }
    }

    pub fn group(&self) -> &Group {
        &self.groups[self.group_cursor]
    }

    pub fn kind(&self) -> &KindGroup {
        &self.group().kinds[self.kind_cursor]
    }

    pub fn row(&self) -> Option<&Row> {
        self.kind().rows.get(self.item_cursor)
    }

    fn step(&mut self, delta: isize) {
        match self.focus {
            Pane::Groups => {
                self.group_cursor = clamp_step(self.group_cursor, delta, self.groups.len());
                self.kind_cursor = 0;
                self.item_cursor = 0;
            }
            Pane::Kinds => {
                self.kind_cursor = clamp_step(self.kind_cursor, delta, self.group().kinds.len());
                self.item_cursor = 0;
            }
            Pane::Items => {
                self.item_cursor = clamp_step(self.item_cursor, delta, self.kind().rows.len());
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
    Verifying,
    Ok(String),
    Failed(String),
    Skipped(String),
}

#[derive(Clone, Debug)]
pub struct ExecItem {
    pub label: String,
    pub detail: String,
    pub status: ExecStatus,
    pub started: Option<std::time::Instant>,
    pub elapsed: std::time::Duration,
}

pub(crate) struct InstallStage {
    pub items: Vec<ExecItem>,
    pub running: bool,
    pub report: Option<InstallReport>,
    pub tick: usize,
    pub scroll: u16,
    pub started: Option<std::time::Instant>,
    pub elapsed: std::time::Duration,
    pub show_details: bool,
}

pub(crate) enum Stage {
    Choose(ChooseStage),
    Where(WhereStage),
    Responses { cursor: usize },
    Review { scroll: u16 },
    Install(InstallStage),
}

impl Stage {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Choose(_) => "Choose",
            Self::Where(_) => "Where",
            Self::Responses { .. } => "Pi responses",
            Self::Review { .. } => "Review",
            Self::Install(_) => "Install",
        }
    }
}

/// Events sent by the install worker thread.
#[derive(Debug)]
pub enum InstallEvent {
    Confirm(String, Vec<String>, std::sync::mpsc::Sender<bool>),
    Detail(usize, String),
    Status(usize, ExecStatus),
    Done(InstallReport),
}

/// The work handed to the install worker thread.
#[derive(Clone)]
pub struct InstallJob {
    pub completed: Vec<usize>,
    pub previously_installed: Vec<String>,
    pub plan: InstallPlan,
    pub(super) wikis: Vec<super::wiki_install::WikiInstall>,
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
    /// (area, first-visible-row) of the profile capability-type column.
    pub kinds: Option<(Rect, usize)>,
}

pub struct Wizard {
    pub(crate) model: Model,
    pub(super) wiki: Option<super::wiki::WikiBrowser>,
    pub(crate) selected: Vec<bool>,
    /// Goal membership and explicit item overrides keep overlapping picks independent.
    pub(crate) picked_goals: HashSet<usize>,
    custom_picks: BTreeMap<usize, bool>,
    pub(crate) setting_on: Vec<bool>,
    pub(crate) agent_on: Vec<bool>,
    pub(crate) skill_scope: SkillScope,
    pub(crate) adhd_enabled: bool,
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
    pub(super) reviewed_job: Option<InstallJob>,
    completed_writes: Vec<String>,
}

const CHOOSE: usize = 0;
const WHERE: usize = 1;
const RESPONSES: usize = 2;
const REVIEW: usize = 3;
const INSTALL: usize = 4;

impl Wizard {
    pub fn new(model: Model) -> Self {
        let stages = vec![
            Stage::Choose(ChooseStage::new(choose_groups(&model))),
            Stage::Where(WhereStage { cursor: 1 }),
            Stage::Responses { cursor: 1 },
            Stage::Review { scroll: 0 },
            Stage::Install(InstallStage {
                items: Vec::new(),
                running: false,
                report: None,
                tick: 0,
                scroll: 0,
                started: None,
                elapsed: std::time::Duration::ZERO,
                show_details: false,
            }),
        ];
        let agent_on = SkillAgent::ALL
            .iter()
            .map(|agent| model.skill_destination.agents.contains(agent))
            .collect();
        let skill_scope = model.skill_destination.scope;
        let uninstalling = model.purpose == WizardPurpose::Uninstall;
        let mut selected = vec![uninstalling; model.resources.len()];
        if !uninstalling && model.status.pi {
            if let Some(index) = model
                .resources
                .iter()
                .position(|resource| resource.install_target == "pi-mcp-adapter")
            {
                selected[index] = true;
            }
        }
        let mut wizard = Self {
            wiki: None,
            selected,
            picked_goals: HashSet::new(),
            custom_picks: BTreeMap::new(),
            setting_on: vec![false; model.settings.len()],
            agent_on,
            skill_scope,
            adhd_enabled: false,
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
            reviewed_job: None,
            completed_writes: Vec::new(),
        };
        wizard.precheck_settings();
        wizard
    }

    // ---- selection helpers -------------------------------------------------

    pub(crate) fn selection(&self) -> Vec<Resource> {
        let include_pi_loom = (self.model.mode == crate::app::SelectionMode::Setup
            && self.model.status.pi)
            || self
                .model
                .resources
                .iter()
                .enumerate()
                .any(|(index, resource)| {
                    self.selected[index]
                        && (resource.id == "tool:pi"
                            || (resource.kind == ResourceKind::PiPackage
                                && !resource.is_automatic_pi_package()))
                });
        self.model
            .resources
            .iter()
            .enumerate()
            .filter(|(index, resource)| {
                (self.selected[*index]
                    || (include_pi_loom
                        && resource.is_automatic_pi_package()
                        && !self.resource_installed(*index))
                    || (self.adhd_enabled
                        && self.model.resources[*index].id == "pi-package:i-have-adhd"
                        && !self.resource_installed(*index)))
                    && (self.wiki.is_none() || resource.group != "Wiki")
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
            crate::expand_skill_dependencies(
                &self.model.resources,
                self.selection(),
                &self.selected_agents(),
            )
        }
    }

    pub(crate) fn selected_settings(&self) -> Vec<SettingSpec> {
        self.model
            .settings
            .iter()
            .zip(&self.setting_on)
            .filter(|(_, on)| **on)
            .map(|(spec, _)| spec.clone())
            .chain(self.adhd_enabled.then(crate::settings::pi_adhd_setting))
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

    pub(crate) fn has_mcp(&self) -> bool {
        self.selection()
            .iter()
            .any(|resource| resource.kind == ResourceKind::McpServer)
    }

    pub(super) fn included_note(&self, index: usize) -> Option<String> {
        let skill = &self.model.resources[index];
        if self.model.purpose != WizardPurpose::Install || skill.kind != ResourceKind::Skill {
            return None;
        }
        let destination = self.skill_destination();
        self.model
            .resources
            .iter()
            .enumerate()
            .find(|(index, package)| {
                package.kind == ResourceKind::PiPackage
                    && package.bundled_skills.contains(&skill.install_target)
                    && ((self.selected[*index]
                        && crate::bundled_skills::selectable(package, &destination))
                        || crate::bundled_skills::provides(
                            package,
                            &skill.install_target,
                            &destination,
                        ))
            })
            .map(|(_, package)| format!("Included with {} for Pi", package.label))
    }

    pub(crate) fn resource_installed(&self, index: usize) -> bool {
        if self.model.purpose == WizardPurpose::Uninstall {
            return false;
        }
        let resource = &self.model.resources[index];
        // A global install cannot satisfy a Vault that has not been chosen.
        if resource.group == "Wiki" {
            return false;
        }
        // Keep MCP selectable across scope changes; configuration is not live health.
        if resource.kind == ResourceKind::McpServer {
            return false;
        }
        if resource.kind == ResourceKind::Skill {
            let destination = self.skill_destination();
            let unchanged_destination = destination.scope == self.model.skill_destination.scope
                && destination.agents == self.model.skill_destination.agents;
            let trees = destination.trees();
            return (unchanged_destination && self.model.installed[index])
                || (!trees.is_empty()
                    && trees.iter().all(|tree| {
                        crate::skills::skill_present_in(tree, &resource.install_target)
                            || crate::bundled_skills::provided_in_tree(
                                &destination.home,
                                tree,
                                &resource.install_target,
                            )
                    }));
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
        self.total_selected() == 0
    }

    pub(crate) fn plan(&self) -> Result<InstallPlan> {
        self.wiki_jobs()?;
        let resources = self
            .expanded_selection()
            .into_iter()
            .filter(|resource| resource.group != "Wiki")
            .collect::<Vec<_>>();
        build_install_plan(
            &resources,
            self.model.status,
            self.model.platform,
            &self.skill_destination(),
        )
    }

    pub(crate) fn total_selected(&self) -> usize {
        self.selection().len()
            + self.selected_settings().len()
            + self.wiki.as_ref().map_or(0, |browser| browser.count())
    }

    /// The items a group stands for: its rows, or every resource for the
    /// "Everything" group.
    pub(crate) fn group_items(&self, group: &Group) -> Vec<Item> {
        group.bulk_items()
    }

    pub(crate) fn item_state(&self, item: Item) -> ItemState {
        match item {
            Item::Resource(index)
                if self.wiki.is_some() && self.model.resources[index].group == "Wiki" =>
            {
                ItemState::Unavailable("Choose a knowledgebase to pick this capability".into())
            }
            Item::Resource(index) => match self.included_note(index) {
                Some(note)
                    if self.resource_installed(index)
                        || self.required_note(index).is_some()
                        || self.selected_agents().is_empty() =>
                {
                    ItemState::Required(note)
                }
                _ if self.resource_installed(index) => ItemState::Installed,
                _ => self.required_note(index).map_or_else(
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
            },
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
            Item::Resource(index) => {
                self.selected[index] = on;
                self.custom_picks.insert(index, on);
            }
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

    fn toggle_goal(&mut self, group_index: usize) {
        let Stage::Choose(stage) = &self.stages[CHOOSE] else {
            return;
        };
        let group = &stage.groups[group_index];
        if group.everything
            || self.model.profiles.is_empty()
            || self.model.purpose == WizardPurpose::Uninstall
        {
            let items = group.bulk_items();
            self.toggle_group(&items);
            return;
        }
        if !self.picked_goals.remove(&group_index) {
            self.picked_goals.insert(group_index);
        }
        for row in &group.bulk_rows {
            let Row::Resource(index) = row else {
                continue;
            };
            if self.resource_installed(*index) {
                continue;
            }
            self.selected[*index] = self.custom_picks.get(index).copied().unwrap_or_else(|| {
                self.picked_goals
                    .iter()
                    .any(|goal| stage.groups[*goal].bulk_rows.contains(row))
            });
        }
        self.precheck_settings();
    }

    pub(crate) fn setup_requirement(&self, index: usize) -> bool {
        let resource = &self.model.resources[index];
        resource.is_automatic_pi_package()
            || (resource.install_target == "pi-mcp-adapter"
                && self.model.status.pi
                && !self.custom_picks.contains_key(&index))
    }

    pub(crate) fn selection_reason(&self, index: usize) -> String {
        if let Some(note) = self.included_note(index) {
            return note;
        }
        if let Some(parent) = self.required_note(index) {
            return format!("Needed by {parent}");
        }
        if self.resource_installed(index) {
            return "Already installed".into();
        }
        if self.setup_requirement(index) {
            return "Needed by Pi setup".into();
        }
        if self.adhd_enabled && self.model.resources[index].id == "pi-package:i-have-adhd" {
            return "Included by your Pi response choice".into();
        }
        if !self.selected[index] {
            return "Optional addition".into();
        }
        if !self.custom_picks.contains_key(&index) {
            if let Stage::Choose(stage) = &self.stages[CHOOSE] {
                let goals = stage
                    .groups
                    .iter()
                    .enumerate()
                    .filter(|(goal, group)| {
                        self.picked_goals.contains(goal)
                            && group.bulk_rows.contains(&Row::Resource(index))
                    })
                    .map(|(_, group)| group.title.as_str())
                    .collect::<Vec<_>>();
                if !goals.is_empty() {
                    return format!("Included by {}", goals.join(", "));
                }
            }
        }
        "Picked individually".into()
    }

    /// The background probe finished: adopt the real installed marks and
    /// drop any picks the probe proved redundant.
    pub fn set_installed(&mut self, installed: Vec<bool>) {
        if self.reviewed_job.is_some() {
            self.probing = false;
            return; // A late probe cannot change an already-reviewed selection.
        }
        if installed.len() == self.model.installed.len() {
            self.model.installed = installed;
            for (index, present) in self.model.installed.iter().enumerate() {
                if *present
                    && self.model.resources[index].group != "Wiki"
                    && !matches!(
                        self.model.resources[index].kind,
                        ResourceKind::Skill | ResourceKind::McpServer
                    )
                {
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
        if self.adhd_enabled && target == "pi-package:i-have-adhd" {
            return Some("ADHD-friendly responses".into());
        }
        for (parent_index, on) in self.selected.iter().enumerate() {
            if !*on {
                continue;
            }
            let expanded = crate::expand_skill_dependencies(
                &self.model.resources,
                vec![self.model.resources[parent_index].clone()],
                &self.selected_agents(),
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
        match index {
            WHERE => {
                self.model.purpose == WizardPurpose::Install
                    && (self.has_skills() || self.has_mcp())
            }
            RESPONSES => {
                self.model.purpose == WizardPurpose::Install
                    && self.model.mode == crate::app::SelectionMode::Setup
                    && self
                        .model
                        .resources
                        .iter()
                        .any(|resource| resource.id == "pi-package:i-have-adhd")
            }
            _ => true,
        }
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
        if self.stage_index == WHERE && self.has_mcp() && !self.has_skills() {
            self.agent_on = SkillAgent::ALL
                .iter()
                .map(|a| *a == SkillAgent::Pi)
                .collect();
        }
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
        let only_wiki = has_wiki
            && selected.iter().all(|resource| resource.group == "Wiki")
            && self.selected_settings().is_empty();
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
            for wiki in self.wiki_jobs().unwrap_or_default() {
                summary.push(format!(
                    "Knowledgebase {}: {}",
                    wiki.record.path.display(),
                    wiki.labels.join(", ")
                ));
                summary.extend(
                    wiki.plan
                        .prerequisites
                        .iter()
                        .chain(&wiki.plan.resources)
                        .map(|step| step.action.display()),
                );
            }
            return Some(Action::Exit(WizardOutcome::DryRun(plan, summary)));
        }
        self.stage_index = INSTALL;
        Some(Action::StartInstall)
    }

    // ---- install execution -------------------------------------------------

    pub(crate) fn can_retry(&self) -> bool {
        matches!(&self.stages[self.stage_index], Stage::Install(stage)
            if !stage.running && stage.report.as_ref().is_some_and(|report| !report.failures.is_empty()))
    }

    /// Retries use the exact reviewed plan, not a new selection or destination.
    pub fn begin_install(&mut self) -> Result<InstallJob> {
        let mut job = match &self.reviewed_job {
            Some(job) => job.clone(),
            None => InstallJob {
                completed: Vec::new(),
                previously_installed: Vec::new(),
                plan: self.plan()?,
                wikis: self.wiki_jobs()?,
                settings: self.selected_settings(),
                paths: self.model.settings_paths.clone(),
                cancelled: Arc::clone(&self.cancelled),
            },
        };
        if let Stage::Install(stage) = &self.stages[self.stage_index] {
            job.previously_installed = self.completed_writes.clone();
            job.completed = stage
                .items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    matches!(item.status, ExecStatus::Ok(_)).then_some(index)
                })
                .collect();
        }
        let plan = &job.plan;
        let settings = &job.settings;
        let mut items = Vec::new();
        for step in &plan.prerequisites {
            items.push(ExecItem {
                label: format!("Install {}", step.target),
                detail: step.action.display(),
                status: ExecStatus::Pending,
                started: None,
                elapsed: std::time::Duration::ZERO,
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
                started: None,
                elapsed: std::time::Duration::ZERO,
            });
        }
        for spec in settings {
            items.push(ExecItem {
                label: format!("Configure {}", spec.label),
                detail: spec
                    .target_path(&self.model.settings_paths)
                    .display()
                    .to_string(),
                status: ExecStatus::Pending,
                started: None,
                elapsed: std::time::Duration::ZERO,
            });
        }
        for wiki in &job.wikis {
            items.push(ExecItem {
                label: wiki.label(),
                detail: wiki.labels.join(", "),
                status: ExecStatus::Pending,
                started: None,
                elapsed: std::time::Duration::ZERO,
            });
        }
        let Stage::Install(stage) = &mut self.stages[self.stage_index] else {
            anyhow::bail!("install started outside the install stage");
        };
        stage.items = items;
        stage.running = true;
        stage.report = None;
        stage.started = Some(std::time::Instant::now());
        stage.elapsed = std::time::Duration::ZERO;
        stage.scroll = 0;
        stage.show_details = false;
        self.confirm_cancel = false;
        self.cancelled.store(false, Ordering::Relaxed);
        self.reviewed_job = Some(job.clone());
        Ok(job)
    }

    pub fn handle_install_event(&mut self, event: InstallEvent) {
        let Stage::Install(stage) = &mut self.stages[self.stage_index] else {
            return;
        };
        match event {
            InstallEvent::Confirm(_, _, reply) => {
                let _ = reply.send(false);
            }
            InstallEvent::Detail(index, detail) => {
                if let Some(item) = stage.items.get_mut(index) {
                    item.detail = detail;
                }
            }
            InstallEvent::Status(index, status) => {
                if let Some(item) = stage.items.get_mut(index) {
                    if matches!(status, ExecStatus::Running | ExecStatus::Verifying) {
                        item.started.get_or_insert_with(std::time::Instant::now);
                    } else if let Some(started) = item.started.take() {
                        item.elapsed = started.elapsed();
                    }
                    item.status = status;
                }
            }
            InstallEvent::Done(report) => {
                for target in &report.installed {
                    if !self.completed_writes.contains(target) {
                        self.completed_writes.push(target.clone());
                    }
                }
                stage.running = false;
                stage.elapsed = stage
                    .started
                    .map_or(std::time::Duration::ZERO, |started| started.elapsed());
                stage.scroll = stage
                    .items
                    .iter()
                    .position(|item| {
                        matches!(item.status, ExecStatus::Failed(_) | ExecStatus::Skipped(_))
                    })
                    .unwrap_or(0) as u16;
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
        if self
            .wiki
            .as_ref()
            .is_some_and(|browser| browser.confirm_unregister.is_some())
        {
            return self.wiki_key(key.code);
        }
        if self.browsing_wiki() {
            let scoped_search = self.wiki.as_ref().is_some_and(|browser| browser.search.is_some()
                || (key.code == KeyCode::Char('/') && browser.record().is_some()
                    && matches!(&self.stages[self.stage_index], Stage::Choose(stage) if stage.focus != Pane::Groups)));
            if scoped_search {
                return self.wiki_key(key.code);
            }
            if key.code == KeyCode::Char('n')
                || (key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                return self.handle_enter();
            }
            let focus = match &self.stages[self.stage_index] {
                Stage::Choose(stage) => stage.focus,
                _ => unreachable!(),
            };
            let navigation = matches!(
                key.code,
                KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Char('j')
                    | KeyCode::Char('k')
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
            );
            if (focus != Pane::Groups && (navigation || key.code == KeyCode::Esc))
                || matches!(
                    key.code,
                    KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Char('h')
                        | KeyCode::Char('l')
                        | KeyCode::Tab
                        | KeyCode::BackTab
                        | KeyCode::Enter
                        | KeyCode::Char(' ')
                        | KeyCode::Char('u')
                )
            {
                return self.wiki_key(key.code);
            }
        }
        if key.code == KeyCode::Char('r') && self.can_retry() {
            return Some(Action::StartInstall);
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

        let profile_lanes =
            self.model.purpose == WizardPurpose::Install && !self.model.profiles.is_empty();
        match &mut self.stages[self.stage_index] {
            Stage::Choose(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => stage.step(-1),
                KeyCode::Down | KeyCode::Char('j') => stage.step(1),
                KeyCode::PageUp => stage.step(-10),
                KeyCode::PageDown => stage.step(10),
                KeyCode::Home => stage.step(isize::MIN / 2),
                KeyCode::End => stage.step(isize::MAX / 2),
                KeyCode::Left | KeyCode::Char('h') => {
                    stage.focus = match (profile_lanes, stage.focus) {
                        (true, Pane::Items) => Pane::Kinds,
                        _ => Pane::Groups,
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    stage.focus = match (profile_lanes, stage.focus) {
                        (true, Pane::Groups) => Pane::Kinds,
                        _ => Pane::Items,
                    }
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    stage.focus = if profile_lanes {
                        match (key.code, stage.focus) {
                            (KeyCode::BackTab, Pane::Groups) => Pane::Items,
                            (KeyCode::BackTab, Pane::Kinds) => Pane::Groups,
                            (KeyCode::BackTab, Pane::Items) => Pane::Kinds,
                            (_, Pane::Groups) => Pane::Kinds,
                            (_, Pane::Kinds) => Pane::Items,
                            (_, Pane::Items) => Pane::Groups,
                        }
                    } else {
                        match stage.focus {
                            Pane::Groups => Pane::Items,
                            Pane::Kinds | Pane::Items => Pane::Groups,
                        }
                    }
                }
                KeyCode::Char(' ') => match stage.focus {
                    Pane::Groups => {
                        let index = stage.group_cursor;
                        self.toggle_goal(index);
                    }
                    Pane::Kinds => {
                        let items = stage.kind().bulk_items();
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
            Stage::Responses { cursor } => match key.code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Home => *cursor = 0,
                KeyCode::Down | KeyCode::Char('j') | KeyCode::End => *cursor = 1,
                KeyCode::Char(' ')
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Left
                | KeyCode::Right => *cursor = 1 - *cursor,
                _ => {}
            },
            Stage::Review { scroll } => match key.code {
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                _ => {}
            },
            Stage::Install(stage) => match key.code {
                KeyCode::Char('d') => stage.show_details = !stage.show_details,
                KeyCode::Up | KeyCode::Char('k') => stage.scroll = stage.scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => stage.scroll = stage.scroll.saturating_add(1),
                KeyCode::PageUp => stage.scroll = stage.scroll.saturating_sub(10),
                KeyCode::PageDown => stage.scroll = stage.scroll.saturating_add(10),
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
                return WizardOutcome::Installed {
                    report: report.clone(),
                    resources: self.expanded_selection(),
                    destination: self.skill_destination(),
                    written: self.completed_writes.clone(),
                };
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
        if self.can_retry() {
            return Some(Action::StartInstall);
        }
        match &self.stages[self.stage_index] {
            Stage::Review { .. } => self.confirm_review(),
            Stage::Install(stage) => stage
                .report
                .as_ref()
                .map(|_| Action::Exit(self.exit_outcome())),
            Stage::Responses { cursor } => {
                self.adhd_enabled = *cursor == 0;
                self.go_forward();
                None
            }
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
        if self.wiki.is_some()
            && matches!(row, Row::Resource(index) if self.model.resources[*index].group == "Wiki")
        {
            if let Stage::Choose(stage) = &mut self.stages[CHOOSE] {
                if let Some(group) = stage
                    .groups
                    .iter()
                    .position(|group| !group.everything && group.rows.contains(row))
                {
                    stage.group_cursor = group;
                    stage.focus = Pane::Kinds;
                    self.search = None;
                }
            }
            return;
        }
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
        } else if self.has_mcp()
            && !self.has_skills()
            && SkillAgent::ALL.get(cursor - 1) != Some(&SkillAgent::Pi)
        {
            // Non-Pi adapters are unverified, not selectable MCP destinations.
        } else if let Some(on) = self.agent_on.get_mut(cursor - 1) {
            *on = !*on;
        }
    }

    #[cfg(test)]
    pub(crate) fn next_actions(&self, report: &crate::InstallReport) -> Vec<String> {
        crate::app::next_actions(&self.expanded_selection(), report)
    }

    pub(crate) fn goal_next_actions(&self, report: &InstallReport) -> Vec<(String, String)> {
        let Stage::Choose(stage) = &self.stages[CHOOSE] else {
            return Vec::new();
        };
        let selected = self.expanded_selection();
        stage
            .groups
            .iter()
            .enumerate()
            .filter(|(index, _)| self.picked_goals.contains(index))
            .filter_map(|(_, group)| {
                let profile = self
                    .model
                    .profiles
                    .iter()
                    .find(|p| p.label == group.title)?;
                let resources = profile
                    .resources
                    .iter()
                    .filter_map(|id| {
                        selected
                            .iter()
                            .find(|r| &r.id == id && r.group != "Wiki")
                            .cloned()
                    })
                    .collect::<Vec<_>>();
                crate::app::next_actions(&resources, report)
                    .into_iter()
                    .next()
                    .map(|action| (group.title.clone(), action))
            })
            .collect()
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
        // Equal scores keep catalog order even though settings now appear
        // inside every profile. Overlapping profiles still keep one hit.
        scored.sort_by_key(|(score, group, row)| {
            let rank = match stage.groups[*group].rows[*row].item() {
                Item::Resource(index) => index,
                Item::Setting(index) => self.model.resources.len() + index,
            };
            (std::cmp::Reverse(*score), rank)
        });
        let mut seen = HashSet::new();
        scored
            .into_iter()
            .filter_map(|(_, group, row)| {
                let item = stage.groups[group].rows[row].item();
                seen.insert(item).then_some((group, row))
            })
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
            let target = stage.groups[group].rows[row].clone();
            if let Some((kind, item)) =
                stage.groups[group]
                    .kinds
                    .iter()
                    .enumerate()
                    .find_map(|(kind, section)| {
                        section
                            .rows
                            .iter()
                            .position(|candidate| candidate == &target)
                            .map(|item| (kind, item))
                    })
            {
                stage.kind_cursor = kind;
                stage.item_cursor = item;
            }
            stage.focus = if self.wiki.is_some()
                && matches!(target, Row::Resource(index) if self.model.resources[index].group == "Wiki")
            {
                Pane::Kinds
            } else {
                Pane::Items
            };
        }
    }

    // ---- mouse -------------------------------------------------------------

    pub fn handle_click(&mut self, column: u16, row: u16) -> Option<Action> {
        if self.show_help
            || self.confirm_quit
            || self.install_running()
            || self
                .wiki
                .as_ref()
                .is_some_and(|browser| browser.confirm_unregister.is_some())
        {
            return None;
        }
        if contains(self.hits.back_button, column, row) {
            if self.browsing_wiki() {
                return self.wiki_key(KeyCode::Left);
            }
            self.go_back();
            return None;
        }
        if contains(self.hits.next_button, column, row) {
            // Continue setup without treating a Wiki navigation row as an install pick.
            return self.handle_enter();
        }
        if let Some((area, offset)) = self.hits.groups {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                if let Stage::Choose(stage) = &mut self.stages[self.stage_index] {
                    if index < stage.groups.len() {
                        stage.focus = Pane::Groups;
                        stage.group_cursor = index;
                        stage.kind_cursor = 0;
                        stage.item_cursor = 0;
                    }
                }
                return None;
            }
        }
        if self.browsing_wiki() {
            if let Some((area, offset)) = self.hits.kinds {
                if contains(area, column, row)
                    && row > area.y
                    && row < area.bottom().saturating_sub(1)
                {
                    let index = offset + row.saturating_sub(area.y + 1) as usize;
                    if let Some(browser) = &mut self.wiki {
                        if index < browser.len() {
                            browser.cursor = index;
                            browser.item_cursor = 0;
                            browser.search = None;
                            if let Stage::Choose(stage) = &mut self.stages[self.stage_index] {
                                stage.focus = Pane::Kinds;
                            }
                            if browser.record().is_none() {
                                return browser.entry().map(Action::PickWiki);
                            }
                        }
                    }
                    return None;
                }
            }
            if let Some((area, offset)) = self.hits.list {
                if contains(area, column, row)
                    && row > area.y
                    && row < area.bottom().saturating_sub(1)
                {
                    if let Stage::Choose(stage) = &mut self.stages[self.stage_index] {
                        stage.focus = Pane::Items;
                    }
                    if let Some(browser) = &mut self.wiki {
                        let index = offset + row.saturating_sub(area.y + 1) as usize;
                        if index < browser.capabilities().len() {
                            browser.item_cursor = index;
                            browser.toggle();
                        }
                    }
                    return None;
                }
            }
        }
        if let Some((area, offset)) = self.hits.kinds {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                if let Stage::Choose(stage) = &mut self.stages[self.stage_index] {
                    if index < stage.group().kinds.len() {
                        stage.focus = Pane::Kinds;
                        stage.kind_cursor = index;
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
                if let Some(row) = stage.kind().rows.get(index).cloned() {
                    stage.focus = Pane::Items;
                    stage.item_cursor = index;
                    self.activate_row(&row);
                }
            }
            Stage::Responses { cursor } if index < 2 => *cursor = index,
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

/// The install chooser starts with overlapping role profiles. Uninstall and
/// profile-less fixtures keep the ownership and resource-kind groups.
fn choose_groups(model: &Model) -> Vec<Group> {
    if model.purpose == WizardPurpose::Install && !model.profiles.is_empty() {
        return profile_groups(model);
    }

    let mut groups = resource_groups(model);
    let mut seen: Vec<&str> = Vec::new();
    for spec in &model.settings {
        if seen.contains(&spec.group.as_str()) {
            continue;
        }
        seen.push(&spec.group);
        let rows = model
            .settings
            .iter()
            .enumerate()
            .filter(|(_, other)| other.group == spec.group)
            .map(|(index, _)| Row::Setting(index))
            .collect::<Vec<_>>();
        groups.push(Group {
            title: format!("Settings · {}", spec.group),
            description: spec.description.clone(),
            bulk_rows: rows.clone(),
            kinds: vec![KindGroup {
                title: "Settings".into(),
                bulk_rows: rows.clone(),
                rows: rows.clone(),
            }],
            rows,
            everything: false,
        });
    }
    groups
}

fn profile_groups(model: &Model) -> Vec<Group> {
    let all_resources = (0..model.resources.len())
        .filter(|index| {
            !model.resources[*index].is_automatic_pi_package()
                && model.resources[*index].group != "Wiki"
        })
        .map(Row::Resource)
        .collect::<Vec<_>>();
    let mut groups = Vec::new();
    for profile in &model.profiles {
        let members = if profile.id == "knowledge-wiki" {
            model
                .resources
                .iter()
                .filter(|resource| resource.group == "Wiki")
                .map(|resource| resource.id.clone())
                .collect()
        } else {
            profile.resources.clone()
        };
        let direct = members
            .iter()
            .filter_map(|id| {
                model
                    .resources
                    .iter()
                    .position(|resource| &resource.id == id)
            })
            .filter(|index| {
                !model.resources[*index].is_automatic_pi_package()
                    && (profile.id == "knowledge-wiki") == (model.resources[*index].group == "Wiki")
            })
            .map(Row::Resource)
            .collect::<Vec<_>>();
        if direct.is_empty() {
            continue;
        }
        let selected = direct
            .iter()
            .filter_map(|row| match row {
                Row::Resource(index) => Some(model.resources[*index].clone()),
                Row::Setting(_) => None,
            })
            .collect();
        let rows = crate::expand_skill_dependencies(
            &model.resources,
            selected,
            &model.skill_destination.agents,
        )
        .iter()
        .filter(|resource| (profile.id == "knowledge-wiki") == (resource.group == "Wiki"))
        .filter_map(|resource| {
            model
                .resources
                .iter()
                .position(|candidate| candidate.id == resource.id)
                .map(Row::Resource)
        })
        .collect::<Vec<_>>();
        let kinds = profile_kinds(model, &rows, &direct);
        let visible_rows = kinds
            .iter()
            .flat_map(|kind| kind.rows.iter().cloned())
            .collect();
        groups.push(Group {
            title: profile.label.clone(),
            description: profile.description.clone(),
            rows: visible_rows,
            bulk_rows: direct,
            kinds,
            everything: false,
        });
    }
    let kinds = profile_kinds(model, &all_resources, &all_resources);
    let rows = kinds
        .iter()
        .flat_map(|kind| kind.rows.iter().cloned())
        .collect();
    groups.push(Group {
        title: "Everything".into(),
        description: "Every general capability. Vault-scoped resources are under Wiki.".into(),
        rows,
        bulk_rows: all_resources,
        kinds,
        everything: true,
    });
    groups
}

fn profile_kinds(model: &Model, rows: &[Row], bulk_rows: &[Row]) -> Vec<KindGroup> {
    let mut kinds = Vec::new();
    for (kind, title) in [
        (ResourceKind::Skill, "Skills"),
        (ResourceKind::Tool, "Tools"),
        (ResourceKind::PiPackage, "Pi packages"),
        (ResourceKind::HerdrPlugin, "Herdr plugins"),
        (ResourceKind::McpServer, "MCP servers"),
    ] {
        let visible = rows
            .iter()
            .filter(
                |row| matches!(row, Row::Resource(index) if model.resources[*index].kind == kind),
            )
            .cloned()
            .collect::<Vec<_>>();
        if visible.is_empty() {
            continue;
        }
        let direct = bulk_rows
            .iter()
            .filter(
                |row| matches!(row, Row::Resource(index) if model.resources[*index].kind == kind),
            )
            .cloned()
            .collect();
        kinds.push(KindGroup {
            title: title.into(),
            rows: visible,
            bulk_rows: direct,
        });
    }
    if !model.settings.is_empty()
        && !rows.iter().any(
            |row| matches!(row, Row::Resource(index) if model.resources[*index].group == "Wiki"),
        )
    {
        let settings = (0..model.settings.len())
            .map(Row::Setting)
            .collect::<Vec<_>>();
        kinds.push(KindGroup {
            title: "Settings".into(),
            rows: settings.clone(),
            bulk_rows: settings,
        });
    }
    kinds
}

fn resource_groups(model: &Model) -> Vec<Group> {
    let mut groups = Vec::new();
    let visible = |index: usize| {
        model.purpose == WizardPurpose::Uninstall
            || !model.resources[index].is_automatic_pi_package()
    };
    let rows = (0..model.resources.len())
        .filter(|index| visible(*index) && model.resources[*index].group != "Wiki")
        .map(Row::Resource)
        .collect::<Vec<_>>();
    if !rows.is_empty() {
        groups.push(Group {
            title: "Everything".into(),
            description: "Every general resource. Vault-scoped resources are under Wiki.".into(),
            rows: rows.clone(),
            bulk_rows: rows.clone(),
            kinds: vec![KindGroup {
                title: "Items".into(),
                rows: rows.clone(),
                bulk_rows: rows,
            }],
            everything: true,
        });
    }
    let mut push_group = |title: String, items: Vec<usize>| {
        let rows = items
            .into_iter()
            .filter(|index| visible(*index))
            .map(Row::Resource)
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            groups.push(Group {
                description: title.clone(),
                title,
                bulk_rows: rows.clone(),
                kinds: vec![KindGroup {
                    title: "Items".into(),
                    rows: rows.clone(),
                    bulk_rows: rows.clone(),
                }],
                rows,
                everything: false,
            });
        }
    };
    push_group(
        "Wiki".into(),
        indices(&model.resources, |resource| resource.group == "Wiki"),
    );
    for category in groups_of(&model.resources, ResourceKind::Skill)
        .into_iter()
        .filter(|category| category != "Wiki")
    {
        let items = indices(&model.resources, |resource| {
            resource.kind == ResourceKind::Skill && resource.group == category
        });
        push_group(format!("Skills · {category}"), items);
    }
    for (kind, title) in [
        (ResourceKind::Tool, "Tools"),
        (ResourceKind::PiPackage, "Pi packages"),
        (ResourceKind::HerdrPlugin, "Herdr plugins"),
        (ResourceKind::McpServer, "MCP servers"),
    ] {
        push_group(
            title.into(),
            indices(&model.resources, |resource| {
                resource.kind == kind && resource.group != "Wiki"
            }),
        );
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
