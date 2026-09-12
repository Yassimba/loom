//! One voice for every non-interactive report (`update`, `init`, `status`,
//! `sync`, scripted `add`): a title line, marked rows with an aligned label
//! column, dim notes under a row, and a one-line verdict.
//!
//! ```text
//! loom update  v0.12.0
//!
//!   ✓ Shared skills         ~/.claude/skills · 12 skills
//!   ✓ Tool manifest         ~/.config/mise/conf.d/loom.toml
//!   ! Herdr                 herdr update exited 1
//!
//! ! 2 updated · 1 failed
//! ```

use crate::InstallPlan;
use anyhow::{Context, Result};
use inquire::Confirm;
use std::io::IsTerminal;
use std::path::Path;

/// The state glyph in front of a row.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Mark {
    /// Done or healthy.
    Ok,
    /// Fine to be missing; optional or skipped.
    Off,
    /// Needs attention.
    Bad,
}

impl Mark {
    /// The same glyph in reports and TUI screens.
    pub fn glyph(self) -> &'static str {
        match self {
            Mark::Ok => "✓",
            Mark::Off => "○",
            Mark::Bad => "!",
        }
    }

    pub fn color(self) -> ratatui::style::Color {
        match self {
            Mark::Ok => theme::OK,
            Mark::Off => theme::WARN,
            Mark::Bad => theme::ERR,
        }
    }
}

/// Only fixed, recognized causes reach compact reports. Tool output may contain
/// credentials or private document text, so it is never the details view.
pub(crate) fn failure_advice(message: &str) -> (&'static str, &'static str) {
    let message = message.to_ascii_lowercase();
    for (signals, cause, next) in [
        (
            &["cancelled", "canceled"][..],
            "Cancelled",
            "Completed work stays. Retry to continue.",
        ),
        (
            &["timed out", "timeout", "etimedout"][..],
            "The operation timed out",
            "Check the connection, then retry.",
        ),
        (
            &["enospc", "no space left"][..],
            "There is not enough disk space",
            "Free disk space, then retry.",
        ),
        (
            &["eacces", "eperm", "permission denied"][..],
            "Permission was denied",
            "Check access to the destination, then retry. Do not run Loom with sudo.",
        ),
        (
            &["401", "403", "unauthorized", "authentication", "rate limit"][..],
            "Access was denied or rate-limited",
            "Check the package source's authentication and access, then retry.",
        ),
        (
            &["enotfound", "econnreset", "econnrefused", "network", "dns"][..],
            "The package source could not be reached",
            "Check the connection and package source, then retry.",
        ),
        (
            &["not on path", "could not start", "unavailable on path"][..],
            "A required command is unavailable",
            "Open a new shell and run loom status to check prerequisites.",
        ),
        (
            &["modified", "overwrite", "conflict"][..],
            "Existing local changes need attention",
            "Review the destination. Keep a copy before resolving the conflict.",
        ),
        (
            &["verification", "did not report", "did not find"][..],
            "Installation could not be verified",
            "Check the destination, then retry verification and installation.",
        ),
    ] {
        if message.starts_with(&cause.to_ascii_lowercase())
            || signals.iter().any(|signal| message.contains(signal))
        {
            return (cause, next);
        }
    }
    (
        "The operation did not complete",
        "Retry the failed item. Raw tool output is hidden to protect secrets.",
    )
}

pub(crate) fn failure_text(message: &str) -> String {
    let (cause, next) = failure_advice(message);
    format!("{cause}. {next}")
}

/// One palette for every TUI screen (wizard, Wiki, progress).
pub(crate) mod theme {
    use ratatui::style::{Color, Modifier, Style};
    pub const ACCENT: Color = Color::Cyan;
    /// Headings and the focused item: accent, bold.
    pub const TITLE: Style = Style::new().fg(ACCENT).add_modifier(Modifier::BOLD);
    pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    pub const OK: Color = Color::Green;
    pub const WARN: Color = Color::Yellow;
    pub const ERR: Color = Color::Red;
}

