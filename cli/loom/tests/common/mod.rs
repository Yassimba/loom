#![allow(dead_code)] // Each integration target uses only its relevant fixtures.

use std::path::PathBuf;

pub fn temp_home(label: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!(
        "loom-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&home).unwrap();
    home
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

/// Execute a fresh installation with no cancellation or progress consumer.
pub fn install(
    plan: &loom::InstallPlan,
    system: &(dyn loom::System + Sync),
) -> loom::InstallReport {
    loom::execute_attempt(
        plan,
        system,
        &std::sync::atomic::AtomicBool::new(false),
        &[],
        &mut |_, _| {},
    )
}
