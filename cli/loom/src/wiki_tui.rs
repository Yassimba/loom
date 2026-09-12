use crate::wiki::{VaultRecord, WikiOperation, WikiRegistry, WikiRequest};
use crate::{CommandSpec, System};
use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use crate::ui::chrome::{self, Crumb};
use crate::ui::theme::{ACCENT, ERR, OK, TITLE};

pub(crate) enum WikiChoice {
    Request(WikiRequest),
    PickPath(WikiOperation),
    InspectVault,
    OpenObsidianDownload,
    Cancelled,
}

enum MenuAction {
    Vault(usize),
    Operation(WikiOperation),
    DownloadObsidian,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Home,
    Capabilities,
    Review,
    Status,
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
    qmd: bool,
    vaults: Vec<VaultRecord>,
    selected_vault: usize,
    obsidian_installed: bool,
    error: Option<String>,
    health: Option<crate::wiki::VaultHealth>,
    /// Folded to `~` in every path shown on screen.
    home: PathBuf,
}

impl WikiWizard {
    fn new(
        home: PathBuf,
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
            qmd: false,
            vaults,
            selected_vault: 0,
            obsidian_installed,
            error: None,
            health: None,
            home,
        }
    }

    fn shown(&self, path: &std::path::Path) -> String {
        crate::ui::tidy_path(path, &self.home)
    }

    fn home_items(&self) -> Vec<MenuAction> {
        let mut items = (0..self.vaults.len())
            .map(MenuAction::Vault)
            .collect::<Vec<_>>();
        if !cfg!(windows) {
            items.extend([
                MenuAction::Operation(WikiOperation::Create),
                MenuAction::Operation(WikiOperation::Adopt),
            ]);
        }
        if !self.obsidian_installed {
            items.push(MenuAction::DownloadObsidian);
        }
        items
    }

    fn action_items(&self) -> Vec<MenuAction> {
        let Some(record) = self.selected_record() else {
            return Vec::new();
        };
        let mut actions = vec![MenuAction::Operation(WikiOperation::Status)];
        if !cfg!(windows) && record.path.is_dir() {
            actions.extend(
                [
                    WikiOperation::Repair,
                    WikiOperation::Open,
                    WikiOperation::Launch,
                ]
                .map(MenuAction::Operation),
            );
            if !self.obsidian_installed {
                actions.push(MenuAction::DownloadObsidian);
            }
        }
        actions.push(MenuAction::Operation(WikiOperation::Unregister));
        actions
    }

    fn menu_label(&self, action: &MenuAction) -> String {
        match action {
            MenuAction::Vault(index) => return self.shown(&self.vaults[*index].path),
            MenuAction::DownloadObsidian => "Open Obsidian download page",
            MenuAction::Operation(operation) => match operation {
                WikiOperation::Create => "Create a new Vault",
                WikiOperation::Adopt => "Connect an existing Vault",
                WikiOperation::Status => "Status",
                WikiOperation::Repair => "Repair",
                WikiOperation::Open => "Open in Obsidian",
                WikiOperation::Launch => "Launch Pi",
                WikiOperation::Unregister => "Unregister",
            },
        }
        .into()
    }

    fn activate(&mut self, action: MenuAction) -> Option<WikiChoice> {
        match action {
            MenuAction::Vault(index) => {
                self.selected_vault = index;
                self.page = Page::Status;
                self.cursor = 0;
                self.health = None;
                Some(WikiChoice::InspectVault)
            }
            MenuAction::DownloadObsidian => Some(WikiChoice::OpenObsidianDownload),
            MenuAction::Operation(operation) => match operation {
                WikiOperation::Create | WikiOperation::Adopt => {
                    Some(WikiChoice::PickPath(operation))
                }
                WikiOperation::Status => {
                    self.page = Page::Status;
                    self.cursor = 0;
                    Some(WikiChoice::InspectVault)
                }
                WikiOperation::Unregister => {
                    self.page = Page::ConfirmUnregister;
                    None
                }
                operation => Some(WikiChoice::Request(self.request(operation))),
            },
        }
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
            qmd: record.map_or(self.qmd, |record| record.qmd),
            yes: false,
        }
    }

    fn set_picked_path(&mut self, operation: WikiOperation, path: PathBuf) {
        self.operation = operation;
        self.path = path.display().to_string();
        self.page = Page::Capabilities;
        self.cursor = 0;
        self.error = None;
    }

    fn enter(&mut self) -> Option<WikiChoice> {
        self.error = None;
        match self.page {
            Page::Home | Page::Actions => {
                let items = if self.page == Page::Home {
                    self.home_items()
                } else {
                    self.action_items()
                };
                return self.activate(items.into_iter().nth(self.cursor)?);
            }
            Page::Capabilities => self.page = Page::Review,
            Page::Review => return Some(WikiChoice::Request(self.request(self.operation.clone()))),
            Page::Status => {
                self.page = Page::Actions;
                self.cursor = 0;
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
            Page::Capabilities => self.page = Page::Home,
            Page::Review => self.page = Page::Capabilities,
            Page::Status => self.page = Page::Home,
            Page::Actions => self.page = Page::Status,
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
        if self.error.take().is_some() {
            return None;
        }
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
            return self.back();
        }
        match self.page {
            Page::Capabilities => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.step(-1, 3),
                KeyCode::Down | KeyCode::Char('j') => self.step(1, 3),
                KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => match self.cursor {
                    0 => self.feynman = !self.feynman,
                    1 => self.confluence = !self.confluence,
                    _ => self.qmd = !self.qmd,
                },
                KeyCode::Enter => return self.enter(),
                _ => {}
            },
            Page::Status => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => self.cursor = (self.cursor + 1).min(100),
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
        let Some([header, body, footer]) =
            chrome::frame_areas(frame, "Or use `loom wiki --help` for scripted setup.")
        else {
            return;
        };
        self.draw_header(frame, header);
        match self.page {
            Page::Home => self.draw_home(frame, body),
            Page::Capabilities => self.draw_capabilities(frame, body),
            Page::Review => self.draw_review(frame, body),
            Page::Status => self.draw_status(frame, body),
            Page::Actions => self.draw_list(
                frame,
                body,
                " Manage Vault ",
                self.action_items()
                    .iter()
                    .map(|action| self.menu_label(action)),
            ),
            Page::ConfirmUnregister => {
                self.draw_list(
                    frame,
                    body,
                    " Manage Vault ",
                    self.action_items()
                        .iter()
                        .map(|action| self.menu_label(action)),
                );
            }
        }
        self.draw_footer(frame, footer);
        if self.page == Page::ConfirmUnregister {
            self.draw_unregister(frame);
        }
        if let Some(error) = &self.error {
            let area = frame.area().centered(
                Constraint::Length(60.min(frame.area().width.saturating_sub(4))),
                Constraint::Length(5),
            );
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(error, Style::new().fg(ERR)),
                    Line::from(""),
                    Line::styled("Press any key to return.", Style::new().dim()),
                ])
                .alignment(Alignment::Center)
                .block(panel(" Folder picker ", true)),
                area,
            );
        }
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let (labels, current): (&[&str], usize) = match self.page {
            Page::Home => (&["Choose", "Capabilities", "Review"], 0),
            Page::Capabilities => (&["Choose", "Capabilities", "Review"], 1),
            Page::Review => (&["Choose", "Capabilities", "Review"], 2),
            Page::Status => (&["Vaults", "Status", "Actions"], 1),
            Page::Actions | Page::ConfirmUnregister => (&["Vaults", "Actions"], 1),
        };
        let crumbs: Vec<Crumb> = labels
            .iter()
            .enumerate()
            .map(|(index, label)| Crumb {
                label: (*label).to_owned(),
                done: index < current,
            })
            .collect();
        chrome::header(frame, area, "wiki", Vec::new(), &crumbs, current);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let hint = match self.page {
            Page::Capabilities => " ↑↓ move · space toggle",
            Page::Review => " exact files are reviewed next",
            Page::ConfirmUnregister => " enter unregister · esc keep",
            Page::Status => " ↑↓ scroll · enter actions · esc back",
            _ => " ↑↓ move · enter continue",
        };
        let (back, next) = match self.page {
            Page::Home => ("", "Select"),
            Page::Capabilities => ("Back", "Next"),
            Page::Review => ("Back", "Set up"),
            Page::Status => ("Vaults", "Actions"),
            Page::Actions => ("Back", "Run"),
            Page::ConfirmUnregister => ("Keep", "Remove"),
        };
        chrome::footer(
            frame,
            area,
            hint,
            (!back.is_empty()).then_some(back),
            (next, true),
        );
    }

    fn draw_home(&self, frame: &mut Frame, area: Rect) {
        if self.home_items().is_empty() {
            frame.render_widget(
                Paragraph::new("No registered Wikis yet. Create or connect a Wiki from WSL2.")
                    .wrap(Wrap { trim: true })
                    .block(panel(" Your Wikis ", true)),
                area,
            );
            return;
        }
        let [menu, details] = if area.width >= 72 {
            Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
                .spacing(1)
                .areas(area)
        } else {
            [area, Rect::default()]
        };
        self.draw_list(
            frame,
            menu,
            " Wiki Vaults ",
            self.home_items()
                .iter()
                .map(|action| self.menu_label(action)),
        );
        if details.width == 0 {
            return;
        }
        if let Some(record) = self.vaults.get(self.cursor) {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(self.shown(&record.path), TITLE),
                    Line::from(""),
                    Line::styled("enter · status and actions", Style::new().dim()),
                ])
                .wrap(Wrap { trim: true })
                .block(panel(" This Wiki ", false)),
                details,
            );
            return;
        }
        let choices = self.home_items();
        let choice = choices.get(self.cursor);
        let (title, copy) = match choice {
            Some(MenuAction::Operation(WikiOperation::Create)) => (
                "Create",
                "Choose a new Vault location with your system folder picker. Loom shows every file before writing it.",
            ),
            Some(MenuAction::Operation(WikiOperation::Adopt)) => (
                "Connect",
                "Choose an Obsidian Vault with your system folder picker. Existing notes stay in place.",
            ),
            _ => (
                "Obsidian",
                "Open the official download page. Obsidian is optional; Markdown and Pi work without it.",
            ),
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(title, TITLE),
                Line::from(""),
                Line::from(copy),
            ])
            .wrap(Wrap { trim: true })
            .block(panel(" Details ", false)),
            details,
        );
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
                .block(panel(title, true))
                .highlight_style(
                    Style::new()
                        .fg(ACCENT)
                        .add_modifier(Modifier::REVERSED)
                        .bold(),
                )
                .highlight_symbol("› "),
            area,
            &mut ListState::default().with_selected(Some(self.cursor)),
        );
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![
            Line::styled(
                self.selected_record()
                    .map(|record| self.shown(&record.path))
                    .unwrap_or_default(),
                TITLE,
            ),
            Line::from(""),
        ];
        if let Some(health) = &self.health {
            lines.extend(health_lines(health));
        } else {
            lines.push(Line::from("Checking this Vault…"));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .scroll((self.cursor as u16, 0))
                .block(panel(" Installed in this Vault ", true)),
            area,
        );
    }

    fn draw_capabilities(&self, frame: &mut Frame, area: Rect) {
        let option = |selected: bool, label: &'static str, active: bool| {
            Line::from(vec![
                Span::styled(if selected { "[x] " } else { "[ ] " }, Style::new().fg(OK)),
                Span::styled(label, if active { TITLE } else { Style::new() }),
            ])
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Optional capabilities", Style::new().bold()),
                Line::from(format!(
                    "Vault: {}",
                    self.shown(std::path::Path::new(self.path.trim()))
                )),
                Line::from(""),
                option(self.feynman, "Feynman research tools", self.cursor == 0),
                option(
                    self.confluence,
                    "Confluence Markdown exporter",
                    self.cursor == 1,
                ),
                option(self.qmd, "QMD local search", self.cursor == 2),
                Line::from(""),
                Line::styled("Selections are enabled for this Vault.", Style::new().dim()),
            ])
            .block(panel(" Capabilities ", true)),
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
                Line::styled(operation, TITLE),
                Line::from(format!(
                    "  {}",
                    self.shown(std::path::Path::new(self.path.trim()))
                )),
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
                Line::from(format!(
                    "QMD        {}",
                    if self.qmd { "included" } else { "not selected" }
                )),
                Line::from(""),
                Line::from(if self.qmd {
                    "First QMD search setup can download about 2 GB of models."
                } else {
                    "QMD search is optional; pick it only if this Vault should index locally."
                }),
                Line::styled(
                    "Loom will preview the exact Vault files before applying them.",
                    Style::new().dim(),
                ),
            ])
            .wrap(Wrap { trim: true })
            .block(panel(" Review ", true)),
            area,
        );
    }

    fn draw_unregister(&self, frame: &mut Frame) {
        let path = self
            .selected_record()
            .map(|record| self.shown(&record.path))
            .unwrap_or_default();
        chrome::confirm_modal(
            frame,
            " Unregister Vault ",
            vec![
                Line::styled("Unregister this Vault?", Style::new().fg(ERR).bold()),
                Line::from(path),
                Line::from(""),
                Line::from("Its files will remain untouched."),
            ],
            ("enter", "unregister"),
            ("esc", "keep"),
        );
    }
}

