use crate::wiki::{VaultRecord, WikiOperation, WikiRegistry, WikiRequest};
use crate::System;
use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

const ACCENT: Color = Color::Cyan;
const OK: Color = Color::Green;
const ERR: Color = Color::Red;

pub(crate) enum WikiChoice {
    Request(WikiRequest),
    OpenObsidianDownload,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Home,
    Path,
    Capabilities,
    Review,
    Vaults,
    Actions,
    ConfirmUnregister,
}

struct WikiWizard {
    page: Page,
    cursor: usize,
    operation: WikiOperation,
    path: String,
    feynman: bool,
    confluence: bool,
    vaults: Vec<VaultRecord>,
    selected_vault: usize,
    obsidian_installed: bool,
    error: Option<String>,
}

impl WikiWizard {
    fn new(
        current: PathBuf,
        vaults: Vec<VaultRecord>,
        feynman_default: bool,
        obsidian_installed: bool,
    ) -> Self {
        Self {
            page: Page::Home,
            cursor: 0,
            operation: WikiOperation::Create,
            path: format!("{}/", current.display()),
            feynman: feynman_default,
            confluence: false,
            vaults,
            selected_vault: 0,
            obsidian_installed,
            error: None,
        }
    }

    fn home_items(&self) -> Vec<&'static str> {
        let mut items = if cfg!(windows) {
            vec!["Manage registered Vaults"]
        } else {
            vec![
                "Create a new Vault",
                "Connect an existing Vault",
                "Manage registered Vaults",
            ]
        };
        if !self.obsidian_installed {
            items.push("Open Obsidian download page");
        }
        items
    }

    fn action_items(&self) -> Vec<&'static str> {
        let Some(record) = self.vaults.get(self.selected_vault) else {
            return Vec::new();
        };
        if !record.path.is_dir() {
            return vec!["Status", "Unregister"];
        }
        let mut actions = vec!["Status", "Repair", "Open in Obsidian", "Launch Pi"];
        if !self.obsidian_installed {
            actions.push("Open Obsidian download page");
        }
        actions.push("Unregister");
        actions
    }

    fn selected_record(&self) -> Option<&VaultRecord> {
        self.vaults.get(self.selected_vault)
    }

    fn step(&mut self, delta: isize, len: usize) {
        if len > 0 {
            self.cursor = (self.cursor as isize + delta).clamp(0, len as isize - 1) as usize;
        }
    }

    fn request(&self, operation: WikiOperation) -> WikiRequest {
        let setup = matches!(operation, WikiOperation::Create | WikiOperation::Adopt);
        let record = (!setup).then(|| self.selected_record()).flatten();
        WikiRequest {
            operation,
            vault: record
                .map(|record| record.path.clone())
                .unwrap_or_else(|| PathBuf::from(self.path.trim())),
            feynman: record.map_or(self.feynman, |record| record.feynman),
            confluence: record.map_or(self.confluence, |record| record.confluence),
            yes: false,
        }
    }

    fn validate_path(&self) -> Result<()> {
        let path = PathBuf::from(self.path.trim());
        anyhow::ensure!(!self.path.trim().is_empty(), "Enter a Vault path");
        match self.operation {
            WikiOperation::Create => {
                let parent = path.parent().context("Enter a Vault folder name")?;
                anyhow::ensure!(parent.is_dir(), "Vault parent does not exist");
            }
            WikiOperation::Adopt => anyhow::ensure!(
                path.is_dir() && path.join(".obsidian").is_dir(),
                "Choose an existing Obsidian Vault with .obsidian/"
            ),
            _ => {}
        }
        Ok(())
    }

    fn enter(&mut self) -> Option<WikiChoice> {
        self.error = None;
        match self.page {
            Page::Home => {
                let choice = self.home_items()[self.cursor];
                if choice == "Open Obsidian download page" {
                    return Some(WikiChoice::OpenObsidianDownload);
                }
                if choice == "Manage registered Vaults" {
                    if self.vaults.is_empty() {
                        self.error = Some("No registered Vaults yet".into());
                    } else {
                        self.page = Page::Vaults;
                        self.cursor = 0;
                    }
                } else {
                    self.operation = if choice == "Create a new Vault" {
                        WikiOperation::Create
                    } else {
                        WikiOperation::Adopt
                    };
                    if self.operation == WikiOperation::Adopt && self.path.ends_with('/') {
                        self.path.pop();
                    }
                    self.page = Page::Path;
                    self.cursor = 0;
                }
            }
            Page::Path => match self.validate_path() {
                Ok(()) => {
                    self.path = self.path.trim().to_owned();
                    self.page = Page::Capabilities;
                    self.cursor = 0;
                }
                Err(error) => self.error = Some(error.to_string()),
            },
            Page::Capabilities => self.page = Page::Review,
            Page::Review => return Some(WikiChoice::Request(self.request(self.operation.clone()))),
            Page::Vaults => {
                self.selected_vault = self.cursor;
                self.page = Page::Actions;
                self.cursor = 0;
            }
            Page::Actions => {
                let action = self.action_items()[self.cursor];
                let operation = match action {
                    "Status" => WikiOperation::Status,
                    "Repair" => WikiOperation::Repair,
                    "Open in Obsidian" => WikiOperation::Open,
                    "Launch Pi" => WikiOperation::Launch,
                    "Open Obsidian download page" => return Some(WikiChoice::OpenObsidianDownload),
                    "Unregister" => {
                        self.page = Page::ConfirmUnregister;
                        return None;
                    }
                    _ => unreachable!(),
                };
                return Some(WikiChoice::Request(self.request(operation)));
            }
            Page::ConfirmUnregister => {
                return Some(WikiChoice::Request(self.request(WikiOperation::Unregister)));
            }
        }
        None
    }

    fn back(&mut self) -> Option<WikiChoice> {
        self.error = None;
        match self.page {
            Page::Home => return Some(WikiChoice::Cancelled),
            Page::Path => self.page = Page::Home,
            Page::Capabilities => self.page = Page::Path,
            Page::Review => self.page = Page::Capabilities,
            Page::Vaults => self.page = Page::Home,
            Page::Actions => self.page = Page::Vaults,
            Page::ConfirmUnregister => self.page = Page::Actions,
        }
        self.cursor = 0;
        None
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<WikiChoice> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(WikiChoice::Cancelled);
        }
        if key.code == KeyCode::Esc || (self.page != Page::Path && key.code == KeyCode::Char('q')) {
            return self.back();
        }
        match self.page {
            Page::Path => match key.code {
                KeyCode::Enter => return self.enter(),
                KeyCode::Backspace => {
                    self.path.pop();
                    self.error = None;
                }
                KeyCode::Char(character) => {
                    self.path.push(character);
                    self.error = None;
                }
                _ => {}
            },
            Page::Capabilities => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.step(-1, 2),
                KeyCode::Down | KeyCode::Char('j') => self.step(1, 2),
                KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                    if self.cursor == 0 {
                        self.feynman = !self.feynman;
                    } else {
                        self.confluence = !self.confluence;
                    }
                }
                KeyCode::Enter => return self.enter(),
                _ => {}
            },
            Page::ConfirmUnregister => {
                if key.code == KeyCode::Enter {
                    return self.enter();
                }
            }
            _ => {
                let len = match self.page {
                    Page::Home => self.home_items().len(),
                    Page::Vaults => self.vaults.len(),
                    Page::Actions => self.action_items().len(),
                    _ => 0,
                };
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => self.step(-1, len),
                    KeyCode::Down | KeyCode::Char('j') => self.step(1, len),
                    KeyCode::Home => self.cursor = 0,
                    KeyCode::End => self.cursor = len.saturating_sub(1),
                    KeyCode::Enter => return self.enter(),
                    _ => {}
                }
            }
        }
        None
    }

    fn draw(&self, frame: &mut Frame) {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" loom wiki", Style::new().fg(ACCENT).bold()),
                Span::styled(
                    concat!("  v", env!("CARGO_PKG_VERSION")),
                    Style::new().dim(),
                ),
            ])),
            header,
        );
        match self.page {
            Page::Home => self.draw_list(frame, body, " Wiki Vaults ", self.home_items()),
            Page::Path => self.draw_path(frame, body),
            Page::Capabilities => self.draw_capabilities(frame, body),
            Page::Review => self.draw_review(frame, body),
            Page::Vaults => self.draw_list(
                frame,
                body,
                " Registered Vaults ",
                self.vaults
                    .iter()
                    .map(|record| record.path.to_string_lossy()),
            ),
            Page::Actions => self.draw_list(frame, body, " Manage Vault ", self.action_items()),
            Page::ConfirmUnregister => self.draw_unregister(frame, body),
        }
        let hint = match self.page {
            Page::Path => " type a path · enter continue · esc back ",
            Page::Capabilities => " ↑↓ move · space toggle · enter continue · esc back ",
            Page::Review => " enter start setup · esc back ",
            Page::ConfirmUnregister => " enter unregister · esc keep ",
            _ => " ↑↓ move · enter select · esc back ",
        };
        frame.render_widget(
            Paragraph::new(Span::styled(hint, Style::new().dim())).alignment(Alignment::Center),
            footer,
        );
        if let Some(error) = &self.error {
            let area = centered(
                frame.area(),
                58.min(frame.area().width.saturating_sub(4)),
                5,
            );
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(error, Style::new().fg(ERR)),
                    Line::from(""),
                    Line::from("Edit the path or press esc to go back."),
                ])
                .alignment(Alignment::Center)
                .block(panel(" Check the path ")),
                area,
            );
        }
    }

    fn draw_list<I, S>(&self, frame: &mut Frame, area: Rect, title: &str, items: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let items = items
            .into_iter()
            .map(|item| ListItem::new(format!("  {}", item.into())))
            .collect::<Vec<_>>();
        frame.render_stateful_widget(
            List::new(items)
                .block(panel(title))
                .highlight_style(Style::new().fg(ACCENT).bold())
                .highlight_symbol("› "),
            area,
            &mut ListState::default().with_selected(Some(self.cursor)),
        );
        if self.page == Page::Home && !self.obsidian_installed {
            let note = Rect::new(
                area.x + 3,
                area.bottom().saturating_sub(4),
                area.width.saturating_sub(6),
                2,
            );
            frame.render_widget(
                Paragraph::new("Obsidian is optional. Markdown and Pi work without it; select the download page below for the official installer.")
                    .style(Style::new().dim())
                    .wrap(Wrap { trim: true }),
                note,
            );
        }
    }

    fn draw_path(&self, frame: &mut Frame, area: Rect) {
        let verb = if self.operation == WikiOperation::Create {
            "new Vault"
        } else {
            "existing Obsidian Vault"
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(format!("Choose the {verb} path"), Style::new().bold()),
                Line::from(""),
                Line::from(vec![
                    Span::styled("> ", Style::new().fg(ACCENT)),
                    Span::raw(&self.path),
                    Span::styled("▏", Style::new().fg(ACCENT)),
                ]),
            ])
            .block(panel(" Vault path ").padding(ratatui::widgets::Padding::uniform(1))),
            area,
        );
    }

    fn draw_capabilities(&self, frame: &mut Frame, area: Rect) {
        let option = |selected: bool, label: &'static str, active: bool| {
            Line::from(vec![
                Span::styled(if selected { "[x] " } else { "[ ] " }, Style::new().fg(OK)),
                Span::styled(
                    label,
                    if active {
                        Style::new().fg(ACCENT).bold()
                    } else {
                        Style::new()
                    },
                ),
            ])
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Optional capabilities", Style::new().bold()),
                Line::from(""),
                option(self.feynman, "Feynman research tools", self.cursor == 0),
                option(
                    self.confluence,
                    "Confluence Markdown exporter",
                    self.cursor == 1,
                ),
                Line::from(""),
                Line::styled("Selections are enabled for this Vault.", Style::new().dim()),
            ])
            .block(panel(" Capabilities ").padding(ratatui::widgets::Padding::uniform(1))),
            area,
        );
    }

    fn draw_review(&self, frame: &mut Frame, area: Rect) {
        let operation = if self.operation == WikiOperation::Create {
            "Create"
        } else {
            "Connect"
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(operation, Style::new().fg(ACCENT).bold()),
                Line::from(format!("  {}", self.path.trim())),
                Line::from(""),
                Line::from(format!(
                    "Feynman    {}",
                    if self.feynman {
                        "included"
                    } else {
                        "not selected"
                    }
                )),
                Line::from(format!(
                    "Confluence {}",
                    if self.confluence {
                        "included"
                    } else {
                        "not selected"
                    }
                )),
                Line::from(""),
                Line::styled(
                    "Loom will preview the exact Vault files before applying them.",
                    Style::new().dim(),
                ),
            ])
            .wrap(Wrap { trim: true })
            .block(panel(" Review ").padding(ratatui::widgets::Padding::uniform(1))),
            area,
        );
    }

    fn draw_unregister(&self, frame: &mut Frame, area: Rect) {
        let path = self
            .selected_record()
            .map(|record| record.path.display().to_string())
            .unwrap_or_default();
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Unregister this Vault?", Style::new().fg(ERR).bold()),
                Line::from(path),
                Line::from(""),
                Line::from("Its files will remain untouched."),
            ])
            .alignment(Alignment::Center)
            .block(panel(" Confirm ")),
            area,
        );
    }
}

