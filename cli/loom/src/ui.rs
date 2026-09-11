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
    use ratatui::style::Color;
    pub const ACCENT: Color = Color::Cyan;
    pub const OK: Color = Color::Green;
    pub const WARN: Color = Color::Yellow;
    pub const ERR: Color = Color::Red;
}

/// The shared TUI frame: one-line header, body, one-line footer, panels, and
/// centered modals. Every full-screen Loom view draws through here so the
/// wizard, `loom wiki`, and progress screens look like one program.
pub(crate) mod chrome {
    use super::theme::{ACCENT, ERR};
    use ratatui::layout::{Alignment, Constraint, Layout, Rect};
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph, Wrap};
    use ratatui::Frame;

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
                Constraint::Length(1),
            ])
            .areas(area),
        )
    }

    /// A breadcrumb step: label plus whether it is done.
    pub struct Crumb {
        pub label: String,
        pub done: bool,
    }

    /// ` loom <command>  vX.Y.Z` on the left; on the right either the full
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
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" loom {command}"), Style::new().fg(ACCENT).bold()),
                Span::styled(
                    concat!("  v", env!("CARGO_PKG_VERSION")),
                    Style::new().dim(),
                ),
            ])),
            area,
        );
        let mut spans = status;
        if area.width < 80 {
            spans.push(Span::styled(
                format!("step {}/{} · ", current + 1, crumbs.len()),
                Style::new().dim(),
            ));
            if let Some(crumb) = crumbs.get(current) {
                spans.push(Span::styled(
                    crumb.label.clone(),
                    Style::new().fg(ACCENT).bold(),
                ));
            }
        } else {
            for (index, crumb) in crumbs.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::styled(" › ", Style::new().dim()));
                }
                spans.push(if index == current {
                    Span::styled(crumb.label.clone(), Style::new().fg(ACCENT).bold())
                } else if crumb.done {
                    Span::styled(format!("✓ {}", crumb.label), Style::new().dim())
                } else {
                    Span::styled(crumb.label.clone(), Style::new().dim())
                });
            }
        }
        spans.push(Span::raw(" "));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
            area,
        );
    }

    pub const BACK_WIDTH: u16 = 11;
    pub const NEXT_WIDTH: u16 = 13;

    /// Dim key hint on the left, `[ ◂ Back ]  [ Next ▸ ]` on the right.
    /// Returns the back and next button rects for mouse hit testing; a
    /// disabled button is drawn dim and returns an empty rect.
    pub fn footer(
        frame: &mut Frame,
        area: Rect,
        hint: &str,
        back: Option<(&str, bool)>,
        next: (&str, bool),
    ) -> (Rect, Rect) {
        let [hint_area, back_area, _, next_area, _] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(if back.is_some() { BACK_WIDTH } else { 0 }),
            Constraint::Length(if back.is_some() { 1 } else { 0 }),
            Constraint::Length(NEXT_WIDTH),
            Constraint::Length(1),
        ])
        .areas(area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(hint, Style::new().dim()))),
            hint_area,
        );
        let mut back_hit = Rect::default();
        if let Some((label, enabled)) = back {
            let style = if enabled {
                Style::new().fg(ACCENT)
            } else {
                Style::new().dim()
            };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(format!("[ ◂ {label:<4} ]"), style))),
                back_area,
            );
            if enabled {
                back_hit = back_area;
            }
        }
        let (label, enabled) = next;
        let style = if enabled {
            Style::new()
                .fg(ACCENT)
                .add_modifier(Modifier::REVERSED)
                .bold()
        } else {
            Style::new().dim()
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!("[ {label:^7} ▸ ]"), style))),
            next_area,
        );
        (back_hit, if enabled { next_area } else { Rect::default() })
    }

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

    pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
        Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width: width.min(area.width),
            height: height.min(area.height),
        }
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
        let width = body
            .iter()
            .map(|line| line.width() as u16)
            .max()
            .unwrap_or(0)
            .max(40)
            + 4;
        let mut lines = body;
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(danger.0.to_owned(), Style::new().fg(ERR).bold()),
            Span::raw(format!(" {}   ", danger.1)),
            Span::styled(safe.0.to_owned(), Style::new().fg(ACCENT).bold()),
            Span::raw(format!(" {}", safe.1)),
        ]));
        let area = centered(
            frame.area(),
            width.min(frame.area().width.saturating_sub(4)),
            lines.len() as u16 + 2,
        );
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .block(panel(title, true)),
            area,
        );
    }
}

const LABEL_WIDTH: usize = 20;
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
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
    for step in plan.prerequisites.iter().chain(&plan.resources) {
        out.row(Mark::Off, step.target.as_str(), step.action.display());
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
