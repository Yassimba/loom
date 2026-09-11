//! Wiki execution owns a terminal only between reviewed Vault/auth prompts.
use crate::{CommandResult, CommandSpec, System};
use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::ui::chrome::{self, Crumb};
use crate::ui::theme::SPINNER;
use crate::ui::theme::{ACCENT, OK, WARN};

struct ProgressSystem<'a> {
    system: &'a (dyn System + Sync),
    cancelled: &'a AtomicBool,
    progress: Mutex<ProgressState>,
    observer: Option<&'a (dyn Fn(&str) + Sync)>,
}

impl<'a> ProgressSystem<'a> {
    fn new(
        system: &'a (dyn System + Sync),
        cancelled: &'a AtomicBool,
        observer: Option<&'a (dyn Fn(&str) + Sync)>,
    ) -> Self {
        Self {
            system,
            cancelled,
            observer,
            progress: Mutex::new(ProgressState {
                current: "Preparing setup",
                started: Instant::now(),
                last_output: Instant::now(),
                completed: Vec::new(),
            }),
        }
    }
}

/// Forward trusted stage labels to the parent chooser; it retains its terminal.
pub(crate) fn in_setup<T>(
    system: &(dyn System + Sync),
    cancelled: &AtomicBool,
    observer: &(dyn Fn(&str) + Sync),
    work: impl FnOnce(&(dyn System + Sync)) -> Result<T>,
) -> Result<T> {
    work(&ProgressSystem::new(system, cancelled, Some(observer)))
}

#[derive(Clone)]
struct ProgressState {
    current: &'static str,
    started: Instant,
    last_output: Instant,
    completed: Vec<(&'static str, Duration)>,
}

#[derive(Clone, Copy)]
struct Stage {
    active: &'static str,
    completed: Option<&'static str>,
}

fn stage(command: &CommandSpec) -> Stage {
    let (active, completed) = match command.program.as_str() {
        "qmd" if command.args.iter().any(|a| a == "pull") => (
            "Downloading search models (may total about 2 GB)",
            Some("Search models ready"),
        ),
        "qmd" if command.args.iter().any(|a| a == "embed") => (
            "Building search embeddings",
            Some("Search embeddings built"),
        ),
        "qmd" if command.args.iter().any(|a| a == "update") => {
            ("Indexing Vault Markdown", Some("Vault Markdown indexed"))
        }
        "qmd" if command.args.iter().any(|a| a == "query") => (
            "First-search check (up to 120 seconds)",
            Some("First search checked"),
        ),
        "qmd" => ("Preparing Vault search", None),
        "pi" if command.args.first().is_some_and(|arg| arg == "list") => (
            "Verifying Vault-local Pi packages",
            Some("Vault packages verified"),
        ),
        "pi" if command
            .args
            .iter()
            .any(|arg| arg.contains("@companion-ai/feynman")) =>
        {
            (
                "Installing Feynman in this Vault",
                Some("Feynman installed"),
            )
        }
        "pi" => (
            "Installing claude-obsidian in this Vault",
            Some("claude-obsidian installed"),
        ),
        "mise" if command.args.first().is_some_and(|arg| arg == "where") => {
            ("Locating the pinned Wiki runtime", None)
        }
        "mise" if command.args.iter().any(|arg| arg == "doctor") => (
            "Checking Vault configuration",
            Some("Vault configuration checked"),
        ),
        "mise" => (
            "Installing selected tools, including QMD",
            Some("Selected tools installed"),
        ),
        _ => ("Preparing setup prerequisites", None),
    };
    Stage { active, completed }
}

impl System for ProgressSystem<'_> {
    fn command_exists(&self, name: &str) -> bool {
        self.system.command_exists(name)
    }
    fn refresh_path(&self) {
        self.system.refresh_path();
    }
    fn github_token(&self) -> Option<String> {
        self.system.github_token()
    }
    fn home_dir(&self) -> Option<PathBuf> {
        self.system.home_dir()
    }
    fn current_dir(&self) -> Option<PathBuf> {
        self.system.current_dir()
    }
    fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
        self.run_controlled(
            command,
            crate::system::MANAGER_COMMAND_TIMEOUT,
            self.cancelled,
        )
    }
    fn run_controlled(
        &self,
        command: &CommandSpec,
        timeout: Duration,
        _: &AtomicBool,
    ) -> Result<CommandResult> {
        anyhow::ensure!(
            !self.cancelled.load(Ordering::Relaxed),
            "Wiki setup cancelled"
        );
        let stage = stage(command);
        if let Some(observer) = self.observer {
            observer(stage.active);
        }
        {
            let mut progress = self.progress.lock().unwrap();
            progress.current = stage.active;
            progress.started = Instant::now();
            progress.last_output = progress.started;
        }
        // Arbitrary installer output can include URLs, credentials and document text.
        // Stream activity, never its contents: no ANSI/OSC, CR, partial lines or secrets
        // can reach the terminal. Retain only the last activity time.
        let result = self
            .system
            .run_streamed(command, timeout, self.cancelled, None, &|_| {
                self.progress.lock().unwrap().last_output = Instant::now();
            });
        if result.as_ref().is_ok_and(|result| result.success) {
            if let Some(completed) = stage.completed {
                let mut progress = self.progress.lock().unwrap();
                let duration = progress.started.elapsed();
                progress.completed.push((completed, duration));
                if progress.completed.len() > 3 {
                    progress.completed.remove(0);
                }
            }
        }
        result.map_err(|error| {
            let message = error.to_string();
            if message.contains("timed out") {
                anyhow::anyhow!(
                    "{} timed out after {}s. Completed work stays; check the connection and retry.",
                    stage.active,
                    timeout.as_secs()
                )
            } else {
                anyhow::anyhow!("{}: {}", stage.active, crate::ui::failure_text(&message))
            }
        })
    }
}