fn panel(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray))
        .title(Span::styled(
            title.to_owned(),
            Style::new().fg(ACCENT).bold(),
        ))
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [vertical] = Layout::vertical([Constraint::Length(height)])
        .flex(ratatui::layout::Flex::Center)
        .areas(area);
    let [horizontal] = Layout::horizontal([Constraint::Length(width)])
        .flex(ratatui::layout::Flex::Center)
        .areas(vertical);
    horizontal
}

pub(crate) fn run(
    system: &(dyn System + Sync),
    feynman_default: bool,
    obsidian_installed: bool,
) -> Result<WikiChoice> {
    anyhow::ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "interactive Wiki setup needs a terminal; use `loom wiki create PATH --yes` or `loom wiki adopt PATH --yes`"
    );
    let home = system.home_dir().context("home directory is unavailable")?;
    let current = system.current_dir().unwrap_or_else(|| PathBuf::from("."));
    let vaults = WikiRegistry::load(&home)?.vaults;
    let mut wizard = WikiWizard::new(current, vaults, feynman_default, obsidian_installed);
    let mut terminal = ratatui::init();
    let outcome = run_loop(&mut terminal, &mut wizard);
    ratatui::restore();
    outcome
}

fn run_loop(terminal: &mut DefaultTerminal, wizard: &mut WikiWizard) -> Result<WikiChoice> {
    loop {
        terminal.draw(|frame| wizard.draw(frame))?;
        if !event::poll(Duration::from_millis(120))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if let Some(choice) = wizard.handle_key(key) {
                return Ok(choice);
            }
        }
    }
}

