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
