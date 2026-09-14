//! Vault-scoped picks inside the setup chooser. Selecting a folder never writes to it.
use super::render::{bordered, ListHit, ACCENT, ERR, TITLE};
use super::state::{clamp_step, movement, Action, Group, Item, Pane, Screen, Wizard};
use crate::wiki::{VaultHealth, VaultRecord, WikiOperation, WikiRegistry};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Capability {
    Essentials,
    Feynman,
    Confluence,
    Qmd,
    Skill(&'static str),
}

pub(super) fn wiki_capabilities() -> &'static [Capability] {
    static LIST: OnceLock<Vec<Capability>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut list = vec![
            Capability::Essentials,
            Capability::Feynman,
            Capability::Confluence,
            Capability::Qmd,
        ];
        list.extend(
            serde_json::from_str::<Vec<&'static str>>(include_str!("../../wiki-skills.json"))
                .expect("embedded wiki-skills.json is invalid")
                .into_iter()
                .map(Capability::Skill),
        );
        list
    })
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Essentials => "Wiki essentials · claude-obsidian",
            Self::Feynman => "Feynman · research companion",
            Self::Confluence => "Confluence · export notes",
            Self::Qmd => "QMD · local search",
            Self::Skill(name) => name,
        }
    }
}

#[derive(Default)]
pub(super) struct WikiPicks {
    pub operation: Option<WikiOperation>,
    pub selected: BTreeSet<Capability>,
}

#[derive(Default)]
pub(super) struct WikiBrowser {
    pub vaults: Vec<VaultRecord>,
    pub cursor: usize,
    pub item_cursor: usize,
    pub search: Option<String>,
    pub health: BTreeMap<PathBuf, VaultHealth>,
    pub checking: bool,
    pub message: Option<String>,
    pub pending: BTreeMap<PathBuf, WikiPicks>,
    pub confirm_unregister: Option<PathBuf>,
    registry_error: Option<String>,
}

impl WikiBrowser {
    pub fn load(&mut self, home: &Path) {
        match WikiRegistry::load(home) {
            Ok(registry) => {
                let selected = self.record().map(|record| record.path.clone());
                let cursor = registry
                    .vaults
                    .iter()
                    .position(|record| !self.vaults.iter().any(|old| old.path == record.path))
                    .or_else(|| {
                        registry
                            .vaults
                            .iter()
                            .position(|record| Some(&record.path) == selected.as_ref())
                    });
                self.vaults = registry.vaults;
                self.registry_error = None;
                self.cursor = cursor
                    .unwrap_or(self.cursor)
                    .min(self.len().saturating_sub(1));
                self.pending.clear();
            }
            Err(error) => {
                self.registry_error = Some(error.to_string());
                self.vaults.clear();
                self.pending.clear();
                self.cursor = 0;
            }
        }
        self.health.clear();
    }

    /// The picker has already validated the path. Keep a session-only row until Install.
    pub fn picked_path(&mut self, operation: WikiOperation, path: PathBuf) {
        if self.registry_error.is_some() {
            return;
        }
        if let Some(index) = self.vaults.iter().position(|record| record.path == path) {
            self.cursor = index;
        } else {
            self.cursor = self.vaults.len();
            self.vaults.push(VaultRecord {
                path: path.clone(),
                feynman: false,
                confluence: false,
                qmd: false,
            });
            self.pending.insert(
                path,
                WikiPicks {
                    operation: Some(operation),
                    selected: BTreeSet::from([Capability::Essentials]),
                },
            );
        }
        self.item_cursor = 0;
        self.search = None;
        self.message =
            Some("Folder chosen. Pick capabilities → Next to review. Nothing changed yet.".into());
    }

    pub fn len(&self) -> usize {
        self.vaults.len() + if cfg!(windows) { 0 } else { 2 }
    }

    pub fn record(&self) -> Option<&VaultRecord> {
        self.vaults.get(self.cursor)
    }

    pub fn entry(&self) -> Option<WikiOperation> {
        if self.registry_error.is_some() || cfg!(windows) {
            return None;
        }
        match self.cursor.checked_sub(self.vaults.len())? {
            0 => Some(WikiOperation::Adopt),
            1 => Some(WikiOperation::Create),
            _ => None,
        }
    }