fn draw_confirmation(
    frame: &mut Frame,
    title: &str,
    lines: &[String],
    scroll: u16,
    yes: bool,
) -> u16 {
    let heading = format!(" {title} ");
    let block = panel(&heading);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    let [preview, controls] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);
    let paragraph = Paragraph::new(lines.iter().cloned().map(Line::from).collect::<Vec<_>>())
        .wrap(Wrap { trim: true });
    let width = preview.width.max(1) as usize;
    let rendered_lines = lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(width))
        .sum::<usize>();
    let max_scroll = rendered_lines.saturating_sub(preview.height as usize) as u16;
    frame.render_widget(paragraph.scroll((scroll.min(max_scroll), 0)), preview);
    let reviewed = scroll >= max_scroll;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    if yes { "  No  " } else { "[ No ]" },
                    if yes {
                        Style::new().dim()
                    } else {
                        Style::new().fg(ACCENT).bold()
                    },
                ),
                Span::raw("    "),
                Span::styled(
                    if yes { "[ Yes ]" } else { "  Yes  " },
                    if yes {
                        Style::new().fg(ERR).bold()
                    } else {
                        Style::new().dim()
                    },
                ),
            ]),
            Line::styled(
                if reviewed {
                    "enter confirm · esc cancel"
                } else {
                    "↑↓ review every change before approval"
                },
                Style::new().dim(),
            ),
        ])
        .alignment(Alignment::Center),
        controls,
    );
    max_scroll
}