pub(crate) fn health_lines(health: &crate::wiki::VaultHealth) -> Vec<Line<'static>> {
    health
        .rows
        .iter()
        .map(|(mark, label, detail)| {
            Line::styled(
                format!("{} {label}: {detail}", mark.glyph()),
                Style::new().fg(mark.color()),
            )
        })
        .collect()
}

use crate::ui::chrome::panel;

fn picker_command(
    system: &dyn System,
    operation: &WikiOperation,
    current: &std::path::Path,
) -> Result<CommandSpec> {
    let start = current.display().to_string();
    if cfg!(target_os = "macos") {
        let script = if *operation == WikiOperation::Create {
            r#"on run argv
set startFolder to POSIX file (item 1 of argv)
return POSIX path of (choose file name with prompt "Create a Loom Wiki Vault" default location startFolder default name "Wiki Vault")
end run"#
        } else {
            r#"on run argv
set startFolder to POSIX file (item 1 of argv)
return POSIX path of (choose folder with prompt "Choose an Obsidian Vault" default location startFolder)
end run"#
        };
        return Ok(CommandSpec::new("osascript", ["-e", script, "--", &start]));
    }
    if system.command_exists("zenity") {
        let mut args = vec!["--file-selection".to_owned()];
        if *operation == WikiOperation::Create {
            args.extend([
                "--save".into(),
                format!("--filename={start}/Wiki Vault"),
                "--title=Create a Loom Wiki Vault".into(),
            ]);
        } else {
            args.extend([
                "--directory".into(),
                format!("--filename={start}/"),
                "--title=Choose an Obsidian Vault".into(),
            ]);
        }
        return Ok(CommandSpec::new("zenity", args));
    }
    if system.command_exists("kdialog") {
        let args = if *operation == WikiOperation::Create {
            vec!["--getsavefilename".into(), format!("{start}/Wiki Vault")]
        } else {
            vec!["--getexistingdirectory".into(), start]
        };
        return Ok(CommandSpec::new("kdialog", args));
    }
    anyhow::bail!(
        "No native folder picker is available. Install zenity or kdialog, or use `loom wiki create PATH`."
    )
}

