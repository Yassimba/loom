use crate::{Resource, ResourceKind, System};
use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    Unix,
    Windows,
}

/// The oldest Node.js Pi supports (its npm package declares
/// `engines.node >= 20.6.0`). Bump when Pi does.
pub const PI_MIN_NODE: (u32, u32, u32) = (20, 6, 0);

/// What `node --version` said, reduced to the decision the planner needs.
/// loom never installs or updates Node itself — it detects and instructs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeStatus {
    Missing,
    TooOld(u32, u32, u32),
    Supported,
}

impl NodeStatus {
    pub fn detect(system: &dyn System) -> Self {
        match system.run(&CommandSpec::new("node", ["--version"])) {
            Ok(result) if result.success => match parse_node_version(result.stdout.trim()) {
                Some(version) if version < PI_MIN_NODE => {
                    Self::TooOld(version.0, version.1, version.2)
                }
                // An unparseable version is not evidence of a problem; let
                // npm be the judge rather than blocking the plan.
                _ => Self::Supported,
            },
            _ => Self::Missing,
        }
    }

    /// The warning to surface, if any — `None` means Node needs nothing.
    pub fn warning(self) -> Option<String> {
        let (major, minor, patch) = PI_MIN_NODE;
        match self {
            Self::Supported => None,
            Self::Missing => Some(format!(
                "Node.js is not on PATH; Pi needs {major}.{minor}.{patch} or newer — \
                 install the current LTS from https://nodejs.org or your package manager"
            )),
            Self::TooOld(found_major, found_minor, found_patch) => Some(format!(
                "Node.js {found_major}.{found_minor}.{found_patch} is older than the \
                 {major}.{minor}.{patch} Pi needs — update it with your package manager"
            )),
        }
    }
}