fn draw_progress(
    frame: &mut Frame,
    activity: &str,
    stage: &str,
    completed: &[(&str, Duration)],
    elapsed: Duration,
    quiet_seconds: u64,
    confirm_cancel: bool,
) {
    let Some([header, body, footer]) = chrome::frame_areas(frame, "") else {
        return;
    };
    let crumbs = [Crumb {
        label: activity.to_owned(),
        done: false,
    }];
    chrome::header(frame, header, "wiki", Vec::new(), &crumbs, 0);

    let width = 64.min(body.width.saturating_sub(4));
    let height = (10 + completed.len().min(3) as u16).min(body.height);
    let panel_area = chrome::centered(body, width, height);
    let spinner = if std::env::var("TERM").is_ok_and(|term| term == "dumb") {
        "."
    } else {
        SPINNER[(elapsed.as_millis() / 120 % SPINNER.len() as u128) as usize]
    };
    let status = if confirm_cancel {
        Line::styled(
            "Cancel Wiki work? Completed work will stay in place.",
            Style::new().fg(WARN),
        )
    } else {
        Line::from(vec![
            Span::styled(format!("{spinner} "), Style::new().fg(ACCENT).bold()),
            Span::styled(stage.to_owned(), Style::new().bold()),
        ])
    };
    let mut lines = vec![
        status,
        Line::from(""),
        Line::styled(
            format!(
                "{}s elapsed · last tool activity {}s ago",
                elapsed.as_secs(),
                quiet_seconds
            ),
            Style::new().dim(),
        ),
    ];
    if !completed.is_empty() {
        lines.push(Line::from(""));
        lines.extend(completed.iter().map(|(label, duration)| {
            Line::styled(
                format!("✓ {label} · {}s", duration.as_secs()),
                Style::new().fg(OK),
            )
        }));
    }
    lines.extend([
        Line::from(""),
        Line::styled(
            "Tool output stays hidden while setup runs.",
            Style::new().dim(),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(chrome::panel(" Progress ", true)),
        panel_area,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            if confirm_cancel {
                " press esc or ctrl-c again to cancel · any other key keeps working"
            } else {
                " esc or ctrl-c cancels · completed work stays"
            },
            Style::new().dim(),
        )),
        footer,
    );
}

fn handle_cancel_key(confirm_cancel: &mut bool, key: ratatui::crossterm::event::KeyEvent) -> bool {
    let cancel_key = key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
    let cancel = cancel_key && *confirm_cancel;
    *confirm_cancel = cancel_key;
    cancel
}

