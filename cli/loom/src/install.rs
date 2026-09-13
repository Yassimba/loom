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
        match system.run_probe(&CommandSpec::new("node", ["--version"])) {
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
    pub mise: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<std::path::PathBuf>,
    pub(crate) private_env: Vec<(String, String)>,
}

impl std::fmt::Debug for CommandSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommandSpec")
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field(
                "private_env",
                &self
                    .private_env
                    .iter()
                    .map(|(name, _)| format!("{name}=<REDACTED>"))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl CommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            cwd: None,
            private_env: Vec::new(),
        }
    }

    pub(crate) fn with_private_env(mut self, name: &str, value: String) -> Self {
        self.private_env.push((name.into(), value));
        self
    }

    /// Run this command with `directory` as its process working directory.
    pub fn in_dir(mut self, directory: impl Into<std::path::PathBuf>) -> Self {
        self.cwd = Some(directory.into());
        self
    }

    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A reviewed installation operation. Commands and verification follow from its kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    BootstrapMise(Platform),
    Tools {
        tools: Vec<String>,
    },
    Skills {
        skills: Vec<String>,
        destination: crate::SkillDestination,
    },
    PiPackage {
        spec: String,
        name: String,
        project: bool,
    },
    HerdrPlugin {
        source: String,
        name: String,
    },
    Mcp {
        server: crate::mcp::Server,
        destination: crate::SkillDestination,
    },
}

