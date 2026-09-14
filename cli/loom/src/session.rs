//! Reviewed installation state, retry bookkeeping, and ownership of this run.
use crate::settings::{SettingSpec, SettingsPaths};
use crate::{
    InstallFailure, InstallPlan, InstallReport, PrerequisiteStatus, Resource, ResourceKind,
    SkillAgent, SkillDestination, SkillScope, StepStatus, System,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct InstallSession {
    pub plan: InstallPlan,
    pub settings: Vec<SettingSpec>,
    pub paths: SettingsPaths,
    pub completed: Vec<usize>,
    pub written: Vec<String>,
    pub ownership: Option<InstallOwnership>,
}

impl InstallSession {
    pub fn new(plan: InstallPlan, settings: Vec<SettingSpec>, paths: SettingsPaths) -> Self {
        Self {
            plan,
            settings,
            paths,
            completed: Vec::new(),
            written: Vec::new(),
            ownership: None,
        }
    }

    pub fn run_attempt(
        &mut self,
        system: &(dyn System + Sync),
        cancelled: &AtomicBool,
        observer: &mut dyn FnMut(usize, StepStatus),
    ) -> InstallReport {
        let mut completed = self.completed.clone();
        let mut prepared_tools = false;
        let mut observe = |index: usize, status: StepStatus| {
            prepared_tools |= status == StepStatus::Prepared
                && matches!(
                    self.plan.steps[index].operation,
                    crate::Operation::Tools { .. }
                );
            match &status {
                StepStatus::Prepared | StepStatus::Installed => {
                    if !completed.contains(&index) {
                        completed.push(index);
                    }
                }
                _ => completed.retain(|i| *i != index),
            }
            observer(index, status);
        };
        let mut report = crate::install::execute_attempt(
            &self.plan,
            system,
            cancelled,
            &self.completed,
            &mut observe,
        );
        self.apply_settings(&mut report, cancelled, &mut observe);
        completed.sort_unstable();
        self.completed = completed;
        self.absorb(&report);
        if prepared_tools && !self.written.iter().any(|target| target == "tools") {
            self.written.push("tools".into());
        }
        report
    }

    fn apply_settings(
        &self,
        report: &mut InstallReport,
        cancelled: &AtomicBool,
        observer: &mut dyn FnMut(usize, StepStatus),
    ) {
        let plan_steps = self.plan.steps.len();
        for (offset, spec) in self.settings.iter().enumerate() {
            let index = plan_steps + offset;
            if cancelled.load(Ordering::Relaxed) {
                observer(index, StepStatus::Skipped("cancelled".into()));
                report.failures.push(InstallFailure {
                    target: spec.id.clone(),
                    message: "cancelled".into(),
                });
                continue;
            }
            if self.completed.contains(&index)
                && crate::settings::setting_state(spec, &self.paths)
                    == crate::settings::SettingState::Applied
            {
                if self.written.contains(&spec.id) {
                    report.installed.push(spec.id.clone());
                }
                observer(index, StepStatus::Installed);
                continue;
            }
            let related_install_failed = spec.related_resource.as_ref().is_some_and(|related| {
                self.plan.resources().any(|step| step.target == *related)
                    && !report.installed.contains(related)
            });
            if related_install_failed {
                report.failures.push(InstallFailure {
                    target: spec.id.clone(),
                    message: "Related package failed; retry it before applying this setting".into(),
                });
                observer(index, StepStatus::Skipped("related package failed".into()));
                continue;
            }
            observer(index, StepStatus::Running);
            match crate::settings::apply_setting(spec, &self.paths) {
                Ok(true) => {
                    report.installed.push(spec.id.clone());
                    observer(index, StepStatus::Installed);
                }
                Ok(false) => observer(index, StepStatus::Installed),
                Err(error) => {
                    let message = error.to_string();
                    report.failures.push(InstallFailure {
                        target: spec.id.clone(),
                        message: message.clone(),
                    });
                    observer(index, StepStatus::Failed(message));
                }
            }
        }
    }

    pub fn absorb(&mut self, report: &InstallReport) {
        for target in &report.installed {
            if !self.written.contains(target) {
                self.written.push(target.clone());
            }
        }
    }

    pub fn record_ownership(&self, system: &dyn System) -> Result<(), String> {
        self.ownership
            .as_ref()
            .map_or(Ok(()), |ownership| ownership.record(system, &self.written))
    }
}

/// Captured before interaction or writes; selecting a destination never recaptures it.
pub(crate) struct InstallOwnership {
    resources: Vec<Resource>,
    destination: SkillDestination,
    settings: Vec<SettingSpec>,
    paths: SettingsPaths,
    status: PrerequisiteStatus,
    setting_before: BTreeMap<String, Option<String>>,
    adapters_before: BTreeSet<std::path::PathBuf>,
    skills_before: BTreeSet<std::path::PathBuf>,
}

impl InstallOwnership {
    pub fn capture(
        resources: &[Resource],
        settings: &[SettingSpec],
        paths: &SettingsPaths,
        destination: &SkillDestination,
        status: PrerequisiteStatus,
    ) -> Self {
        let mut ownership = Self {
            resources: resources.to_vec(),
            destination: destination.clone(),
            settings: settings.to_vec(),
            paths: paths.clone(),
            status,
            setting_before: setting_snapshots(settings, paths),
            adapters_before: BTreeSet::new(),
            skills_before: BTreeSet::new(),
        };
        ownership.include_destination(destination);
        ownership
    }

    pub fn include_destination(&mut self, destination: &SkillDestination) {
        if adapter_existed(destination) {
            self.adapters_before
                .insert(destination.opencode_adapter_path());
        }
        self.skills_before
            .extend(existing_skill_paths(&self.resources, destination));
    }

    pub fn select(&mut self, resources: Vec<Resource>, destination: SkillDestination) {
        self.resources = resources;
        self.destination = destination;
    }

    pub fn record(&self, system: &dyn System, written: &[String]) -> Result<(), String> {
        let resources = &self.resources;
        let destination = &self.destination;
        let settings = &self.settings;
        let setting_before = &self.setting_before;
        let settings_paths = &self.paths;
        let adapter_existed = self
            .adapters_before
            .contains(&destination.opencode_adapter_path());
        let skills_before = &self.skills_before;
        let prerequisite_status = self.status;
        use crate::ownership::{
            digest_path, InstallState, OwnedPathKind, OwnedResource, OwnershipScope, Receipt,
        };

        let home = system
            .home_dir()
            .ok_or_else(|| "home directory is unavailable".to_string())?;
        let scope = match destination.scope {
            SkillScope::Global => OwnershipScope::Global,
            SkillScope::Project => OwnershipScope::Project {
                root: destination
                    .project_root
                    .canonicalize()
                    .unwrap_or_else(|_| destination.project_root.clone()),
            },
        };
        let owned_id = |scope: &OwnershipScope, id: &str| match scope {
            OwnershipScope::Global => id.to_owned(),
            OwnershipScope::Project { root } => format!("project:{}:{id}", root.display()),
        };
        let succeeded = |resource: &Resource| {
            written.contains(&resource.id)
                || (resource.kind == ResourceKind::Skill
                    && written.iter().any(|target| target == "skills"))
                || (resource.kind == ResourceKind::Tool
                    && written.iter().any(|target| target == "tools"))
        };
        let mut state = InstallState::load(&home)?;
        for resource in resources.iter().filter(|resource| succeeded(resource)) {
            let resource_scope = if resource.kind == ResourceKind::Skill {
                scope.clone()
            } else {
                OwnershipScope::Global
            };
            let id = owned_id(&resource_scope, &resource.id);
            let mut dependencies = resource
                .dependencies
                .iter()
                .map(|dependency| {
                    let target = resources.iter().find(|candidate| {
                        candidate.id == *dependency || candidate.install_target == *dependency
                    });
                    let scope = if target.is_some_and(|target| target.kind != ResourceKind::Skill) {
                        &OwnershipScope::Global
                    } else {
                        &resource_scope
                    };
                    owned_id(
                        scope,
                        &target.map_or_else(
                            || format!("skill:{dependency}"),
                            |target| target.id.clone(),
                        ),
                    )
                })
                .collect::<Vec<_>>();
            match resource.kind {
                ResourceKind::PiPackage => dependencies.push("tool:pi".into()),
                ResourceKind::HerdrPlugin => dependencies.push("tool:herdr".into()),
                ResourceKind::Tool => dependencies.push("core:mise".into()),
                ResourceKind::Skill | ResourceKind::McpServer => {}
            }
            dependencies.push("core:loom".into());
            dependencies.sort();
            dependencies.dedup();
            let receipts = match resource.kind {
                ResourceKind::McpServer => Vec::new(),
                ResourceKind::Skill => destination
                    .trees()
                    .into_iter()
                    .map(|tree| tree.join(&resource.install_target))
                    .filter(|path| path.is_dir() && !skills_before.contains(path))
                    .map(|path| path.canonicalize().unwrap_or(path))
                    .map(|path| {
                        Ok(Receipt::Path {
                            digest: digest_path(&path)?,
                            path,
                            path_kind: OwnedPathKind::Tree,
                            before: None,
                        })
                    })
                    .collect::<std::result::Result<Vec<_>, String>>()?,
                ResourceKind::PiPackage => vec![Receipt::Manager {
                    manager: "pi".into(),
                    target: resource.pi_install_spec(),
                }],
                ResourceKind::HerdrPlugin => vec![Receipt::Manager {
                    manager: "herdr".into(),
                    target: resource.id.trim_start_matches("herdr-plugin:").into(),
                }],
                ResourceKind::Tool => std::iter::once(resource.install_target.clone())
                    .chain(resource.companions.iter().cloned())
                    .map(|key| Receipt::MiseTool { key })
                    .collect(),
            };
            if !receipts.is_empty() {
                state.record(OwnedResource {
                    id,
                    scope: resource_scope,
                    depends_on: dependencies,
                    receipts,
                });
            }
        }
        if written.iter().any(|target| target == "tools") {
            for (needed, id, key) in [
                (
                    !prerequisite_status.pi
                        && resources
                            .iter()
                            .any(|resource| resource.kind == ResourceKind::PiPackage),
                    "tool:pi",
                    crate::manifest::PI_TOOL_KEY,
                ),
                (
                    !prerequisite_status.herdr
                        && resources
                            .iter()
                            .any(|resource| resource.kind == ResourceKind::HerdrPlugin),
                    "tool:herdr",
                    "herdr",
                ),
            ] {
                if needed {
                    state.record(OwnedResource {
                        id: id.into(),
                        scope: OwnershipScope::Global,
                        depends_on: vec!["core:loom".into(), "core:mise".into()],
                        receipts: vec![Receipt::MiseTool { key: key.into() }],
                    });
                }
            }
        }
        for setting in settings
            .iter()
            .filter(|setting| written.contains(&setting.id))
        {
            let path = setting.target_path(settings_paths).to_path_buf();
            if !path.is_file() {
                continue;
            }
            state.record(OwnedResource {
                id: owned_id(&OwnershipScope::Global, &format!("setting:{}", setting.id)),
                scope: OwnershipScope::Global,
                depends_on: setting
                    .related_resource
                    .iter()
                    .map(|id| id.to_owned())
                    .collect(),
                receipts: vec![Receipt::Path {
                    digest: digest_path(&path)?,
                    path,
                    path_kind: OwnedPathKind::File,
                    before: setting_before.get(&setting.id).cloned().flatten(),
                }],
            });
        }
        if !adapter_existed
            && destination.agents.contains(&SkillAgent::OpenCode)
            && written.iter().any(|target| target == "skills")
        {
            let path = destination.opencode_adapter_path();
            if path.is_file() {
                let path = path.canonicalize().unwrap_or(path);
                state.record(OwnedResource {
                    id: owned_id(&scope, "adapter:opencode"),
                    scope: scope.clone(),
                    depends_on: vec!["core:loom".into()],
                    receipts: vec![Receipt::Path {
                        digest: digest_path(&path)?,
                        path,
                        path_kind: OwnedPathKind::File,
                        before: None,
                    }],
                });
            }
        }
        state.save(&home)
    }
}

fn setting_snapshots(
    settings: &[SettingSpec],
    paths: &SettingsPaths,
) -> BTreeMap<String, Option<String>> {
    settings
        .iter()
        .map(|setting| {
            (
                setting.id.clone(),
                fs::read_to_string(setting.target_path(paths)).ok(),
            )
        })
        .collect()
}

pub(crate) fn existing_skill_paths(
    resources: &[Resource],
    destination: &SkillDestination,
) -> BTreeSet<std::path::PathBuf> {
    destination
        .trees()
        .into_iter()
        .flat_map(|tree| {
            resources
                .iter()
                .filter(|resource| resource.kind == ResourceKind::Skill)
                .map(move |resource| tree.join(&resource.install_target))
        })
        .filter(|path| path.symlink_metadata().is_ok())
        .collect()
}

pub(crate) fn adapter_existed(destination: &SkillDestination) -> bool {
    destination.agents.contains(&SkillAgent::OpenCode)
        && destination
            .opencode_adapter_path()
            .symlink_metadata()
            .is_ok()
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