/// The shared TUI frame: one-line header, body, responsive footer, panels, and
/// centered modals. Every full-screen Loom view draws through here so the
/// wizard, `loom wiki`, and progress screens look like one program.
pub(crate) mod chrome {
    use super::theme::{ACCENT, ERR, TITLE};
    use ratatui::layout::{Alignment, Constraint, Layout, Rect};
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph, Wrap};
    use ratatui::Frame;
    use unicode_width::UnicodeWidthStr;

    pub const MIN_WIDTH: u16 = 40;
    pub const MIN_HEIGHT: u16 = 10;

    /// Header, body, footer. Returns `None` after drawing a resize notice when
    /// the terminal is too small.
    pub fn frame_areas(frame: &mut Frame, fallback: &str) -> Option<[Rect; 3]> {
        let area = frame.area();
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled("loom needs a little more room", Style::new().bold()),
                    Line::from(format!(
                        "Resize to at least {MIN_WIDTH} columns by {MIN_HEIGHT} rows."
                    )),
                    Line::from(fallback.to_owned()),
                ])
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
                area,
            );
            return None;
        }
        Some(
            Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(if area.width < 70 { 2 } else { 1 }),
            ])
            .areas(area),
        )
    }

    /// A breadcrumb step: label plus whether it is done.
    pub struct Crumb {
        pub label: String,
        pub done: bool,
    }

    /// ` loom <command>` on the left; on the right either the full
    /// `✓ Done › Current › Next` trail or, when narrow, `step 2/4 · Current`.
    /// `status` (for example `3 picked`) sits before the trail.
    pub fn header(
        frame: &mut Frame,
        area: Rect,
        command: &str,
        status: Vec<Span<'static>>,
        crumbs: &[Crumb],
        current: usize,
    ) {
        let brand = Line::from(Span::styled(format!(" loom {command}"), TITLE));
        let available = usize::from(area.width).saturating_sub(brand.width() + 2);
        let mut trail = Vec::new();
        for (index, crumb) in crumbs.iter().enumerate() {
            if index > 0 {
                trail.push(Span::styled(" › ", Style::new().dim()));
            }
            trail.push(if index == current {
                Span::styled(crumb.label.clone(), TITLE)
            } else if crumb.done {
                Span::styled(format!("✓ {}", crumb.label), Style::new().dim())
            } else {
                Span::styled(crumb.label.clone(), Style::new().dim())
            });
        }
        let mut trail = Line::from(trail);
        if area.width < 80 || trail.width() > available {
            trail = Line::default();
            if let Some(crumb) = crumbs.get(current) {
                let step = format!("step {}/{} · ", current + 1, crumbs.len());
                if crumbs.len() > 1 && step.width() + crumb.label.width() <= available {
                    trail.push_span(Span::styled(step, Style::new().dim()));
                }
                trail.push_span(Span::styled(crumb.label.clone(), TITLE));
            }
        }
        let status = Line::from(status);
        if status.width() + trail.width() <= available {
            trail.spans.splice(0..0, status.spans);
        }
        trail.push_span(" ");
        let [brand_area, _, trail_area] = Layout::horizontal([
            Constraint::Length(brand.width() as u16),
            Constraint::Min(1),
            Constraint::Length(trail.width().min(available + 1) as u16),
        ])
        .areas(area);
        frame.render_widget(Paragraph::new(brand), brand_area);
        frame.render_widget(
            Paragraph::new(trail).alignment(Alignment::Right),
            trail_area,
        );
    }

    pub const BACK_WIDTH: u16 = 11;
    pub const NEXT_WIDTH: u16 = 13;

    /// Key hints beside the buttons, or above them in a narrow terminal.
    /// Returns hit rectangles. Missing Back and disabled Next have no hit target;
    /// disabled Next remains visible but dim.
    pub fn footer(
        frame: &mut Frame,
        area: Rect,
        hint: &str,
        back: Option<&str>,
        next: (&str, bool),
    ) -> (Rect, Rect) {
        let back_label = back.map_or(String::new(), |label| format!("[ ◂ {label:<4} ]"));
        let next_label = format!("[ {:^7} ▸ ]", next.0);
        let [hint_row, button_row] = if area.height > 1 {
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area)
        } else {
            [area, area]
        };
        let [hint_area, back_area, _, next_area, _] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(back_label.width() as u16),
            Constraint::Length(if back.is_some() { 1 } else { 0 }),
            Constraint::Length(next_label.width() as u16),
            Constraint::Length(1),
        ])
        .areas(button_row);
        frame.render_widget(
            Paragraph::new(Span::styled(hint, Style::new().dim())),
            if area.height > 1 { hint_row } else { hint_area },
        );
        let back_hit = if back.is_some() {
            frame.render_widget(
                Paragraph::new(Span::styled(back_label, Style::new().fg(ACCENT))),
                back_area,
            );
            back_area
        } else {
            Rect::default()
        };
        let (_, enabled) = next;
        let style = if enabled {
            Style::new()
                .fg(ACCENT)
                .add_modifier(Modifier::REVERSED)
                .bold()
        } else {
            Style::new().dim()
        };
        frame.render_widget(Paragraph::new(Span::styled(next_label, style)), next_area);
        (back_hit, if enabled { next_area } else { Rect::default() })
    }

    /// Columns a `panel` takes from its area: two borders plus padding.
    /// Subtract this from the panel width to get the content width.
    pub const PANEL_FRAME: u16 = 4;

    /// A rounded, titled panel; accent when focused, dim otherwise. Content
    /// always gets one column of horizontal padding.
    pub fn panel(title: &str, focused: bool) -> Block<'_> {
        let style = if focused {
            Style::new().fg(ACCENT)
        } else {
            Style::new().dim()
        };
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(style)
            .title_style(if focused { style.bold() } else { style })
            .title(title.to_owned())
            .padding(Padding::horizontal(1))
    }

    /// A centered question over the current screen. `danger` is the key that
    /// commits (drawn red), `safe` the key that backs out (drawn accent).
    pub fn confirm_modal(
        frame: &mut Frame,
        title: &str,
        body: Vec<Line<'static>>,
        danger: (&str, &str),
        safe: (&str, &str),
    ) {
        let width = (body.iter().map(Line::width).max().unwrap_or(0).max(40) + PANEL_FRAME as usize)
            .min(frame.area().width.saturating_sub(4) as usize) as u16;
        let content_width = width.saturating_sub(PANEL_FRAME).max(1);
        let mut danger = Line::from(vec![
            Span::styled(danger.0.to_owned(), Style::new().fg(ERR).bold()),
            Span::raw(format!(" {}", danger.1)),
        ]);
        let safe = Line::from(vec![
            Span::styled(safe.0.to_owned(), TITLE),
            Span::raw(format!(" {}", safe.1)),
        ]);
        let actions = if danger.width() + safe.width() + 3 <= usize::from(content_width) {
            danger.push_span("   ");
            danger.spans.extend(safe.spans);
            vec![danger]
        } else {
            vec![danger, safe]
        };
        let actions = Paragraph::new(actions)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        let body = Paragraph::new(body)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        let action_height = actions.line_count(content_width) as u16;
        let height = (body.line_count(content_width) as u16 + action_height + 3)
            .min(frame.area().height.saturating_sub(2));
        let area = frame
            .area()
            .centered(Constraint::Length(width), Constraint::Length(height));
        let block = panel(title, true);
        let [body_area, _, actions_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(action_height),
        ])
        .areas(block.inner(area));
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(body, body_area);
        frame.render_widget(actions, actions_area);
    }
}

