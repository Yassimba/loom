//! Per-Vault work uses the regular installer and the existing reviewed Wiki setup.
use super::state::{ExecStatus, InstallEvent, Model, Wizard};
use super::wiki::{Capability, WikiBrowser};
use crate::wiki::{VaultRecord, WikiOperation, WikiOutcome, WikiRequest};
use crate::{InstallPlan, Resource, SkillAgent, SkillDestination, SkillScope, System};
use anyhow::Result;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Clone)]
pub(super) struct WikiInstall {
    pub record: VaultRecord,
    pub request: Option<WikiRequest>,
    pub labels: Vec<String>,
    pub resources: Vec<Resource>,
    pub plan: InstallPlan,
    pub destination: SkillDestination,
}

impl WikiBrowser {
    pub(super) fn jobs(&self, model: &Model) -> Result<Vec<WikiInstall>> {
        self.vaults
            .iter()
            .filter(|record| !self.selected(record).is_empty())
            .map(|record| {
                let selected = self.selected(record);
                let operation = self
                    .pending
                    .get(&record.path)
                    .and_then(|p| p.operation.clone())
                    .unwrap_or(WikiOperation::Repair);
                let feynman = record.feynman || selected.contains(&Capability::Feynman);
                let confluence = record.confluence || selected.contains(&Capability::Confluence);
                let qmd = record.qmd || selected.contains(&Capability::Qmd);
                let needs_wiki = selected.iter().any(|c| !matches!(c, Capability::Skill(_)));
                let request = needs_wiki.then(|| WikiRequest {
                    operation,
                    vault: record.path.clone(),
                    feynman,
                    confluence,
                    qmd,
                    yes: false,
                });
                let direct = selected
                    .iter()
                    .filter_map(|capability| {
                        let Capability::Skill(name) = capability else {
                            return None;
                        };
                        Some(
                            model
                                .resources
                                .iter()
                                .find(|r| {
                                    r.kind == crate::ResourceKind::Skill
                                        && r.install_target == *name
                                })
                                .cloned()
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Knowledgebase skill is missing from the catalog: {name}"
                                    )
                                }),
                        )
                    })
                    .collect::<Result<Vec<_>>>()?;
                let destination = SkillDestination {
                    home: model.skill_destination.home.clone(),
                    project_root: record.path.clone(),
                    scope: SkillScope::Project,
                    agents: vec![SkillAgent::AgentsStandard],
                };
                let resources =
                    crate::expand_skill_dependencies(&model.resources, direct, &destination.agents);
                let plan = crate::build_install_plan(
                    &resources,
                    model.status,
                    model.platform,
                    &destination,
                )?;
                Ok(WikiInstall {
                    record: VaultRecord {
                        path: record.path.clone(),
                        feynman,
                        confluence,
                        qmd,
                    },
                    request,
                    labels: selected.iter().map(|c| c.label().to_owned()).collect(),
                    resources,
                    plan,
                    destination,
                })
            })
            .collect()
    }
}

impl Wizard {
    pub(super) fn wiki_jobs(&self) -> Result<Vec<WikiInstall>> {
        self.wiki
            .as_ref()
            .map(|browser| browser.jobs(&self.model))
            .unwrap_or_else(|| Ok(Vec::new()))
    }
}

impl WikiInstall {
    pub fn id(&self) -> String {
        format!("wiki:{}", self.record.path.display())
    }

    pub fn label(&self) -> String {
        format!(
            "Knowledgebase · {}",
            self.record
                .path
                .file_name()
                .unwrap_or(self.record.path.as_os_str())
                .to_string_lossy()
        )
    }

    pub fn present(&self, system: &(dyn System + Sync), cancelled: &AtomicBool) -> bool {
        crate::wiki::WikiRegistry::load(&self.destination.home).is_ok_and(|registry| {
            registry.vaults.iter().any(|record| {
                record.path == self.record.path
                    && (!self.record.feynman || record.feynman)
                    && (!self.record.confluence || record.confluence)
                    && (!self.record.qmd || record.qmd)
            })
        }) && (self.request.is_none() || crate::wiki::inspect_vault(system, &self.record).healthy)
            && self
                .plan
                .prerequisites
                .iter()
                .chain(&self.plan.resources)
                .all(|step| crate::install::step_is_present(step, system, cancelled))
    }