    pub fn next_probe(&mut self) -> Option<VaultRecord> {
        if self.checking || self.registry_error.is_some() {
            return None;
        }
        let record = self.record()?;
        if self.health.contains_key(&record.path)
            || self
                .pending
                .get(&record.path)
                .is_some_and(|p| p.operation == Some(WikiOperation::Create))
        {
            return None;
        }
        let record = record.clone();
        self.checking = true;
        Some(record)
    }

    fn health_ready(&self, record: &VaultRecord, label: &str) -> bool {
        self.health.get(&record.path).is_some_and(|health| {
            health
                .rows
                .iter()
                .any(|(mark, name, _)| *name == label && matches!(mark, crate::ui::Mark::Ok))
        })
    }

    pub fn installed(&self, record: &VaultRecord, capability: Capability) -> bool {
        match capability {
            Capability::Essentials => {
                !self
                    .pending
                    .get(&record.path)
                    .is_some_and(|p| p.operation.is_some())
                    && self.health_ready(record, "claude-obsidian")
            }
            Capability::Feynman => self.health_ready(record, "Feynman"),
            Capability::Confluence => self.health_ready(record, "Confluence"),
            Capability::Qmd => self.health_ready(record, "qmd"),
            Capability::Skill(name) => {
                crate::skills::skill_present_in(&record.path.join(".agents/skills"), name)
            }
        }
    }

    pub fn selected(&self, record: &VaultRecord) -> BTreeSet<Capability> {
        let mut selected = self
            .pending
            .get(&record.path)
            .map(|p| p.selected.clone())
            .unwrap_or_default();
        selected.retain(|capability| !self.installed(record, *capability));
        if !selected.is_empty() && !self.installed(record, Capability::Essentials) {
            selected.insert(Capability::Essentials);
        }
        selected
    }

    pub fn count(&self) -> usize {
        self.vaults
            .iter()
            .map(|record| {
                self.pending.get(&record.path).map_or(0, |picks| {
                    picks
                        .selected
                        .iter()
                        .filter(|capability| !self.installed(record, **capability))
                        .count()
                })
            })
            .sum()
    }

    pub fn capabilities(&self) -> Vec<Capability> {
        let query = self.search.as_ref().map(|query| query.to_lowercase());
        wiki_capabilities()
            .iter()
            .copied()
            .filter(|capability| {
                query
                    .as_ref()
                    .is_none_or(|query| capability.label().to_lowercase().contains(query))
            })
            .collect()
    }