const LABEL_WIDTH: usize = 20;
use theme::SPINNER;
const ASCII_SPINNER: [&str; 4] = ["-", "\\", "|", "/"];

pub struct Out {
    terminal: bool,
    color: bool,
    ascii: bool,
}

impl Out {
    pub fn detect() -> Self {
        let term_is_dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
        let terminal = std::io::stdout().is_terminal();
        Self {
            terminal,
            color: terminal && std::env::var_os("NO_COLOR").is_none() && !term_is_dumb,
            ascii: term_is_dumb,
        }
    }

    /// No color, plain newlines: for tests and captured output.
    #[cfg(test)]
    pub fn plain() -> Self {
        Self {
            terminal: false,
            color: false,
            ascii: false,
        }
    }

    fn line_ending(&self) -> &'static str {
        // A raw-mode terminal (ratatui just exited) needs the carriage return.
        if self.terminal {
            "\r\n"
        } else {
            "\n"
        }
    }

    pub fn line(&self, value: impl AsRef<str>) {
        print!("{}{}", value.as_ref(), self.line_ending());
    }

    pub fn blank(&self) {
        self.line("");
    }

    fn paint(&self, code: &str, value: impl AsRef<str>) -> String {
        if self.color {
            format!("\x1b[{code}m{}\x1b[0m", value.as_ref())
        } else {
            value.as_ref().to_owned()
        }
    }

    pub fn accent(&self, value: impl AsRef<str>) -> String {
        self.paint("1;36", value)
    }

    pub fn good(&self, value: impl AsRef<str>) -> String {
        self.paint("32", value)
    }

    pub fn warn(&self, value: impl AsRef<str>) -> String {
        self.paint("33", value)
    }

    pub fn bold(&self, value: impl AsRef<str>) -> String {
        self.paint("1", value)
    }

    pub fn muted(&self, value: impl AsRef<str>) -> String {
        self.paint("2", value)
    }

    pub fn mark(&self, mark: Mark) -> String {
        match (self.ascii, mark) {
            (true, Mark::Ok) => "OK".into(),
            (true, Mark::Off) => "-".into(),
            (_, Mark::Ok) => self.paint("1;32", "✓"),
            (_, Mark::Off) => self.paint("33", "○"),
            (_, Mark::Bad) => self.paint("1;31", "!"),
        }
    }

    /// `loom <command>  <context>` then a blank line.
    pub fn title(&self, command: &str, context: impl AsRef<str>) {
        self.line(format!(
            "{}  {}",
            self.accent(format!("loom {command}")),
            self.muted(context)
        ));
        self.blank();
    }

    pub fn section(&self, title: &str) {
        self.line(self.bold(title));
    }

    /// `  ✓ label   detail` with the label padded to one column.
    pub fn row(&self, mark: Mark, label: &str, detail: impl AsRef<str>) {
        let detail = detail.as_ref();
        let padded = format!("{label:<LABEL_WIDTH$}");
        if detail.is_empty() {
            self.line(format!("  {} {}", self.mark(mark), padded.trim_end()));
        } else {
            self.line(format!("  {} {padded}  {detail}", self.mark(mark)));
        }
    }

    /// A dim continuation line under a row.
    pub fn note(&self, text: impl AsRef<str>) {
        self.line(format!(
            "    {}{}",
            " ".repeat(LABEL_WIDTH + 2),
            self.muted(text)
        ));
    }

    /// The one-line verdict, after a blank line.
    pub fn verdict(&self, ok: bool, text: impl AsRef<str>) {
        self.blank();
        let mark = if ok { Mark::Ok } else { Mark::Bad };
        self.line(format!("{} {}", self.mark(mark), self.bold(text)));
    }

    /// What to do now, after the verdict.
    pub fn next(&self, text: impl AsRef<str>) {
        self.line(format!("  {} {}", self.accent("next"), text.as_ref()));
    }

    /// A dim aside after the verdict; information, not an action.
    pub fn hint(&self, text: impl AsRef<str>) {
        self.line(format!("  {}", self.muted(text)));
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    fn progress_spinner(&self, frame: usize) -> &'static str {
        if self.ascii {
            ASCII_SPINNER[frame % ASCII_SPINNER.len()]
        } else {
            SPINNER[frame % SPINNER.len()]
        }
    }

    /// One animated, in-place status line while work runs.
    /// Captured output stays static so logs never fill with animation frames.
    pub fn progress(&self, text: impl AsRef<str>, frame: usize) {
        use std::io::Write;
        if self.terminal {
            print!(
                "\r\x1b[2K  {} {}",
                self.accent(self.progress_spinner(frame)),
                self.muted(text.as_ref())
            );
            let _ = std::io::stdout().flush();
        } else {
            self.line(format!("  ... {}", text.as_ref()));
        }
    }

    /// Clear the status line before the report rows take its place.
    pub fn progress_done(&self) {
        use std::io::Write;
        if self.terminal {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
        }
    }
}

