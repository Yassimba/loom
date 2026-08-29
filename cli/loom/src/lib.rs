pub mod app;
mod catalog;
pub mod init;
mod install;
mod jsonc;
pub mod manifest;
mod pi_compat;
pub mod settings;
mod skills;
pub mod status;
mod system;
pub mod ui;
pub mod update;
pub mod wizard;

pub use catalog::{Catalog, Resource, ResourceKind};
pub use install::{
    build_install_plan, execute_install_plan, execute_install_plan_with,
    execute_install_plan_with_control, CommandSpec, InstallFailure, InstallPlan, InstallReport,
    InstallStep, NodeStatus, Platform, PrerequisiteStatus, Runtime, StepAction, StepStatus,
    VerificationSpec, PI_MIN_NODE,
};
pub use skills::{
    detect_skill_agents, detect_skill_trees, expand_skill_dependencies, SkillAgent,
    SkillDestination, SkillScope,
};
pub use system::{CommandResult, RealSystem, System};
