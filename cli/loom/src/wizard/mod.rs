//! The interactive setup wizard: a multi-stage, GUI-style ratatui app that
//! selects resources, runtimes, and settings, then runs the install live in
//! the terminal.

mod install_view;
mod render;
mod review;
mod state;
#[cfg(test)]
mod tests;
mod wiki;
mod wiki_install;

use state::{Action, ExecStatus, InstallEvent, InstallJob, Wizard};
pub use state::{Model, WizardOutcome, WizardPurpose};

use crate::settings::apply_setting;
use crate::{
    execute_install_plan_with_control, InstallFailure, SkillDestination, SkillScope, StepStatus,
    System,
};
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
    if wizard.model.purpose == WizardPurpose::Install {
        let mut browser = wiki::WikiBrowser::default();
        browser.load(&wizard.model.skill_destination.home);
        wizard.wiki = Some(browser);
    }
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
    let (wiki_sender, wiki_receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        // Probe installed state in the background so the first frame is
        // instant; marks fill in when the managers have answered.
        if wizard.model.purpose == WizardPurpose::Install {
            let resources = wizard.model.resources.clone();
            let status = wizard.model.status;
            let destination = wizard.skill_destination();
            let probe_sender = probe_sender.clone();
            scope.spawn(move || {
                let probe = |scope| {
                    let destination = SkillDestination {
                        scope,
                        ..destination.clone()
                    };
                    crate::app::detect_installed(&resources, status, system, &destination)
                };
                let _ = probe_sender.send((probe(SkillScope::Global), probe(SkillScope::Project)));
            });
        }
        loop {
            while let Ok(install_event) = receiver.try_recv() {
                if let InstallEvent::Confirm(title, lines, reply) = install_event {
                    let result = crate::wiki_tui::confirm_in(terminal, &title, &lines);
                    let _ = reply.send(result.as_ref().is_ok_and(|yes| *yes));
                    result?;
                } else {
                    wizard.handle_install_event(install_event);
                }
            }
            while let Ok((global, project)) = probe_receiver.try_recv() {
                wizard.set_installed_scoped(global, project);
            }
            while let Ok((path, health)) = wiki_receiver.try_recv() {
                if let Some(browser) = &mut wizard.wiki {
                    browser.health.insert(path, health);
                    browser.checking = false;
                }
            }
            if wizard.browsing_wiki() {
                if let Some(record) = wizard.wiki.as_mut().and_then(wiki::WikiBrowser::next_probe) {
                    let sender = wiki_sender.clone();
                    scope.spawn(move || {
                        let health = crate::wiki::inspect_vault(system, &record);
                        let _ = sender.send((record.path, health));
                    });
                }
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
                Some(Action::PickWiki(operation)) => {
                    let _ = execute!(std::io::stdout(), DisableMouseCapture);
                    ratatui::restore();
                    let current = system
                        .current_dir()
                        .unwrap_or_else(|| wizard.model.skill_destination.project_root.clone());
                    let result = crate::wiki_tui::pick_vault_path(system, &operation, &current)
                        .and_then(|path| {
                            path.map(|path| {
                                crate::wiki::absolute_vault_target(
                                    system,
                                    &path,
                                    operation == crate::wiki::WikiOperation::Create,
                                )
                            })
                            .transpose()
                        });
                    *terminal = ratatui::init();
                    let _ = execute!(std::io::stdout(), EnableMouseCapture);
                    if let Some(browser) = &mut wizard.wiki {
                        match result {
                            Ok(Some(path)) => browser.picked_path(operation, path),
                            Ok(None) => {
                                browser.message =
                                    Some("Cancelled; your setup picks are unchanged.".into())
                            }
                            Err(error) => {
                                browser.message = Some(crate::ui::failure_text(&error.to_string()))
                            }
                        }
                    }
                    if let state::Stage::Choose(stage) = &mut wizard.stages[wizard.stage_index] {
                        stage.focus = state::Pane::Kinds;
                    }
                }
                Some(Action::UnregisterWiki(path)) => {
                    let home = wizard.model.skill_destination.home.clone();
                    let result = crate::wiki::WikiRegistry::load(&home).and_then(|mut registry| {
                        registry.unregister(&path);
                        registry.save(&home)
                    });
                    if let Some(browser) = &mut wizard.wiki {
                        match result {
                            Ok(()) => {
                                browser.load(&home);
                                browser.message = Some(format!(
                                    "Unregistered {}; files were not changed.",
                                    path.display()
                                ));
                            }
                            Err(error) => {
                                browser.message = Some(crate::ui::failure_text(&error.to_string()))
                            }
                        }
                    }
                }
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
    let mut report = crate::InstallReport::default();
    let mut pending = crate::InstallPlan::default();
    let mut indices = Vec::new();
    for (index, step) in job
        .plan
        .prerequisites
        .iter()
        .chain(&job.plan.resources)
        .enumerate()
    {
        if job.completed.contains(&index) {
            let _ = sender.send(InstallEvent::Status(index, ExecStatus::Verifying));
            if crate::install::step_is_present(step, system, &job.cancelled) {
                let _ = sender.send(InstallEvent::Status(
                    index,
                    ExecStatus::Ok("already installed; verified".into()),
                ));
                if index >= job.plan.prerequisites.len() || step.target == "tools" {
                    report.installed.push(step.target.clone());
                }
                continue;
            }
        }
        indices.push(index);
        if index < job.plan.prerequisites.len() {
            pending.prerequisites.push(step.clone());
        } else {
            pending.resources.push(step.clone());
        }
    }
    let attempt = execute_install_plan_with_control(
        &pending,
        system,
        &job.cancelled,
        &mut |index, status| {
            let index = indices[index];
            let status = match status {
                StepStatus::Running => ExecStatus::Running,
                StepStatus::Verifying => ExecStatus::Verifying,
                StepStatus::Prepared | StepStatus::Installed => {
                    let step = job
                        .plan
                        .prerequisites
                        .iter()
                        .chain(&job.plan.resources)
                        .nth(index);
                    if step.is_some_and(|step| step.target == "tools") {
                        report.installed.push("tools".into());
                    }
                    ExecStatus::Ok(
                        if step.is_some_and(|s| s.target.starts_with("mcp-server:")) {
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
    report.installed.extend(attempt.installed);
    report.failures.extend(attempt.failures);
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
        if job.completed.contains(&index)
            && crate::settings::setting_state(spec, &job.paths)
                == crate::settings::SettingState::Applied
        {
            if job.previously_installed.contains(&spec.id) {
                report.installed.push(spec.id.clone());
            }
            let _ = sender.send(InstallEvent::Status(
                index,
                ExecStatus::Ok("already set; verified".into()),
            ));
            continue;
        }
        let related_install_failed = spec.related_resource.as_ref().is_some_and(|related| {
            job.plan
                .resources
                .iter()
                .any(|step| step.target == *related)
                && !report.installed.contains(related)
        });
        if related_install_failed {
            report.failures.push(InstallFailure {
                target: spec.id.clone(),
                message: "Related package failed; retry it before applying this setting".into(),
            });
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
    for (offset, wiki) in job.wikis.iter().enumerate() {
        let index = plan_steps + job.settings.len() + offset;
        let result = if job.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            Err(anyhow::anyhow!("cancelled"))
        } else if job.completed.contains(&index) && wiki.present(system, &job.cancelled) {
            Ok("Already installed in this Wiki; verified.".into())
        } else {
            let _ = sender.send(InstallEvent::Status(index, ExecStatus::Running));
            wiki.run(system, &job.cancelled, index, sender, &job.paths)
        };
        let status = match result {
            Ok(note) => {
                report.installed.push(wiki.id());
                let _ = sender.send(InstallEvent::Detail(index, note.clone()));
                ExecStatus::Ok(note)
            }
            Err(error) => {
                let message = error.to_string();
                report.failures.push(InstallFailure {
                    target: wiki.id(),
                    message: message.clone(),
                });
                ExecStatus::Failed(message)
            }
        };
        let _ = sender.send(InstallEvent::Status(index, status));
    }
    let _ = sender.send(InstallEvent::Done(report));
}
