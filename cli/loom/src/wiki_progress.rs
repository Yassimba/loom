//! Wiki execution owns a terminal only between reviewed Vault/auth prompts.
use crate::{CommandResult, CommandSpec, System};
use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::widgets::{Block, Paragraph};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
            while !worker.is_finished() {
                terminal.draw(|frame| {
                    let spinner = ["|", "/", "-", "\\"][(started.elapsed().as_millis() / 120 % 4) as usize];
                    let text = format!("{spinner} {}\n\nElapsed: {}s\nTool output received: {} bytes (contents hidden)\n\nExact progress unavailable. Esc / Ctrl-C cancels.\nCancellation stops work; completed installs remain.",
                        *progress.stage.lock().unwrap(), started.elapsed().as_secs(), progress.bytes.load(Ordering::Relaxed));
                    frame.render_widget(Paragraph::new(text).block(Block::bordered().title(" Wiki setup ")), frame.area());
                })?;
                if event::poll(Duration::from_millis(120))? {
                    if let Event::Key(key) = event::read()? {
                        if key.code == KeyCode::Esc
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL))
                        {
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