    pub fn toggle(&mut self) {
        let Some(record) = self.record() else {
            return;
        };
        if cfg!(windows)
            || (self
                .pending
                .get(&record.path)
                .is_none_or(|p| p.operation.is_none())
                && !record.path.is_dir())
        {
            self.message = Some("Vault unavailable. Missing Vaults are never recreated; Windows setup requires WSL2.".into());
            return;
        }
        let Some(capability) = self.capabilities().get(self.item_cursor).copied() else {
            return;
        };
        let path = record.path.clone();
        if self
            .pending
            .get_mut(&path)
            .is_some_and(|picks| picks.selected.remove(&capability))
        {
            self.message = None;
            return;
        }
        let record = self.record().unwrap();
        if self.installed(record, capability) {
            self.message = Some(
                "Already installed in this Wiki. This chooser never uninstalls anything.".into(),
            );
            return;
        }
        self.pending
            .entry(path)
            .or_default()
            .selected
            .insert(capability);
        self.message = None;
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        list: Rect,
        details: Rect,
        focus: Pane,
        home: &Path,
    ) -> (ListHit, ListHit) {
        let mut state = ListState::default().with_selected((self.len() > 0).then_some(self.cursor));
        if list.width > 0 {
            let mut items = self
                .vaults
                .iter()
                .map(|record| {
                    let pending = !self.selected(record).is_empty();
                    let (mark, color) = if pending {
                        ("[x]", ACCENT)
                    } else if self.health.get(&record.path).is_some_and(|h| h.healthy) {
                        ("✓", Color::Green)
                    } else {
                        ("○", Color::Yellow)
                    };
                    let name = record
                        .path
                        .file_name()
                        .unwrap_or(record.path.as_os_str())
                        .to_string_lossy();
                    ListItem::new(format!(" {mark} {name}")).style(Style::new().fg(color))
                })
                .collect::<Vec<_>>();
            if !cfg!(windows) {
                items.extend([
                    ListItem::new(" + Connect existing Wiki"),
                    ListItem::new(" + Create new Wiki"),
                ]);
            }
            frame.render_stateful_widget(
                List::new(items)
                    .block(bordered(" Your Wikis ", focus == Pane::Kinds))
                    .highlight_style(Style::new().fg(ACCENT).add_modifier(Modifier::REVERSED))
                    .highlight_symbol("› "),
                list,
                &mut state,
            );
        }
        let list_hit = (list.width > 0).then_some((list, state.offset()));
        if details.width == 0 {
            return (list_hit, None);
        }
        if self.registry_error.is_none() {
            if let Some(record) = self.record() {
                return (
                    list_hit,
                    self.draw_capabilities(frame, details, focus, record, home),
                );
            }
        }
        let mut lines = Vec::new();
        if let Some(error) = &self.registry_error {
            lines.push(Line::styled(error.clone(), Style::new().fg(Color::Red)));
            lines.push(Line::from(
                "Repair the registry before changing a Wiki; no records replaced.",
            ));
        } else {
            if self.vaults.is_empty() {
                lines.push(Line::from("No registered Wikis yet."));
            }
            lines.push(Line::from(if cfg!(windows) {
                "Create or connect from WSL2."
            } else if self.cursor == self.vaults.len() {
                "Connect: choose an existing Obsidian folder. Notes stay in place."
            } else {
                "Create: choose a parent folder and a new Wiki name."
            }));
            lines.push(Line::from("Enter opens only the folder picker, then returns here. Pick capabilities before Review / Install."));
        }
        if let Some(message) = &self.message {
            lines.insert(0, Line::from(message.clone()));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(bordered(" This Wiki · capabilities ", focus == Pane::Items)),
            details,
        );
        (list_hit, None)
    }

    fn draw_capabilities(
        &self,
        frame: &mut Frame,
        details: Rect,
        focus: Pane,
        record: &VaultRecord,
        home: &Path,
    ) -> ListHit {
        let [items_area, info] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).areas(details);
        let selected = self.selected(record);
        let capabilities = self.capabilities();
        let current = capabilities.get(self.item_cursor).copied();
        let items = capabilities
            .into_iter()
            .map(|capability| {
                let (mark, color) = if self.installed(record, capability) {
                    ("✓", Color::Green)
                } else if capability == Capability::Essentials
                    && selected.contains(&capability)
                    && self
                        .pending
                        .get(&record.path)
                        .is_none_or(|p| !p.selected.contains(&capability))
                {
                    ("[-]", ACCENT)
                } else if selected.contains(&capability) {
                    ("[x]", ACCENT)
                } else {
                    ("[ ]", Color::Reset)
                };
                ListItem::new(format!(" {mark} {}", capability.label()))
                    .style(Style::new().fg(color))
            })
            .collect::<Vec<_>>();
        let heading = self.search.as_ref().map_or_else(
            || " This Wiki · capabilities ".into(),
            |query| format!(" Find here: {query} "),
        );
        let mut state = ListState::default().with_selected(Some(self.item_cursor));
        frame.render_stateful_widget(
            List::new(if items.is_empty() {
                vec![ListItem::new("No matches · esc clears search")]
            } else {
                items
            })
            .block(bordered(&heading, focus == Pane::Items))
            .highlight_style(Style::new().fg(ACCENT).add_modifier(Modifier::REVERSED))
            .highlight_symbol("› "),
            items_area,
            &mut state,
        );
        let detail = self.message.clone().unwrap_or_else(|| {
            if !self.health.contains_key(&record.path)
                && self
                    .pending
                    .get(&record.path)
                    .is_none_or(|picks| picks.operation.is_none())
            {
                "Checking this Wiki…".into()
            } else if current.is_some_and(|capability| selected.contains(&capability)) {
                "[x] Will be added to this Wiki.".into()
            } else if current.is_some_and(|capability| self.installed(record, capability)) {
                "✓ Already installed in this Wiki.".into()
            } else {
                match current {
                    Some(Capability::Qmd) if self.health_ready(record, "QMD (shared)") => {
                        "QMD is installed on this machine. Select it to enable this Wiki.".into()
                    }
                    Some(Capability::Confluence)
                        if self.health_ready(record, "Confluence (shared)") =>
                    {
                        "Confluence is installed on this machine. Select it for this Wiki.".into()
                    }
                    _ => "Select to add to this Wiki.".into(),
                }
            }
        });
        let lines = vec![
            Line::styled(super::render::tidy(&record.path, home), TITLE),
            Line::from(detail),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), info);
        Some((items_area, state.offset()))
    }
}