impl Operation {
    fn command(&self) -> Option<CommandSpec> {
        Some(match self {
            Self::BootstrapMise(Platform::Unix) => CommandSpec::new("sh", ["-c", "curl -fsSL https://mise.run | sh"]),
            Self::BootstrapMise(Platform::Windows) => CommandSpec::new("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", "winget install --id jdx.mise --silent --accept-package-agreements --accept-source-agreements"]),
            Self::PiPackage { spec, project, .. } => {
                let mut args = vec!["install"];
                if *project { args.push("-l"); }
                args.push(spec);
                CommandSpec::new("pi", args)
            }
            Self::HerdrPlugin { source, .. } => CommandSpec::new("herdr", ["plugin", "install", source, "--yes"]),
            _ => return None,
        })
    }

    pub fn display(&self) -> String {
        match self {
            Self::Mcp {
                server,
                destination,
            } => format!(
                "configure {} → {}; {}",
                server.name(),
                crate::mcp::config_path(destination).display(),
                crate::mcp::EXPOSURE_NOTE
            ),
            Self::Skills {
                skills,
                destination,
            } => format!(
                "copy skills into {} targets ({}), except enabled package-provided Pi skills: {}{}",
                destination.scope.label().to_lowercase(),
                destination.display(),
                skills.join(", "),
                if destination.scope == crate::SkillScope::Project {
                    format!(
                        "; {}",
                        crate::bundled_skills::project_launch_note(&destination.project_root)
                    )
                } else {
                    String::new()
                }
            ),
            Self::Tools { tools } => format!(
                "add to the mise selection and install: {}{}",
                tools.join(", "),
                if tools.iter().any(|key| key == crate::mcp::SEM_TOOL_KEY) {
                    " (machine-wide Sem v0.24.0)"
                } else {
                    ""
                }
            ),
            Self::BootstrapMise(_) | Self::PiPackage { .. } | Self::HerdrPlugin { .. } => {
                self.command().expect("command operation").display()
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallStep {
    pub target: String,
    pub operation: Operation,
}

impl InstallStep {
    pub fn manager(&self) -> &'static str {
        match self.operation {
            Operation::BootstrapMise(_) | Operation::Tools { .. } => "mise",
            Operation::Skills { .. } => "skills",
            Operation::PiPackage { .. } | Operation::Mcp { .. } => "pi",
            Operation::HerdrPlugin { .. } => "herdr",
        }
    }

    pub fn is_prerequisite(&self) -> bool {
        matches!(
            self.operation,
            Operation::BootstrapMise(_) | Operation::Tools { .. }
        )
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstallPlan {
    pub steps: Vec<InstallStep>,
}

impl InstallPlan {
    pub fn prerequisites(&self) -> impl Iterator<Item = &InstallStep> {
        self.steps.iter().filter(|step| step.is_prerequisite())
    }

    pub fn resources(&self) -> impl Iterator<Item = &InstallStep> {
        self.steps.iter().filter(|step| !step.is_prerequisite())
    }

    pub fn prerequisite_count(&self) -> usize {
        self.prerequisites().count()
    }
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
    status: PrerequisiteStatus,
    platform: Platform,
    skill_destination: &crate::skills::SkillDestination,
) -> Result<InstallPlan> {
    let mcp_servers = resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::McpServer)
        .map(|resource| crate::mcp::Server::from_name(&resource.install_target))
        .collect::<Result<Vec<_>>>()?;
    for server in &mcp_servers {
        crate::mcp::preflight(*server, skill_destination)?;
    }
    let needs_pi = !mcp_servers.is_empty()
        || resources
            .iter()
            .any(|resource| resource.kind == ResourceKind::PiPackage);
    let needs_herdr = resources
        .iter()
        .any(|resource| resource.kind == ResourceKind::HerdrPlugin);

    // Every runtime arrives through Loom's pinned mise selection. This gives
    // uninstall one ownership path instead of hidden npm/curl fallbacks.
    let mut tools = resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::Tool)
        .flat_map(|tool| {
            std::iter::once(tool.install_target.clone()).chain(tool.companions.iter().cloned())
        })
        .collect::<Vec<_>>();
    if mcp_servers.contains(&crate::mcp::Server::Sem)
        && !tools.iter().any(|key| key == crate::mcp::SEM_TOOL_KEY)
    {
        tools.push(crate::mcp::SEM_TOOL_KEY.into());
    }
    if needs_pi && !status.pi && !tools.contains(&crate::manifest::PI_TOOL_KEY.to_string()) {
        tools.push(crate::manifest::PI_TOOL_KEY.into());
    }
    if needs_herdr && !status.herdr && !tools.contains(&"herdr".to_string()) {
        tools.push("herdr".into());
    }

    let mut steps = Vec::new();
    if !tools.is_empty() && !status.mise {
        steps.push(InstallStep {
            target: "mise".into(),
            operation: Operation::BootstrapMise(platform),
        });
    }
    if !tools.is_empty() {
        steps.push(InstallStep {
            target: "tools".into(),
            operation: Operation::Tools { tools },
        });
    }
    let skills = resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::Skill)
        .map(|skill| skill.install_target.clone())
        .collect::<Vec<_>>();
    if !skills.is_empty() {
        anyhow::ensure!(
            !skill_destination.agents.is_empty(),
            "installing skills needs at least one selected agent"
        );
        steps.push(InstallStep {
            target: "skills".into(),
            operation: Operation::Skills {
                skills,
                destination: skill_destination.clone(),
            },
        });
    }
    for resource in resources {
        let operation = match resource.kind {
            ResourceKind::PiPackage => Operation::PiPackage {
                spec: resource.pi_install_spec(),
                name: resource.install_target.clone(),
                project: false,
            },
            ResourceKind::HerdrPlugin => Operation::HerdrPlugin {
                source: resource.install_target.clone(),
                name: resource.id.trim_start_matches("herdr-plugin:").into(),
            },
            _ => continue,
        };
        steps.push(InstallStep {
            target: resource.id.clone(),
            operation,
        });
    }
    for server in mcp_servers {
        steps.push(InstallStep {
            target: format!("mcp-server:{}", server.name()),
            operation: Operation::Mcp {
                server,
                destination: skill_destination.clone(),
            },
        });
    }
    Ok(InstallPlan { steps })
}

/// Progress of one plan step, indexed over prerequisites then resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepStatus {
    Running,
    Verifying,
    Prepared,
    Installed,
    Failed(String),
    Skipped(String),
}