pub(crate) fn run<T: Send>(
    system: &(dyn System + Sync),
    interactive: bool,
    activity: &str,
    work: impl FnOnce(&dyn System, &AtomicBool) -> Result<T> + Send,
) -> Result<T> {
    if !interactive {
        return work(system, &AtomicBool::new(false));
    }
    let cancelled = AtomicBool::new(false);
    let progress = ProgressSystem::new(system, &cancelled, None);
    let mut terminal = ratatui::init();
    let result = std::thread::scope(|scope| {
        let worker = scope.spawn(|| work(&progress, &cancelled));
        let started = Instant::now();
        let ui = (|| -> Result<()> {
            let mut confirm_cancel = false;
            while !worker.is_finished() {
                terminal.draw(|frame| {
                    let state = progress.progress.lock().unwrap().clone();
                    draw_progress(
                        frame,
                        activity,
                        state.current,
                        &state.completed,
                        started.elapsed(),
                        state.last_output.elapsed().as_secs(),
                        confirm_cancel,
                    );
                })?;
                if event::poll(Duration::from_millis(120))? {
                    if let Event::Key(key) = event::read()? {
                        if handle_cancel_key(&mut confirm_cancel, key) {
                            cancelled.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
            Ok(())
        })();
        if ui.is_err() {
            cancelled.store(true, Ordering::Relaxed);
        }
        let result = worker
            .join()
            .map_err(|_| anyhow::anyhow!("Wiki setup worker failed"));
        ui?;
        anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Wiki setup cancelled");
        result?
    });
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn screen(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn progress_screen_is_compact_and_names_the_active_work() {
        let mut terminal = Terminal::new(TestBackend::new(80, 18)).unwrap();
        terminal
            .draw(|frame| {
                draw_progress(
                    frame,
                    "Vault 1/3 · second-brain",
                    "Building search embeddings",
                    &[
                        ("Vault Markdown indexed", Duration::from_secs(4)),
                        ("Search models ready", Duration::from_secs(2)),
                    ],
                    Duration::from_secs(12),
                    5,
                    false,
                )
            })
            .unwrap();

        let screen = screen(&terminal);
        assert!(screen.contains("loom"));
        assert!(screen.contains("Vault 1/3 · second-brain"));
        assert!(screen.contains("Building search embeddings"));
        assert!(screen.contains("Vault Markdown indexed · 4s"));
        assert!(screen.contains("Search models ready · 2s"));
        assert!(screen.contains("12s elapsed · last tool activity 5s ago"));
        assert!(!screen.contains("KB activity"));
        assert!(screen.contains("esc or ctrl-c cancels"));
    }

    #[test]
    fn progress_keeps_the_three_latest_completed_stages() {
        struct Fake;
        impl System for Fake {
            fn command_exists(&self, _: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn run(&self, _: &CommandSpec) -> Result<CommandResult> {
                Ok(CommandResult {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                })
            }
        }
        let cancelled = AtomicBool::new(false);
        let progress = ProgressSystem::new(&Fake, &cancelled, None);

        for command in ["update", "pull", "embed", "query"] {
            progress.run(&CommandSpec::new("qmd", [command])).unwrap();
        }

        let state = progress.progress.lock().unwrap();
        assert_eq!(state.completed.len(), 3);
        assert_eq!(state.completed[0].0, "Search models ready");
        assert_eq!(state.completed[2].0, "First search checked");
    }

    #[test]
    fn cancellation_requires_two_consecutive_cancel_keys() {
        let mut confirm = false;
        let escape = ratatui::crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            ratatui::crossterm::event::KeyModifiers::NONE,
        );
        let other = ratatui::crossterm::event::KeyEvent::new(
            KeyCode::Char('x'),
            ratatui::crossterm::event::KeyModifiers::NONE,
        );

        assert!(!handle_cancel_key(&mut confirm, escape));
        assert!(!handle_cancel_key(&mut confirm, other));
        assert!(!handle_cancel_key(&mut confirm, escape));
        assert!(handle_cancel_key(&mut confirm, escape));
    }

    #[test]
    fn progress_names_never_contain_command_arguments() {
        let command = CommandSpec::new("qmd", ["query", "secret\x1b]0;title\x07\rtext"]);
        assert_eq!(
            stage(&command).active,
            "First-search check (up to 120 seconds)"
        );
    }
    #[test]
    fn scripted_work_has_no_terminal() {
        struct Fake;
        impl System for Fake {
            fn command_exists(&self, _: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn run(&self, _: &CommandSpec) -> Result<CommandResult> {
                unreachable!()
            }
        }
        assert_eq!(
            run(&Fake, false, "Refreshing Wiki Vault", |_, cancelled| {
                assert!(!cancelled.load(Ordering::Relaxed));
                Ok(42)
            })
            .unwrap(),
            42
        );
    }
}
