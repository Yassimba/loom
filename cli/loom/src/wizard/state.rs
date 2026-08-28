//! The wizard's state machine: stages, selection state, key and mouse
//! handling, and install progress. Everything here is terminal-free so the
//! whole flow is unit-testable; rendering lives in `render.rs`.

use crate::settings::{SettingSpec, SettingState, SettingsPaths};
use crate::{
    build_install_plan, InstallPlan, InstallReport, Platform, PrerequisiteStatus, Resource,
    ResourceKind, Runtime, SkillAgent, SkillDestination, SkillScope,
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

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Focus {
    Categories,
    Skills,
}

pub(crate) struct Category {
    pub name: String,
    pub items: Vec<usize>,
}

/// A row of the Welcome runtime list.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum PickRow {
    /// The manager itself, offered as its own install.
    Runtime(Runtime),
    /// The manager is already on PATH; shown, but not actionable.
    InstalledRuntime(Runtime),
    /// A one-keypress selection bundle.
    Preset(Preset),
}

/// Selection bundles offered on the Welcome screen. Applying one replaces
/// the resource selection; fine-tuning continues in the stages. The curated
/// bundles come from the catalog (manifest/presets.json) — configuration,
/// not code; Everything and Empty are algorithmic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Preset {
    /// Everything in the catalog that is not already installed.
    Everything,
    /// A curated bundle: index into the catalog's presets.
    Catalog(usize),
    /// Clear the selection and pick by hand.
    Empty,
}

pub(crate) struct PickStage {
    pub title: &'static str,
    pub blurb: &'static str,
    /// The manager this stage's resources need; opting out of installing it
    /// on the Welcome screen hides the whole stage.
    pub runtime: Runtime,
    pub items: Vec<usize>,
    pub cursor: usize,
}

pub(crate) struct SkillsStage {
    pub categories: Vec<Category>,
    pub category_cursor: usize,
    pub skill_cursor: usize,
    pub focus: Focus,
}

pub(crate) struct AgentsStage {
    /// Row zero is scope; remaining rows follow `SkillAgent::ALL`.
    pub cursor: usize,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) enum SettingRow {
    Header(String),
    Setting(usize),
}

