//! Native skill installer. Skills are plain directories in this repo;
//! installing one means downloading the repo tarball once and copying
//! `skills/<name>` into the selected agent and scope destinations.
//! No package manager sits in between.

use crate::{CommandSpec, Resource, ResourceKind, System};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const TARBALL_URL: &str = "https://codeload.github.com/Yassimba/loom/tar.gz/refs/heads/main";

const OPENCODE_PLUGIN_SOURCE: &str = "manifest/opencode/plugins/loom-session-env.js";
const OPENCODE_PLUGIN_NAME: &str = "loom-session-env.js";

/// A user-facing skill destination. `AgentsStandard` is the portable
/// `.agents/skills` tree used directly by several hosts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum SkillAgent {
    Claude,
    #[value(name = "agents")]
    AgentsStandard,
    Codex,
    Pi,
    #[value(name = "opencode")]
    OpenCode,
    Cursor,
    Grok,
}

impl SkillAgent {
    pub const ALL: [Self; 7] = [
        Self::Claude,
        Self::AgentsStandard,
        Self::Codex,
        Self::Pi,
        Self::OpenCode,
        Self::Cursor,
        Self::Grok,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::AgentsStandard => "Agent Skills standard",
            Self::Codex => "Codex",
            Self::Pi => "Pi",
            Self::OpenCode => "OpenCode",
            Self::Cursor => "Cursor",
            Self::Grok => "Grok",
        }
    }

    pub(crate) fn status_label(self) -> &'static str {
        match self {
            Self::AgentsStandard => "Agents",
            _ => self.label(),
        }
    }

    fn global_agent_dir(self, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home.join(".claude"),
            Self::AgentsStandard => home.join(".agents"),
            Self::Codex => home.join(".codex"),
            Self::Pi => home.join(".pi").join("agent"),
            Self::OpenCode => home.join(".config").join("opencode"),
            Self::Cursor => home.join(".cursor"),
            Self::Grok => home.join(".grok"),
        }
    }

    pub(crate) fn global_skill_tree(self, home: &Path) -> PathBuf {
        self.global_agent_dir(home).join("skills")
    }

    pub(crate) fn project_skill_tree(self, root: &Path) -> PathBuf {
        match self {
            Self::Claude => root.join(".claude").join("skills"),
            Self::AgentsStandard | Self::Codex => root.join(".agents").join("skills"),
            Self::Pi => root.join(".pi").join("skills"),
            Self::OpenCode => root.join(".opencode").join("skills"),
            Self::Cursor => root.join(".cursor").join("skills"),
            Self::Grok => root.join(".grok").join("skills"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, clap::ValueEnum)]
pub enum SkillScope {
    #[default]
    Global,
    Project,
}

impl SkillScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Project => "Project",
        }
    }
}

/// Exact agent and scope selection carried from the UI into execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillDestination {
    pub agents: Vec<SkillAgent>,
    pub scope: SkillScope,
    pub home: PathBuf,
    pub project_root: PathBuf,
}

impl SkillDestination {
    pub fn new(
        agents: Vec<SkillAgent>,
        scope: SkillScope,
        home: &Path,
        current_dir: &Path,
    ) -> Self {
        let agents = SkillAgent::ALL
            .into_iter()
            .filter(|agent| agents.contains(agent))
            .collect();
        Self {
            agents,
            scope,
            home: home.to_path_buf(),
            project_root: project_root(current_dir),
        }
    }

