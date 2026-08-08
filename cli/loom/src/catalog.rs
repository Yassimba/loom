use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    Skill,
    PiPackage,
    HerdrPlugin,
    Tool,
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skill => write!(formatter, "Skill"),
            Self::PiPackage => write!(formatter, "Pi package"),
            Self::HerdrPlugin => write!(formatter, "Herdr plugin"),
            Self::Tool => write!(formatter, "Tool"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: String,
    pub kind: ResourceKind,
    pub group: String,
    pub label: String,
    pub description: String,
    pub install_target: String,
    pub next_action: String,
    /// Skills this resource invokes (skill resources only); installs pull
    /// them in transitively.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// The executable to probe on PATH (tool resources only).
    #[serde(default)]
    pub bin: Option<String>,
    /// Exact version pin (external Pi packages only): installs go through
    /// `pi install npm:<target>@<version>` and update only when the pin does.
    #[serde(default)]
    pub version: Option<String>,
}

/// A curated selection bundle from manifest/presets.json, offered on the
/// wizard's Welcome screen. Targets are resource install targets.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresetSpec {
    pub id: String,
    pub label: String,
    pub description: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub schema_version: u32,
    #[serde(default)]
    pub presets: Vec<PresetSpec>,
    pub resources: Vec<Resource>,
}

impl Catalog {
    pub fn embedded() -> Result<Self> {
        let catalog: Self = serde_json::from_str(include_str!("../../../setup-catalog.json"))
            .context("embedded setup catalog is invalid")?;
        anyhow::ensure!(
            catalog.schema_version == 1,
            "unsupported setup catalog schema"
        );
        Ok(catalog)
    }

    pub fn find(&self, ids: &[String]) -> Result<Vec<Resource>> {
        ids.iter()
            .map(|id| {
                self.resources
                    .iter()
                    .find(|resource| &resource.id == id)
                    .cloned()
                    .with_context(|| format!("unknown resource: {id}"))
            })
            .collect()
    }
}
