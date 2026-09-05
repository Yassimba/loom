//! A package only replaces a standalone Pi skill when its enabled files exist.
//! Unknown filters/layouts deliberately fall back to the standalone skill.
use crate::{Resource, ResourceKind, SkillDestination, SkillScope};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Catalog packages that bundle shared skills.
pub(crate) fn packages() -> &'static [Resource] {
    static PACKAGES: OnceLock<Vec<Resource>> = OnceLock::new();
    PACKAGES.get_or_init(|| {
        crate::Catalog::embedded()
            .map(|catalog| {
                catalog
                    .resources
                    .into_iter()
                    .filter(|resource| {
                        resource.kind == ResourceKind::PiPackage
                            && !resource.bundled_skills.is_empty()
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

fn settings(root: &Path) -> Option<Value> {
    match fs::read(root.join("settings.json")) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
            .ok()
            .filter(Value::is_object),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(serde_json::json!({})),
        Err(_) => None,
    }
}

fn identity(source: &str) -> &str {
    source
        .rfind('@')
        .filter(|index| *index > 4)
        .map_or(source, |index| &source[..index])
}

fn entry<'a>(settings: &'a Value, package: &Resource) -> Option<&'a Value> {
    let spec = package.pi_install_spec();
    settings.get("packages")?.as_array()?.iter().find(|entry| {
        entry
            .as_str()
            .or_else(|| entry.get("source")?.as_str())
            .is_some_and(|source| identity(source) == identity(&spec))
    })
}

fn unfiltered(entry: Option<&Value>) -> bool {
    // Top-level skills filters affect local/auto discovery, not package files.
    entry.is_none_or(|value| {
        value.is_string()
            || (value.is_object()
                && value.get("skills").is_none()
                && value.get("autoload").is_none_or(|value| value == true))
    })
}

fn roots(destination: &SkillDestination) -> Vec<PathBuf> {
    let global = destination.home.join(".pi/agent");
    if std::env::var_os("PI_CODING_AGENT_DIR")
        .is_some_and(|value| !value.is_empty() && Path::new(&value) != global)
    {
        return Vec::new();
    }
    match destination.scope {
        SkillScope::Global => vec![global],
        SkillScope::Project => vec![destination.project_root.join(".pi"), global],
    }
}

/// Whether selecting a package can supply its skill without overriding filters.
pub(crate) fn selectable(package: &Resource, destination: &SkillDestination) -> bool {
    let roots = roots(destination);
    !roots.is_empty()
        && roots.iter().all(|root| {
            settings(root).is_some_and(|settings| unfiltered(entry(&settings, package)))
        })
}

fn package_root(root: &Path, package: &Resource) -> Option<PathBuf> {
    if let Some(source) = &package.source {
        let repository = identity(source).strip_prefix("git:")?;
        // The reviewed catalog uses host/owner/repo Git shorthand only.
        if repository
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
            || repository.contains(':')
        {
            return None;
        }
        Some(root.join("git").join(repository))
    } else {
        Some(root.join("npm/node_modules").join(&package.install_target))
    }
}

pub(crate) fn provides(package: &Resource, name: &str, destination: &SkillDestination) -> bool {
    if !package.bundled_skills.iter().any(|skill| skill == name) {
        return false;
    }
    for root in roots(destination) {
        let Some(settings) = settings(&root) else {
            return false;
        };
        let configured = entry(&settings, package);
        if !unfiltered(configured) {
            return false;
        }
        if configured.is_none() {
            continue;
        }
        let Some(package_root) = package_root(&root, package) else {
            return false;
        };
        let relative = format!("skills/{name}/SKILL.md");
        if !package_root.join(&relative).is_file() {
            return false;
        }
        let Ok(bytes) = fs::read(package_root.join("package.json")) else {
            return false;
        };
        let Ok(manifest) = serde_json::from_slice::<Value>(&bytes) else {
            return false;
        };
        if !manifest.is_object() {
            return false;
        }
        // No pi manifest means Pi uses the conventional skills/ directory.
        if manifest.get("pi").is_none() {
            return true;
        }
        let Some(paths) = manifest.pointer("/pi/skills").and_then(Value::as_array) else {
            return false;
        };
        // Plain manifest directories/files only. A glob or exclusion makes the
        // result uncertain, so do not remove the user's standalone copy.
        return paths.iter().all(|value| {
            value.as_str().is_some_and(|path| {
                !path.contains(['*', '?', '[', '!', '+'])
                    && !path.starts_with('-')
                    && !Path::new(path).is_absolute()
                    && !path.split('/').any(|part| part == "..")
            })
        }) && paths.iter().filter_map(Value::as_str).any(|path| {
            let path = path
                .strip_prefix("./")
                .unwrap_or(path)
                .trim_end_matches('/');
            relative == path || relative.starts_with(&format!("{path}/"))
        });
    }
    false
}

fn shared_is_bundle(
    package: &Resource,
    shared: &Path,
    name: &str,
    destination: &SkillDestination,
) -> bool {
    roots(destination)
        .iter()
        .filter_map(|root| package_root(root, package))
        .any(|root| {
            crate::ownership::same_path(shared, &root.join("skills").join(name).join("SKILL.md"))
        })
}

/// Names an enabled Pi package currently supplies for the global Pi tree.
pub fn provided_bundled_skills(home: &Path) -> Vec<String> {
    let tree = home.join(".pi/agent/skills");
    packages()
        .iter()
        .flat_map(|package| package.bundled_skills.iter())
        .filter(|name| provided_in_tree(home, &tree, name))
        .cloned()
        .collect()
}

/// Whether an enabled Pi package already supplies `name` for this skill tree.
pub(crate) fn provided_in_tree(home: &Path, tree: &Path, name: &str) -> bool {
    destination_for_tree(home, tree).is_some_and(|destination| {
        packages()
            .iter()
            .any(|package| provides(package, name, &destination))
    })
}

pub(crate) fn destination_for_tree(home: &Path, tree: &Path) -> Option<SkillDestination> {
    let (scope, project) = if tree == home.join(".pi/agent/skills") {
        (SkillScope::Global, home)
    } else if tree.ends_with(".pi/skills") {
        (SkillScope::Project, tree.parent()?.parent()?)
    } else {
        return None;
    };
    Some(SkillDestination::new(
        vec![crate::SkillAgent::Pi],
        scope,
        home,
        project,
    ))
}

/// Remove only unchanged, receipt-owned real copies. Repo symlinks are handled
/// by sync-skills.sh, which knows the exact repository source they belong to.
pub(crate) fn reconcile_copy(home: &Path, path: &Path) -> Result<(), String> {
    let Some(destination) = path
        .parent()
        .and_then(|tree| destination_for_tree(home, tree))
    else {
        return Ok(());
    };
    let boundary = if destination.scope == SkillScope::Global {
        home
    } else {
        &destination.project_root
    };
    if path
        .ancestors()
        .take_while(|ancestor| *ancestor != boundary)
        .chain(std::iter::once(boundary))
        .any(|ancestor| {
            ancestor
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
        })
    {
        return Ok(());
    }
    if !plain_tree(path) {
        return Ok(());
    }
    let mut state = crate::InstallState::load(home)?;
    if !crate::skills::owned_unchanged(&state, path) {
        return Ok(());
    }
    // Prune receipts while the path still resolves; nothing is saved unless
    // the removal succeeds.
    state.resources.retain(|_, resource| {
        let previous = resource.receipts.len();
        resource.receipts.retain(|receipt| !matches!(receipt, crate::Receipt::Path { path: owned, .. } if crate::ownership::same_path(owned, path)));
        previous == resource.receipts.len() || !resource.receipts.is_empty()
    });
    fs::remove_dir_all(path)
        .map_err(|error| format!("could not remove {}: {error}", path.display()))?;
    state.save(home)
}

fn plain_tree(path: &Path) -> bool {
    let Ok(mut entries) = fs::read_dir(path) else {
        return false;
    };
    entries.all(|entry| {
        entry.is_ok_and(|entry| {
            entry
                .file_type()
                .is_ok_and(|kind| kind.is_file() || (kind.is_dir() && plain_tree(&entry.path())))
        })
    })
}

pub fn reconcile_installed(home: &Path) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    let mut trees = vec![home.join(".pi/agent/skills")];
    trees.extend(
        crate::skills::read_skill_projects(home)?
            .iter()
            .map(|root| root.join(".pi/skills")),
    );
    let names = packages()
        .iter()
        .flat_map(|package| package.bundled_skills.iter().cloned())
        .collect::<Vec<_>>();
    for tree in &trees {
        if let Some(destination) = destination_for_tree(home, tree) {
            check_shared_scope(&destination, &names)?;
        }
    }
    reconcile_exclusions(home)?;
    for tree in trees {
        let Some(destination) = destination_for_tree(home, &tree) else {
            continue;
        };
        notes.extend(exclude_shared(&destination, &names)?);
        for package in packages() {
            for name in &package.bundled_skills {
                if provides(package, name, &destination) {
                    reconcile_copy(home, &tree.join(name))?;
                }
            }
        }
    }
    Ok(notes)
}

pub(crate) fn project_launch_note(root: &Path) -> String {
    format!("Start Pi from {} for shared-skill exclusions; nested launches do not inherit its .pi/settings.json", root.display())
}

fn exclusion_settings(path: &Path) -> Result<Value, String> {
    // Never replace a user-managed settings symlink or traverse a redirected Pi directory.
    for ancestor in path.ancestors().take(3) {
        if ancestor
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(format!(
                "refusing to change symlinked Pi settings: {}",
                path.display()
            ));
        }
    }
    let value = settings(path.parent().ok_or("Pi settings have no parent")?)
        .ok_or_else(|| format!("could not read valid Pi settings: {}", path.display()))?;
    if value.get("packages").is_some_and(|packages| {
        !packages.as_array().is_some_and(|entries| {
            entries
                .iter()
                .all(|entry| entry.is_string() || entry.get("source").is_some_and(Value::is_string))
        })
    }) {
        return Err(format!(
            "{}.packages must be an array of package sources",
            path.display()
        ));
    }
    if value.get("skills").is_some_and(|skills| {
        !skills
            .as_array()
            .is_some_and(|entries| entries.iter().all(Value::is_string))
    }) {
        return Err(format!(
            "{}.skills must be an array of strings",
            path.display()
        ));
    }
    Ok(value)
}

fn same_exclusion(value: &Value, entry: &str) -> bool {
    value.as_str().is_some_and(|value| {
        value == entry
            || value
                .strip_prefix('-')
                .zip(entry.strip_prefix('-'))
                .is_some_and(|(left, right)| {
                    crate::ownership::same_path(Path::new(left), Path::new(right))
                })
    })
}

pub(crate) fn exclusion_present(path: &Path, entry: &str) -> Result<bool, String> {
    Ok(exclusion_settings(path)?
        .get("skills")
        .and_then(Value::as_array)
        .is_some_and(|entries| entries.iter().any(|value| same_exclusion(value, entry))))
}

pub(crate) fn remove_exclusion(path: &Path, entry: &str) -> Result<(), String> {
    let mut config = exclusion_settings(path)?;
    let Some(entries) = config.get_mut("skills").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let previous = entries.len();
    entries.retain(|value| !same_exclusion(value, entry));
    if entries.len() != previous {
        crate::fs_tx::atomic_write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&config).unwrap()).as_bytes(),
        )?;
    }
    Ok(())
}

