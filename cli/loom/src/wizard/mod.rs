//! The interactive setup wizard: a multi-stage, GUI-style ratatui app that
//! selects resources, runtimes, and settings, then runs the install live in
//! the terminal.

mod choose;
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

use crate::session::InstallOwnership;
use crate::{InstallFailure, SkillAgent, SkillDestination, SkillScope, StepStatus, System};
use anyhow::Result;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::DefaultTerminal;
use std::io::IsTerminal;
use std::sync::mpsc;

pub fn run_wizard(model: Model, system: &(dyn System + Sync)) -> Result<WizardOutcome> {
    anyhow::ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "interactive setup needs a terminal: open a new shell and run `loom setup`, or pass --skill, --pi-package, or --herdr-plugin (skills also accept --agent and --scope)"
    );
    let installing = model.purpose == WizardPurpose::Install;
    let ownership = installing.then(|| {
        let mut settings = model.settings.clone();
        settings.push(crate::settings::pi_adhd_setting());
        let mut before = InstallOwnership::capture(
            &model.resources,
            &settings,
            &model.settings_paths,
            &model.skill_destination,
            model.status,
        );
        for scope in [SkillScope::Global, SkillScope::Project] {
            before.include_destination(&SkillDestination {
                scope,
                agents: SkillAgent::ALL.to_vec(),
                ..model.skill_destination.clone()
            });
        }
        before
    });
    let mut browser = wiki::WikiBrowser::default();
    if installing {
        browser.load(&model.skill_destination.home);
    }
    let mut wizard = Wizard::new(model, browser);
    wizard.probing = installing;
    let mut outcome = ratatui::run(|terminal| {
        let _ = execute!(std::io::stdout(), EnableMouseCapture);
        let outcome = run_loop(terminal, &mut wizard, system, ownership);
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        outcome
    });
    if let Ok(WizardOutcome::Installed { report, .. }) = &mut outcome {
        if let Some(job) = &wizard.reviewed_job {
            if let Err(message) = job.session.record_ownership(system) {
                report.failures.push(InstallFailure {
                    target: "ownership ledger".into(),
                    message,
                });
            }
        }
    }
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
    mut ownership: Option<InstallOwnership>,
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
                let global_dest = SkillDestination {
                    scope: SkillScope::Global,
                    ..destination.clone()
                };
                let project_dest = SkillDestination {
                    scope: SkillScope::Project,
                    ..destination
                };
                let (global, project) = std::thread::scope(|probes| {
                    let global = probes.spawn(|| {
                        crate::app::detect_installed(&resources, status, system, &global_dest)
                    });
                    let project = probes.spawn(|| {
                        crate::app::detect_installed(&resources, status, system, &project_dest)
                    });
                    (
                        global.join().expect("global install probe"),
                        project.join().expect("project install probe"),
                    )
                });
                let _ = probe_sender.send((global, project));
            });
        }
        let mut dirty = true;
        loop {
            while let Ok(install_event) = receiver.try_recv() {
                if let InstallEvent::Confirm(title, lines, reply) = install_event {
                    let result = crate::wiki_tui::confirm_in(terminal, &title, &lines);
                    let _ = reply.send(result.as_ref().is_ok_and(|yes| *yes));
                    result?;
                } else {
                    wizard.handle_install_event(install_event);
                }
                dirty = true;
            }
            while let Ok((global, project)) = probe_receiver.try_recv() {
                wizard.set_installed_scoped(global, project);
                dirty = true;
            }
            while let Ok((path, health)) = wiki_receiver.try_recv() {
                wizard.wiki.health.insert(path, health);
                wizard.wiki.checking = false;
                dirty = true;
            }
            if wizard.browsing_wiki() {
                if let Some(record) = wizard.wiki.next_probe() {
                    let sender = wiki_sender.clone();
                    scope.spawn(move || {
                        let health = crate::wiki::inspect_vault(system, &record);
                        let _ = sender.send((record.path, health));
                    });
                }
            }
            if dirty || wizard.needs_animate() {
                terminal.draw(|frame| wizard.draw(frame))?;
                dirty = false;
            }
            if !event::poll(wizard.poll_timeout())? {
                if wizard.needs_animate() {
                    wizard.tick();
                }
                continue;
            }
            let action = match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    dirty = true;
                    wizard.handle_key(key)
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        dirty = true;
                        wizard.handle_click(mouse.column, mouse.row)
                    }
                    MouseEventKind::ScrollDown => {
                        dirty = true;
                        wizard.handle_scroll(true);
                        None
                    }
                    MouseEventKind::ScrollUp => {
                        dirty = true;
                        wizard.handle_scroll(false);
                        None
                    }
                    _ => None,
                },
                Event::Resize(_, _) => {
                    dirty = true;
                    None
                }
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
                    {
                        let browser = &mut wizard.wiki;
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
                    wizard.choose.focus = state::Pane::Kinds;
                }
                Some(Action::UnregisterWiki(path)) => {
                    let home = wizard.model.skill_destination.home.clone();
                    let result = crate::wiki::WikiRegistry::load(&home).and_then(|mut registry| {
                        registry.unregister(&path);
                        registry.save(&home)
                    });
                    {
                        let browser = &mut wizard.wiki;
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
                    let mut job = wizard.begin_install()?;
                    if let Some(mut before) = ownership.take() {
                        before.select(wizard.expanded_selection(), wizard.skill_destination());
                        job.session.ownership = Some(before);
                    }
                    let sender = sender.clone();
                    scope.spawn(move || run_install_job(job, system, &sender));
                }
                None => {}
            }
        }
    })
}

