//! Read source files from git trees or the working tree.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::lang::detect_language;
use crate::types::Snapshot;

fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn assert_git_repo(cwd: &Path) -> Result<()> {
    git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .map(|_| ())
        .map_err(|_| anyhow::anyhow!("Not a git repository: {}", cwd.display()))
}

pub fn verify_commit(cwd: &Path, reference: &str) -> Result<()> {
    git(
        cwd,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )
    .map(|_| ())
    .map_err(|_| anyhow::anyhow!("Unknown git ref: {reference}"))
}

pub fn resolve_snapshots(from: Option<&str>, to: Option<&str>) -> (Snapshot, Snapshot) {
    // git-diff defaults: no args → HEAD vs worktree; one arg → that vs worktree
    let left = Snapshot::Commit(from.unwrap_or("HEAD").to_string());
    let right = match to {
        Some(to) => Snapshot::Commit(to.to_string()),
        None => Snapshot::Worktree,
    };
    (left, right)
}

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "coverage",
    ".git",
    ".next",
    ".turbo",
    "out",
    "target",
];

fn is_source_file(path: &str) -> bool {
    detect_language(path).is_some()
}

fn walk_worktree(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let full = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk_worktree(root, &full, out);
            continue;
        }
        if file_type.is_file() && is_source_file(&name) {
            if let Ok(rel) = full.strip_prefix(root) {
                out.push(
                    rel.to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                );
            }
        }
    }
}

fn list_commit_source_files(cwd: &Path, reference: &str) -> Result<Vec<String>> {
    let output = git(cwd, &["ls-tree", "-r", "--name-only", reference])?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && is_source_file(line))
        .map(str::to_string)
        .collect())
}

fn path_allowed(file: &str, path_filters: &[String]) -> bool {
    if path_filters.is_empty() {
        return true;
    }
    path_filters.iter().any(|filter| {
        let normalized = filter
            .strip_prefix("./")
            .unwrap_or(filter)
            .trim_end_matches('/');
        file == normalized
            || file.starts_with(&format!("{normalized}/"))
            || file.ends_with(normalized)
    })
}

pub fn list_source_files(
    cwd: &Path,
    snapshot: &Snapshot,
    path_filters: &[String],
) -> Result<Vec<String>> {
    let mut files = match snapshot {
        Snapshot::Worktree => {
            let mut out = Vec::new();
            walk_worktree(cwd, cwd, &mut out);
            out
        }
        Snapshot::Commit(reference) => list_commit_source_files(cwd, reference)?,
    };
    files.retain(|file| path_allowed(file, path_filters));
    files.sort();
    Ok(files)
}

pub fn read_snapshot_file(cwd: &Path, snapshot: &Snapshot, file: &str) -> Option<String> {
    match snapshot {
        Snapshot::Worktree => {
            let full: PathBuf = cwd.join(file);
            std::fs::read_to_string(full).ok()
        }
        Snapshot::Commit(reference) => git(cwd, &["show", &format!("{reference}:{file}")]).ok(),
    }
}