fn exclusion_destination(home: &Path, path: &Path) -> Option<SkillDestination> {
    if path.file_name()? != "settings.json" {
        return None;
    }
    destination_for_tree(home, &path.parent()?.join("skills"))
}

/// Drop only our exact entries when their provider no longer supplies a skill.
/// Direct `pi remove` is reconciled on the next Loom install/update or skill sync.
pub(crate) fn reconcile_exclusions(home: &Path) -> Result<(), String> {
    let mut state = crate::InstallState::load(home)?;
    let mut removed = Vec::new();
    for (id, resource) in &state.resources {
        for receipt in &resource.receipts {
            let crate::Receipt::PiSkillExclusion { path, entry } = receipt else {
                continue;
            };
            let Some(destination) = exclusion_destination(home, path) else {
                continue;
            };
            exclusion_settings(path)?;
            let name = Path::new(entry.trim_start_matches('-'))
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let enabled = packages()
                .iter()
                .find(|package| package.id == *id)
                .is_some_and(|package| {
                    provides(package, name, &destination)
                        && !shared_is_bundle(
                            package,
                            Path::new(entry.trim_start_matches('-')),
                            name,
                            &destination,
                        )
                });
            if !enabled {
                remove_exclusion(path, entry)?;
                removed.push(receipt.clone());
            }
        }
    }
    if removed.is_empty() {
        return Ok(());
    }
    state.resources.retain(|_, resource| {
        let previous = resource.receipts.len();
        resource
            .receipts
            .retain(|receipt| !removed.contains(receipt));
        previous == resource.receipts.len() || !resource.receipts.is_empty()
    });
    state.save(home)
}

