use crate::wiki::{VaultRecord, WikiOperation, WikiRegistry, WikiRequest};
use crate::{CommandSpec, System};
use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
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
    PickPath(WikiOperation),
    OpenObsidianDownload,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Home,
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
        if cfg!(windows) || !record.path.is_dir() {
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
                    let operation = if choice == "Create a new Vault" {
                        WikiOperation::Create
                    } else {
                        WikiOperation::Adopt
                    };
                    return Some(WikiChoice::PickPath(operation));
                }
            }
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
            Page::Capabilities => self.page = Page::Home,
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
        if self.error.take().is_some() {
            return None;
        }
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
            return self.back();
        }
        match self.page {
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
        if frame.area().width < 40 || frame.area().height < 10 {
            frame.render_widget(
                Paragraph::new("loom wiki needs at least 40 columns by 10 rows")
                    .alignment(Alignment::Center),
                frame.area(),
            );
            return;
        }
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        self.draw_header(frame, header);
        match self.page {
            Page::Home => self.draw_home(frame, body),
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
        self.draw_footer(frame, footer);
        if let Some(error) = &self.error {
            let area = centered(
                frame.area(),
                60.min(frame.area().width.saturating_sub(4)),
                5,
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
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" loom wiki", Style::new().fg(ACCENT).bold()),
                Span::styled(
                    concat!("  v", env!("CARGO_PKG_VERSION")),
                    Style::new().dim(),
                ),
            ])),
            area,
        );
        let trail = match self.page {
            Page::Home => vec![("Choose", true), ("Capabilities", false), ("Review", false)],
            Page::Capabilities => vec![
                ("✓ Choose", false),
                ("Capabilities", true),
                ("Review", false),
            ],
            Page::Review => vec![
                ("✓ Choose", false),
                ("✓ Capabilities", false),
                ("Review", true),
            ],
            Page::Vaults => vec![("Vaults", true), ("Action", false)],
            Page::Actions | Page::ConfirmUnregister => vec![("✓ Vaults", false), ("Action", true)],
        };
        if area.width < 80 {
            let active = trail.iter().position(|(_, active)| *active).unwrap_or(0);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!("step {}/{} · ", active + 1, trail.len()),
                        Style::new().dim(),
                    ),
                    Span::styled(
                        trail[active].0.trim_start_matches("✓ "),
                        Style::new().fg(ACCENT).bold(),
                    ),
                    Span::raw(" "),
                ]))
                .alignment(Alignment::Right),
                area,
            );
            return;
        }
        let mut spans = Vec::new();
        for (index, (label, active)) in trail.into_iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(" › ", Style::new().dim()));
            }
            spans.push(Span::styled(
                label,
                if active {
                    Style::new().fg(ACCENT).bold()
                } else {
                    Style::new().dim()
                },
            ));
        }
        spans.push(Span::raw(" "));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
            area,
        );
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let hint = match self.page {
            Page::Capabilities => " ↑↓ move · space toggle",
            Page::Review => " exact files are reviewed next",
            Page::ConfirmUnregister => " enter unregister · esc keep",
            _ => " ↑↓ move · enter select",
        };
        let (back, next) = match self.page {
            Page::Home => ("", "Select"),
            Page::Capabilities => ("Back", "Next"),
            Page::Review => ("Back", "Set up"),
            Page::Vaults => ("Back", "Select"),
            Page::Actions => ("Back", "Run"),
            Page::ConfirmUnregister => ("Keep", "Remove"),
        };
        let [hint_area, controls_area] = Layout::horizontal([
            Constraint::Min(1),
            Constraint::Length(if back.is_empty() { 13 } else { 25 }),
        ])
        .areas(area);
        frame.render_widget(
            Paragraph::new(Span::styled(hint, Style::new().dim())),
            hint_area,
        );
        let mut controls = Vec::new();
        if !back.is_empty() {
            controls.push(Span::styled(
                format!("[ ◂ {back} ]  "),
                Style::new().fg(ACCENT),
            ));
        }
        controls.push(Span::styled(
            format!("[ {next:^7} ▸ ]"),
            Style::new()
                .fg(ACCENT)
                .add_modifier(Modifier::REVERSED)
                .bold(),
        ));
        frame.render_widget(
            Paragraph::new(Line::from(controls)).alignment(Alignment::Right),
            controls_area,
        );
    }

    fn draw_home(&self, frame: &mut Frame, area: Rect) {
        let [menu, details] = if area.width >= 72 {
            Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
                .spacing(1)
                .areas(area)
        } else {
            [area, Rect::default()]
        };
        self.draw_list(frame, menu, " Wiki Vaults ", self.home_items());
        if details.width == 0 {
            return;
        }
        let choice = self.home_items()[self.cursor];
        let (title, copy) = match choice {
            "Create a new Vault" => (
                "Create",
                "Choose a new Vault location with your system folder picker. Loom shows every file before writing it.",
            ),
            "Connect an existing Vault" => (
                "Connect",
                "Choose an Obsidian Vault with your system folder picker. Existing notes stay in place.",
            ),
            "Manage registered Vaults" => (
                "Manage",
                "Check, repair, open, launch, or unregister a Vault already known to Loom.",
            ),
            _ => (
                "Obsidian",
                "Open the official download page. Obsidian is optional; Markdown and Pi work without it.",
            ),
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(title, Style::new().fg(ACCENT).bold()),
                Line::from(""),
                Line::from(copy),
            ])
            .wrap(Wrap { trim: true })
            .block(panel(" Details ", false).padding(Padding::horizontal(1))),
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
            .block(panel(" Capabilities ", true).padding(Padding::uniform(1))),
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
            .block(panel(" Review ", true).padding(Padding::uniform(1))),
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
            .block(panel(" Confirm ", true)),
            area,
        );
    }
}

fn panel(title: &str, focused: bool) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(if focused {
            Style::new().fg(ACCENT)
        } else {
            Style::new().dim()
        })
        .title_style(if focused {
            Style::new().fg(ACCENT).bold()
        } else {
            Style::new().dim()
        })
        .title(title.to_owned())
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

fn pick_vault_path(
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
    let mut wizard = WikiWizard::new(current.clone(), vaults, feynman_default, obsidian_installed);
    loop {
        let mut terminal = ratatui::init();
        let outcome = run_loop(&mut terminal, &mut wizard);
        ratatui::restore();
        match outcome? {
            WikiChoice::PickPath(operation) => {
                match pick_vault_path(system, &operation, &current) {
                    Ok(Some(path)) => wizard.set_picked_path(operation, path),
                    Ok(None) => {}
                    Err(error) => wizard.error = Some(error.to_string()),
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
    fn home_matches_the_main_wizard_chrome() {
        let wizard = WikiWizard::new(std::env::temp_dir(), Vec::new(), false, true);
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
        let wizard = WikiWizard::new(std::env::temp_dir(), Vec::new(), false, true);
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
    fn picker_error_consumes_the_dismissal_key() {
        let mut wizard = WikiWizard::new(std::env::temp_dir(), Vec::new(), false, true);
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
        let mut wizard = WikiWizard::new(root.clone(), Vec::new(), true, false);

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
        let mut wizard = WikiWizard::new(std::env::temp_dir(), Vec::new(), false, true);

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