/// A path with the home directory folded to `~`.
pub fn tidy_path(path: &Path, home: &Path) -> String {
    path.strip_prefix(home).map_or_else(
        |_| path.display().to_string(),
        |relative| format!("~/{}", relative.display()),
    )
}

pub fn print_plan(out: &Out, plan: &InstallPlan) {
    out.section("Plan");
    for step in &plan.steps {
        out.row(Mark::Off, step.target.as_str(), step.operation.display());
    }
}

pub fn confirm_plan() -> Result<bool> {
    Confirm::new("Run this plan?")
        .with_default(false)
        .prompt()
        .context("confirmation was cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_keeps_command_and_current_step_separate_while_scanning() {
        use ratatui::{backend::TestBackend, text::Span, Terminal};
        let crumbs =
            ["Choose", "Where", "Responses", "Review", "Install"].map(|label| chrome::Crumb {
                label: label.into(),
                done: false,
            });
        for width in [40, 50, 70, 80, 100, 120] {
            let mut terminal = Terminal::new(TestBackend::new(width, 10)).unwrap();
            terminal
                .draw(|frame| {
                    let [header, _, _] = chrome::frame_areas(frame, "").unwrap();
                    chrome::header(
                        frame,
                        header,
                        "uninstall",
                        vec![
                            Span::raw("scanning installed…   "),
                            Span::raw("123 picked   "),
                        ],
                        &crumbs,
                        2,
                    );
                })
                .unwrap();
            let header = terminal.backend().buffer().content[..width as usize]
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(header.starts_with(" loom uninstall "), "{width}: {header}");
            assert!(header.contains("Responses"), "{width}: {header}");
            assert!(
                !header.contains(env!("CARGO_PKG_VERSION")),
                "{width}: {header}"
            );
        }
    }

    #[test]
    fn narrow_confirmation_wraps_the_warning_and_keeps_both_actions() {
        use ratatui::{backend::TestBackend, text::Line, Terminal};
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal
            .draw(|frame| {
                chrome::confirm_modal(
                    frame,
                    " Cancel install? ",
                    vec![Line::from(
                        "Cancel the running install? Completed changes stay in place.",
                    )],
                    ("ctrl-c", "cancel"),
                    ("wait", "continue installing"),
                )
            })
            .unwrap();
        let output = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for text in [
            "Cancel the running install?",
            "Completed changes stay in place.",
            "ctrl-c",
            "continue installing",
        ] {
            assert!(output.contains(text), "missing {text}: {output}");
        }
    }

    #[test]
    fn command_failures_keep_causes_but_never_echo_private_output() {
        for (diagnostic, expected) in [
            ("ETIMEDOUT", "timed out"),
            ("EACCES", "Permission was denied"),
            ("ENOSPC", "disk space"),
            ("ENOTFOUND", "could not be reached"),
            ("cancelled while running command", "Cancelled"),
            ("unrecognized failure", "did not complete"),
        ] {
            for stderr in [true, false] {
                let private = format!("{diagnostic}\nAuthorization: Bearer SECRET\nhttps://user:password@host/private\nprivate note\x1b]0;title\x07");
                let result = crate::CommandResult {
                    success: false,
                    stdout: if stderr {
                        String::new()
                    } else {
                        private.clone()
                    },
                    stderr: if stderr { private } else { String::new() },
                };
                let message = crate::install::command_failure_message(&result);
                assert!(message.contains(expected), "{message}");
                for secret in ["SECRET", "password", "private note", "\x1b"] {
                    assert!(!message.contains(secret), "{message}");
                }
                assert_eq!(failure_text(&message), message);
            }
        }
        let empty = crate::CommandResult {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert!(
            crate::install::command_failure_message(&empty).contains("without an error message")
        );
    }

    #[test]
    fn terminal_rows_end_hard_and_plain_rows_soft() {
        let interactive = Out {
            terminal: true,
            color: true,
            ascii: false,
        };
        assert_eq!(interactive.line_ending(), "\r\n");
        assert_eq!(Out::plain().line_ending(), "\n");
    }

    #[test]
    fn plain_output_carries_no_escape_codes() {
        let out = Out::plain();
        assert_eq!(out.accent("x"), "x");
        assert_eq!(out.mark(Mark::Bad), "!");
    }

    #[test]
    fn dumb_terminal_marks_are_ascii() {
        let out = Out {
            terminal: true,
            color: false,
            ascii: true,
        };
        assert_eq!(out.mark(Mark::Ok), "OK");
        assert_eq!(out.mark(Mark::Off), "-");
        assert_eq!(out.mark(Mark::Bad), "!");
    }

    #[test]
    fn progress_spinner_animates_and_falls_back_to_ascii() {
        let unicode = Out {
            terminal: true,
            color: true,
            ascii: false,
        };
        let ascii = Out {
            terminal: true,
            color: false,
            ascii: true,
        };
        assert_eq!(unicode.progress_spinner(0), "⠋");
        assert_eq!(unicode.progress_spinner(1), "⠙");
        assert_eq!(ascii.progress_spinner(0), "-");
        assert_eq!(ascii.progress_spinner(1), "\\");
    }

    #[test]
    fn home_folds_to_tilde() {
        let home = Path::new("/Users/me");
        assert_eq!(
            tidy_path(Path::new("/Users/me/.claude/skills"), home),
            "~/.claude/skills"
        );
        assert_eq!(tidy_path(Path::new("/opt/x"), home), "/opt/x");
    }
}