pub(crate) fn check_shared_scope(
    destination: &SkillDestination,
    names: &[String],
) -> Result<(), String> {
    if destination.scope != SkillScope::Project {
        return Ok(());
    }
    let mut global = destination.clone();
    global.scope = SkillScope::Global;
    for name in names {
        let shared = destination
            .home
            .join(".agents/skills")
            .join(name)
            .join("SKILL.md");
        if shared.is_file()
            && packages()
                .iter()
                .any(|package| provides(package, name, destination))
            && !packages()
                .iter()
                .any(|package| provides(package, name, &global))
        {
            return Err(format!("project-only Pi provider conflicts with {}; {} cannot exclude global discovery — install the provider globally or resolve this collision manually", shared.display(), destination.project_root.join(".pi/settings.json").display()));
        }
    }
    Ok(())
}

/// Exclude shared copies only from Pi discovery; never delete another host's skill.
pub(crate) fn exclude_shared(
    destination: &SkillDestination,
    names: &[String],
) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    check_shared_scope(destination, names)?;
    let mut scopes = vec![destination.clone()];
    if destination.scope == SkillScope::Project {
        let mut global = destination.clone();
        global.scope = SkillScope::Global;
        scopes.push(global);
    }
    for destination in scopes {
        let root = match destination.scope {
            SkillScope::Global => &destination.home,
            SkillScope::Project => &destination.project_root,
        };
        let path = match destination.scope {
            SkillScope::Global => root.join(".pi/agent/settings.json"),
            SkillScope::Project => root.join(".pi/settings.json"),
        };
        for name in names {
            let shared = root.join(".agents/skills").join(name).join("SKILL.md");
            if !shared.is_file() {
                continue;
            }
            let Some(package) = packages()
                .iter()
                .find(|package| provides(package, name, &destination))
            else {
                continue;
            };
            if shared_is_bundle(package, &shared, name, &destination) {
                continue;
            }
            let entry = format!("-{}", shared.display());
            let mut config = exclusion_settings(&path)?;
            let entries = config
                .as_object_mut()
                .unwrap()
                .entry("skills")
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .unwrap();
            if entries.iter().any(|value| same_exclusion(value, &entry)) {
                continue;
            } // Preexisting exclusions remain user-owned.
            if entries
                .iter()
                .any(|value| value.as_str().is_some_and(|value| value.starts_with('+')))
            {
                return Err(format!(
                    "{} has explicit skill inclusions; resolve the shared-skill collision manually",
                    path.display()
                ));
            }
            let mut state = crate::InstallState::load(&destination.home)?;
            entries.push(Value::String(entry.clone()));
            crate::fs_tx::atomic_write(
                &path,
                format!("{}\n", serde_json::to_string_pretty(&config).unwrap()).as_bytes(),
            )?;
            state.record(crate::OwnedResource {
                id: package.id.clone(),
                scope: crate::OwnershipScope::Global,
                depends_on: Vec::new(),
                receipts: vec![crate::Receipt::PiSkillExclusion {
                    path: path.clone(),
                    entry: entry.clone(),
                }],
            });
            if destination.scope == SkillScope::Project {
                notes.push(project_launch_note(&destination.project_root));
            }
            if let Err(error) = state.save(&destination.home) {
                remove_exclusion(&path, &entry)?;
                return Err(error);
            }
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exclusions_match_equivalent_paths() {
        let root = std::env::temp_dir().join(format!("loom-exclusion-path-{}", std::process::id()));
        let skill = root.join("skills/ponytail/SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(&skill, "# skill").unwrap();
        let equivalent = root.join("skills/../skills/ponytail/SKILL.md");

        assert!(same_exclusion(
            &Value::String(format!("-{}", equivalent.display())),
            &format!("-{}", skill.display())
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn provides_requires_enabled_registered_files_in_the_correct_scope() {
        let home = std::env::temp_dir().join(format!(
            "loom-bundled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut destination = SkillDestination::new(
            vec![crate::SkillAgent::Pi],
            SkillScope::Global,
            &home,
            &home.join("project"),
        );
        let catalog = crate::Catalog::embedded().unwrap();
        for id in [
            "pi-package:i-have-adhd",
            "pi-package:@dietrichgebert/ponytail",
        ] {
            let package = &catalog.find(&[id.into()]).unwrap()[0];
            let name = &package.bundled_skills[0];
            let root = home.join(".pi/agent");
            let source = package_root(&root, package).unwrap();
            fs::create_dir_all(source.join(format!("skills/{name}"))).unwrap();
            let skill = source.join(format!("skills/{name}/SKILL.md"));
            fs::write(&skill, "# skill").unwrap();
            fs::write(
                source.join("package.json"),
                r#"{"pi":{"skills":["./skills"]}}"#,
            )
            .unwrap();
            for config in [
                json!({}),
                json!({"packages":[{"source": package.pi_install_spec(), "skills":[]}]}),
                json!({"packages":[{"source":package.pi_install_spec(),"autoload":false}]}),
            ] {
                fs::write(root.join("settings.json"), config.to_string()).unwrap();
                assert!(!provides(package, name, &destination));
            }
            fs::write(
                root.join("settings.json"),
                json!({"packages":[package.pi_install_spec()]}).to_string(),
            )
            .unwrap();
            assert!(provides(package, name, &destination));
            destination.scope = SkillScope::Project;
            let project = destination.project_root.join(".pi");
            fs::create_dir_all(&project).unwrap();
            fs::write(
                project.join("settings.json"),
                json!({"packages":[{"source":package.pi_install_spec(),"skills":[]}]}).to_string(),
            )
            .unwrap();
            assert!(
                !provides(package, name, &destination),
                "local filters override global package"
            );
            fs::remove_file(project.join("settings.json")).unwrap();
            assert!(
                provides(package, name, &destination),
                "global package is inherited by project"
            );
            destination.scope = SkillScope::Global;
            fs::remove_file(&skill).unwrap();
            assert!(
                !provides(package, name, &destination),
                "settings alone are not proof"
            );
        }
        fs::remove_dir_all(home).unwrap();
    }
}