/// Verify completed rows in place and execute the pending rows with their reviewed indices.
pub fn execute_attempt(
    plan: &InstallPlan,
    system: &(dyn System + Sync),
    cancelled: &std::sync::atomic::AtomicBool,
    completed: &[usize],
    observer: &mut dyn FnMut(usize, StepStatus),
) -> InstallReport {
    let mut report = InstallReport::default();
    let mut pending = Vec::new();
    for (index, step) in plan.steps.iter().enumerate() {
        if completed.contains(&index) {
            observer(index, StepStatus::Verifying);
            if step_is_present(step, system, cancelled) {
                observer(index, StepStatus::Installed);
                if !step.is_prerequisite() || step.target == "tools" {
                    report.installed.push(step.target.clone());
                }
                continue;
            }
        }
        pending.push(IndexedStep { index, step });
    }
    // Every pending MCP destination is validated before any worker can mutate state.
    for indexed in &pending {
        if let Operation::Mcp {
            server,
            destination,
        } = &indexed.step.operation
        {
            if let Err(error) = crate::mcp::preflight(*server, destination) {
                observer(indexed.index, StepStatus::Failed(error.to_string()));
                report.failures.push(InstallFailure {
                    target: indexed.step.target.clone(),
                    message: error.to_string(),
                });
                return report;
            }
        }
    }
    let repository = &crate::skills::Repository::default();
    let lanes = install_lanes(&pending);
    let mut outcomes = vec![None; plan.steps.len()];
    let (sender, events) = std::sync::mpsc::channel();
    let mut status = |index: usize, value: StepStatus| {
        if !matches!(value, StepStatus::Running | StepStatus::Verifying) {
            outcomes[index] = Some(value.clone());
        }
        observer(index, value);
    };
    std::thread::scope(|scope| {
        let start = |lane| {
            let sender = sender.clone();
            scope.spawn(move || {
                let failed = execute_lane(lane, system, cancelled, repository, &sender);
                let _ = sender.send(Event::LaneDone(lane.manager, failed));
            });
        };
        let mut waiting = Vec::new();
        let mut running = 0;
        for lane in &lanes {
            if lane.waits_for.is_some() {
                waiting.push(lane);
            } else {
                start(lane);
                running += 1;
            }
        }
        while running > 0 {
            match events
                .recv()
                .expect("install workers retain their event sender")
            {
                Event::Status(index, value) => status(index, value),
                Event::LaneDone(manager, failed) => {
                    running -= 1;
                    let mut finished = vec![(manager, failed)];
                    while let Some((manager, failed)) = finished.pop() {
                        let mut index = 0;
                        while index < waiting.len() {
                            if waiting[index].waits_for != Some(manager) {
                                index += 1;
                                continue;
                            }
                            let lane = waiting.swap_remove(index);
                            if failed
                                || (matches!(lane.manager, "pi" | "herdr")
                                    && !system.command_exists(lane.manager))
                            {
                                for step in &lane.steps {
                                    status(
                                        step.index,
                                        StepStatus::Skipped(skipped_message(lane.manager)),
                                    );
                                }
                                finished.push((lane.manager, true));
                            } else {
                                start(lane);
                                running += 1;
                            }
                        }
                    }
                }
            }
        }
    });
    // Verified rows precede this attempt, then outcomes follow original plan order.
    for indexed in pending {
        match outcomes[indexed.index].take() {
            Some(StepStatus::Failed(message) | StepStatus::Skipped(message)) => {
                report.failures.push(InstallFailure {
                    target: indexed.step.target.clone(),
                    message,
                })
            }
            Some(StepStatus::Installed) => report.installed.push(indexed.step.target.clone()),
            _ => {}
        }
    }
    report
}

#[derive(Clone, Copy)]
struct IndexedStep<'a> {
    index: usize,
    step: &'a InstallStep,
}

struct InstallLane<'a> {
    manager: &'static str,
    steps: Vec<IndexedStep<'a>>,
    waits_for: Option<&'static str>,
}

enum Event {
    Status(usize, StepStatus),
    LaneDone(&'static str, bool),
}

fn install_lanes<'a>(pending: &[IndexedStep<'a>]) -> Vec<InstallLane<'a>> {
    let mut lanes = Vec::<InstallLane<'_>>::new();
    for indexed in pending {
        let manager = indexed.step.manager();
        match lanes.iter_mut().find(|lane| lane.manager == manager) {
            Some(lane) => lane.steps.push(*indexed),
            None => lanes.push(InstallLane {
                manager,
                steps: vec![*indexed],
                waits_for: None,
            }),
        }
    }
    let bundled_names = crate::bundled_skills::packages()
        .iter()
        .filter(|package| {
            pending
                .iter()
                .any(|indexed| indexed.step.target == package.id)
        })
        .flat_map(|package| &package.bundled_skills)
        .collect::<Vec<_>>();
    let needs_bundled_pi = pending.iter().any(|indexed| matches!(&indexed.step.operation,
        Operation::Skills { skills, destination }
            if (destination.agents.contains(&crate::SkillAgent::Pi)
                || destination.agents.contains(&crate::SkillAgent::AgentsStandard)
                || (destination.scope == crate::SkillScope::Project && destination.agents.contains(&crate::SkillAgent::Codex)))
                && skills.iter().any(|name| bundled_names.contains(&name))
    ));
    let configures_mcp = pending
        .iter()
        .any(|indexed| matches!(indexed.step.operation, Operation::Mcp { .. }));
    if needs_bundled_pi || configures_mcp {
        if let Some(lane) = lanes.iter_mut().find(|lane| lane.manager == "skills") {
            lane.waits_for = Some("pi");
        }
    }
    for (manager, tool_key) in [("pi", crate::manifest::PI_TOOL_KEY), ("herdr", "herdr")] {
        let runtime_via_mise = pending.iter().any(|indexed| {
            matches!(&indexed.step.operation,
                Operation::Tools { tools } if tools.iter().any(|tool| tool == tool_key)
            )
        });
        if runtime_via_mise
            || (manager == "pi"
                && configures_mcp
                && lanes.iter().any(|lane| lane.manager == "mise"))
        {
            if let Some(lane) = lanes.iter_mut().find(|lane| lane.manager == manager) {
                lane.waits_for = Some("mise");
            }
        }
    }
    lanes
}

