//! The wizard's state machine: four stages (Choose → Where → Review →
//! Install), overlapping role profiles over one selection, key and mouse
//! handling, and install progress. Everything here is terminal-free so the whole flow is
//! unit-testable; rendering lives in `render.rs`.

use super::choose::choose_groups;
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

/// One capability type inside a profile, including its visible dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KindGroup {
    pub title: String,
    pub rows: Vec<Item>,
}

impl KindGroup {
    /// Expansion appends dependencies after direct members, preserving their
    /// order. Type-wide picks skip those dependencies and include settings.
    pub fn bulk_items<'a>(&'a self, group: &'a Group) -> impl Iterator<Item = Item> + 'a {
        self.rows
            .iter()
            .copied()
            .filter(|item| matches!(item, Item::Setting(_)) || group.bulk_rows.contains(item))
    }
}

/// A profile or legacy uninstall group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Group {
    pub title: String,
    pub description: String,
    pub bulk_rows: Vec<Item>,
    pub kinds: Vec<KindGroup>,
    pub everything: bool,
}

impl Group {
    pub fn items(&self) -> impl Iterator<Item = Item> + '_ {
        self.kinds.iter().flat_map(|kind| kind.rows.iter().copied())
    }
}

/// Which column has the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pane {
    Groups,
    Kinds,
    Items,
}

impl Pane {
    pub const ALL: [Self; 3] = [Self::Groups, Self::Kinds, Self::Items];

    pub(super) fn navigate(&mut self, code: KeyCode, panes: &[Self]) -> bool {
        let current = panes.iter().position(|pane| pane == self);
        let last = panes.len() - 1;
        let index = match code {
            KeyCode::Left | KeyCode::Char('h') => current.unwrap_or(0).saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => {
                current.map_or(last, |index| (index + 1).min(last))
            }
            KeyCode::Tab => current.map_or(0, |index| (index + 1) % panes.len()),
            KeyCode::BackTab => current.map_or(0, |index| (index + last) % panes.len()),
            _ => return false,
        };
        *self = panes[index];
        true
    }
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
            .position(|group| !group.everything && group.items().next().is_some())
            .or_else(|| {
                groups
                    .iter()
                    .position(|group| group.items().next().is_some())
            })
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

