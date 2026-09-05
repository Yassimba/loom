pub mod app;
mod bundled_skills;
mod catalog;
pub mod diagrams;
mod fs_tx;
pub mod init;
mod install;
mod jsonc;
pub mod manifest;
pub mod ownership;
mod pi_compat;
pub mod settings;
mod skills;
pub mod status;
mod system;
pub mod ui;
pub mod uninstall;
pub mod update;
pub mod wiki;
mod wiki_confluence;
mod wiki_progress;
mod wiki_tui;
pub mod wizard;

pub use bundled_skills::{
    provided_bundled_skills, reconcile_installed as reconcile_bundled_skills,
};
pub use catalog::{Catalog, Profile, Resource, ResourceKind};
pub use install::{
    build_install_plan, execute_install_plan, execute_install_plan_with,
    execute_install_plan_with_control, CommandSpec, InstallFailure, InstallPlan, InstallReport,
    InstallStep, NodeStatus, Platform, PrerequisiteStatus, StepAction, StepStatus,
    VerificationSpec, PI_MIN_NODE,
};
pub use ownership::{
    digest_path, InstallState, OwnedPathKind, OwnedResource, OwnershipScope, Receipt,
};
pub use skills::{
    detect_skill_agents, detect_skill_trees, expand_skill_dependencies, project_root, SkillAgent,
    SkillDestination, SkillScope,
};
pub use system::{CommandResult, RealSystem, System};
pub use uninstall::{
    build_uninstall_plan, execute_uninstall_plan, receipt_status, ReceiptStatus, UninstallFailure,
    UninstallOptions, UninstallPlan, UninstallReport, UninstallRequest, UninstallStep,
};
