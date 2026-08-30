use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const STATE_PATH: &str = ".config/loom/install-state.json";

const INIT_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "CODING_STANDARDS.md",
    ".gitignore",
    "ai-docs/agents/issue-tracker.md",
    "ai-docs/agents/domain.md",
    "ai-docs/agents/editor.md",
];
const INIT_TREES: &[&str] = &[".beads", ".codegraph"];

#[derive(Clone, Debug)]
pub struct ProjectSnapshot {
    root: PathBuf,
    files: BTreeMap<PathBuf, Option<String>>,
    trees: BTreeMap<PathBuf, Option<String>>,
}

pub fn snapshot_project(root: &Path) -> ProjectSnapshot {
    ProjectSnapshot {
        root: root.to_path_buf(),
        files: INIT_FILES
            .iter()
            .map(|relative| {
                let path = root.join(relative);
                (path.clone(), fs::read_to_string(path).ok())
            })
            .collect(),
        trees: INIT_TREES
            .iter()
            .map(|relative| {
                let path = root.join(relative);
                let digest = path.is_dir().then(|| digest_path(&path).ok()).flatten();
                (path, digest)
            })
            .collect(),
    }
}

pub fn record_project_changes(home: &Path, before: ProjectSnapshot) -> Result<(), String> {
    let mut receipts = Vec::new();
    for (path, previous) in before.files {
        let current = fs::read_to_string(&path).ok();
        if current == previous || current.is_none() {
            continue;
        }
        receipts.push(Receipt::Path {
            digest: digest_path(&path)?,
            path,
            path_kind: OwnedPathKind::File,
            before: previous,
        });
    }
    for (path, previous_digest) in before.trees {
        if previous_digest.is_none() && path.is_dir() {
            if path.file_name().is_some_and(|name| name == ".codegraph") {
                receipts.extend([
                    Receipt::Command {
                        program: "codegraph".into(),
                        args: vec!["uninit".into()],
                    },
                    Receipt::Command {
                        program: "codegraph".into(),
                        args: vec![
                            "uninstall".into(),
                            "--yes".into(),
                            "--location".into(),
                            "local".into(),
                        ],
                    },
                ]);
            }
            receipts.push(Receipt::Path {
                digest: digest_path(&path)?,
                path,
                path_kind: OwnedPathKind::Tree,
                before: None,
            });
        }
    }
    if receipts.is_empty() {
        return Ok(());
    }
    let mut state = InstallState::load(home)?;
    let canonical_root = before
        .root
        .canonicalize()
        .unwrap_or_else(|_| before.root.clone());
    state.record(OwnedResource {
        id: format!("project:{}:init", canonical_root.display()),
        scope: OwnershipScope::Project {
            root: canonical_root,
        },
        depends_on: vec!["core:loom".into()],
        receipts,
    });
    state.save(home)
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallState {
    pub schema_version: u32,
    pub resources: BTreeMap<String, OwnedResource>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedResource {
    pub id: String,
    pub scope: OwnershipScope,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub receipts: Vec<Receipt>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OwnershipScope {
    Global,
    Project { root: PathBuf },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Receipt {
    Manager {
        manager: String,
        target: String,
    },
    Command {
        program: String,
        args: Vec<String>,
    },
    MiseTool {
        key: String,
    },
    Path {
        path: PathBuf,
        path_kind: OwnedPathKind,
        digest: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<String>,
    },
    ActivationLine {
        path: PathBuf,
        line: String,
    },
    MiseInstallation {
        root: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        executable: Option<PathBuf>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manager: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path_entry_added: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnedPathKind {
    File,
    Tree,
}

pub fn digest_path(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    if path.is_file() {
        hasher.update(
            fs::read(path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?,
        );
    } else if path.is_dir() {
        digest_tree(path, path, &mut hasher)?;
    } else {
        return Err(format!("{} is not a file or directory", path.display()));
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_tree(root: &Path, directory: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).expect("tree entry is below root");
        hasher.update(relative.to_string_lossy().as_bytes());
        let kind = entry
            .file_type()
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if kind.is_dir() {
            hasher.update(b"d");
            digest_tree(root, &path, hasher)?;
        } else if kind.is_file() {
            hasher.update(b"f");
            hasher.update(
                fs::read(&path)
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?,
            );
        } else {
            hasher.update(b"other");
        }
    }
    Ok(())
}

pub fn record_bootstrap_from_env(home: &Path) -> Result<(), String> {
    if env::var_os("LOOM_BOOTSTRAP").is_none() {
        return Ok(());
    }
    let mut state = InstallState::load(home)?;
    let mut mise_receipts = Vec::new();
    if env::var_os("LOOM_BOOTSTRAP_MISE_INSTALLED").as_deref() == Some(std::ffi::OsStr::new("1")) {
        if let Some(root) = env::var_os("LOOM_BOOTSTRAP_MISE_ROOT") {
            mise_receipts.push(Receipt::MiseInstallation {
                root: PathBuf::from(root),
                executable: env::var_os("LOOM_BOOTSTRAP_MISE_EXECUTABLE").map(PathBuf::from),
                manager: env::var("LOOM_BOOTSTRAP_MISE_MANAGER").ok(),
                path_entry_added: env::var_os("LOOM_BOOTSTRAP_MISE_PATH_ADDED")
                    .filter(|value| value == "1")
                    .and_then(|_| env::var_os("LOOM_BOOTSTRAP_MISE_PATH_ENTRY"))
                    .map(PathBuf::from),
            });
        }
    }
    if let (Some(path), Some(line)) = (
        env::var_os("LOOM_BOOTSTRAP_ACTIVATION_PATH"),
        env::var("LOOM_BOOTSTRAP_ACTIVATION_LINE").ok(),
    ) {
        mise_receipts.push(Receipt::ActivationLine {
            path: PathBuf::from(path),
            line,
        });
    }
    if let Ok(paths) = env::var("LOOM_BOOTSTRAP_ACTIVATION_PATHS_JSON") {
        if let Ok(paths) = serde_json::from_str::<Vec<PathBuf>>(&paths) {
            if let Ok(line) = env::var("LOOM_BOOTSTRAP_ACTIVATION_LINE") {
                mise_receipts.extend(paths.into_iter().map(|path| Receipt::ActivationLine {
                    path,
                    line: line.clone(),
                }));
            }
        }
    }
    state.record(OwnedResource {
        id: "core:mise".into(),
        scope: OwnershipScope::Global,
        depends_on: Vec::new(),
        receipts: mise_receipts,
    });
    state.record(OwnedResource {
        id: "core:node".into(),
        scope: OwnershipScope::Global,
        depends_on: vec!["core:mise".into()],
        receipts: vec![Receipt::MiseTool { key: "node".into() }],
    });
    state.record(OwnedResource {
        id: "core:loom".into(),
        scope: OwnershipScope::Global,
        depends_on: vec!["core:node".into(), "core:mise".into()],
        receipts: vec![Receipt::MiseTool {
            key: "github:Yassimba/loom[exe=loom]".into(),
        }],
    });
    state.save(home)
}

impl InstallState {
    pub fn load(home: &Path) -> Result<Self, String> {
        let path = home.join(STATE_PATH);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    schema_version: 1,
                    resources: BTreeMap::new(),
                })
            }
            Err(error) => return Err(format!("could not read {}: {error}", path.display())),
        };
        let state: Self = serde_json::from_str(&text)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        if state.schema_version != 1 {
            return Err(format!(
                "unsupported ownership ledger version {} in {}",
                state.schema_version,
                path.display()
            ));
        }
        Ok(state)
    }

    pub fn save(&self, home: &Path) -> Result<(), String> {
        let path = home.join(STATE_PATH);
        let parent = path.parent().expect("state path has a parent");
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        let temporary = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).expect("ownership state serializes");
        fs::write(&temporary, format!("{text}\n"))
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))
    }

    pub fn record(&mut self, mut resource: OwnedResource) {
        let Some(existing) = self.resources.get_mut(&resource.id) else {
            self.resources.insert(resource.id.clone(), resource);
            return;
        };
        existing.scope = resource.scope;
        existing.depends_on.append(&mut resource.depends_on);
        existing.depends_on.sort();
        existing.depends_on.dedup();
        for mut receipt in resource.receipts {
            if let Some(index) = existing
                .receipts
                .iter()
                .position(|owned| same_contribution(owned, &receipt))
            {
                if let (
                    Receipt::Path {
                        before: original, ..
                    },
                    Receipt::Path {
                        before: refreshed, ..
                    },
                ) = (&existing.receipts[index], &mut receipt)
                {
                    *refreshed = original.clone();
                }
                existing.receipts[index] = receipt;
            } else {
                existing.receipts.push(receipt);
            }
        }
    }
}