/// The worker owns the reviewed session until the attempt finishes.
fn run_install_job(
    mut job: InstallJob,
    system: &(dyn System + Sync),
    sender: &mpsc::Sender<InstallEvent>,
) {
    let completed = job.session.completed.clone();
    let plan_steps = job.session.plan.steps.len();
    let mcp_steps = job
        .session
        .plan
        .steps
        .iter()
        .map(|step| matches!(step.operation, crate::Operation::Mcp { .. }))
        .collect::<Vec<_>>();
    let mut report = job
        .session
        .run_attempt(system, &job.cancelled, &mut |index, status| {
            let status = wizard_status(index, plan_steps, &mcp_steps, &completed, status);
            let _ = sender.send(InstallEvent::Status(index, status));
        });
    for (offset, wiki) in job.wikis.iter().enumerate() {
        let index = plan_steps + job.session.settings.len() + offset;
        let result = if job.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            Err(anyhow::anyhow!("cancelled"))
        } else if completed.contains(&index) && wiki.present(system, &job.cancelled) {
            Ok("Already installed in this Wiki; verified.".into())
        } else {
            let _ = sender.send(InstallEvent::Status(index, ExecStatus::Running));
            wiki.run(system, &job.cancelled, index, sender, &job.session.paths)
        };
        let status = match result {
            Ok(note) => {
                report.installed.push(wiki.id());
                if !job.session.completed.contains(&index) {
                    job.session.completed.push(index);
                }
                let _ = sender.send(InstallEvent::Detail(index, note.clone()));
                ExecStatus::Ok(note)
            }
            Err(error) => {
                job.session.completed.retain(|i| *i != index);
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
    job.session.absorb(&report);
    let _ = sender.send(InstallEvent::Finished(Box::new(job), report));
}

fn wizard_status(
    index: usize,
    plan_steps: usize,
    mcp_steps: &[bool],
    completed: &[usize],
    status: StepStatus,
) -> ExecStatus {
    match status {
        StepStatus::Running => ExecStatus::Running,
        StepStatus::Verifying => ExecStatus::Verifying,
        StepStatus::Prepared | StepStatus::Installed => ExecStatus::Ok(
            if index < plan_steps {
                if completed.contains(&index) {
                    "already installed; verified"
                } else if mcp_steps[index] {
                    "configured; live health not checked"
                } else {
                    "installed"
                }
            } else if completed.contains(&index) {
                "already set; verified"
            } else {
                "saved"
            }
            .into(),
        ),
        StepStatus::Failed(message) => ExecStatus::Failed(message),
        StepStatus::Skipped(message) => ExecStatus::Skipped(message),
    }
}