/// Parse `v20.6.0` (or `20.6.0`) into a comparable triple.
fn parse_node_version(raw: &str) -> Option<(u32, u32, u32)> {
    let mut parts = raw.trim_start_matches('v').split('.');
    let mut next = || parts.next()?.parse::<u32>().ok();
    Some((next()?, next()?, next().unwrap_or(0)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrerequisiteStatus {
    pub pi: bool,
    pub herdr: bool,
    pub npm: bool,
    pub mise: bool,
    pub node: NodeStatus,
}

/// A manager the user can also install on its own, without selecting any
/// resource that depends on it. Skills need no runtime: the CLI copies them
/// into the agent trees itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Runtime {
    Pi,
    Herdr,
    Mise,
}

impl Runtime {
    pub fn installed(self, status: PrerequisiteStatus) -> bool {
        match self {
            Self::Pi => status.pi,
            Self::Herdr => status.herdr,
            Self::Mise => status.mise,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationSpec {
    Command {
        command: CommandSpec,
        needle: Option<String>,
    },
}

/// What a plan step does when executed: run a manager command, or copy
/// skills into an exact agent-and-scope destination natively.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepAction {
    Command(CommandSpec),
    CopySkills {
        skills: Vec<String>,
        destination: crate::skills::SkillDestination,
    },
    SyncTools {
        tools: Vec<String>,
    },
}

impl StepAction {
    pub fn display(&self) -> String {
        match self {
            Self::Command(command) => command.display(),
            Self::CopySkills {
                skills,
                destination,
            } => format!(
                "copy skills into {} targets ({}): {}",
                destination.scope.label().to_lowercase(),
                destination.display(),
                skills.join(", ")
            ),
            Self::SyncTools { tools } => format!(
                "add to the mise selection and install: {}",
                tools.join(", ")
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallStep {
    pub target: String,
    pub manager: String,
    pub action: StepAction,
    pub verification: Option<VerificationSpec>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstallPlan {
    pub prerequisites: Vec<InstallStep>,
    pub resources: Vec<InstallStep>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFailure {
    pub target: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub failures: Vec<InstallFailure>,
}

pub fn build_install_plan(
    resources: &[Resource],
    runtimes: &[Runtime],
    status: PrerequisiteStatus,
    platform: Platform,
    skill_destination: &crate::skills::SkillDestination,
) -> Result<InstallPlan> {
    let needs_pi = runtimes.contains(&Runtime::Pi)
        || resources
            .iter()
            .any(|resource| resource.kind == ResourceKind::PiPackage);
    let needs_herdr = runtimes.contains(&Runtime::Herdr)
        || resources
            .iter()
            .any(|resource| resource.kind == ResourceKind::HerdrPlugin);

    // Selected manifest tools install through mise; a missing Pi rides along
    // as a tool when mise is available (the manifest pins it), and only
    // falls back to a global npm install on mise-less machines.
    let mut tools = resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::Tool)
        .flat_map(|tool| {
            std::iter::once(tool.install_target.clone()).chain(tool.companions.iter().cloned())
        })
        .collect::<Vec<_>>();
    let pi_via_mise = needs_pi && !status.pi && status.mise;
    if pi_via_mise && !tools.contains(&crate::manifest::PI_TOOL_KEY.to_string()) {
        tools.push(crate::manifest::PI_TOOL_KEY.into());
    }

    if !(needs_pi && !status.pi && status.mise) && needs_pi {
        anyhow::ensure!(
            status.pi || status.npm,
            "installing Pi needs npm, which is not on PATH; install Node.js first"
        );
        // Same preflight for the Node runtime itself: a too-old Node would
        // make `npm install` fail with an opaque engines error mid-plan.
        if !status.pi {
            if let Some(warning) = status.node.warning() {
                anyhow::bail!("installing Pi is blocked: {warning}");
            }
        }
    }

    let mut prerequisites = Vec::new();
    if !tools.is_empty() && !status.mise {
        prerequisites.push(prerequisite_step(
            "mise",
            platform,
            "curl -fsSL https://mise.run | sh",
            "winget install --id jdx.mise --silent --accept-package-agreements --accept-source-agreements",
        ));
    }
    if !tools.is_empty() {
        // Prerequisite, not a resource step: prerequisites run sequentially,
        // so a Pi arriving via the manifest is on PATH before any pi-package
        // steps run (resource groups execute concurrently per manager).
        prerequisites.push(InstallStep {
            target: "tools".into(),
            manager: "mise".into(),
            action: StepAction::SyncTools {
                tools: tools.clone(),
            },
            // Verified inside the sync itself: `mise install` fails loudly.
            verification: None,
        });
    }
    if needs_pi && !status.pi && !pi_via_mise {
        prerequisites.push(InstallStep {
            target: "Pi".into(),
            manager: "pi".into(),
            action: StepAction::Command(CommandSpec::new(
                "npm",
                ["install", "--global", "@earendil-works/pi-coding-agent"],
            )),
            verification: None,
        });
    }
    if needs_herdr && !status.herdr {
        prerequisites.push(prerequisite_step(
            "herdr",
            platform,
            "curl -fsSL https://herdr.dev/install.sh | sh",
            "irm https://herdr.dev/install.ps1 | iex",
        ));
    }

    let skills = resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::Skill)
        .map(|skill| skill.install_target.clone())
        .collect::<Vec<_>>();
    let mut steps = Vec::new();
    if !skills.is_empty() {
        anyhow::ensure!(
            !skill_destination.agents.is_empty(),
            "installing skills needs at least one selected agent"
        );
        steps.push(InstallStep {
            target: "skills".into(),
            manager: "skills".into(),
            action: StepAction::CopySkills {
                skills,
                destination: skill_destination.clone(),
            },
            // Verified inside the copy itself: each tree must end up with
            // <skill>/SKILL.md.
            verification: None,
        });
    }
    for resource in resources {
        let (manager, command, verification) = match resource.kind {
            ResourceKind::Skill | ResourceKind::Tool => continue,
            ResourceKind::PiPackage => (
                "pi",
                CommandSpec::new("pi", ["install", &resource.pi_install_spec()]),
                VerificationSpec::Command {
                    command: CommandSpec::new("pi", ["list"]),
                    needle: Some(resource.install_target.clone()),
                },
            ),
            ResourceKind::HerdrPlugin => (
                "herdr",
                CommandSpec::new(
                    "herdr",
                    [
                        "plugin",
                        "install",
                        resource.install_target.as_str(),
                        "--yes",
                    ],
                ),
                VerificationSpec::Command {
                    command: CommandSpec::new("herdr", ["plugin", "list"]),
                    needle: Some(resource.id.trim_start_matches("herdr-plugin:").into()),
                },
            ),
        };
        steps.push(InstallStep {
            target: resource.id.clone(),
            manager: manager.into(),
            action: StepAction::Command(command),
            verification: Some(verification),
        });
    }
    Ok(InstallPlan {
        prerequisites,
        resources: steps,
    })
}

/// Progress of one plan step, indexed over prerequisites then resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepStatus {
    Running,
    Prepared,
    Installed,
    Failed(String),
    Skipped(String),
}

pub fn execute_install_plan(plan: &InstallPlan, system: &(dyn System + Sync)) -> InstallReport {
    execute_install_plan_with(plan, system, &mut |_, _| {})
}

pub fn execute_install_plan_with(
    plan: &InstallPlan,
    system: &(dyn System + Sync),
    observer: &mut dyn FnMut(usize, StepStatus),
) -> InstallReport {
    let lanes = install_lanes(plan);
    let mut outcomes = vec![None; plan.prerequisites.len() + plan.resources.len()];
    let (status_sender, statuses) = std::sync::mpsc::channel::<(usize, StepStatus)>();
    let (finish_sender, finishes) = std::sync::mpsc::channel::<(String, LaneOutcome)>();
    let mut handle_status = |index: usize, status: StepStatus| {
        if !matches!(status, StepStatus::Running) {
            outcomes[index] = Some(status.clone());
        }
        observer(index, status);
    };
    std::thread::scope(|scope| {
        let mut waiting = lanes
            .iter()
            .filter(|lane| lane.waits_for.is_some())
            .collect::<Vec<_>>();
        let mut running = 0;
        for lane in lanes.iter().filter(|lane| lane.waits_for.is_none()) {
            let status_sender = status_sender.clone();
            let finish_sender = finish_sender.clone();
            running += 1;
            scope.spawn(move || {
                let outcome = execute_lane(lane, system, &status_sender);
                let _ = finish_sender.send((lane.manager.to_owned(), outcome));
            });
        }

        while running > 0 {
            while let Ok((index, status)) = statuses.try_recv() {
                handle_status(index, status);
            }
            let Ok((manager, outcome)) =
                finishes.recv_timeout(std::time::Duration::from_millis(10))
            else {
                continue;
            };
            running -= 1;

            let mut index = 0;
            while index < waiting.len() {
                if waiting[index].waits_for != Some(manager.as_str()) {
                    index += 1;
                    continue;
                }
                let lane = waiting.swap_remove(index);
                let dependency_failed =
                    outcome.unavailable || (lane.manager == "pi" && !system.command_exists("pi"));
                if dependency_failed {
                    let message = format!("{} is unavailable", display_name(lane.manager));
                    for step in &lane.steps {
                        handle_status(step.index, StepStatus::Skipped(message.clone()));
                    }
                    continue;
                }
                let status_sender = status_sender.clone();
                let finish_sender = finish_sender.clone();
                running += 1;
                scope.spawn(move || {
                    let outcome = execute_lane(lane, system, &status_sender);
                    let _ = finish_sender.send((lane.manager.to_owned(), outcome));
                });
            }
        }
        while let Ok((index, status)) = statuses.try_recv() {
            handle_status(index, status);
        }
    });

    // Fold outcomes in plan order so reports stay deterministic regardless
    // of which manager finished first.
    let mut report = InstallReport::default();
    for (index, outcome) in outcomes.into_iter().enumerate() {
        let (step, resource) = if index < plan.prerequisites.len() {
            (&plan.prerequisites[index], false)
        } else {
            (&plan.resources[index - plan.prerequisites.len()], true)
        };
        match outcome {
            Some(StepStatus::Failed(message) | StepStatus::Skipped(message)) => {
                report.failures.push(InstallFailure {
                    target: step.target.clone(),
                    message,
                });
            }
            Some(StepStatus::Installed) if resource => report.installed.push(step.target.clone()),
            _ => {}
        }
    }
    report
}

#[derive(Clone, Copy)]
enum StepPhase {
    Prerequisite,
    Resource,
}

struct IndexedStep<'a> {
    index: usize,
    step: &'a InstallStep,
    phase: StepPhase,
}

struct InstallLane<'a> {
    manager: &'a str,
    steps: Vec<IndexedStep<'a>>,
    waits_for: Option<&'a str>,
}

fn install_lanes(plan: &InstallPlan) -> Vec<InstallLane<'_>> {
    let mut lanes = Vec::<InstallLane<'_>>::new();
    let steps =
        plan.prerequisites
            .iter()
            .enumerate()
            .map(|(index, step)| (index, step, StepPhase::Prerequisite))
            .chain(plan.resources.iter().enumerate().map(|(offset, step)| {
                (plan.prerequisites.len() + offset, step, StepPhase::Resource)
            }));
    for (index, step, phase) in steps {
        let indexed = IndexedStep { index, step, phase };
        match lanes.iter_mut().find(|lane| lane.manager == step.manager) {
            Some(lane) => lane.steps.push(indexed),
            None => lanes.push(InstallLane {
                manager: &step.manager,
                steps: vec![indexed],
                waits_for: None,
            }),
        }
    }
    let pi_via_mise = plan.prerequisites.iter().any(|step| {
        matches!(
            &step.action,
            StepAction::SyncTools { tools }
                if tools.iter().any(|tool| tool == crate::manifest::PI_TOOL_KEY)
        )
    });
    if pi_via_mise {
        if let Some(lane) = lanes.iter_mut().find(|lane| lane.manager == "pi") {
            lane.waits_for = Some("mise");
        }
    }
    lanes
}

struct LaneOutcome {
    unavailable: bool,
}

fn execute_lane(
    lane: &InstallLane<'_>,
    system: &dyn System,
    sender: &std::sync::mpsc::Sender<(usize, StepStatus)>,
) -> LaneOutcome {
    let mut unavailable = false;
    for indexed in &lane.steps {
        if unavailable {
            let message = format!("{} is unavailable", display_name(lane.manager));
            let _ = sender.send((indexed.index, StepStatus::Skipped(message)));
            continue;
        }
        let _ = sender.send((indexed.index, StepStatus::Running));
        let failure = execute_step(indexed.step, system).or_else(|| {
            if matches!(indexed.phase, StepPhase::Prerequisite) {
                system.refresh_path();
                (!system.command_exists(lane.manager)).then(|| {
                    format!(
                        "installer completed, but {} is still unavailable on PATH",
                        lane.manager
                    )
                })
            } else {
                None
            }
        });
        let status = match failure {
            Some(message) => {
                if matches!(indexed.phase, StepPhase::Prerequisite) {
                    unavailable = true;
                }
                StepStatus::Failed(message)
            }
            None if matches!(indexed.phase, StepPhase::Prerequisite) => StepStatus::Prepared,
            None => StepStatus::Installed,
        };
        let _ = sender.send((indexed.index, status));
    }
    LaneOutcome { unavailable }
}

fn execute_step(step: &InstallStep, system: &dyn System) -> Option<String> {
    let command = match &step.action {
        StepAction::CopySkills {
            skills,
            destination,
        } => return crate::skills::install_skills(system, skills, destination).err(),
        StepAction::SyncTools { tools } => {
            return crate::manifest::sync_selected(system, tools).err()
        }
        StepAction::Command(command) => command,
    };
    match system.run(command) {
        Ok(result) if result.success => {}
        Ok(result) => return Some(command_failure_message(&result)),
        Err(error) => return Some(error.to_string()),
    }
    let Some(verification) = &step.verification else {
        return None;
    };
    match verification {
        VerificationSpec::Command { command, needle } => match system.run(command) {
            Ok(result) if !result.success => Some(format!(
                "verification failed: {}",
                command_failure_message(&result)
            )),
            Ok(result) => {
                let output = format!("{}\n{}", result.stdout, result.stderr);
                needle
                    .as_ref()
                    .filter(|needle| !output.contains(needle.as_str()))
                    .map(|needle| format!("verification did not find {needle}"))
            }
            Err(error) => Some(format!("verification failed: {error}")),
        },
    }
}

/// The interesting half of a failed command's output: stderr, else stdout.
pub(crate) fn command_failure_message(result: &crate::CommandResult) -> String {
    if result.stderr.trim().is_empty() {
        result.stdout.trim().to_owned()
    } else {
        result.stderr.trim().to_owned()
    }
}

/// How a manager is written in reports: as its product name.
pub fn display_name(manager: &str) -> String {
    match manager {
        "herdr" => "Herdr".into(),
        "pi" => "Pi".into(),
        other => other.into(),
    }
}

fn prerequisite_step(
    manager: &str,
    platform: Platform,
    unix_script: &str,
    windows_script: &str,
) -> InstallStep {
    let command = match platform {
        Platform::Unix => CommandSpec::new("sh", ["-c", unix_script]),
        Platform::Windows => CommandSpec::new(
            "powershell",
            [
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                windows_script,
            ],
        ),
    };
    InstallStep {
        target: display_name(manager),
        manager: manager.into(),
        action: StepAction::Command(command),
        verification: None,
    }
}