    pub fn trees(&self) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        self.agents
            .iter()
            .map(|agent| match self.scope {
                SkillScope::Global => agent.global_skill_tree(&self.home),
                SkillScope::Project => agent.project_skill_tree(&self.project_root),
            })
            .filter(|tree| seen.insert(tree.clone()))
            .collect()
    }

    pub fn display(&self) -> String {
        self.trees()
            .iter()
            .map(|tree| tree.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Prefer the Git worktree root so invoking Loom in a nested package still
/// installs project skills for the whole repository. Outside Git, cwd is the
/// project boundary the caller explicitly chose.
pub fn project_root(current_dir: &Path) -> PathBuf {
    current_dir
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .unwrap_or(current_dir)
        .to_path_buf()
}

/// Agents already configured globally, preserving the historic behavior
/// when a scripted invocation omits `--agent`.
pub fn detect_skill_agents(home: &Path) -> Vec<SkillAgent> {
    SkillAgent::ALL
        .into_iter()
        .filter(|agent| agent.global_agent_dir(home).is_dir())
        .collect()
}

/// The supported agent directories, for user-facing messages:
/// `~/.claude, ~/.agents, ~/.codex, ~/.pi/agent, ~/.config/opencode, ~/.cursor, ~/.grok`.
pub(crate) fn agent_dirs_display() -> String {
    SkillAgent::ALL
        .map(|agent| format!("~/{}", agent.global_agent_dir(Path::new("")).display()))
        .join(", ")
}

/// One definition of "installed": the skill's directory (real or symlinked)
/// holds a SKILL.md in this tree. The wizard's detection and update's
/// refresh union must agree on this.
pub(crate) fn skill_present_in(tree: &Path, name: &str) -> bool {
    tree.join(name).join("SKILL.md").is_file()
}

/// Existing global skill trees, used for backward-compatible detection and
/// update inventory.
pub fn detect_skill_trees(home: &Path) -> Vec<PathBuf> {
    SkillAgent::ALL
        .into_iter()
        .filter(|agent| agent.global_agent_dir(home).is_dir())
        .map(|agent| agent.global_skill_tree(home))
        .collect()
}

pub(crate) fn opencode_adapter_path(home: &Path) -> PathBuf {
    home.join(".config")
        .join("opencode")
        .join("plugins")
        .join(OPENCODE_PLUGIN_NAME)
}

pub(crate) fn project_opencode_adapter_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".opencode")
        .join("plugins")
        .join(OPENCODE_PLUGIN_NAME)
}

/// The selection plus the transitive closure of catalog skill dependencies,
/// deduplicated, dependencies appended after the explicit selection.
pub fn expand_skill_dependencies(all: &[Resource], selection: Vec<Resource>) -> Vec<Resource> {
    let mut expanded = selection;
    let mut seen = expanded
        .iter()
        .map(|resource| resource.id.clone())
        .collect::<HashSet<_>>();
    let mut cursor = 0;
    while cursor < expanded.len() {
        let dependencies = expanded[cursor].dependencies.clone();
        cursor += 1;
        for name in dependencies {
            let Some(dependency) = all
                .iter()
                .find(|candidate| candidate.install_target == name)
                .cloned()
            else {
                continue; // catalog generation guarantees deps resolve; stay lenient here
            };
            if seen.insert(dependency.id.clone()) {
                expanded.push(dependency);
            }
        }
    }
    expanded
}

/// What happened to one skill in one tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CopyOutcome {
    Installed,
    /// The tree entry is a symlink (e.g. a dev machine linking into a repo
    /// checkout); never write through it.
    SkippedSymlink,
}

/// Per-tree summary of an install run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeReport {
    pub tree: PathBuf,
    pub installed: usize,
    pub skipped_symlinks: Vec<String>,
}