/// Why the remaining steps of a lane were skipped.
fn skipped_message(manager: &str) -> String {
    if manager == "skills" {
        "Pi package installation did not complete; skills skipped — fix the package failure and retry".into()
    } else {
        format!("{} is unavailable", display_name(manager))
    }
}

fn execute_lane(
    lane: &InstallLane<'_>,
    system: &(dyn System + Sync),
    cancelled: &std::sync::atomic::AtomicBool,
    repository: &crate::skills::Repository,
    sender: &std::sync::mpsc::Sender<Event>,
) -> bool {
    let mut failed = false;
    for (offset, indexed) in lane.steps.iter().enumerate() {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            for skipped in &lane.steps[offset..] {
                let _ = sender.send(Event::Status(
                    skipped.index,
                    StepStatus::Skipped("cancelled".into()),
                ));
            }
            return true;
        }
        let prerequisite = indexed.step.is_prerequisite();
        let _ = sender.send(Event::Status(indexed.index, StepStatus::Running));
        let failure = execute_step(indexed.step, system, cancelled, repository, || {
            let _ = sender.send(Event::Status(indexed.index, StepStatus::Verifying));
        })
        .or_else(|| {
            if prerequisite {
                system.refresh_path();
                (!system.command_exists(lane.manager)).then(|| {
                    format!(
                        "installer completed, but {} is still unavailable on PATH",
                        lane.manager
                    )
                })
            } else if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                Some("cancelled".into())
            } else {
                crate::pi_compat::apply_for_package(&indexed.step.target, system)
                    .err()
                    .map(|error| error.to_string())
            }
        });
        match failure {
            Some(message) => {
                failed = true;
                let _ = sender.send(Event::Status(indexed.index, StepStatus::Failed(message)));
                if prerequisite {
                    for skipped in &lane.steps[offset + 1..] {
                        let _ = sender.send(Event::Status(
                            skipped.index,
                            StepStatus::Skipped(skipped_message(lane.manager)),
                        ));
                    }
                    return true;
                }
            }
            None => {
                let value = if prerequisite {
                    StepStatus::Prepared
                } else {
                    StepStatus::Installed
                };
                let _ = sender.send(Event::Status(indexed.index, value));
            }
        }
    }
    failed
}

fn execute_step(
    step: &InstallStep,
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
    repository: &crate::skills::Repository,
    verifying: impl FnOnce(),
) -> Option<String> {
    execute_action(step, system, cancelled, repository).or_else(|| {
        if matches!(
            step.operation,
            Operation::PiPackage { .. } | Operation::HerdrPlugin { .. }
        ) {
            verifying();
        }
        verify_step(step, system, cancelled)
    })
}

/// Recheck completed work before a retry. A step without evidence is rerun.
pub(crate) fn step_is_present(
    step: &InstallStep,
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
) -> bool {
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    match &step.operation {
        Operation::Mcp {
            server,
            destination,
        } => crate::mcp::configured(*server, destination, system),
        Operation::Skills {
            skills,
            destination,
        } => {
            let trees = destination.trees();
            !trees.is_empty()
                && trees.iter().all(|tree| {
                    skills.iter().all(|name| {
                        crate::skills::skill_present_in(tree, name)
                            || crate::bundled_skills::provided_in_tree(
                                &destination.home,
                                tree,
                                name,
                            )
                    })
                })
        }
        Operation::Tools { tools } => {
            let Some(home) = system.home_dir() else {
                return false;
            };
            let selected = crate::manifest::selected_keys(&home);
            let Ok(catalog) = crate::Catalog::embedded() else {
                return false;
            };
            tools.iter().all(|key| {
                selected.contains(key)
                    && catalog
                        .resources
                        .iter()
                        .find(|resource| resource.install_target == *key)
                        .and_then(|resource| resource.bin.as_deref())
                        .is_some_and(|bin| system.command_exists(bin))
            })
        }
        Operation::BootstrapMise(_) => false,
        Operation::PiPackage { .. } | Operation::HerdrPlugin { .. } => {
            verify_step(step, system, cancelled).is_none()
        }
    }
}