pub(crate) fn pick_vault_path(
    system: &dyn System,
    operation: &WikiOperation,
    current: &std::path::Path,
) -> Result<Option<PathBuf>> {
    let output = system.run(&picker_command(system, operation, current)?)?;
    if !output.success {
        let message = output.stderr.trim();
        if message.is_empty() || message.to_ascii_lowercase().contains("cancel") {
            return Ok(None);
        }
        anyhow::bail!("Folder picker failed: {message}");
    }
    let path = PathBuf::from(output.stdout.trim());
    if path.as_os_str().is_empty() {
        return Ok(None);
    }
    match operation {
        WikiOperation::Create => {
            anyhow::ensure!(!path.exists(), "Choose a new folder name for this Vault");
            anyhow::ensure!(
                path.parent().is_some_and(std::path::Path::is_dir),
                "The selected parent folder does not exist"
            );
        }
        WikiOperation::Adopt => anyhow::ensure!(
            path.is_dir() && path.join(".obsidian").is_dir(),
            "Choose an existing Obsidian Vault with a .obsidian folder"
        ),
        _ => unreachable!(),
    }
    Ok(Some(path))
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
    let mut wizard = WikiWizard::new(
        home,
        current.clone(),
        vaults,
        feynman_default,
        obsidian_installed,
    );
    run_session(system, &mut wizard, &current)
}