pub(crate) fn confirm(title: &str, lines: &[String]) -> Result<bool> {
    require_terminal()?;
    let mut yes = false;
    let mut scroll = 0u16;
    let mut max_scroll = 0u16;
    let mut terminal = ratatui::init();
    let result = (|| -> Result<bool> {
        loop {
            terminal.draw(|frame| {
                max_scroll = draw_confirmation(frame, title, lines, scroll, yes);
            })?;
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == KeyEventKind::Release {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                break Ok(false);
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => scroll = (scroll + 1).min(max_scroll),
                KeyCode::Home => scroll = 0,
                KeyCode::End => scroll = max_scroll,
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if scroll >= max_scroll => {
                    yes = !yes
                }
                KeyCode::Char('y') if scroll >= max_scroll => break Ok(true),
                KeyCode::Char('n') | KeyCode::Esc => break Ok(false),
                KeyCode::Enter if !yes || scroll >= max_scroll => break Ok(yes),
                _ => {}
            }
        }
    })();
    ratatui::restore();
    result
}

pub(crate) fn select(title: &str, choices: &[&str]) -> Result<Option<usize>> {
    require_terminal()?;
    let mut cursor = 0usize;
    let mut terminal = ratatui::init();
    let result = (|| -> Result<Option<usize>> {
        loop {
            terminal.draw(|frame| {
                let items = choices
                    .iter()
                    .map(|choice| ListItem::new(format!("  {choice}")))
                    .collect::<Vec<_>>();
                frame.render_stateful_widget(
                    List::new(items)
                        .block(panel(&format!(" {title} ")))
                        .highlight_style(Style::new().fg(ACCENT).bold())
                        .highlight_symbol("› "),
                    frame.area(),
                    &mut ListState::default().with_selected(Some(cursor)),
                );
            })?;
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == KeyEventKind::Release {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                break Ok(None);
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => cursor = cursor.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    cursor = (cursor + 1).min(choices.len().saturating_sub(1))
                }
                KeyCode::Enter => break Ok(Some(cursor)),
                KeyCode::Esc | KeyCode::Char('q') => break Ok(None),
                _ => {}
            }
        }
    })();
    ratatui::restore();
    result
}

