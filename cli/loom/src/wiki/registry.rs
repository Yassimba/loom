use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultRecord {
    pub path: PathBuf,
    pub feynman: bool,
    #[serde(default)]
    pub confluence: bool,
    #[serde(default)]
    pub qmd: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiRegistry {
    schema_version: u32,
    pub vaults: Vec<VaultRecord>,
}

impl Default for WikiRegistry {
    fn default() -> Self {
        Self {
            schema_version: 1,
            vaults: Vec::new(),
        }
    }
}

impl WikiRegistry {
    pub(crate) fn path(home: &Path) -> PathBuf {
        home.join(".config").join("loom").join("wiki-vaults.json")
    }

    pub fn load(home: &Path) -> Result<Self> {
        let path = Self::path(home);
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        };
        let registry: Self = serde_json::from_str(&content)
            .with_context(|| format!("invalid Wiki registry: {}", path.display()))?;
        anyhow::ensure!(
            registry.schema_version == 1,
            "unsupported Wiki registry schema"
        );
        anyhow::ensure!(
            registry.vaults.iter().all(|vault| vault.path.is_absolute()),
            "Wiki registry paths must be absolute"
        );
        Ok(registry)
    }

    pub fn save(&mut self, home: &Path) -> Result<()> {
        self.vaults.sort_by(|a, b| a.path.cmp(&b.path));
        self.vaults.dedup_by(|a, b| a.path == b.path);
        let path = Self::path(home);
        let parent = path.parent().context("Wiki registry has no parent")?;
        fs::create_dir_all(parent)?;
        let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn unregister(&mut self, path: &Path) -> bool {
        let before = self.vaults.len();
        self.vaults.retain(|record| record.path != path);
        self.vaults.len() != before
    }

    pub(crate) fn register(&mut self, path: PathBuf, feynman: bool, confluence: bool, qmd: bool) {
        if let Some(record) = self.vaults.iter_mut().find(|record| record.path == path) {
            record.feynman = feynman;
            record.confluence = confluence;
            record.qmd = qmd;
        } else {
            self.vaults.push(VaultRecord {
                path,
                feynman,
                confluence,
                qmd,
            });
        }
    }
}
