use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn sibling(path: &Path, suffix: &str) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?
        .to_string_lossy();
    Ok(path.with_file_name(format!(".{name}.{suffix}")))
}

fn remove_path(path: &Path) -> Result<(), String> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("could not inspect {}: {error}", path.display())),
    }
    .map_err(|error| format!("could not remove {}: {error}", path.display()))
}

/// Restore a backup left between the two replacement renames. Call this
/// before reading transactional state, not only before its next write.
pub fn recover(target: &Path) -> Result<(), String> {
    let backup = sibling(target, "loom-old")?;
    if !target.exists() && backup.exists() {
        fs::rename(&backup, target).map_err(|error| {
            format!(
                "could not recover {} from {}: {error}",
                target.display(),
                backup.display()
            )
        })?;
    } else if target.exists() {
        let _ = remove_path(&backup);
    }
    Ok(())
}

/// Replace `target` with a fully staged sibling. If a prior process stopped
/// between the two renames, recover its backup before trying again.
pub fn replace_staged(target: &Path, incoming: &Path) -> Result<(), String> {
    recover(target)?;
    let backup = sibling(target, "loom-old")?;
    let had_target = target.symlink_metadata().is_ok();
    if had_target {
        fs::rename(target, &backup).map_err(|error| {
            format!(
                "could not stage replacement of {}: {error}",
                target.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(incoming, target) {
        if had_target {
            let _ = fs::rename(&backup, target);
        }
        return Err(format!("could not install {}: {error}", target.display()));
    }
    // The new target is committed. A stale backup is safe and the next
    // replacement removes it, so cleanup failure must not misreport commit.
    let _ = remove_path(&backup);
    Ok(())
}

/// Flush a sibling file before replacing the destination through
/// `replace_staged`, so interrupted writes leave either the old or new file.
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let incoming = sibling(path, "loom-new")?;
    remove_path(&incoming)?;
    let result = (|| {
        let mut file = fs::File::create(&incoming)
            .map_err(|error| format!("could not write {}: {error}", incoming.display()))?;
        file.write_all(content)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("could not flush {}: {error}", incoming.display()))?;
        replace_staged(path, &incoming)
    })();
    if result.is_err() {
        let _ = remove_path(&incoming);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "loom-fs-tx-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn atomic_write_replaces_existing_content_and_cleans_siblings() {
        let root = temp("write");
        let target = root.join("selection.toml");
        fs::write(&target, "old").unwrap();

        atomic_write(&target, b"new").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn replacement_recovers_an_interrupted_backup() {
        let root = temp("recover");
        let target = root.join("skill");
        let backup = root.join(".skill.loom-old");
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("SKILL.md"), "old").unwrap();
        let incoming = root.join(".skill.loom-new");
        fs::create_dir(&incoming).unwrap();
        fs::write(incoming.join("SKILL.md"), "new").unwrap();

        replace_staged(&target, &incoming).unwrap();

        assert_eq!(fs::read_to_string(target.join("SKILL.md")).unwrap(), "new");
        assert!(!backup.exists());
        fs::remove_dir_all(root).ok();
    }
}
