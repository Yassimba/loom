//! Wiki execution owns a terminal only between reviewed Vault/auth prompts.
use crate::{CommandResult, CommandSpec, System};
use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph, Wrap};
use ratatui::Frame;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ACCENT: Color = Color::Cyan;
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

struct ProgressSystem<'a> {
    system: &'a (dyn System + Sync),
    cancelled: &'a AtomicBool,
    stage: Mutex<&'static str>,
    bytes: AtomicUsize,
}

fn stage(command: &CommandSpec) -> &'static str {
    match command.program.as_str() {
        "qmd" if command.args.iter().any(|a| a == "pull") => {
            "Downloading search models (may total about 2 GB)"
        }
        "qmd" if command.args.iter().any(|a| a == "embed") => "Building search embeddings",
        "qmd" if command.args.iter().any(|a| a == "update") => "Indexing Vault Markdown",
        "qmd" if command.args.iter().any(|a| a == "query") => {
            "First-search check (up to 120 seconds)"
        }
        "qmd" => "Preparing Vault search",
        "pi" => "Installing Vault-local Pi packages",
        "mise" => "Installing selected tools, including QMD",
        _ => "Preparing setup prerequisites",
    }
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
        *self.stage.lock().unwrap() = stage(command);
        self.bytes.store(0, Ordering::Relaxed);
        // Arbitrary installer output can include URLs, credentials and document text.
        // Stream activity, never its contents: no ANSI/OSC, CR, partial lines or secrets
        // can reach the terminal. Atomic counters keep progress memory bounded.
        self.system
            .run_streamed(command, timeout, self.cancelled, None, &|chunk| {
                self.bytes.fetch_add(chunk.len(), Ordering::Relaxed);
            })
            .map_err(|_| anyhow::anyhow!("{} failed, timed out or was cancelled", stage(command)))
    }
}

fn readable_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn draw_progress(
    frame: &mut Frame,
    stage: &str,
    elapsed: Duration,
    bytes: usize,
    confirm_cancel: bool,
) {
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
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Setting up Vault ",
            Style::new().fg(ACCENT).bold(),
        ))
        .alignment(Alignment::Right),
        header,
    );

    let width = 64.min(body.width.saturating_sub(4));
    let height = 9.min(body.height);
    let panel_area = Rect::new(
        body.x + body.width.saturating_sub(width) / 2,
        body.y + body.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let spinner = if std::env::var("TERM").is_ok_and(|term| term == "dumb") {
        "."
    } else {
        SPINNER[(elapsed.as_millis() / 120 % SPINNER.len() as u128) as usize]
    };
    let status = if confirm_cancel {
        Line::styled(
            "Cancel setup? Completed work will stay in place.",
            Style::new().fg(Color::Yellow),
        )
    } else {
        Line::from(vec![
            Span::styled(format!("{spinner} "), Style::new().fg(ACCENT).bold()),
            Span::styled(stage.to_owned(), Style::new().bold()),
        ])
    };
    frame.render_widget(
        Paragraph::new(vec![
            status,
            Line::from(""),
            Line::styled(
                format!(
                    "{}s elapsed  ·  {} activity",
                    elapsed.as_secs(),
                    readable_bytes(bytes)
                ),
                Style::new().dim(),
            ),
            Line::from(""),
            Line::styled(
                "Tool output stays hidden while setup runs.",
                Style::new().dim(),
            ),
        ])
        .wrap(Wrap { trim: true })
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(ACCENT))
                .title_style(Style::new().fg(ACCENT).bold())
                .title(" Progress ")
                .padding(Padding::uniform(1)),
        ),
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
    work: impl FnOnce(&dyn System, &AtomicBool) -> Result<T> + Send,
) -> Result<T> {
    if !interactive {
        return work(system, &AtomicBool::new(false));
    }
    let cancelled = AtomicBool::new(false);
    let progress = ProgressSystem {
        system,
        cancelled: &cancelled,
        stage: Mutex::new("Preparing setup"),
        bytes: AtomicUsize::new(0),
    };
    let mut terminal = ratatui::init();
    let result = std::thread::scope(|scope| {
        let worker = scope.spawn(|| work(&progress, &cancelled));
        let started = Instant::now();
        let ui = (|| -> Result<()> {
            let mut confirm_cancel = false;
            while !worker.is_finished() {
                terminal.draw(|frame| {
                    draw_progress(
                        frame,
                        *progress.stage.lock().unwrap(),
                        started.elapsed(),
                        progress.bytes.load(Ordering::Relaxed),
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
                    "Building search embeddings",
                    Duration::from_secs(12),
                    2048,
                    false,
                )
            })
            .unwrap();

        let screen = screen(&terminal);
        assert!(screen.contains("loom wiki"));
        assert!(screen.contains("Building search embeddings"));
        assert!(screen.contains("12s elapsed  ·  2.0 KB activity"));
        assert!(screen.contains("esc or ctrl-c cancels"));
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
        assert_eq!(stage(&command), "First-search check (up to 120 seconds)");
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
            run(&Fake, false, |_, cancelled| {
                assert!(!cancelled.load(Ordering::Relaxed));
                Ok(42)
            })
            .unwrap(),
            42
        );
    }
}
