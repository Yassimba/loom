//! Unix terminal buffers reviewed against the original UI.
use std::path::Path;

/// Render with a fixed terminal environment without racing other tests.
pub(crate) fn isolated(test: &str) -> bool {
    if std::env::var("LOOM_SNAPSHOT_CHILD").as_deref() == Ok(test) {
        return true;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env("LOOM_SNAPSHOT_CHILD", test)
        .env("TERM", "xterm-256color")
        .env_remove("NO_COLOR")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{test}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

pub(crate) fn assert_snapshot(name: &str, actual: &str) {
    // Windows has different paths and platform actions; its behavior tests still run.
    if cfg!(windows) {
        return;
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.snap"));
    if std::env::var_os("LOOM_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    pretty_assertions::assert_eq!(actual, expected, "snapshot {}", path.display());
}