pub(crate) struct SettingsStage {
    pub rows: Vec<SettingRow>,
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

/// The Welcome stage doubles as a quick-install surface: every runtime is
/// listed with its status, and the missing ones can be toggled right there.
pub(crate) struct WelcomeStage {
    pub rows: Vec<PickRow>,
    pub cursor: usize,
}

pub(crate) enum Stage {
    Welcome(WelcomeStage),
    Pick(PickStage),
    Agents(AgentsStage),
    Skills(SkillsStage),
    Settings(SettingsStage),
    Review { scroll: u16 },
    Install(InstallStage),
}

impl Stage {
    pub fn title(&self) -> &str {
        match self {
            Self::Welcome(_) => "Welcome",
            Self::Pick(stage) => stage.title,
            Self::Agents(_) => "Agents",
            Self::Skills(_) => "Skills",
            Self::Settings(_) => "Settings",
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
    pub sidebar: Rect,
    pub sidebar_rows: usize,
    pub back_button: Rect,
    pub next_button: Rect,
    /// (area, first-visible-row) of the primary and secondary lists.
    pub primary_list: Option<(Rect, usize)>,
    pub secondary_list: Option<(Rect, usize)>,
}

#[derive(PartialEq, Eq)]
struct SelectionSnapshot {
    selected: Vec<bool>,
    setting_on: Vec<bool>,
    agent_on: Vec<bool>,
    skill_scope: SkillScope,
}

pub struct Wizard {
    pub(crate) model: Model,
    pub(crate) selected: Vec<bool>,
    /// Toggles for runtimes that are not yet installed.
    pub(crate) runtime_on: Vec<(Runtime, bool)>,
    pub(crate) setting_on: Vec<bool>,
    pub(crate) agent_on: Vec<bool>,
    pub(crate) skill_scope: SkillScope,
    /// Settings the user explicitly toggled; contextual pre-checks leave
    /// those alone.
    pub(crate) setting_touched: Vec<bool>,
    pub(crate) stages: Vec<Stage>,
    pub(crate) stage_index: usize,
    pub(crate) max_visited: usize,
    pub(crate) hits: HitMap,
    /// `Some(query)` while `/` search filters the current stage's list.
    pub(crate) search: Option<String>,
    pub(crate) search_cursor: usize,
    pub(crate) show_help: bool,
    /// True while the installed-state probe still runs in the background.
    pub(crate) probing: bool,
    /// Quit confirmation pending (a non-empty selection would be discarded).
    pub(crate) confirm_quit: bool,
    /// The last vertical direction moved; space auto-advances the same way.
    last_dir: i8,
    /// Selection snapshots (resources, settings) for `u`; capped.
    undo_stack: Vec<SelectionSnapshot>,
}

impl Wizard {
    pub fn new(model: Model) -> Self {
        let welcome_rows = [Runtime::Mise, Runtime::Herdr, Runtime::Pi]
            .into_iter()
            .map(|runtime| {
                if runtime.installed(model.status) {
                    PickRow::InstalledRuntime(runtime)
                } else {
                    PickRow::Runtime(runtime)
                }
            })
            .chain(std::iter::once(PickRow::Preset(Preset::Everything)))
            .chain((0..model.presets.len()).map(|index| PickRow::Preset(Preset::Catalog(index))))
            .chain(std::iter::once(PickRow::Preset(Preset::Empty)))
            .collect::<Vec<_>>();
        let welcome_cursor = welcome_rows
            .iter()
            .position(|row| matches!(row, PickRow::Runtime(_)))
            .unwrap_or(0);
        let mut stages = vec![Stage::Welcome(WelcomeStage {
            rows: welcome_rows,
            cursor: welcome_cursor,
        })];
        for (kind, runtime, title, blurb) in [
            (
                ResourceKind::Tool,
                Runtime::Mise,
                "Tools",
                "CLI and TUI tools from the pinned manifest, installed with mise.",
            ),
            (
                ResourceKind::HerdrPlugin,
                Runtime::Herdr,
                "Herdr",
                "Plugins for Herdr, the terminal multiplexer for coding agents.",
            ),
            (
                ResourceKind::PiPackage,
                Runtime::Pi,
                "Pi",
                "Packages that extend the Pi coding agent.",
            ),
        ] {
            let items = indices_of_kind(&model.resources, &kind);
            if items.is_empty() {
                continue;
            }
            stages.push(Stage::Pick(PickStage {
                title,
                blurb,
                runtime,
                items,
                cursor: 0,
            }));
        }
        let categories = skill_categories(&model.resources);
        if !categories.is_empty() {
            stages.push(Stage::Skills(SkillsStage {
                categories,
                category_cursor: 0,
                skill_cursor: 0,
                focus: Focus::Categories,
            }));
            stages.push(Stage::Agents(AgentsStage { cursor: 0 }));
        }
        if !model.settings.is_empty() {
            stages.push(Stage::Settings(SettingsStage {
                rows: setting_rows(&model.settings),
                cursor: first_setting_row(&setting_rows(&model.settings)),
            }));
        }
        stages.push(Stage::Review { scroll: 0 });
        stages.push(Stage::Install(InstallStage {
            items: Vec::new(),
            running: false,
            report: None,
            tick: 0,
            scroll: 0,
        }));

        let runtime_on = [Runtime::Mise, Runtime::Herdr, Runtime::Pi]
            .into_iter()
            .filter(|runtime| !runtime.installed(model.status))
            .map(|runtime| (runtime, true))
            .collect();
        let agent_on = SkillAgent::ALL
            .iter()
            .map(|agent| model.skill_destination.agents.contains(agent))
            .collect();
        let skill_scope = model.skill_destination.scope;
        Self {
            selected: vec![false; model.resources.len()],
            setting_on: vec![false; model.settings.len()],
            agent_on,
            skill_scope,
            setting_touched: vec![false; model.settings.len()],
            runtime_on,
            stages,
            stage_index: 0,
            max_visited: 0,
            model,
            hits: HitMap::default(),
            search: None,
            search_cursor: 0,
            show_help: false,
            probing: false,
            confirm_quit: false,
            last_dir: 1,
            undo_stack: Vec::new(),
        }
    }

    // ---- selection helpers -------------------------------------------------

    pub(crate) fn runtime_selected(&self, runtime: Runtime) -> bool {
        self.runtime_on
            .iter()
            .any(|(candidate, on)| *candidate == runtime && *on)
    }

    fn toggle_runtime(&mut self, runtime: Runtime) {
        if let Some(entry) = self
            .runtime_on
            .iter_mut()
            .find(|(candidate, _)| *candidate == runtime)
        {
            entry.1 = !entry.1;
        }
    }

    /// The manager a resource kind installs through; skills need none — the
    /// CLI copies them into the agent trees itself.
    fn runtime_for_kind(kind: &ResourceKind) -> Option<Runtime> {
        match kind {
            ResourceKind::HerdrPlugin => Some(Runtime::Herdr),
            ResourceKind::PiPackage => Some(Runtime::Pi),
            ResourceKind::Tool => Some(Runtime::Mise),
            ResourceKind::Skill => None,
        }
    }

    /// An ecosystem is enabled when its manager is installed or the user
    /// left its Welcome-screen install toggle on.
    pub(crate) fn ecosystem_enabled(&self, runtime: Runtime) -> bool {
        runtime.installed(self.model.status) || self.runtime_selected(runtime)
    }

    /// Picks in a disabled ecosystem are kept (they come back if the user
    /// re-enables the runtime) but excluded from everything downstream.
    pub(crate) fn selection(&self) -> Vec<Resource> {
        self.model
            .resources
            .iter()
            .zip(&self.selected)
            .filter(|(resource, on)| {
                **on && Self::runtime_for_kind(&resource.kind)
                    .is_none_or(|runtime| self.ecosystem_enabled(runtime))
            })
            .map(|(resource, _)| resource.clone())
            .collect()
    }

    pub(crate) fn selected_runtimes(&self) -> Vec<Runtime> {
        self.runtime_on
            .iter()
            .filter(|(_, on)| *on)
            .map(|(runtime, _)| runtime)
            .copied()
            .collect()
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
        self.selection().is_empty()
            && self.selected_runtimes().is_empty()
            && self.selected_settings().is_empty()
    }

    pub(crate) fn plan(&self) -> Result<InstallPlan> {
        let selection = crate::expand_skill_dependencies(&self.model.resources, self.selection());
        build_install_plan(
            &selection,
            &self.selected_runtimes(),
            self.model.status,
            self.model.platform,
            &self.skill_destination(),
        )
    }

    pub(crate) fn selected_count(&self, items: &[usize]) -> usize {
        items.iter().filter(|&&index| self.selected[index]).count()
    }

    pub(crate) fn installed_count(&self, items: &[usize]) -> usize {
        items
            .iter()
            .filter(|&&index| self.resource_installed(index))
            .count()
    }

    pub(crate) fn actionable(&self, items: &[usize]) -> Vec<usize> {
        items
            .iter()
            .copied()
            .filter(|&index| !self.resource_installed(index))
            .collect()
    }

    pub(crate) fn total_selected(&self) -> usize {
        self.selection().len()
            + self.selected_runtimes().len()
            + self.setting_on.iter().filter(|on| **on).count()
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

    // ---- navigation --------------------------------------------------------

    pub(crate) fn install_running(&self) -> bool {
        matches!(
            &self.stages[self.stage_index],
            Stage::Install(stage) if stage.running
        )
    }

    fn review_index(&self) -> usize {
        self.stages.len() - 2
    }

    /// A stage is hidden when the user opted out of installing the runtime
    /// its resources need.
    pub(crate) fn stage_visible(&self, index: usize) -> bool {
        match &self.stages[index] {
            Stage::Pick(stage) => self.ecosystem_enabled(stage.runtime),
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
        let next =
            (self.stage_index + 1..=self.review_index()).find(|&index| self.stage_visible(index));
        if let Some(index) = next {
            self.stage_index = index;
            self.entered_stage();
        }
    }

    fn go_back(&mut self) {
        self.search = None;
        if matches!(self.stages[self.stage_index], Stage::Install(_)) {
            return;
        }
        if let Some(index) = (0..self.stage_index)
            .rev()
            .find(|&index| self.stage_visible(index))
        {
            self.stage_index = index;
        }
    }

    /// Jump to the `row`th *visible* stage, as shown in the sidebar.
    fn jump_to(&mut self, row: usize) {
        let Some(&index) = self.visible_stages().get(row) else {
            return;
        };
        let install_stage = self.stages.len() - 1;
        if index >= install_stage || index > self.max_visited {
            return;
        }
        self.stage_index = index;
        self.entered_stage();
    }

    fn entered_stage(&mut self) {
        self.search = None;
        self.max_visited = self.max_visited.max(self.stage_index);
        if let Stage::Settings(_) = &self.stages[self.stage_index] {
            self.precheck_settings();
        }
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
        self.stage_index = self.stages.len() - 1;
        self.max_visited = self.stage_index;
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
            if let Stage::Install(stage) = &self.stages[self.stage_index] {
                if let Some(report) = &stage.report {
                    return Some(Action::Exit(WizardOutcome::Installed(report.clone())));
                }
            }
            return Some(Action::Exit(WizardOutcome::Cancelled));
        }
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
            if let Stage::Install(stage) = &self.stages[self.stage_index] {
                if let Some(report) = &stage.report {
                    return Some(Action::Exit(WizardOutcome::Installed(report.clone())));
                }
            }
            // Esc steps back first; on the first stage it means leaving.
            if key.code == KeyCode::Esc && self.stage_index > 0 {
                self.go_back();
                return None;
            }
            // Quitting with picks pending asks; an empty hand just leaves.
            if self.total_selected() > 0 {
                self.confirm_quit = true;
                return None;
            }
            return Some(Action::Exit(WizardOutcome::Cancelled));
        }
        // Remember the travel direction so space advances the way you move.
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('G') => self.last_dir = -1,
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('g') => self.last_dir = 1,
            _ => {}
        }
        if key.code == KeyCode::Enter {
            return self.handle_enter();
        }
        match key.code {
            KeyCode::Char('?') => {
                self.show_help = true;
                return None;
            }
            KeyCode::Char('u') => {
                self.undo();
                return None;
            }
            KeyCode::Char('/')
                if matches!(
                    self.stages[self.stage_index],
                    Stage::Pick(_) | Stage::Skills(_)
                ) =>
            {
                self.search = Some(String::new());
                self.search_cursor = 0;
                return None;
            }
            _ => {}
        }

        // Snapshot before selection-mutating keys so `u` can restore; the
        // stack dedupes, so a no-op toggle costs nothing.
        if matches!(
            key.code,
            KeyCode::Char(' ') | KeyCode::Char('a') | KeyCode::Char('A')
        ) {
            self.push_undo();
        }
        let blocked = self.blocked_flags();
        // -1 steps back, +1 steps forward; resolved after the borrow of the
        // current stage ends.
        let mut navigate = 0i8;
        match &mut self.stages[self.stage_index] {
            Stage::Welcome(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => stage.cursor = stage.cursor.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    stage.cursor = (stage.cursor + 1).min(stage.rows.len() - 1);
                }
                KeyCode::Char('g') => stage.cursor = 0,
                KeyCode::Char('G') => stage.cursor = stage.rows.len() - 1,
                KeyCode::Char(' ') => {
                    let row = stage.rows[stage.cursor];
                    self.toggle_pick_row(row);
                }
                KeyCode::Char('a') => {
                    let rows = stage.rows.clone();
                    self.toggle_pick_all(&rows);
                }
                KeyCode::Right | KeyCode::Char('l') => navigate = 1,
                _ => {}
            },
            Stage::Pick(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => stage.cursor = stage.cursor.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    stage.cursor = (stage.cursor + 1).min(stage.items.len() - 1);
                }
                KeyCode::Char('g') => stage.cursor = 0,
                KeyCode::Char('G') => stage.cursor = stage.items.len() - 1,
                KeyCode::Char(' ') => {
                    let index = stage.items[stage.cursor];
                    if !blocked[index] {
                        self.selected[index] = !self.selected[index];
                    }
                    // Auto-advance the way the cursor was travelling.
                    stage.cursor = if self.last_dir >= 0 {
                        (stage.cursor + 1).min(stage.items.len() - 1)
                    } else {
                        stage.cursor.saturating_sub(1)
                    };
                }
                KeyCode::Char('a') => {
                    toggle_group(&mut self.selected, &stage.items, &blocked);
                }
                KeyCode::Right | KeyCode::Char('l') => navigate = 1,
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Char('b') => {
                    navigate = -1;
                }
                _ => {}
            },
            Stage::Agents(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => stage.cursor = stage.cursor.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    stage.cursor = (stage.cursor + 1).min(SkillAgent::ALL.len());
                }
                KeyCode::Char('g') => stage.cursor = 0,
                KeyCode::Char('G') => stage.cursor = SkillAgent::ALL.len(),
                KeyCode::Char(' ') if stage.cursor == 0 => {
                    self.skill_scope = match self.skill_scope {
                        SkillScope::Global => SkillScope::Project,
                        SkillScope::Project => SkillScope::Global,
                    };
                }
                KeyCode::Char(' ') => {
                    let index = stage.cursor - 1;
                    self.agent_on[index] = !self.agent_on[index];
                    stage.cursor = if self.last_dir >= 0 {
                        (stage.cursor + 1).min(SkillAgent::ALL.len())
                    } else {
                        stage.cursor.saturating_sub(1)
                    };
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    let all_on = self.agent_on.iter().all(|on| *on);
                    self.agent_on.fill(!all_on);
                }
                KeyCode::Right | KeyCode::Char('l') => navigate = 1,
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Char('b') => {
                    navigate = -1;
                }
                _ => {}
            },
            Stage::Skills(stage) => {
                // The yazi ladder: h/l climb one continuous rail —
                // [previous stage] ← categories ⇄ skills → [next stage] —
                // and j/k in the skills pane flow across category borders.
                let category_items = stage.categories[stage.category_cursor].items.clone();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => match stage.focus {
                        Focus::Categories => {
                            stage.category_cursor = stage.category_cursor.saturating_sub(1);
                            stage.skill_cursor = 0;
                        }
                        Focus::Skills => {
                            if stage.skill_cursor > 0 {
                                stage.skill_cursor -= 1;
                            } else if stage.category_cursor > 0 {
                                stage.category_cursor -= 1;
                                stage.skill_cursor =
                                    stage.categories[stage.category_cursor].items.len() - 1;
                            }
                        }
                    },
                    KeyCode::Down | KeyCode::Char('j') => match stage.focus {
                        Focus::Categories => {
                            stage.category_cursor =
                                (stage.category_cursor + 1).min(stage.categories.len() - 1);
                            stage.skill_cursor = 0;
                        }
                        Focus::Skills => {
                            if stage.skill_cursor + 1 < category_items.len() {
                                stage.skill_cursor += 1;
                            } else if stage.category_cursor + 1 < stage.categories.len() {
                                stage.category_cursor += 1;
                                stage.skill_cursor = 0;
                            }
                        }
                    },
                    KeyCode::Char('g') => match stage.focus {
                        Focus::Categories => {
                            stage.category_cursor = 0;
                            stage.skill_cursor = 0;
                        }
                        Focus::Skills => stage.skill_cursor = 0,
                    },
                    KeyCode::Char('G') => match stage.focus {
                        Focus::Categories => {
                            stage.category_cursor = stage.categories.len() - 1;
                            stage.skill_cursor = 0;
                        }
                        Focus::Skills => stage.skill_cursor = category_items.len() - 1,
                    },
                    KeyCode::Char(' ') => match stage.focus {
                        Focus::Categories => {
                            toggle_group(&mut self.selected, &category_items, &blocked)
                        }
                        Focus::Skills => {
                            let index = category_items[stage.skill_cursor];
                            if !blocked[index] {
                                self.selected[index] = !self.selected[index];
                            }
                            // Auto-advance with the cross-category flow, in
                            // the direction the cursor was travelling.
                            if self.last_dir >= 0 {
                                if stage.skill_cursor + 1 < category_items.len() {
                                    stage.skill_cursor += 1;
                                } else if stage.category_cursor + 1 < stage.categories.len() {
                                    stage.category_cursor += 1;
                                    stage.skill_cursor = 0;
                                }
                            } else if stage.skill_cursor > 0 {
                                stage.skill_cursor -= 1;
                            } else if stage.category_cursor > 0 {
                                stage.category_cursor -= 1;
                                stage.skill_cursor =
                                    stage.categories[stage.category_cursor].items.len() - 1;
                            }
                        }
                    },
                    KeyCode::Char('a') => {
                        toggle_group(&mut self.selected, &category_items, &blocked)
                    }
                    KeyCode::Char('A') => {
                        let all = stage
                            .categories
                            .iter()
                            .flat_map(|category| category.items.iter().copied())
                            .collect::<Vec<_>>();
                        toggle_group(&mut self.selected, &all, &blocked);
                    }
                    KeyCode::Tab => {
                        stage.focus = match stage.focus {
                            Focus::Categories => Focus::Skills,
                            Focus::Skills => Focus::Categories,
                        };
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if stage.focus == Focus::Categories {
                            stage.focus = Focus::Skills;
                        } else {
                            navigate = 1;
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if stage.focus == Focus::Skills {
                            stage.focus = Focus::Categories;
                        } else {
                            navigate = -1;
                        }
                    }
                    KeyCode::Backspace | KeyCode::Char('b') => navigate = -1,
                    _ => {}
                }
            }
            Stage::Settings(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    stage.cursor = previous_setting_row(&stage.rows, stage.cursor);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    stage.cursor = next_setting_row(&stage.rows, stage.cursor);
                }
                KeyCode::Char('g') => stage.cursor = first_setting_row(&stage.rows),
                KeyCode::Char('G') => {
                    if let Some(last) = stage
                        .rows
                        .iter()
                        .rposition(|row| matches!(row, SettingRow::Setting(_)))
                    {
                        stage.cursor = last;
                    }
                }
                KeyCode::Char(' ') => {
                    if let SettingRow::Setting(index) = stage.rows[stage.cursor] {
                        if self.model.setting_states[index] != SettingState::Applied {
                            self.setting_on[index] = !self.setting_on[index];
                            self.setting_touched[index] = true;
                        }
                    }
                    stage.cursor = if self.last_dir >= 0 {
                        next_setting_row(&stage.rows, stage.cursor)
                    } else {
                        previous_setting_row(&stage.rows, stage.cursor)
                    };
                }
                KeyCode::Char('a') => {
                    let actionable = (0..self.model.settings.len())
                        .filter(|&index| !self.setting_applied(index))
                        .collect::<Vec<_>>();
                    let all_on = actionable.iter().all(|&index| self.setting_on[index]);
                    for index in actionable {
                        self.setting_on[index] = !all_on;
                        self.setting_touched[index] = true;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => navigate = 1,
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Char('b') => {
                    navigate = -1;
                }
                _ => {}
            },
            Stage::Review { scroll } => match key.code {
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Char('b') => {
                    navigate = -1;
                }
                _ => {}
            },
            Stage::Install(stage) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => stage.scroll = stage.scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => stage.scroll = stage.scroll.saturating_add(1),
                _ => {}
            },
        }
        match navigate {
            -1 => self.go_back(),
            1 => self.go_forward(),
            _ => {}
        }
        None
    }

    fn handle_enter(&mut self) -> Option<Action> {
        match &self.stages[self.stage_index] {
            Stage::Review { .. } => self.confirm_review(),
            Stage::Install(stage) => stage
                .report
                .as_ref()
                .map(|report| Action::Exit(WizardOutcome::Installed(report.clone()))),
            // A preset row is an action, not a checkbox: Enter on it means
            // "start with this bundle" — apply, then advance.
            Stage::Welcome(stage) => {
                if let PickRow::Preset(preset) = stage.rows[stage.cursor] {
                    self.apply_preset(preset);
                }
                self.go_forward();
                None
            }
            _ => {
                self.go_forward();
                None
            }
        }
    }

    fn toggle_pick_row(&mut self, row: PickRow) {
        match row {
            PickRow::Runtime(runtime) => self.toggle_runtime(runtime),
            PickRow::InstalledRuntime(_) => {}
            PickRow::Preset(preset) => self.apply_preset(preset),
        }
    }

    /// Replace the resource selection with a bundle. Undoable with `u`.
    fn apply_preset(&mut self, preset: Preset) {
        self.push_undo();
        self.selected.fill(false);
        match preset {
            Preset::Empty => {}
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
    }

    pub(crate) fn preset_label(&self, preset: Preset) -> &str {
        match preset {
            Preset::Everything => "Everything",
            Preset::Empty => "Start empty",
            Preset::Catalog(index) => &self.model.presets[index].label,
        }
    }

    pub(crate) fn preset_blurb(&self, preset: Preset) -> &str {
        match preset {
            Preset::Everything => "select the whole catalog, deselect what you don't want",
            Preset::Empty => "clear the selection and pick by hand",
            Preset::Catalog(index) => &self.model.presets[index].description,
        }
    }

    fn push_undo(&mut self) {
        let snapshot = SelectionSnapshot {
            selected: self.selected.clone(),
            setting_on: self.setting_on.clone(),
            agent_on: self.agent_on.clone(),
            skill_scope: self.skill_scope,
        };
        if self.undo_stack.last() == Some(&snapshot) {
            return;
        }
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    pub(crate) fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.selected = snapshot.selected;
            self.setting_on = snapshot.setting_on;
            self.agent_on = snapshot.agent_on;
            self.skill_scope = snapshot.skill_scope;
        }
    }

    fn toggle_pick_all(&mut self, rows: &[PickRow]) {
        let all_on = rows.iter().all(|row| match row {
            PickRow::Runtime(runtime) => self.runtime_selected(*runtime),
            PickRow::InstalledRuntime(_) | PickRow::Preset(_) => true,
        });
        for row in rows {
            if let PickRow::Runtime(runtime) = row {
                if let Some(entry) = self
                    .runtime_on
                    .iter_mut()
                    .find(|(candidate, _)| candidate == runtime)
                {
                    entry.1 = !all_on;
                }
            }
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

    /// Per-resource toggle blocks: installed, or required by a selection.
    fn blocked_flags(&self) -> Vec<bool> {
        (0..self.model.resources.len())
            .map(|index| self.resource_installed(index) || self.required_note(index).is_some())
            .collect()
    }

    // ---- search ------------------------------------------------------------

    /// The current stage's resource indices that match the live query, in
    /// list order. Empty query matches everything.
    pub(crate) fn search_matches(&self) -> Vec<usize> {
        let Some(query) = &self.search else {
            return Vec::new();
        };
        let candidates: Vec<usize> = match &self.stages[self.stage_index] {
            Stage::Pick(stage) => stage.items.clone(),
            Stage::Skills(stage) => stage
                .categories
                .iter()
                .flat_map(|category| category.items.iter().copied())
                .collect(),
            _ => Vec::new(),
        };
        candidates
            .into_iter()
            .filter(|&index| {
                let resource = &self.model.resources[index];
                fuzzy_match(&resource.label, query) || fuzzy_match(&resource.description, query)
            })
            .collect()
    }

    fn handle_search_key(&mut self, code: KeyCode) -> Option<Action> {
        match code {
            KeyCode::Esc => {
                self.search = None;
            }
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
            KeyCode::Up => {
                self.last_dir = -1;
                self.search_cursor = self.search_cursor.saturating_sub(1);
            }
            KeyCode::Down => {
                self.last_dir = 1;
                let len = self.search_matches().len();
                if len > 0 {
                    self.search_cursor = (self.search_cursor + 1).min(len - 1);
                }
            }
            KeyCode::Char(' ') => {
                let matches = self.search_matches();
                if let Some(&index) = matches.get(self.search_cursor) {
                    if !self.resource_installed(index) && self.required_note(index).is_none() {
                        self.push_undo();
                        self.selected[index] = !self.selected[index];
                    }
                    // Auto-advance the way the cursor was travelling.
                    self.search_cursor = if self.last_dir >= 0 {
                        (self.search_cursor + 1).min(matches.len() - 1)
                    } else {
                        self.search_cursor.saturating_sub(1)
                    };
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
        let matches = self.search_matches();
        let hit = matches.get(self.search_cursor).copied();
        self.search = None;
        let Some(hit) = hit else { return };
        match &mut self.stages[self.stage_index] {
            Stage::Pick(stage) => {
                if let Some(position) = stage.items.iter().position(|&item| item == hit) {
                    stage.cursor = position;
                }
            }
            Stage::Skills(stage) => {
                for (category_index, category) in stage.categories.iter().enumerate() {
                    if let Some(position) = category.items.iter().position(|&item| item == hit) {
                        stage.category_cursor = category_index;
                        stage.skill_cursor = position;
                        stage.focus = Focus::Skills;
                        break;
                    }
                }
            }
            _ => {}
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
        if contains(self.hits.sidebar, column, row) {
            let index = row.saturating_sub(self.hits.sidebar.y + 1) as usize;
            if index < self.hits.sidebar_rows {
                self.jump_to(index);
            }
            return None;
        }
        if let Some((area, offset)) = self.hits.primary_list {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                self.click_primary(index);
                return None;
            }
        }
        if let Some((area, offset)) = self.hits.secondary_list {
            if contains(area, column, row) {
                let index = offset + row.saturating_sub(area.y + 1) as usize;
                self.click_secondary(index);
            }
        }
        None
    }

    fn click_primary(&mut self, index: usize) {
        if self.search.is_some() {
            let matches = self.search_matches();
            if let Some(&hit) = matches.get(index) {
                self.search_cursor = index;
                if !self.resource_installed(hit) && self.required_note(hit).is_none() {
                    self.push_undo();
                    self.selected[hit] = !self.selected[hit];
                }
            }
            return;
        }
        match &mut self.stages[self.stage_index] {
            Stage::Welcome(stage) => {
                if index < stage.rows.len() {
                    stage.cursor = index;
                    let row = stage.rows[index];
                    self.toggle_pick_row(row);
                }
            }
            Stage::Pick(stage) => {
                if index < stage.items.len() {
                    stage.cursor = index;
                    let item = stage.items[index];
                    if !self.resource_installed(item) && self.required_note(item).is_none() {
                        self.selected[item] = !self.selected[item];
                    }
                }
            }
            Stage::Agents(stage) => {
                if index <= SkillAgent::ALL.len() {
                    stage.cursor = index;
                    if index == 0 {
                        self.skill_scope = match self.skill_scope {
                            SkillScope::Global => SkillScope::Project,
                            SkillScope::Project => SkillScope::Global,
                        };
                    } else {
                        self.agent_on[index - 1] = !self.agent_on[index - 1];
                    }
                }
            }
            Stage::Skills(stage) => {
                if index < stage.categories.len() {
                    stage.focus = Focus::Categories;
                    stage.category_cursor = index;
                    stage.skill_cursor = 0;
                }
            }
            Stage::Settings(stage) => {
                if let Some(SettingRow::Setting(setting)) = stage.rows.get(index).cloned() {
                    stage.cursor = index;
                    if self.model.setting_states[setting] != SettingState::Applied {
                        self.setting_on[setting] = !self.setting_on[setting];
                        self.setting_touched[setting] = true;
                    }
                }
            }
            _ => {}
        }
    }

    fn click_secondary(&mut self, index: usize) {
        if let Stage::Skills(stage) = &mut self.stages[self.stage_index] {
            let category = &stage.categories[stage.category_cursor];
            if index < category.items.len() {
                stage.focus = Focus::Skills;
                stage.skill_cursor = index;
                let item = category.items[index];
                if !self.resource_installed(item) && self.required_note(item).is_none() {
                    self.selected[item] = !self.selected[item];
                }
            }
        }
    }

    pub fn handle_scroll(&mut self, down: bool) {
        let code = if down { KeyCode::Down } else { KeyCode::Up };
        let _ = self.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
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

fn indices_of_kind(resources: &[Resource], kind: &ResourceKind) -> Vec<usize> {
    resources
        .iter()
        .enumerate()
        .filter(|(_, resource)| resource.kind == *kind)
        .map(|(index, _)| index)
        .collect()
}

fn skill_categories(resources: &[Resource]) -> Vec<Category> {
    let mut categories: Vec<Category> = Vec::new();
    for (index, resource) in resources.iter().enumerate() {
        if resource.kind != ResourceKind::Skill {
            continue;
        }
        match categories
            .iter_mut()
            .find(|category| category.name == resource.group)
        {
            Some(category) => category.items.push(index),
            None => categories.push(Category {
                name: resource.group.clone(),
                items: vec![index],
            }),
        }
    }
    categories
}

fn setting_rows(settings: &[SettingSpec]) -> Vec<SettingRow> {
    let mut rows = Vec::new();
    let mut group: Option<&str> = None;
    for (index, spec) in settings.iter().enumerate() {
        if group != Some(spec.group.as_str()) {
            rows.push(SettingRow::Header(spec.group.clone()));
            group = Some(spec.group.as_str());
        }
        rows.push(SettingRow::Setting(index));
    }
    rows
}

fn first_setting_row(rows: &[SettingRow]) -> usize {
    rows.iter()
        .position(|row| matches!(row, SettingRow::Setting(_)))
        .unwrap_or(0)
}

fn next_setting_row(rows: &[SettingRow], cursor: usize) -> usize {
    rows.iter()
        .enumerate()
        .skip(cursor + 1)
        .find(|(_, row)| matches!(row, SettingRow::Setting(_)))
        .map_or(cursor, |(index, _)| index)
}

fn previous_setting_row(rows: &[SettingRow], cursor: usize) -> usize {
    rows.iter()
        .enumerate()
        .take(cursor)
        .rev()
        .find(|(_, row)| matches!(row, SettingRow::Setting(_)))
        .map_or(cursor, |(index, _)| index)
}

/// Select-all over a group, skipping resources that are already installed.
fn toggle_group(selected: &mut [bool], items: &[usize], installed: &[bool]) {
    let actionable = items
        .iter()
        .copied()
        .filter(|&index| !installed[index])
        .collect::<Vec<_>>();
    let all_selected = actionable.iter().all(|&index| selected[index]);
    for index in actionable {
        selected[index] = !all_selected;
    }
}
