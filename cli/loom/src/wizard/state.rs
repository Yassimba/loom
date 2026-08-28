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

/// Everything the wizard needs to know up front; pure data so tests can
/// construct it without touching the file system.
pub struct Model {
    pub resources: Vec<Resource>,
    /// Curated selection bundles from the catalog (manifest/presets.json).
    pub presets: Vec<crate::PresetSpec>,
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
    Installed(InstallReport),
}

/// What the event loop must do after a key or mouse event.
pub enum Action {
    Exit(WizardOutcome),
    StartInstall,
}

/// Selection bundles at the top of the Choose list. Catalog bundles add to
/// the selection; Everything and Clear replace it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Preset {
    /// Everything in the catalog that is not already installed.
    Everything,
    /// A curated bundle: index into the catalog's presets.
    Catalog(usize),
    /// Clear the selection and pick by hand.
    Clear,
}

/// One selectable thing under a group header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Item {
    Resource(usize),
    Setting(usize),
}

/// A row of the Choose list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Row {
    /// A group header; space toggles every actionable item below it.
    Header {
        title: String,
        items: Vec<Item>,
    },
    Preset(Preset),
    Resource(usize),
    Setting(usize),
}

pub(crate) struct ChooseStage {
    pub rows: Vec<Row>,
    pub cursor: usize,
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
}

/// Screen regions remembered from the last draw, for mouse hit-testing.
#[derive(Default)]
pub(crate) struct HitMap {
    pub back_button: Rect,
    pub next_button: Rect,
    /// (area, first-visible-row) of the stage's list.
    pub list: Option<(Rect, usize)>,
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
}

const CHOOSE: usize = 0;
const WHERE: usize = 1;
const REVIEW: usize = 2;
const INSTALL: usize = 3;