fn require_terminal() -> Result<()> {
    anyhow::ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "interactive Wiki setup needs a terminal; rerun with --yes for scripted setup"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::Terminal;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn long_confirmation_keeps_controls_visible_and_requires_scrolling() {
        let mut terminal = Terminal::new(TestBackend::new(50, 5)).unwrap();
        let lines = (0..10)
            .map(|index| format!("changed-{index}"))
            .collect::<Vec<_>>();
        let mut max_scroll = 0;
        terminal
            .draw(|frame| max_scroll = draw_confirmation(frame, "Review", &lines, 0, false))
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(max_scroll > 0);
        assert!(screen.contains("[ No ]"));
        assert!(screen.contains("review every change"));
    }

    #[test]
    #[cfg(not(windows))]
    fn create_flow_collects_path_and_feynman_in_the_tui() {
        let root = std::env::temp_dir().join(format!("loom-wiki-tui-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut wizard = WikiWizard::new(root.clone(), Vec::new(), true, false);

        wizard.handle_key(key(KeyCode::Enter));
        assert_eq!(wizard.page, Page::Path);
        for character in "Notes".chars() {
            wizard.handle_key(key(KeyCode::Char(character)));
        }
        wizard.handle_key(key(KeyCode::Enter));
        assert_eq!(wizard.page, Page::Capabilities);
        wizard.handle_key(key(KeyCode::Down));
        wizard.handle_key(key(KeyCode::Char(' ')));
        wizard.handle_key(key(KeyCode::Enter));
        assert_eq!(wizard.page, Page::Review);
        let WikiChoice::Request(request) = wizard.handle_key(key(KeyCode::Enter)).unwrap() else {
            panic!("expected Wiki request");
        };
        assert_eq!(request.operation, WikiOperation::Create);
        assert_eq!(request.vault, root.join("Notes"));
        assert!(request.feynman);
        assert!(request.confluence);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(not(windows))]
    fn create_uses_the_reviewed_path_when_a_vault_is_already_registered() {
        let root = std::env::temp_dir().join(format!("loom-wiki-target-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let registered = VaultRecord {
            path: root.join("registered"),
            feynman: false,
            confluence: false,
        };
        let mut wizard = WikiWizard::new(root.clone(), vec![registered], true, true);

        wizard.handle_key(key(KeyCode::Enter));
        wizard.path.push_str("new-vault ");
        wizard.handle_key(key(KeyCode::Enter));
        wizard.handle_key(key(KeyCode::Enter));
        let WikiChoice::Request(request) = wizard.handle_key(key(KeyCode::Enter)).unwrap() else {
            panic!("expected Wiki request");
        };

        assert_eq!(request.vault, root.join("new-vault"));
        assert!(request.feynman);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ctrl_c_cancels_instead_of_editing_the_path() {
        let root = std::env::temp_dir();
        let mut wizard = WikiWizard::new(root.clone(), Vec::new(), false, true);
        wizard.page = Page::Path;
        let before = wizard.path.clone();

        assert!(matches!(
            wizard.handle_key(ctrl(KeyCode::Char('c'))),
            Some(WikiChoice::Cancelled)
        ));
        assert_eq!(wizard.path, before);
    }

    #[test]
    fn unregister_requires_a_second_enter_and_keeps_the_registered_path() {
        let record = VaultRecord {
            path: PathBuf::from("/tmp/Vault"),
            feynman: false,
            confluence: false,
        };
        let mut wizard = WikiWizard::new(PathBuf::from("/tmp"), vec![record], false, true);
        wizard.cursor = wizard
            .home_items()
            .iter()
            .position(|item| *item == "Manage registered Vaults")
            .unwrap();
        wizard.handle_key(key(KeyCode::Enter));
        wizard.handle_key(key(KeyCode::Enter));
        wizard.cursor = wizard.action_items().len() - 1;
        assert!(wizard.handle_key(key(KeyCode::Enter)).is_none());
        assert_eq!(wizard.page, Page::ConfirmUnregister);
        let WikiChoice::Request(request) = wizard.handle_key(key(KeyCode::Enter)).unwrap() else {
            panic!("expected unregister request");
        };
        assert_eq!(request.operation, WikiOperation::Unregister);
        assert_eq!(request.vault, PathBuf::from("/tmp/Vault"));
    }
}