fn same_contribution(left: &Receipt, right: &Receipt) -> bool {
    match (left, right) {
        (
            Receipt::Manager {
                manager: a,
                target: x,
            },
            Receipt::Manager {
                manager: b,
                target: y,
            },
        ) => a == b && x == y,
        (
            Receipt::Command {
                program: a,
                args: x,
            },
            Receipt::Command {
                program: b,
                args: y,
            },
        ) => a == b && x == y,
        (Receipt::MiseTool { key: a }, Receipt::MiseTool { key: b }) => a == b,
        (Receipt::Path { path: a, .. }, Receipt::Path { path: b, .. }) => a == b,
        (
            Receipt::ActivationLine { path: a, line: x },
            Receipt::ActivationLine { path: b, line: y },
        ) => a == b && x == y,
        (Receipt::MiseInstallation { root: a, .. }, Receipt::MiseInstallation { root: b, .. }) => {
            a == b
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(label: &str) -> PathBuf {
        let home = env::temp_dir().join(format!(
            "loom-ownership-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home).unwrap();
        home
    }

    fn state_with_one_resource() -> InstallState {
        let mut state = InstallState {
            schema_version: 1,
            resources: BTreeMap::new(),
        };
        state.record(OwnedResource {
            id: "skill:tdd".into(),
            scope: OwnershipScope::Global,
            depends_on: vec!["core:loom".into()],
            receipts: vec![Receipt::Manager {
                manager: "skills".into(),
                target: "tdd".into(),
            }],
        });
        state
    }

    #[test]
    fn repeated_records_merge_distinct_contributions_and_refresh_matching_ones() {
        let mut state = state_with_one_resource();
        state.resources.get_mut("skill:tdd").unwrap().receipts = vec![Receipt::Path {
            path: "/tmp/first-tree".into(),
            path_kind: OwnedPathKind::File,
            digest: "old".into(),
            before: None,
        }];
        state.record(OwnedResource {
            id: "skill:tdd".into(),
            scope: OwnershipScope::Global,
            depends_on: vec!["skill:test".into()],
            receipts: vec![
                Receipt::Path {
                    path: "/tmp/first-tree".into(),
                    path_kind: OwnedPathKind::File,
                    digest: "refreshed".into(),
                    before: Some("loom-generated".into()),
                },
                Receipt::Path {
                    path: "/tmp/second-tree".into(),
                    path_kind: OwnedPathKind::Tree,
                    digest: "new".into(),
                    before: None,
                },
            ],
        });

        let resource = &state.resources["skill:tdd"];
        assert_eq!(resource.depends_on, ["core:loom", "skill:test"]);
        assert_eq!(resource.receipts.len(), 2);
        assert!(matches!(
            &resource.receipts[0],
            Receipt::Path {
                digest,
                before: None,
                ..
            } if digest == "refreshed"
        ));
    }

    #[test]
    fn ledger_round_trips_through_the_atomic_file() {
        let home = temp_home("round-trip");
        let state = state_with_one_resource();

        state.save(&home).unwrap();

        assert_eq!(InstallState::load(&home).unwrap(), state);
        assert!(!home.join(format!("{STATE_PATH}.tmp")).exists());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn failed_save_does_not_replace_an_existing_ledger() {
        let home = temp_home("failed-save");
        let original = state_with_one_resource();
        original.save(&home).unwrap();
        let path = home.join(STATE_PATH);
        let before = fs::read(&path).unwrap();
        fs::create_dir(path.with_extension("json.tmp")).unwrap();
        let mut changed = original;
        changed.resources.clear();

        assert!(changed.save(&home).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn digests_are_stable_and_sensitive_to_paths_and_content() {
        let home = temp_home("digest");
        let tree = home.join("tree");
        fs::create_dir_all(&tree).unwrap();
        fs::write(tree.join("a"), "same").unwrap();
        let first = digest_path(&tree).unwrap();
        assert_eq!(digest_path(&tree).unwrap(), first);

        fs::rename(tree.join("a"), tree.join("b")).unwrap();
        assert_ne!(digest_path(&tree).unwrap(), first);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn new_codegraph_state_records_its_command_inverses_before_the_tree() {
        let home = temp_home("codegraph");
        let project = home.join("project");
        fs::create_dir_all(&project).unwrap();
        let snapshot = snapshot_project(&project);
        fs::create_dir(project.join(".codegraph")).unwrap();

        record_project_changes(&home, snapshot).unwrap();

        let state = InstallState::load(&home).unwrap();
        let receipts = &state.resources.values().next().unwrap().receipts;
        assert!(matches!(
            &receipts[0],
            Receipt::Command { program, args }
                if program == "codegraph" && args == &["uninit"]
        ));
        assert!(matches!(
            &receipts[1],
            Receipt::Command { program, args }
                if program == "codegraph"
                    && args == &["uninstall", "--yes", "--location", "local"]
        ));
        assert!(matches!(receipts[2], Receipt::Path { .. }));
        fs::remove_dir_all(home).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn project_root_canonicalizes_symlinked_worktrees() {
        use std::os::unix::fs::symlink;
        let home = temp_home("canonical");
        let real = home.join("real");
        fs::create_dir_all(real.join(".git")).unwrap();
        let linked = home.join("linked");
        symlink(&real, &linked).unwrap();

        let snapshot = snapshot_project(&linked);
        fs::write(linked.join("AGENTS.md"), "managed").unwrap();
        record_project_changes(&home, snapshot).unwrap();
        let state = InstallState::load(&home).unwrap();
        let scope = &state.resources.values().next().unwrap().scope;

        assert_eq!(
            scope,
            &OwnershipScope::Project {
                root: real.canonicalize().unwrap()
            }
        );
        fs::remove_dir_all(home).unwrap();
    }
}