    pub fn run(
        &self,
        system: &(dyn System + Sync),
        cancelled: &AtomicBool,
        index: usize,
        sender: &mpsc::Sender<InstallEvent>,
        paths: &crate::settings::SettingsPaths,
    ) -> Result<String> {
        anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "cancelled");
        let mut notes = Vec::new();
        let path = &self.record.path;
        let registry = crate::wiki::WikiRegistry::load(&self.destination.home)?;
        let create = self
            .request
            .as_ref()
            .is_some_and(|request| request.operation == WikiOperation::Create);
        anyhow::ensure!(
            crate::wiki::absolute_vault_target(system, path, create)? == *path,
            "Knowledgebase path changed since review: {}",
            path.display()
        );
        if path.exists() {
            anyhow::ensure!(
                path.symlink_metadata()?.is_dir() && path.canonicalize()? == *path,
                "Knowledgebase path moved or is no longer a real directory: {}",
                path.display()
            );
        }
        if let Some(request) = &self.request {
            let mut request = request.clone();
            if let Some(record) = registry.vaults.iter().find(|record| record.path == *path) {
                request.feynman |= record.feynman;
                request.confluence |= record.confluence;
                request.qmd |= record.qmd;
            }
            crate::wiki_progress::in_setup(
                system,
                cancelled,
                &|label| {
                    let _ = sender.send(InstallEvent::Detail(index, label.to_owned()));
                },
                |system| {
                    let mut confirm = |title: &str, changes: &[String]| {
                        let (reply, answer) = mpsc::channel();
                        let mut lines =
                            vec![format!("Knowledgebase: {}", path.display()), String::new()];
                        lines.extend_from_slice(changes);
                        sender.send(InstallEvent::Confirm(title.into(), lines, reply))?;
                        loop {
                            anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "cancelled");
                            match answer.recv_timeout(Duration::from_millis(120)) {
                                Ok(yes) => return Ok(yes),
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(error) => return Err(error.into()),
                            }
                        }
                    };
                    let result = crate::wiki::setup_with_confirmation(
                        &request,
                        system,
                        Some(&mut confirm),
                        cancelled,
                        &mut notes,
                    )?;
                    anyhow::ensure!(
                        result == WikiOutcome::Finished(true),
                        "File plan declined; completed work stays. Retry to continue."
                    );
                    Ok(())
                },
            )?;
        }
        anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "cancelled");
        anyhow::ensure!(
            path.is_dir(),
            "Knowledgebase is missing; not recreated: {}",
            path.display()
        );
        let before = crate::app::existing_skill_paths(&self.resources, &self.destination);
        let _ = sender.send(InstallEvent::Detail(
            index,
            "Installing selected skills in this Wiki".into(),
        ));
        let mut tools_installed = false;
        let mut report = crate::execute_install_plan_with_control(
            &self.plan,
            system,
            cancelled,
            &mut |index, status| {
                if matches!(
                    status,
                    crate::StepStatus::Prepared | crate::StepStatus::Installed
                ) && self
                    .plan
                    .prerequisites
                    .iter()
                    .chain(&self.plan.resources)
                    .nth(index)
                    .is_some_and(|step| matches!(step.action, crate::StepAction::SyncTools { .. }))
                {
                    tools_installed = true;
                }
            },
        );
        if tools_installed {
            report.installed.push("tools".into());
        }
        crate::app::record_install_ownership(
            system,
            &self.resources,
            &self.destination,
            &[],
            &BTreeMap::new(),
            paths,
            false,
            &before,
            crate::PrerequisiteStatus {
                pi: true,
                herdr: true,
                mise: true,
            },
            &report,
        )
        .map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            report.failures.is_empty(),
            "{}",
            report
                .failures
                .iter()
                .map(|f| format!("{}: {}", f.target, f.message))
                .collect::<Vec<_>>()
                .join("; ")
        );
        let _ = sender.send(InstallEvent::Status(index, ExecStatus::Verifying));
        anyhow::ensure!(
            self.present(system, cancelled),
            "Knowledgebase verification failed; completed work stays. Retry to continue."
        );
        notes.push("Installed in this Wiki; accounts not verified.".into());
        Ok(notes.join(" "))
    }
}