impl Wizard {
    pub fn new(model: Model) -> Self {
        let rows = choose_rows(&model);
        let cursor = rows
            .iter()
            .position(|row| matches!(row, Row::Resource(_) | Row::Setting(_)))
            .unwrap_or(0);
        let stages = vec![
            Stage::Choose(ChooseStage { rows, cursor }),
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
        let mut wizard = Self {
            selected: vec![false; model.resources.len()],
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
        };
        wizard.precheck_settings();
        wizard
    }

    // ---- selection helpers -------------------------------------------------

    pub(crate) fn selection(&self) -> Vec<Resource> {
        self.model
            .resources
            .iter()
            .zip(&self.selected)
            .filter(|(_, on)| **on)
            .map(|(resource, _)| resource.clone())
            .collect()
    }

    /// The selection with skill dependencies pulled in.
    pub(crate) fn expanded_selection(&self) -> Vec<Resource> {
        crate::expand_skill_dependencies(&self.model.resources, self.selection())
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

    pub(crate) fn nothing_chosen(&self) -> bool {
        self.selection().is_empty() && self.selected_settings().is_empty()
    }

    pub(crate) fn plan(&self) -> Result<InstallPlan> {
        build_install_plan(
            &self.expanded_selection(),
            &[],
            self.model.status,
            self.model.platform,
            &self.skill_destination(),
        )
    }

    pub(crate) fn total_selected(&self) -> usize {
        self.selection().len() + self.setting_on.iter().filter(|on| **on).count()
    }

    /// Whether an item is on: selected, or required by a selection.
    pub(crate) fn item_on(&self, item: Item) -> bool {
        match item {
            Item::Resource(index) => self.selected[index] || self.required_note(index).is_some(),
            Item::Setting(index) => self.setting_on[index],
        }
    }

    /// Items the user can still act on: not installed, not locked in.
    pub(crate) fn actionable(&self, items: &[Item]) -> Vec<Item> {
        items
            .iter()
            .copied()
            .filter(|item| match *item {
                Item::Resource(index) => {
                    !self.resource_installed(index) && self.required_note(index).is_none()
                }
                Item::Setting(index) => !self.setting_applied(index),
            })
            .collect()
    }

    /// (on, actionable) for a group header.
    pub(crate) fn group_counts(&self, items: &[Item]) -> (usize, usize) {
        let actionable = self.actionable(items);
        let on = actionable
            .iter()
            .filter(|item| self.item_on(**item))
            .count();
        (on, actionable.len())
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

    /// Apply a bundle: catalog bundles add, Everything and Clear replace.
    fn apply_preset(&mut self, preset: Preset) {
        match preset {
            Preset::Clear => {
                self.selected.fill(false);
                self.setting_on.fill(false);
                self.setting_touched.fill(false);
            }
            Preset::Everything => {
                let available = (0..self.selected.len())
                    .map(|index| !self.resource_installed(index))
                    .collect::<Vec<_>>();
                for (on, available) in self.selected.iter_mut().zip(available) {
                    *on = available;
                }
            }
            Preset::Catalog(preset_index) => {
                for target in &self.model.presets[preset_index].targets {
                    if let Some(index) = self
                        .model
                        .resources
                        .iter()
                        .position(|resource| resource.install_target == *target)
                    {
                        if !self.resource_installed(index) {
                            self.selected[index] = true;
                        }
                    }
                }
            }
        }
        self.precheck_settings();
    }

    pub(crate) fn preset_label(&self, preset: Preset) -> &str {
        match preset {
            Preset::Everything => "Everything",
            Preset::Clear => "Clear selection",
            Preset::Catalog(index) => &self.model.presets[index].label,
        }
    }

    pub(crate) fn preset_blurb(&self, preset: Preset) -> &str {
        match preset {
            Preset::Everything => "select the whole catalog, then deselect what you don't want",
            Preset::Clear => "start from nothing and pick by hand",
            Preset::Catalog(index) => &self.model.presets[index].description,
        }
    }

    /// Resource indices a catalog bundle would add (not installed, not yet
    /// selected).
    pub(crate) fn preset_adds(&self, preset: Preset) -> usize {
        match preset {
            Preset::Clear => 0,
            Preset::Everything => (0..self.selected.len())
                .filter(|&index| !self.resource_installed(index) && !self.selected[index])
                .count(),
            Preset::Catalog(preset_index) => self.model.presets[preset_index]
                .targets
                .iter()
                .filter_map(|target| {
                    self.model
                        .resources
                        .iter()
                        .position(|resource| resource.install_target == *target)
                })
                .filter(|&index| !self.resource_installed(index) && !self.selected[index])
                .count(),
        }
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
        }
        self.probing = false;
    }

    /// The selected resource (label) that pulls `index` in as a dependency,
    /// when `index` is neither installed nor directly selected. Locked rows:
    /// shown as selected, not deselectable while the parent stays picked.
    pub(crate) fn required_note(&self, index: usize) -> Option<String> {
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
        index != WHERE || self.has_skills()
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
        let Ok(plan) = self.plan() else {
            // The review screen explains why the plan cannot run; stay.
            return None;
        };
        if self.model.dry_run {
            let summary = self
                .selected_settings()
                .iter()
                .flat_map(|spec| {
                    let path = spec.target_path(&self.model.settings_paths).display();
                    spec.change_summary()
                        .into_iter()
                        .map(move |line| format!("{path}: {line}"))
                        .collect::<Vec<_>>()
                })
                .collect();
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
        Ok(InstallJob {
            plan,
            settings,
            paths: self.model.settings_paths.clone(),
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
            // Interrupting a running install would leave managers half
            // configured; the worker finishes, then keys work again.
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
            Stage::Choose(stage) => {
                let last = stage.rows.len().saturating_sub(1);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        stage.cursor = stage.cursor.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        stage.cursor = (stage.cursor + 1).min(last)
                    }
                    KeyCode::PageUp => stage.cursor = stage.cursor.saturating_sub(10),
                    KeyCode::PageDown => stage.cursor = (stage.cursor + 10).min(last),
                    KeyCode::Home => stage.cursor = 0,
                    KeyCode::End => stage.cursor = last,
                    KeyCode::Tab => stage.cursor = next_header(&stage.rows, stage.cursor),
                    KeyCode::BackTab => stage.cursor = previous_header(&stage.rows, stage.cursor),
                    KeyCode::Char(' ') => {
                        let row = stage.rows[stage.cursor].clone();
                        let cursor = stage.cursor;
                        self.activate_row(&row);
                        // Space picks and steps down, so a run of picks is a
                        // run of spaces.
                        if let Stage::Choose(stage) = &mut self.stages[CHOOSE] {
                            if matches!(row, Row::Resource(_) | Row::Setting(_)) {
                                stage.cursor = (cursor + 1).min(last);
                            }
                        }
                    }
                    _ => {}
                }
            }
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
                KeyCode::Down | KeyCode::Char('j') => stage.scroll = stage.scroll.saturating_add(1),
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
                return WizardOutcome::Installed(report.clone());
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
            Stage::Install(stage) => stage
                .report
                .as_ref()
                .map(|report| Action::Exit(WizardOutcome::Installed(report.clone()))),
            // Enter on a bundle means "start with this": apply, then go on.
            Stage::Choose(stage) => {
                if let Row::Preset(preset) = stage.rows[stage.cursor] {
                    self.apply_preset(preset);
                }
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
            Row::Header { items, .. } => self.toggle_group(items),
            Row::Preset(preset) => self.apply_preset(*preset),
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

    // ---- search ------------------------------------------------------------

    /// Choose rows (indices) whose resource or setting matches the live
    /// query, in list order. Empty query matches everything.
    pub(crate) fn search_matches(&self) -> Vec<usize> {
        let (Some(query), Stage::Choose(stage)) = (&self.search, &self.stages[CHOOSE]) else {
            return Vec::new();
        };
        stage
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| match row {
                Row::Resource(index) => {
                    let resource = &self.model.resources[*index];
                    fuzzy_match(&resource.label, query) || fuzzy_match(&resource.description, query)
                }
                Row::Setting(index) => {
                    let spec = &self.model.settings[*index];
                    fuzzy_match(&spec.label, query) || fuzzy_match(&spec.description, query)
                }
                _ => false,
            })
            .map(|(row_index, _)| row_index)
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
                if let Some(&row_index) = matches.get(self.search_cursor) {
                    if let Stage::Choose(stage) = &self.stages[CHOOSE] {
                        let row = stage.rows[row_index].clone();
                        self.activate_row(&row);
                    }
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
        if let (Some(hit), Stage::Choose(stage)) = (hit, &mut self.stages[CHOOSE]) {
            stage.cursor = hit;
        }
    }

    // ---- mouse -------------------------------------------------------------

    pub fn handle_click(&mut self, column: u16, row: u16) -> Option<Action> {
        if self.install_running() {
            return None;
        }
        if contains(self.hits.back_button, column, row) {
            self.go_back();
            return None;
        }
        if contains(self.hits.next_button, column, row) {
            return self.handle_enter();
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
            let matches = self.search_matches();
            if let Some(&row_index) = matches.get(index) {
                self.search_cursor = index;
                if let Stage::Choose(stage) = &self.stages[CHOOSE] {
                    let row = stage.rows[row_index].clone();
                    self.activate_row(&row);
                }
            }
            return;
        }
        match &mut self.stages[self.stage_index] {
            Stage::Choose(stage) => {
                if let Some(row) = stage.rows.get(index).cloned() {
                    stage.cursor = index;
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
        let code = if down { KeyCode::Down } else { KeyCode::Up };
        let _ = self.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
}

/// The Choose list: bundles first, then skills by category, tools, Pi
/// packages, Herdr plugins, and settings by group.
fn choose_rows(model: &Model) -> Vec<Row> {
    let mut rows = Vec::new();
    let bundles = std::iter::once(Preset::Everything)
        .chain((0..model.presets.len()).map(Preset::Catalog))
        .chain(std::iter::once(Preset::Clear))
        .collect::<Vec<_>>();
    if !model.resources.is_empty() {
        rows.push(Row::Header {
            title: "Start with a bundle".into(),
            items: Vec::new(),
        });
        rows.extend(bundles.into_iter().map(Row::Preset));
    }
    let mut push_group = |title: String, items: Vec<usize>| {
        if items.is_empty() {
            return;
        }
        rows.push(Row::Header {
            title,
            items: items.iter().map(|&index| Item::Resource(index)).collect(),
        });
        rows.extend(items.into_iter().map(Row::Resource));
    };
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
            indices(&model.resources, |resource| resource.kind == kind),
        );
    }
    let mut group: Option<&str> = None;
    for (index, spec) in model.settings.iter().enumerate() {
        if group != Some(spec.group.as_str()) {
            group = Some(spec.group.as_str());
            let items = model
                .settings
                .iter()
                .enumerate()
                .filter(|(_, other)| other.group == spec.group)
                .map(|(index, _)| Item::Setting(index))
                .collect();
            rows.push(Row::Header {
                title: format!("Settings · {}", spec.group),
                items,
            });
        }
        rows.push(Row::Setting(index));
    }
    rows
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

fn next_header(rows: &[Row], cursor: usize) -> usize {
    rows.iter()
        .enumerate()
        .skip(cursor + 1)
        .find(|(_, row)| matches!(row, Row::Header { .. }))
        .map_or(cursor, |(index, _)| index)
}

fn previous_header(rows: &[Row], cursor: usize) -> usize {
    rows.iter()
        .enumerate()
        .take(cursor)
        .rev()
        .find(|(_, row)| matches!(row, Row::Header { .. }))
        .map_or(cursor, |(index, _)| index)
}

/// Case-insensitive subsequence match: every query char appears in order.
fn fuzzy_match(haystack: &str, query: &str) -> bool {
    let haystack = haystack.to_lowercase();
    let mut chars = haystack.chars();
    query
        .to_lowercase()
        .chars()
        .all(|needle| chars.by_ref().any(|hay| hay == needle))
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}