fn execute_action(
    step: &InstallStep,
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
    repository: &crate::skills::Repository,
) -> Option<String> {
    let command = match &step.operation {
        Operation::Mcp {
            server,
            destination,
        } => {
            return crate::mcp::install(*server, destination, system)
                .err()
                .map(|e| e.to_string())
        }
        Operation::Skills {
            skills,
            destination,
        } => {
            return crate::skills::install_skills(
                system,
                repository,
                skills,
                destination,
                cancelled,
            )
            .err()
        }
        Operation::Tools { tools } => {
            return crate::manifest::sync_selected_from(system, tools, cancelled, repository).err()
        }
        _ => &step.operation.command().expect("command operation"),
    };
    match system.run_controlled(command, crate::system::MANAGER_COMMAND_TIMEOUT, cancelled) {
        Ok(result) if result.success => None,
        Ok(result) => Some(command_failure_message(&result)),
        Err(error) => Some(error.to_string()),
    }
}

fn path_package_matches(path: &std::path::Path, target: &str, unscoped: &str) -> bool {
    if let Some(name) = std::fs::read(path.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|package| package["name"].as_str().map(str::to_owned))
    {
        return name == target;
    }
    path.to_string_lossy()
        .to_ascii_lowercase()
        .contains(&unscoped.to_ascii_lowercase())
}

/// Pi may list both scopes; only the reviewed destination is evidence.
pub(crate) fn pi_package_installed(listed: &str, target: &str, project: bool) -> bool {
    let target = target.strip_prefix("npm:").unwrap_or(target);
    let target = target
        .rsplit_once('@')
        .filter(|(name, _)| !name.is_empty())
        .map_or(target, |(name, _)| name);
    let target = target.strip_prefix("git:").map_or(target, |source| {
        source
            .rsplit('/')
            .next()
            .unwrap_or(source)
            .trim_end_matches(".git")
    });
    let mut in_scope = false;
    let unscoped = target.rsplit('/').next().unwrap_or(target);
    listed.lines().map(str::trim).any(|line| {
        match line {
            "User packages:" => {
                in_scope = !project;
                return false;
            }
            "Project packages:" => {
                in_scope = project;
                return false;
            }
            _ => {}
        }
        if !in_scope {
            return false;
        }
        if let Some(spec) = line.strip_prefix("npm:") {
            return spec
                .strip_prefix(target)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('@'));
        }
        if let Some(source) = line.strip_prefix("git:") {
            return source.rsplit('/').next().is_some_and(|name| {
                name.split('@')
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(".git")
                    == unscoped
            });
        }
        let path = std::path::Path::new(line);
        path.is_absolute() && path.is_dir() && path_package_matches(path, target, unscoped)
    })
}

fn verify_step(
    step: &InstallStep,
    system: &dyn System,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Option<String> {
    let (command, needle, project) = match &step.operation {
        Operation::PiPackage { name, project, .. } => {
            (CommandSpec::new("pi", ["list"]), name, Some(*project))
        }
        Operation::HerdrPlugin { name, .. } => {
            (CommandSpec::new("herdr", ["plugin", "list"]), name, None)
        }
        _ => return None,
    };
    match system.run_controlled(&command, crate::system::PROBE_COMMAND_TIMEOUT, cancelled) {
        Ok(result) if !result.success => Some(format!(
            "verification failed: {}",
            command_failure_message(&result)
        )),
        Ok(result) => {
            let present = match project {
                Some(project) => pi_package_installed(&result.stdout, needle, project),
                None => result.stdout.contains(needle),
            };
            (!present)
                .then(|| format!("verification did not find {needle} in the selected destination"))
        }
        Err(error) => Some(format!("verification failed: {error}")),
    }
}

/// Reduce tool diagnostics to a safe cause and recovery action. Never echo
/// arbitrary stderr/stdout: installers can include credentials or private text.
pub(crate) fn command_failure_message(result: &crate::CommandResult) -> String {
    let message = if result.stderr.trim().is_empty() {
        result.stdout.trim()
    } else {
        result.stderr.trim()
    };
    if message.is_empty() {
        "The tool exited without an error message. Retry the failed item.".into()
    } else {
        crate::ui::failure_text(message)
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
