//! The interactive setup wizard: a multi-stage, GUI-style ratatui app that
//! selects resources, runtimes, and settings, then runs the install live in
//! the terminal.

mod render;
mod state;
#[cfg(test)]
mod tests;

use state::{Action, ExecStatus, InstallEvent, InstallJob, Wizard};
pub use state::{Model, WizardOutcome, WizardPurpose};

use crate::settings::apply_setting;
use crate::{execute_install_plan_with_control, InstallFailure, StepStatus, System};
use anyhow::Result;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::DefaultTerminal;
use std::io::IsTerminal;
use std::sync::mpsc;
use std::time::Duration;

pub fn run_wizard(model: Model, system: &(dyn System + Sync)) -> Result<WizardOutcome> {
    anyhow::ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "interactive setup needs a terminal: open a new shell and run `loom setup`, or pass --skill, --pi-package, or --herdr-plugin (skills also accept --agent and --scope)"
    );
    let mut wizard = Wizard::new(model);
    wizard.probing = wizard.model.purpose == WizardPurpose::Install;
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let outcome = run_loop(&mut terminal, &mut wizard, system);
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    outcome.map_err(terminal_error)
}

/// crossterm's input-reader failure means the terminal could not be polled
/// (a redirected stdin, a pane without a controlling terminal). Name the way
/// out instead of echoing the library's internals.
fn terminal_error(error: anyhow::Error) -> anyhow::Error {
    if error.to_string().contains("input reader") {
        anyhow::anyhow!(
            "loom could not read your terminal. Open a new shell and run `loom setup` directly (not through a pipe)."
        )
    } else {
        error
    }
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    wizard: &mut Wizard,
    system: &(dyn System + Sync),
) -> Result<WizardOutcome> {
    let (sender, receiver) = mpsc::channel();
    let (probe_sender, probe_receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        // Probe installed state in the background so the first frame is
        // instant; marks fill in when the managers have answered.
        if wizard.model.purpose == WizardPurpose::Install {
            let resources = wizard.model.resources.clone();
            let status = wizard.model.status;
            let destination = wizard.skill_destination();
            scope.spawn(move || {
                let _ = probe_sender.send(crate::app::detect_installed(
                    &resources,
                    status,
                    system,
                    &destination,
                ));
            });
        }
        loop {
            while let Ok(install_event) = receiver.try_recv() {
                wizard.handle_install_event(install_event);
            }
            while let Ok(installed) = probe_receiver.try_recv() {
                wizard.set_installed(installed);
            }
            terminal.draw(|frame| wizard.draw(frame))?;
            if !event::poll(Duration::from_millis(120))? {
                wizard.tick();
                continue;
            }
            let action = match event::read()? {
                Event::Key(key) => wizard.handle_key(key),
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        wizard.handle_click(mouse.column, mouse.row)
                    }
                    MouseEventKind::ScrollDown => {
                        wizard.handle_scroll(true);
                        None
                    }
                    MouseEventKind::ScrollUp => {
                        wizard.handle_scroll(false);
                        None
                    }
                    _ => None,
                },
                _ => None,
            };
            match action {
                Some(Action::Exit(outcome)) => return Ok(outcome),
                Some(Action::StartInstall) => {
                    let job = wizard.begin_install()?;
                    let sender = sender.clone();
                    scope.spawn(move || run_install_job(job, system, &sender));
                }
                None => {}
            }
        }
    })
}

/// Runs on the worker thread: plan steps first, then settings, then Done.
fn run_install_job(
    job: InstallJob,
    system: &(dyn System + Sync),
    sender: &mpsc::Sender<InstallEvent>,
) {
    let plan_steps = job.plan.prerequisites.len() + job.plan.resources.len();
    let mut report = execute_install_plan_with_control(
        &job.plan,
        system,
        &job.cancelled,
        &mut |index, status| {
            let status = match status {
                StepStatus::Running => ExecStatus::Running,
                StepStatus::Prepared | StepStatus::Installed => {
                    let step = job
                        .plan
                        .prerequisites
                        .iter()
                        .chain(&job.plan.resources)
                        .nth(index);
                    ExecStatus::Ok(
                        if step.is_some_and(|s| s.target == "mcp-server:sem") {
                            "configured; live health not checked"
                        } else {
                            "installed"
                        }
                        .into(),
                    )
                }
                StepStatus::Failed(message) => ExecStatus::Failed(message),
                StepStatus::Skipped(message) => ExecStatus::Skipped(message),
            };
            let _ = sender.send(InstallEvent::Status(index, status));
        },
    );
    for (offset, spec) in job.settings.iter().enumerate() {
        if job.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = sender.send(InstallEvent::Status(
                plan_steps + offset,
                ExecStatus::Skipped("cancelled".into()),
            ));
            report.failures.push(InstallFailure {
                target: spec.id.clone(),
                message: "cancelled".into(),
            });
            continue;
        }
        let index = plan_steps + offset;
        let related_install_failed = spec.related_resource.as_ref().is_some_and(|related| {
            job.plan
                .resources
                .iter()
                .any(|step| step.target == *related)
                && !report.installed.contains(related)
        });
        if related_install_failed {
            let _ = sender.send(InstallEvent::Status(
                index,
                ExecStatus::Skipped("related package failed".into()),
            ));
            continue;
        }
        let _ = sender.send(InstallEvent::Status(index, ExecStatus::Running));
        let status = match apply_setting(spec, &job.paths) {
            Ok(true) => {
                report.installed.push(spec.id.clone());
                ExecStatus::Ok("saved".into())
            }
            Ok(false) => ExecStatus::Ok("already set".into()),
            Err(error) => {
                let message = error.to_string();
                report.failures.push(InstallFailure {
                    target: spec.id.clone(),
                    message: message.clone(),
                });
                ExecStatus::Failed(message)
            }
        };
        let _ = sender.send(InstallEvent::Status(index, status));
    }
    let _ = sender.send(InstallEvent::Done(report));
}