    pub fn row(&self) -> Option<&Item> {
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

/// Copy through whichever clipboard tool the platform has; false when none.
fn copy_to_clipboard(text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(windows) {
        &[("clip", &[])]
    } else {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
    };
    candidates.iter().any(|(program, args)| {
        let Ok(mut child) = Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let written = child
            .stdin
            .take()
            .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        written && child.wait().is_ok_and(|status| status.success())
    })
}

/// Vertical movement for a scrolled view; the renderer clamps the bottom.
fn scroll_key(scroll: &mut u16, code: KeyCode) {
    *scroll = match code {
        KeyCode::Up | KeyCode::Char('k') => scroll.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => scroll.saturating_add(1),
        KeyCode::PageUp => scroll.saturating_sub(10),
        KeyCode::PageDown => scroll.saturating_add(10),
        KeyCode::Home => 0,
        _ => *scroll,
    };
}

pub(super) fn clamp_step(cursor: usize, delta: isize, len: usize) -> usize {
    cursor
        .saturating_add_signed(delta)
        .min(len.saturating_sub(1))
}

pub(super) fn movement(code: KeyCode) -> Option<isize> {
    match code {
        KeyCode::Up | KeyCode::Char('k') => Some(-1),
        KeyCode::Down | KeyCode::Char('j') => Some(1),
        KeyCode::PageUp => Some(-10),
        KeyCode::PageDown => Some(10),
        KeyCode::Home => Some(isize::MIN),
        KeyCode::End => Some(isize::MAX),
        _ => None,
    }
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
    /// The command last copied to the clipboard, shown as confirmation.
    pub copied: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum Screen {
    Choose,
    Where,
    Responses,
    Review,
    Install,
}

impl Screen {
    pub const ALL: [Self; 5] = [
        Self::Choose,
        Self::Where,
        Self::Responses,
        Self::Review,
        Self::Install,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Choose => "Choose",
            Self::Where => "Where",
            Self::Responses => "Pi responses",
            Self::Review => "Review",
            Self::Install => "Install",
        }
    }
}

/// Events sent by the install worker thread.
#[derive(Debug)]
pub enum InstallEvent {
    Confirm(String, Vec<String>, std::sync::mpsc::Sender<bool>),
    Detail(usize, String),
    Status(usize, ExecStatus),
    Finished(Box<InstallJob>, InstallReport),
}

/// The work handed to the install worker thread.
pub struct InstallJob {
    pub session: crate::session::InstallSession,
    pub(super) wikis: Vec<super::wiki_install::WikiInstall>,
    pub cancelled: Arc<AtomicBool>,
}

impl std::fmt::Debug for InstallJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallJob")
            .field("plan", &self.session.plan)
            .finish_non_exhaustive()
    }
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
    pub(super) wiki: super::wiki::WikiBrowser,
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
    pub(crate) screen: Screen,
    pub(crate) choose: ChooseStage,
    pub(crate) where_cursor: usize,
    pub(crate) responses_cursor: usize,
    pub(crate) review_scroll: u16,
    pub(crate) install: InstallStage,
    pub(crate) hits: HitMap,
    /// `Some(query)` while `/` search filters the Choose list.
    pub(crate) search: Option<String>,
    pub(crate) search_cursor: usize,
    pub(crate) show_help: bool,
    /// True while the installed-state probe still runs in the background.
    pub(crate) probing: bool,
    /// Installed marks the probe found for each scope; project scope also
    /// honours global installs, global scope only its own.
    installed_global: Vec<bool>,
    installed_project: Vec<bool>,
    /// Quit confirmation pending (a non-empty selection would be discarded).
    pub(crate) confirm_quit: bool,
    /// First Ctrl-C during install arms cancellation; a second confirms it.
    pub(crate) confirm_cancel: bool,
    /// Review Next/Install enablement, computed once per draw.
    pub(super) review_next_enabled: bool,
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(super) reviewed_job: Option<InstallJob>,
}

impl Wizard {
    pub fn new(model: Model, wiki: super::wiki::WikiBrowser) -> Self {
        let choose = ChooseStage::new(choose_groups(&model));
        let install = InstallStage {
            items: Vec::new(),
            running: false,
            report: None,
            tick: 0,
            scroll: 0,
            started: None,
            elapsed: std::time::Duration::ZERO,
            show_details: false,
            copied: None,
        };
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
        let installed_marks = model.installed.clone();
        let mut wizard = Self {
            wiki,
            selected,
            picked_goals: HashSet::new(),
            custom_picks: BTreeMap::new(),
            setting_on: vec![false; model.settings.len()],
            agent_on,
            skill_scope,
            adhd_enabled: false,
            setting_touched: vec![false; model.settings.len()],
            choose,
            where_cursor: 1,
            responses_cursor: 1,
            review_scroll: 0,
            install,
            screen: Screen::Choose,
            model,
            hits: HitMap::default(),
            search: None,
            search_cursor: 0,
            show_help: false,
            probing: false,
            installed_global: installed_marks.clone(),
            installed_project: installed_marks,
            confirm_quit: false,
            confirm_cancel: false,
            review_next_enabled: true,
            cancelled: Arc::new(AtomicBool::new(false)),
            reviewed_job: None,
        };
        wizard.precheck_settings();
        wizard
    }

    // ---- selection helpers -------------------------------------------------