fn run_session(
    system: &(dyn System + Sync),
    wizard: &mut WikiWizard,
    current: &std::path::Path,
) -> Result<WikiChoice> {
    loop {
        match ratatui::run(|terminal| run_loop(terminal, wizard))? {
            WikiChoice::PickPath(operation) => match pick_vault_path(system, &operation, current) {
                Ok(Some(path)) => wizard.set_picked_path(operation, path),
                Ok(None) => {}
                Err(error) => wizard.error = Some(error.to_string()),
            },
            WikiChoice::InspectVault => {
                if let Some(record) = wizard.selected_record() {
                    let out = crate::ui::Out::detect();
                    out.progress(
                        format!("Checking registered Vault {}", record.path.display()),
                        0,
                    );
                    wizard.health = Some(crate::wiki::inspect_vault(system, record));
                    out.progress_done();
                }
            }
            choice => return Ok(choice),
        }
    }
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
    let block = panel(&heading, true);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    let [preview, controls] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);
    let paragraph = Paragraph::new(lines.iter().cloned().map(Line::from).collect::<Vec<_>>())
        .wrap(Wrap { trim: true });
    let max_scroll = paragraph
        .line_count(preview.width.max(1))
        .saturating_sub(preview.height as usize)
        .min(u16::MAX as usize) as u16;
    frame.render_widget(paragraph.scroll((scroll.min(max_scroll), 0)), preview);
    let reviewed = scroll >= max_scroll;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    if yes { "  No  " } else { "[ No ]" },
                    if yes { Style::new().dim() } else { TITLE },
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
    ratatui::run(|terminal| confirm_in(terminal, title, lines))
}