/// Download the repo once and copy every named skill into every detected
/// skill tree, then verify each copy landed with its SKILL.md. Returns the
/// per-tree summaries, or one aggregated failure message.
pub fn install_skills(
    system: &dyn System,
    skills: &[String],
    destination: &SkillDestination,
) -> Result<Vec<TreeReport>, String> {
    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let trees = destination.trees();
    if trees.is_empty() {
        return Err("no skill agents selected".to_string());
    }

    let staging = home.join(".cache").join("loom").join("staging");
    let result = fetch_repo(system, &staging).and_then(|repo_root| {
        let source_root = repo_root.join("skills");
        let mut missing = skills
            .iter()
            .filter(|name| !source_root.join(name.as_str()).join("SKILL.md").is_file())
            .peekable();
        if missing.peek().is_some() {
            return Err(format!(
                "downloaded repo is missing skills: {}",
                missing.cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        install_opencode_adapter(&home, &repo_root, destination)?;
        copy_into_trees(&source_root, &trees, skills)
    });
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Install the Loom-owned session adapter when OpenCode is present. The
/// plugin gives shell tools the current raw session ID; it owns one fixed
/// filename, so updates replace only Loom's previous copy.
fn install_opencode_adapter(
    home: &Path,
    repo_root: &Path,
    destination: &SkillDestination,
) -> Result<(), String> {
    if !destination.agents.contains(&SkillAgent::OpenCode) {
        return Ok(());
    }

    let target = match destination.scope {
        SkillScope::Global => opencode_adapter_path(home),
        SkillScope::Project => project_opencode_adapter_path(&destination.project_root),
    };
    install_opencode_adapter_at(repo_root, &target)
}

fn install_opencode_adapter_at(repo_root: &Path, target: &Path) -> Result<(), String> {
    let source = repo_root.join(OPENCODE_PLUGIN_SOURCE);
    if !source.is_file() {
        return Err(format!(
            "downloaded repo is missing the OpenCode adapter: {OPENCODE_PLUGIN_SOURCE}"
        ));
    }
    let parent = target
        .parent()
        .ok_or_else(|| "OpenCode adapter target has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let incoming = parent.join(format!(".{OPENCODE_PLUGIN_NAME}.loom-new"));
    let _ = fs::remove_file(&incoming);
    fs::copy(&source, &incoming)
        .map_err(|error| format!("could not stage OpenCode adapter: {error}"))?;
    if target.exists() || target.symlink_metadata().is_ok() {
        fs::remove_file(target)
            .map_err(|error| format!("could not replace {}: {error}", target.display()))?;
    }
    fs::rename(&incoming, target)
        .map_err(|error| format!("could not install {}: {error}", target.display()))?;
    Ok(())
}

/// Refresh only skill copies that already exist globally or in the current
/// project. Each tree keeps its own set, so update never widens an earlier
/// agent or scope choice.
pub fn refresh_installed_skills(
    system: &dyn System,
    resources: &[Resource],
) -> Result<Vec<TreeReport>, String> {
    let home = system
        .home_dir()
        .ok_or_else(|| "home directory is unavailable".to_string())?;
    let current_dir = system
        .current_dir()
        .ok_or_else(|| "current directory is unavailable".to_string())?;
    let project = project_root(&current_dir);
    let mut trees = detect_skill_trees(&home);
    trees.extend(
        SkillAgent::ALL
            .into_iter()
            .map(|agent| agent.project_skill_tree(&project))
            .filter(|tree| tree.is_dir()),
    );
    let mut seen = HashSet::new();
    trees.retain(|tree| seen.insert(tree.clone()));

    let copies = trees
        .into_iter()
        .filter_map(|tree| {
            let installed = resources
                .iter()
                .filter(|resource| resource.kind == ResourceKind::Skill)
                .filter(|resource| skill_present_in(&tree, &resource.install_target))
                .cloned()
                .collect::<Vec<_>>();
            if installed.is_empty() {
                return None;
            }
            // Dependencies may name tools (a skill that drives a CLI); only
            // skills are copied into a skill tree.
            let names = expand_skill_dependencies(resources, installed)
                .into_iter()
                .filter(|resource| resource.kind == ResourceKind::Skill)
                .map(|resource| resource.install_target)
                .collect::<Vec<_>>();
            Some((tree, names))
        })
        .collect::<Vec<_>>();
    if copies.is_empty() {
        return Ok(Vec::new());
    }

    let staging = home.join(".cache").join("loom").join("staging");
    let result = fetch_repo(system, &staging).and_then(|repo_root| {
        let source_root = repo_root.join("skills");
        let mut reports = Vec::new();
        for (tree, names) in &copies {
            let mut copied = copy_into_trees(&source_root, std::slice::from_ref(tree), names)?;
            reports.append(&mut copied);
            let adapter = if *tree == home.join(".config").join("opencode").join("skills") {
                Some(opencode_adapter_path(&home))
            } else if *tree == project.join(".opencode").join("skills") {
                Some(project_opencode_adapter_path(&project))
            } else {
                None
            };
            if let Some(adapter) = adapter {
                install_opencode_adapter_at(&repo_root, &adapter)?;
            }
        }
        Ok(reports)
    });
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Download and unpack the repo tarball into `staging`; returns the extracted
/// repo root. Shells out to curl and tar (present on macOS, Linux, and
/// Windows 10+) through `System`, so tests can intercept both.
/// `LOOM_REPO_DIR` overrides the download with a local checkout — for
/// testing unpushed skills and manifest changes end to end.
pub(crate) fn fetch_repo(system: &dyn System, staging: &Path) -> Result<PathBuf, String> {
    if let Some(local) = std::env::var_os("LOOM_REPO_DIR") {
        let root = PathBuf::from(local);
        if !root.join("skills").is_dir() {
            return Err(format!(
                "LOOM_REPO_DIR does not look like the repo: {}",
                root.display()
            ));
        }
        return Ok(root);
    }
    let _ = fs::remove_dir_all(staging);
    fs::create_dir_all(staging)
        .map_err(|error| format!("could not create {}: {error}", staging.display()))?;
    let tarball = staging.join("repo.tar.gz");
    for spec in [
        CommandSpec::new(
            "curl",
            ["-fsSL", TARBALL_URL, "-o", &tarball.display().to_string()],
        ),
        CommandSpec::new(
            "tar",
            [
                "-xzf",
                &tarball.display().to_string(),
                "-C",
                &staging.display().to_string(),
            ],
        ),
    ] {
        match system.run(&spec) {
            Ok(result) if result.success => {}
            Ok(result) => {
                return Err(format!(
                    "{} failed: {}",
                    spec.program,
                    crate::install::command_failure_message(&result)
                ));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    // The tarball unpacks to a single top-level directory whose name embeds
    // the ref; locate it rather than hardcoding "loom-main".
    let entries = fs::read_dir(staging)
        .map_err(|error| format!("could not read {}: {error}", staging.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    match entries.as_slice() {
        [root] => Ok(root.clone()),
        _ => Err("downloaded tarball did not unpack to a single directory".to_string()),
    }
}

fn copy_into_trees(
    source_root: &Path,
    trees: &[PathBuf],
    skills: &[String],
) -> Result<Vec<TreeReport>, String> {
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    for tree in trees {
        if let Err(error) = fs::create_dir_all(tree) {
            failures.push(format!("could not create {}: {error}", tree.display()));
            continue;
        }
        let mut report = TreeReport {
            tree: tree.clone(),
            installed: 0,
            skipped_symlinks: Vec::new(),
        };
        for name in skills {
            match copy_skill(source_root, tree, name) {
                Ok(CopyOutcome::Installed) => {
                    if tree.join(name).join("SKILL.md").is_file() {
                        report.installed += 1;
                    } else {
                        failures.push(format!(
                            "{} landed without a SKILL.md in {}",
                            name,
                            tree.display()
                        ));
                    }
                }
                Ok(CopyOutcome::SkippedSymlink) => report.skipped_symlinks.push(name.clone()),
                Err(error) => failures.push(format!("{name} into {}: {error}", tree.display())),
            }
        }
        reports.push(report);
    }
    if failures.is_empty() {
        Ok(reports)
    } else {
        Err(failures.join("; "))
    }
}

/// Copy one skill into one tree. Replacement goes through a temp sibling and
/// a rename, so a half-written skill never sits at the final path.
fn copy_skill(source_root: &Path, tree: &Path, name: &str) -> Result<CopyOutcome, String> {
    let target = tree.join(name);
    if target
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Ok(CopyOutcome::SkippedSymlink);
    }
    let incoming = tree.join(format!(".{name}.loom-new"));
    let _ = fs::remove_dir_all(&incoming);
    copy_dir(&source_root.join(name), &incoming).map_err(|error| error.to_string())?;
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    }
    fs::rename(&incoming, &target).map_err(|error| error.to_string())?;
    Ok(CopyOutcome::Installed)
}

fn copy_dir(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_str() == Some(".DS_Store") {
            continue;
        }
        let from = entry.path();
        let to = destination.join(&name);
        if entry.file_type()?.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceKind;

    fn skill(name: &str, dependencies: &[&str]) -> Resource {
        Resource {
            id: format!("skill:{name}"),
            kind: ResourceKind::Skill,
            group: "Coding".into(),
            label: name.into(),
            description: String::new(),
            install_target: name.into(),
            next_action: String::new(),
            dependencies: dependencies.iter().map(ToString::to_string).collect(),
            bin: None,
            version: None,
            companions: Vec::new(),
        }
    }

    fn temp_home(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let home = std::env::temp_dir().join(format!(
            "loom-skills-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&home).expect("create temp home");
        home
    }

    fn write_skill(root: &Path, name: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(dir.join("SKILL.md"), format!("# {name}\n")).expect("write SKILL.md");
    }

    #[test]
    fn trees_are_detected_only_for_existing_agent_dirs() {
        let home = temp_home("detect");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".pi").join("agent")).unwrap();
        fs::create_dir_all(home.join(".config").join("opencode")).unwrap();

        let trees = detect_skill_trees(&home);

        assert_eq!(
            trees,
            vec![
                home.join(".claude").join("skills"),
                home.join(".pi").join("agent").join("skills"),
                home.join(".config").join("opencode").join("skills"),
            ]
        );
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn project_destinations_follow_agent_conventions_and_deduplicate() {
        // Capability/seam: user-selected agent and scope routing. This fails
        // if a host path drifts or Codex duplicates the shared .agents tree.
        let home = temp_home("project-destination");
        let project = home.join("work");
        fs::create_dir_all(project.join(".git")).unwrap();
        let destination = SkillDestination::new(
            SkillAgent::ALL.to_vec(),
            SkillScope::Project,
            &home,
            &project.join("nested"),
        );

        assert_eq!(
            destination.trees(),
            vec![
                project.join(".claude/skills"),
                project.join(".agents/skills"),
                project.join(".pi/skills"),
                project.join(".opencode/skills"),
                project.join(".cursor/skills"),
                project.join(".grok/skills"),
            ]
        );
        fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn cursor_and_grok_use_native_trees() {
        let home = Path::new("/tmp/loom-home");
        let project = Path::new("/tmp/loom-project");
        assert_eq!(
            SkillAgent::Cursor.global_skill_tree(home),
            home.join(".cursor").join("skills")
        );
        assert_eq!(
            SkillAgent::Grok.global_skill_tree(home),
            home.join(".grok").join("skills")
        );
        assert_eq!(
            SkillAgent::Cursor.project_skill_tree(project),
            project.join(".cursor").join("skills")
        );
        assert_eq!(
            SkillAgent::Grok.project_skill_tree(project),
            project.join(".grok").join("skills")
        );
    }

    #[test]
    fn dependency_closure_is_transitive_and_cycle_tolerant() {
        let all = vec![
            skill("release", &["commit"]),
            skill("commit", &["writing"]),
            skill("writing", &["release"]), // cycle back to the root
            skill("unrelated", &[]),
        ];

        let expanded = expand_skill_dependencies(&all, vec![all[0].clone()]);

        assert_eq!(
            expanded
                .iter()
                .map(|resource| resource.label.as_str())
                .collect::<Vec<_>>(),
            vec!["release", "commit", "writing"]
        );
    }

    #[test]
    fn copying_replaces_stale_content_via_temp_sibling() {
        let home = temp_home("replace");
        let source = home.join("source");
        write_skill(&source, "tdd");
        let tree = home.join("tree");
        fs::create_dir_all(tree.join("tdd")).unwrap();
        fs::write(tree.join("tdd").join("stale.md"), "old").unwrap();

        let outcome = copy_skill(&source, &tree, "tdd").expect("copy");

        assert_eq!(outcome, CopyOutcome::Installed);
        assert!(tree.join("tdd").join("SKILL.md").is_file());
        assert!(!tree.join("tdd").join("stale.md").exists());
        assert!(!tree.join(".tdd.loom-new").exists());
        fs::remove_dir_all(&home).ok();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_skills_are_never_written_through() {
        let home = temp_home("symlink");
        let source = home.join("source");
        write_skill(&source, "tdd");
        let checkout = home.join("checkout");
        write_skill(&checkout, "tdd");
        let tree = home.join("tree");
        fs::create_dir_all(&tree).unwrap();
        std::os::unix::fs::symlink(checkout.join("tdd"), tree.join("tdd")).unwrap();

        let outcome = copy_skill(&source, &tree, "tdd").expect("copy");

        assert_eq!(outcome, CopyOutcome::SkippedSymlink);
        assert!(tree.join("tdd").symlink_metadata().unwrap().is_symlink());
        fs::remove_dir_all(&home).ok();
    }
}