    fn include_automatic_pi_loom(&self) -> bool {
        (self.model.mode == crate::app::SelectionMode::Setup && self.model.status.pi)
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
                })
    }

    fn resource_in_selection(&self, index: usize, include_pi_loom: bool) -> bool {
        let resource = &self.model.resources[index];
        (self.selected[index]
            || (include_pi_loom
                && resource.is_automatic_pi_package()
                && !self.resource_installed(index))
            || (self.adhd_enabled
                && resource.id == "pi-package:i-have-adhd"
                && !self.resource_installed(index)))
            && resource.group != "Wiki"
            && (self.model.purpose == WizardPurpose::Install || self.required_note(index).is_none())
    }

    pub(super) fn in_selection(&self, index: usize) -> bool {
        self.resource_in_selection(index, self.include_automatic_pi_loom())
    }

    fn selection_indices(&self) -> impl Iterator<Item = usize> + '_ {
        let include_pi_loom = self.include_automatic_pi_loom();
        (0..self.model.resources.len())
            .filter(move |&index| self.resource_in_selection(index, include_pi_loom))
    }

    pub(crate) fn selection(&self) -> Vec<Resource> {
        self.selection_indices()
            .map(|index| self.model.resources[index].clone())
            .collect()
    }

    /// The selection with skill dependencies pulled in.
    pub(crate) fn expanded_selection(&self) -> Vec<Resource> {
        if self.uninstalling() {
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

    pub(crate) fn skill_count(&self) -> usize {
        let agents = self.selected_agents();
        let mut names = std::collections::BTreeSet::new();
        for index in self.selection_indices() {
            let resource = &self.model.resources[index];
            if resource.kind == ResourceKind::Skill {
                names.insert(resource.install_target.as_str());
            }
            if resource.kind == ResourceKind::PiPackage && !agents.is_empty() {
                names.extend(resource.bundled_skills.iter().map(String::as_str));
            }
            for dependency in &resource.dependencies {
                if let Some(candidate) = self.model.resources.iter().find(|candidate| {
                    (candidate.id == *dependency || candidate.install_target == *dependency)
                        && candidate.kind == ResourceKind::Skill
                }) {
                    names.insert(candidate.install_target.as_str());
                }
            }
        }
        names.len()
    }

    pub(crate) fn has_skills(&self) -> bool {
        self.skill_count() != 0
    }

    pub(crate) fn has_mcp(&self) -> bool {
        self.selection_indices()
            .any(|index| self.model.resources[index].kind == ResourceKind::McpServer)
    }

    pub(super) fn included_note(&self, index: usize) -> Option<String> {
        let skill = &self.model.resources[index];
        if self.uninstalling() || skill.kind != ResourceKind::Skill {
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
        if self.uninstalling() {
            return false;
        }
        let resource = &self.model.resources[index];
        // A global install cannot satisfy a Vault that has not been chosen.
        if resource.group == "Wiki" {
            return false;
        }
        if resource.kind == ResourceKind::McpServer {
            return self.installed_global[index]
                || (self.skill_scope == SkillScope::Project && self.installed_project[index]);
        }
        if resource.kind == ResourceKind::Skill {
            let destination = self.skill_destination();
            let unchanged_destination = destination.scope == self.model.skill_destination.scope
                && destination.agents == self.model.skill_destination.agents;
            return (unchanged_destination && self.model.installed[index])
                || self.skill_in_every_tree(index, &destination)
                || self.skill_installed_globally(index);
        }
        self.model.installed[index]
    }

    /// The skill is in every selected agent's global tree. A global install
    /// serves every project, so it counts in project scope too.
    fn skill_installed_globally(&self, index: usize) -> bool {
        let mut destination = self.skill_destination();
        destination.scope = SkillScope::Global;
        self.skill_in_every_tree(index, &destination)
    }

    fn skill_in_every_tree(&self, index: usize, destination: &SkillDestination) -> bool {
        let target = &self.model.resources[index].install_target;
        let trees = destination.trees();
        !trees.is_empty()
            && trees.iter().all(|tree| {
                crate::skills::skill_present_in(tree, target)
                    || crate::bundled_skills::provided_in_tree(&destination.home, tree, target)
            })
    }

    /// In project scope: installed, but only because of a global install.
    pub(crate) fn installed_globally_only(&self, index: usize) -> bool {
        if self.skill_scope != SkillScope::Project {
            return false;
        }
        let resource = &self.model.resources[index];
        match resource.kind {
            ResourceKind::McpServer => {
                self.installed_global[index] && !self.installed_project[index]
            }
            ResourceKind::Skill => {
                self.skill_installed_globally(index)
                    && !self.skill_in_every_tree(index, &self.skill_destination())
            }
            _ => false,
        }
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

    /// What the user picked themselves: the selection without the setup
    /// requirements Loom adds on its own (Pi adapter, Loom package).
    pub(crate) fn user_picked(&self) -> usize {
        let picked = self
            .selection_indices()
            .filter(|&index| !self.setup_requirement(index))
            .count();
        picked + self.selected_settings().len() + self.wiki.count()
    }

    /// The Pi responses answer, for the Review screens; `None` when the
    /// question was never asked.
    pub(crate) fn responses_summary(&self) -> Option<&'static str> {
        self.stage_visible(Screen::Responses)
            .then_some(if self.adhd_enabled {
                "Pi responses: always enable ADHD-friendly responses"
            } else {
                "Pi responses: leave settings unchanged"
            })
    }

    pub(crate) fn uninstalling(&self) -> bool {
        self.model.purpose == WizardPurpose::Uninstall
    }

    /// Goal-based setup: the Choose screen shows Goals | Types | Capabilities.
    pub(crate) fn profile_mode(&self) -> bool {
        self.model.purpose == WizardPurpose::Install && !self.model.profiles.is_empty()
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
        self.selection_indices().count() + self.selected_settings().len() + self.wiki.count()
    }

    pub(crate) fn item_state(&self, item: Item) -> ItemState {
        match item {
            Item::Resource(index)
                if !self.uninstalling() && self.model.resources[index].group == "Wiki" =>
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
                        if self.uninstalling() {
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
        for (index, spec) in self.model.settings.iter().enumerate() {
            if self.setting_touched[index] || self.setting_applied(index) {
                continue;
            }
            self.setting_on[index] = match &spec.related_resource {
                Some(resource_id) => {
                    self.model
                        .resources
                        .iter()
                        .enumerate()
                        .any(|(resource_index, resource)| {
                            resource.id == *resource_id
                                && (self.in_selection(resource_index)
                                    || self.resource_installed(resource_index))
                        })
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

    /// Back to a blank slate: no goals, no individual picks, no settings.
    fn clear_picks(&mut self) {
        self.picked_goals.clear();
        self.custom_picks.clear();
        for index in 0..self.selected.len() {
            if !self.setup_requirement(index) {
                self.selected[index] = false;
            }
        }
        self.setting_on.fill(false);
        self.precheck_settings();
    }

    fn toggle_goal(&mut self, group_index: usize) {
        let stage = &self.choose;
        let group = &stage.groups[group_index];
        if group.everything || self.model.profiles.is_empty() || self.uninstalling() {
            let items = group.bulk_rows.clone();
            self.toggle_group(&items);
            return;
        }
        if !self.picked_goals.remove(&group_index) {
            self.picked_goals.insert(group_index);
        }
        for row in &group.bulk_rows {
            let Item::Resource(index) = row else {
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
        if self.installed_globally_only(index) {
            return "Already installed globally".into();
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
        if self.selected[index] && !self.custom_picks.contains_key(&index) {
            let stage = &self.choose;
            let goals = stage
                .groups
                .iter()
                .enumerate()
                .filter(|(goal, group)| {
                    self.picked_goals.contains(goal)
                        && group.bulk_rows.contains(&Item::Resource(index))
                })
                .map(|(_, group)| group.title.as_str())
                .collect::<Vec<_>>();
            if !goals.is_empty() {
                return format!("Included by {}", goals.join(", "));
            }
        }
        String::new()
    }

    /// The background probe finished: adopt the real installed marks and
    /// drop any picks the probe proved redundant.
    /// Probe results for both scopes; the model keeps the one it started in.
    pub fn set_installed_scoped(&mut self, global: Vec<bool>, project: Vec<bool>) {
        let current = match self.model.skill_destination.scope {
            SkillScope::Global => global.clone(),
            SkillScope::Project => project.clone(),
        };
        if global.len() == self.model.installed.len() && project.len() == global.len() {
            self.installed_global = global;
            self.installed_project = project;
        }
        self.set_installed(current);
    }

    pub fn set_installed(&mut self, installed: Vec<bool>) {
        if self.screen == Screen::Install {
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
        if self.uninstalling() {
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
        self.screen == Screen::Install && self.install.running
    }

    /// Where only exists when skills are going somewhere.
    pub(crate) fn stage_visible(&self, screen: Screen) -> bool {
        match screen {
            Screen::Where => !self.uninstalling() && (self.has_skills() || self.has_mcp()),
            Screen::Responses => {
                !self.uninstalling()
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

    pub(crate) fn visible_stages(&self) -> Vec<Screen> {
        Screen::ALL
            .into_iter()
            .filter(|&screen| self.stage_visible(screen))
            .collect()
    }

    fn go_forward(&mut self) {
        if let Some(screen) = Screen::ALL.into_iter().find(|&screen| {
            screen > self.screen && screen <= Screen::Review && self.stage_visible(screen)
        }) {
            self.screen = screen;
            self.entered_stage();
        }
    }

    fn go_back(&mut self) {
        self.search = None;
        if self.screen == Screen::Install {
            return;
        }
        if let Some(screen) = Screen::ALL
            .into_iter()
            .rev()
            .find(|&screen| screen < self.screen && self.stage_visible(screen))
        {
            self.screen = screen;
        }
    }

    fn entered_stage(&mut self) {
        self.search = None;
        if self.screen == Screen::Where && self.has_mcp() && !self.has_skills() {
            self.agent_on = SkillAgent::ALL
                .iter()
                .map(|a| *a == SkillAgent::Pi)
                .collect();
        }
        if self.screen == Screen::Review {
            self.review_scroll = 0;
        }
    }

    fn confirm_review(&mut self) -> Option<Action> {
        if self.nothing_chosen() {
            return Some(Action::Exit(WizardOutcome::NothingSelected));
        }
        if self.uninstalling() {
            return Some(Action::Exit(WizardOutcome::UninstallSelection(
                self.selection_indices()
                    .map(|index| self.model.resources[index].id.clone())
                    .collect(),
            )));
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
            for wiki in self.wiki_jobs().unwrap_or_default() {
                summary.push(format!(
                    "Knowledgebase {}: {}",
                    wiki.record.path.display(),
                    wiki.labels.join(", ")
                ));
                summary.extend(wiki.plan.steps.iter().map(|step| step.operation.display()));
            }
            return Some(Action::Exit(WizardOutcome::DryRun(plan, summary)));
        }
        self.screen = Screen::Install;
        Some(Action::StartInstall)
    }

    // ---- install execution -------------------------------------------------

    pub(crate) fn can_retry(&self) -> bool {
        self.screen == Screen::Install
            && !self.install.running
            && self
                .install
                .report
                .as_ref()
                .is_some_and(|report| !report.failures.is_empty())
    }

    /// Retries use the exact reviewed plan, not a new selection or destination.
    pub fn begin_install(&mut self) -> Result<InstallJob> {
        let job = match self.reviewed_job.take() {
            Some(job) => job,
            None => InstallJob {
                session: crate::session::InstallSession::new(
                    self.plan()?,
                    self.selected_settings(),
                    self.model.settings_paths.clone(),
                ),
                wikis: self.wiki_jobs()?,
                cancelled: Arc::clone(&self.cancelled),
            },
        };
        let plan = &job.session.plan;
        let settings = &job.session.settings;
        let mut items = Vec::new();
        for step in plan.prerequisites() {
            items.push(ExecItem {
                label: format!("Install {}", step.target),
                detail: step.operation.display(),
                status: ExecStatus::Pending,
                started: None,
                elapsed: std::time::Duration::ZERO,
            });
        }
        for step in plan.resources() {
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
                detail: step.operation.display(),
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
        anyhow::ensure!(
            self.screen == Screen::Install,
            "install started outside the install stage"
        );
        let stage = &mut self.install;
        stage.items = items;
        stage.running = true;
        stage.report = None;
        stage.started = Some(std::time::Instant::now());
        stage.elapsed = std::time::Duration::ZERO;
        stage.scroll = 0;
        stage.show_details = false;
        self.confirm_cancel = false;
        self.cancelled.store(false, Ordering::Relaxed);
        Ok(job)
    }

    pub fn handle_install_event(&mut self, event: InstallEvent) {
        if self.screen != Screen::Install {
            return;
        }
        match event {
            InstallEvent::Confirm(_, _, reply) => {
                let _ = reply.send(false);
            }
            InstallEvent::Detail(index, detail) => {
                if let Some(item) = self.install.items.get_mut(index) {
                    item.detail = detail;
                }
            }
            InstallEvent::Status(index, status) => {
                if let Some(item) = self.install.items.get_mut(index) {
                    if matches!(status, ExecStatus::Running | ExecStatus::Verifying) {
                        item.started.get_or_insert_with(std::time::Instant::now);
                    } else if let Some(started) = item.started.take() {
                        item.elapsed = started.elapsed();
                    }
                    item.status = status;
                }
            }
            InstallEvent::Finished(job, report) => {
                self.reviewed_job = Some(*job);
                self.finish_install(report);
            }
        }
    }

    fn finish_install(&mut self, report: InstallReport) {
        let stage = &mut self.install;
        stage.running = false;
        stage.elapsed = stage
            .started
            .map_or(std::time::Duration::ZERO, |started| started.elapsed());
        stage.scroll = stage
            .items
            .iter()
            .position(|item| matches!(item.status, ExecStatus::Failed(_) | ExecStatus::Skipped(_)))
            .unwrap_or(0) as u16;
        stage.report = Some(report);
        self.confirm_cancel = false;
    }

    pub fn tick(&mut self) {
        if self.screen == Screen::Install {
            self.install.tick = self.install.tick.wrapping_add(1);
        }
    }

    pub(super) fn needs_animate(&self) -> bool {
        self.install.running
    }

    pub(super) fn poll_timeout(&self) -> std::time::Duration {
        if self.needs_animate() {
            std::time::Duration::from_millis(80)
        } else if self.probing || self.wiki.checking {
            std::time::Duration::from_millis(8)
        } else {
            std::time::Duration::from_millis(500)
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
            // y/q confirm the discard; anything else (a reflex enter or esc)
            // stays, so a double-tap cannot throw picks away.
            self.confirm_quit = false;
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('q')) {
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
        if self.wiki.confirm_unregister.is_some() {
            return self.wiki_key(key.code);
        }
        if self.browsing_wiki() {
            let scoped_search = self.wiki.search.is_some()
                || (key.code == KeyCode::Char('/')
                    && self.wiki.record().is_some()
                    && self.screen == Screen::Choose
                    && self.choose.focus != Pane::Groups);
            if scoped_search {
                return self.wiki_key(key.code);
            }
            if key.code == KeyCode::Char('n')
                || (key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                return self.handle_enter();
            }
            let focus = self.choose.focus;
            let navigation = movement(key.code).is_some();
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
            if self.screen == Screen::Install {
                return Some(Action::Exit(self.exit_outcome()));
            }
            if self.screen != Screen::Choose {
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
        if key.code == KeyCode::Char('c') && self.screen == Screen::Choose && !self.browsing_wiki()
        {
            self.clear_picks();
            return None;
        }
        if key.code == KeyCode::Char('/') && self.screen == Screen::Choose {
            self.search = Some(String::new());
            self.search_cursor = 0;
            return None;
        }

        match self.screen {
            Screen::Choose => self.choose_key(key.code),
            Screen::Where => {
                let last = SkillAgent::ALL.len();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.where_cursor = self.where_cursor.saturating_sub(1)
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.where_cursor = (self.where_cursor + 1).min(last)
                    }
                    KeyCode::Home => self.where_cursor = 0,
                    KeyCode::End => self.where_cursor = last,
                    KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                        self.toggle_where_row(self.where_cursor);
                        if key.code == KeyCode::Char(' ') && self.where_cursor > 0 {
                            self.where_cursor = (self.where_cursor + 1).min(last);
                        }
                    }
                    _ => {}
                }
            }
            Screen::Responses => match key.code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Home => self.responses_cursor = 0,
                KeyCode::Down | KeyCode::Char('j') | KeyCode::End => self.responses_cursor = 1,
                KeyCode::Char(' ')
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Left
                | KeyCode::Right => self.responses_cursor = 1 - self.responses_cursor,
                _ => {}
            },
            Screen::Review => scroll_key(&mut self.review_scroll, key.code),
            Screen::Install => match key.code {
                KeyCode::Char('d') => self.install.show_details = !self.install.show_details,
                KeyCode::Char('c') if !self.install.running => {
                    if let Some(command) = self.next_command() {
                        if copy_to_clipboard(&command) {
                            self.install.copied = Some(command);
                        }
                    }
                }
                code => scroll_key(&mut self.install.scroll, code),
            },
        }
        None
    }

    fn choose_key(&mut self, code: KeyCode) {
        let panes: &[_] = if self.profile_mode() {
            &Pane::ALL
        } else {
            &[Pane::Groups, Pane::Items]
        };
        let stage = &mut self.choose;
        if let Some(delta) = movement(code) {
            stage.step(delta);
            return;
        }
        if stage.focus.navigate(code, panes) || code != KeyCode::Char(' ') {
            return;
        }
        match stage.focus {
            Pane::Groups => {
                let index = stage.group_cursor;
                self.toggle_goal(index);
            }
            Pane::Kinds => {
                let items = stage.kind().bulk_items(stage.group()).collect::<Vec<_>>();
                self.toggle_group(&items);
            }
            Pane::Items => {
                if let Some(item) = stage.row().copied() {
                    self.activate_row(&item);
                    self.choose.step(1);
                }
            }
        }
    }

    /// Leaving from the install screen keeps the report; elsewhere a
    /// non-empty selection asks first.
    fn exit_outcome(&self) -> WizardOutcome {
        if self.screen == Screen::Install {
            let stage = &self.install;
            if let Some(report) = &stage.report {
                return WizardOutcome::Installed {
                    report: report.clone(),
                    resources: self.expanded_selection(),
                    destination: self.skill_destination(),
                    written: self
                        .reviewed_job
                        .as_ref()
                        .map(|job| job.session.written.clone())
                        .unwrap_or_default(),
                };
            }
        }
        WizardOutcome::Cancelled
    }

    fn quit(&mut self) -> Option<Action> {
        if self.screen == Screen::Install {
            return Some(Action::Exit(self.exit_outcome()));
        }
        if self.user_picked() > 0 {
            self.confirm_quit = true;
            return None;
        }
        Some(Action::Exit(WizardOutcome::Cancelled))
    }

    fn handle_enter(&mut self) -> Option<Action> {
        if self.can_retry() {
            return Some(Action::StartInstall);
        }
        match self.screen {
            Screen::Review => self.confirm_review(),
            Screen::Install => self
                .install
                .report
                .as_ref()
                .map(|_| Action::Exit(self.exit_outcome())),
            Screen::Responses => {
                self.adhd_enabled = self.responses_cursor == 0;
                self.go_forward();
                None
            }
            Screen::Choose | Screen::Where => {
                self.go_forward();
                None
            }
        }
    }

    fn activate_row(&mut self, item: &Item) {
        if !self.uninstalling()
            && matches!(item, Item::Resource(index) if self.model.resources[*index].group == "Wiki")
        {
            if let Some(group) = self.choose.groups.iter().position(|group| {
                !group.everything && group.items().any(|candidate| candidate == *item)
            }) {
                self.choose.group_cursor = group;
                self.choose.focus = Pane::Kinds;
                self.search = None;
            }
            return;
        }
        self.toggle_item(*item);
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

    /// The first `backticked` command in the next actions, for copying.
    pub(crate) fn next_command(&self) -> Option<String> {
        let stage = &self.install;
        let report = stage.report.as_ref()?;
        if !report.failures.is_empty() {
            return None;
        }
        let mut actions = self
            .goal_next_actions(report)
            .into_iter()
            .map(|(_, action)| action)
            .collect::<Vec<_>>();
        actions.push(crate::app::install_next_action(
            self.model.mode,
            &self.expanded_selection(),
            report,
        ));
        actions
            .iter()
            .find_map(|action| action.split('`').nth(1).map(str::to_owned))
    }

    pub(crate) fn goal_next_actions(&self, report: &InstallReport) -> Vec<(String, String)> {
        let stage = &self.choose;
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
    pub(crate) fn search_matches(&self) -> Vec<(usize, Item)> {
        let Some(query) = &self.search else {
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
        let mut seen = HashSet::new();
        for (group_index, group) in self.choose.groups.iter().enumerate() {
            for item in group.items() {
                if !seen.insert(item) {
                    continue;
                }
                let (label, description) = match item {
                    Item::Resource(index) => {
                        let resource = &self.model.resources[index];
                        (&resource.label, &resource.description)
                    }
                    Item::Setting(index) => {
                        let spec = &self.model.settings[index];
                        (&spec.label, &spec.description)
                    }
                };
                let best = score_of(label)
                    .map(|score| score * 2)
                    .into_iter()
                    .chain(score_of(description))
                    .max();
                if let Some(score) = best {
                    scored.push((score, group_index, item));
                }
            }
        }
        // Stable ties retain catalog order and the first profile showing the item.
        scored.sort_by_key(|(score, _, item)| {
            let rank = match item {
                Item::Resource(index) => *index,
                Item::Setting(index) => self.model.resources.len() + index,
            };
            (std::cmp::Reverse(*score), rank)
        });
        scored
            .into_iter()
            .map(|(_, group, item)| (group, item))
            .collect()
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
                    let row = hit.1;
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
        if let Some((group, target)) = hit {
            let stage = &mut self.choose;
            stage.group_cursor = group;
            if let Some((kind, item)) =
                stage.groups[group]
                    .kinds
                    .iter()
                    .enumerate()
                    .find_map(|(kind, section)| {
                        section
                            .rows
                            .iter()
                            .position(|candidate| *candidate == target)
                            .map(|item| (kind, item))
                    })
            {
                stage.kind_cursor = kind;
                stage.item_cursor = item;
            }
            stage.focus = if self.model.purpose == WizardPurpose::Install
                && matches!(target, Item::Resource(index) if self.model.resources[index].group == "Wiki")
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
            || self.wiki.confirm_unregister.is_some()
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
            return self.handle_enter();
        }
        if let Some((area, offset)) = self.hits.groups {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                if self.screen == Screen::Choose && index < self.choose.groups.len() {
                    self.choose.focus = Pane::Groups;
                    self.choose.group_cursor = index;
                    self.choose.kind_cursor = 0;
                    self.choose.item_cursor = 0;
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
                    if index < self.wiki.len() {
                        self.wiki.cursor = index;
                        self.wiki.item_cursor = 0;
                        self.wiki.search = None;
                        self.choose.focus = Pane::Kinds;
                        if self.wiki.record().is_none() {
                            return self.wiki.entry().map(Action::PickWiki);
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
                    self.choose.focus = Pane::Items;
                    let index = offset + row.saturating_sub(area.y + 1) as usize;
                    if index < self.wiki.capabilities().len() {
                        self.wiki.item_cursor = index;
                        self.wiki.toggle();
                    }
                    return None;
                }
            }
        }
        if let Some((area, offset)) = self.hits.kinds {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                if self.screen == Screen::Choose {
                    let stage = &mut self.choose;
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
                let row = hit.1;
                self.activate_row(&row);
            }
            return;
        }
        match self.screen {
            Screen::Choose => {
                let stage = &mut self.choose;
                if let Some(row) = stage.kind().rows.get(index).cloned() {
                    stage.focus = Pane::Items;
                    stage.item_cursor = index;
                    self.activate_row(&row);
                }
            }
            Screen::Responses if index < 2 => self.responses_cursor = index,
            Screen::Where if index <= SkillAgent::ALL.len() => {
                self.where_cursor = index;
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

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}