impl Wizard {
    pub(super) fn is_wiki_group(&self, group: &Group) -> bool {
        self.model.purpose == super::state::WizardPurpose::Install && !group.everything && group.items().any(|row| {
            matches!(row, Item::Resource(index) if self.model.resources[index].group == "Wiki")
        })
    }

    pub(super) fn browsing_wiki(&self) -> bool {
        self.search.is_none()
            && self.screen == Screen::Choose
            && self.is_wiki_group(self.choose.group())
    }

    pub(super) fn wiki_key(&mut self, code: KeyCode) -> Option<Action> {
        if self.screen != Screen::Choose {
            return None;
        }
        let stage = &mut self.choose;
        let browser = &mut self.wiki;
        if let Some(path) = browser.confirm_unregister.take() {
            return match code {
                KeyCode::Enter => Some(Action::UnregisterWiki(path)),
                KeyCode::Esc => None,
                _ => {
                    browser.confirm_unregister = Some(path);
                    None
                }
            };
        }
        if let Some(query) = &mut browser.search {
            match code {
                KeyCode::Enter | KeyCode::Esc => {
                    let capability = browser.capabilities().get(browser.item_cursor).copied();
                    browser.search = None;
                    browser.item_cursor = wiki_capabilities()
                        .iter()
                        .position(|c| Some(*c) == capability)
                        .unwrap_or(0);
                    return None;
                }
                KeyCode::Char(' ') => {
                    browser.toggle();
                    return None;
                }
                KeyCode::Char(c) => {
                    query.push(c);
                    browser.item_cursor = 0;
                    return None;
                }
                KeyCode::Backspace => {
                    query.pop();
                    browser.item_cursor = 0;
                    return None;
                }
                _ => {}
            }
        } else if code == KeyCode::Char('/') {
            browser.search = Some(String::new());
            browser.item_cursor = 0;
            stage.focus = Pane::Items;
            return None;
        }
        let navigation = if code == KeyCode::Esc {
            KeyCode::Left
        } else {
            code
        };
        if stage.focus.navigate(navigation, &Pane::ALL) {
            return None;
        }
        if let Some(delta) = movement(code) {
            let (cursor, len) = match stage.focus {
                Pane::Kinds => {
                    browser.item_cursor = 0;
                    let len = browser.len();
                    (&mut browser.cursor, len)
                }
                Pane::Items => {
                    let len = browser.capabilities().len();
                    (&mut browser.item_cursor, len)
                }
                Pane::Groups => return None,
            };
            let previous = *cursor;
            *cursor = clamp_step(*cursor, delta, len);
            if stage.focus == Pane::Kinds && *cursor != previous {
                browser.message = None;
            }
            return None;
        }
        match code {
            KeyCode::Char('u') => {
                browser.confirm_unregister = browser.record().map(|record| record.path.clone());
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if stage.focus == Pane::Groups {
                    stage.focus = Pane::Kinds;
                } else if stage.focus == Pane::Kinds && browser.record().is_some() {
                    stage.focus = Pane::Items;
                } else if browser.record().is_some() {
                    browser.toggle();
                } else {
                    return browser.entry().map(Action::PickWiki);
                }
            }
            _ => {}
        }
        None
    }

    pub(super) fn render_wiki_unregister(&self, frame: &mut Frame) {
        let Some(path) = self.wiki.confirm_unregister.as_ref() else {
            return;
        };
        crate::ui::chrome::confirm_modal(
            frame,
            " Unregister Wiki ",
            vec![
                Line::styled("Unregister this Wiki?", Style::new().fg(ERR).bold()),
                Line::from(path.display().to_string()),
                Line::from(""),
                Line::from("Its files will remain untouched."),
            ],
            ("enter", "unregister"),
            ("esc", "keep"),
        );
    }
}