/// Reuse the setup terminal for an exact file-plan approval, not another setup wizard.
pub(crate) fn confirm_in(
    terminal: &mut DefaultTerminal,
    title: &str,
    lines: &[String],
) -> Result<bool> {
    let mut yes = false;
    let mut scroll = 0u16;
    let mut max_scroll = 0u16;
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
            KeyCode::Down | KeyCode::Char('j') => scroll = scroll.saturating_add(1).min(max_scroll),
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
}

pub(crate) fn select(title: &str, choices: &[&str]) -> Result<Option<usize>> {
    require_terminal()?;
    let mut cursor = 0usize;
    ratatui::run(|terminal| loop {
        terminal.draw(|frame| {
            let items = choices
                .iter()
                .map(|choice| ListItem::new(format!("  {choice}")))
                .collect::<Vec<_>>();
            frame.render_stateful_widget(
                List::new(items)
                    .block(panel(&format!(" {title} "), true))
                    .highlight_style(
                        Style::new()
                            .fg(ACCENT)
                            .add_modifier(Modifier::REVERSED)
                            .bold(),
                    )
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
    })
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
    fn confirmation_counts_word_wrapped_rows_before_approval() {
        let mut terminal = Terminal::new(TestBackend::new(14, 5)).unwrap();
        let lines = vec!["123456 abcdef LASTXX".into()];
        let mut max_scroll = 0;
        terminal
            .draw(|frame| max_scroll = draw_confirmation(frame, "Review", &lines, 0, false))
            .unwrap();
        assert_eq!(max_scroll, 2);
        terminal
            .draw(|frame| {
                draw_confirmation(frame, "Review", &lines, max_scroll, false);
            })
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(screen.contains("LASTXX"));
    }

    #[test]
    #[cfg(not(windows))]
    fn home_matches_the_main_wizard_chrome() {
        let wizard = WikiWizard::new(
            std::env::temp_dir(),
            std::env::temp_dir(),
            Vec::new(),
            false,
            true,
        );
        let mut terminal = Terminal::new(TestBackend::new(90, 18)).unwrap();
        terminal.draw(|frame| wizard.draw(frame)).unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(screen.contains("Choose › Capabilities › Review"));
        assert!(screen.contains("system folder picker"));
        assert!(screen.contains("[ Select  ▸ ]"));
    }

    #[test]
    #[cfg(not(windows))]
    fn narrow_home_uses_compact_step_chrome() {
        let wizard = WikiWizard::new(
            std::env::temp_dir(),
            std::env::temp_dir(),
            Vec::new(),
            false,
            true,
        );
        let mut terminal = Terminal::new(TestBackend::new(50, 14)).unwrap();
        terminal.draw(|frame| wizard.draw(frame)).unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(screen.contains("step 1/3 · Choose"));
        assert!(!screen.contains("Choose › Capabilities"));
    }

    #[test]
    #[cfg(not(windows))]
    fn picker_error_consumes_the_dismissal_key() {
        let mut wizard = WikiWizard::new(
            std::env::temp_dir(),
            std::env::temp_dir(),
            Vec::new(),
            false,
            true,
        );
        wizard.error = Some("picker failed".into());

        assert!(wizard.handle_key(key(KeyCode::Enter)).is_none());
        assert_eq!(wizard.page, Page::Home);
        assert!(matches!(
            wizard.handle_key(key(KeyCode::Enter)),
            Some(WikiChoice::PickPath(WikiOperation::Create))
        ));
    }

    #[test]
    #[cfg(not(windows))]
    fn create_flow_uses_the_picked_path_and_collects_capabilities() {
        let root = std::env::temp_dir().join(format!("loom-wiki-tui-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut wizard = WikiWizard::new(root.clone(), root.clone(), Vec::new(), true, false);

        assert!(matches!(
            wizard.handle_key(key(KeyCode::Enter)),
            Some(WikiChoice::PickPath(WikiOperation::Create))
        ));
        wizard.set_picked_path(WikiOperation::Create, root.join("Notes"));
        wizard.handle_key(key(KeyCode::Down));
        wizard.handle_key(key(KeyCode::Char(' ')));
        wizard.handle_key(key(KeyCode::Enter));
        let WikiChoice::Request(request) = wizard.handle_key(key(KeyCode::Enter)).unwrap() else {
            panic!("expected Wiki request");
        };

        assert_eq!(request.vault, root.join("Notes"));
        assert!(request.feynman);
        assert!(request.confluence);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ctrl_c_cancels_from_the_wizard() {
        let mut wizard = WikiWizard::new(
            std::env::temp_dir(),
            std::env::temp_dir(),
            Vec::new(),
            false,
            true,
        );

        assert!(matches!(
            wizard.handle_key(ctrl(KeyCode::Char('c'))),
            Some(WikiChoice::Cancelled)
        ));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn mac_picker_uses_the_native_save_panel_for_a_new_vault() {
        struct Fake;
        impl System for Fake {
            fn command_exists(&self, _: &str) -> bool {
                false
            }
            fn refresh_path(&self) {}
            fn run(&self, _: &CommandSpec) -> Result<crate::CommandResult> {
                unreachable!()
            }
        }

        let command =
            picker_command(&Fake, &WikiOperation::Create, std::path::Path::new("/tmp")).unwrap();
        assert_eq!(command.program, "osascript");
        assert!(command
            .args
            .iter()
            .any(|arg| arg.contains("choose file name")));
    }

    #[test]
    #[cfg(not(windows))]
    fn folder_picker_validates_existing_and_new_paths_without_writing() {
        struct Picked {
            path: PathBuf,
            cancelled: bool,
        }
        impl System for Picked {
            fn command_exists(&self, _: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn run(&self, _: &CommandSpec) -> Result<crate::CommandResult> {
                Ok(crate::CommandResult {
                    success: !self.cancelled,
                    stdout: self.path.display().to_string(),
                    stderr: String::new(),
                })
            }
        }
        let root = std::env::temp_dir().join(format!("loom-wiki-picker-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Existing Wiki/.obsidian")).unwrap();
        let target = root.join("New Wiki");
        let mut system = Picked {
            path: target.clone(),
            cancelled: false,
        };
        assert_eq!(
            pick_vault_path(&system, &WikiOperation::Create, &root).unwrap(),
            Some(target.clone())
        );
        assert!(!target.exists(), "choosing a name must not create the Wiki");
        assert!(pick_vault_path(&system, &WikiOperation::Adopt, &root).is_err());
        system.path = root.join("Existing Wiki");
        assert_eq!(
            pick_vault_path(&system, &WikiOperation::Adopt, &root).unwrap(),
            Some(system.path.clone())
        );
        assert!(pick_vault_path(&system, &WikiOperation::Create, &root).is_err());
        system.cancelled = true;
        assert!(pick_vault_path(&system, &WikiOperation::Create, &root)
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unregister_requires_a_second_enter_and_keeps_the_registered_path() {
        let record = VaultRecord {
            path: PathBuf::from("/tmp/Vault"),
            feynman: false,
            confluence: false,
            qmd: false,
        };
        let mut wizard = WikiWizard::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
            vec![record],
            false,
            true,
        );
        assert!(matches!(
            wizard.handle_key(key(KeyCode::Enter)),
            Some(WikiChoice::InspectVault)
        ));
        assert_eq!(wizard.page, Page::Status);
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
